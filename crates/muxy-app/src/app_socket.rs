//! The socket `muxy browser` reaches the app on: one JSON request and one
//! reply per connection, from processes of this user only.

use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use muxy_app_core::browser::api::{Request, encode_reply};
use serde_json::Value;

/// Requests answered at once; more wait for a free slot.
const CONCURRENT: usize = 16;

pub(crate) struct Incoming {
    pub request: Request,
    reply: async_channel::Sender<Result<Value, String>>,
}

impl Incoming {
    pub(crate) fn reply(self, result: Result<Value, String>) {
        let _ = self.reply.try_send(result);
    }
}

/// The listening socket. Dropping it removes the file.
#[derive(Debug)]
pub(crate) struct AppSocket {
    path: PathBuf,
}

impl Drop for AppSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Listens at `path`, unless another app already answers there.
pub(crate) fn listen(path: &Path) -> io::Result<(AppSocket, async_channel::Receiver<Incoming>)> {
    if UnixStream::connect(path).is_ok() {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "another Muxy app answers browser commands",
        ));
    }
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    let (requests, incoming) = async_channel::bounded(64);
    let busy = Arc::new(AtomicUsize::new(0));
    std::thread::Builder::new()
        .name("muxy-app-socket".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if requests.is_closed() {
                    break;
                }
                let requests = requests.clone();
                let busy = Arc::clone(&busy);
                while busy.load(Ordering::Acquire) >= CONCURRENT {
                    std::thread::sleep(Duration::from_millis(20));
                }
                busy.fetch_add(1, Ordering::AcqRel);
                let spawned = std::thread::Builder::new()
                    .name("muxy-app-request".into())
                    .spawn({
                        let busy = Arc::clone(&busy);
                        move || {
                            let _ = answer(stream, &requests);
                            busy.fetch_sub(1, Ordering::AcqRel);
                        }
                    });
                if spawned.is_err() {
                    busy.fetch_sub(1, Ordering::AcqRel);
                }
            }
        })?;
    Ok((
        AppSocket {
            path: path.to_owned(),
        },
        incoming,
    ))
}

fn answer(mut stream: UnixStream, requests: &async_channel::Sender<Incoming>) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let result = match Request::read(&stream) {
        Ok(request) => {
            let (reply, replied) = async_channel::bounded(1);
            if requests.send_blocking(Incoming { request, reply }).is_err() {
                Err("Muxy is closing".to_owned())
            } else {
                replied
                    .recv_blocking()
                    .unwrap_or_else(|_| Err("Muxy is closing".to_owned()))
            }
        }
        Err(error) => Err(format!("invalid request: {error}")),
    };
    stream.write_all(&encode_reply(&result))?;
    stream.flush()
}
