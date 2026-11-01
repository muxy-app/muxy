mod retention;

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use muxy_protocol::transport::{
    BindError, ByteStream, Listener, StreamCancellation, UnixSocketListener,
};
use muxy_protocol::wire::WireError;
use muxy_server::{Registry, RemoteAccess, ServerEvent, connection};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use crate::remote_listener::RemoteListener;
use crate::{args::Args, logging, settings_file};

const DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

/// The connections being served; `gone` wakes whoever waits for them to end.
#[derive(Default)]
struct Clients {
    serving: Mutex<BTreeMap<u64, Client>>,
    gone: Condvar,
}

impl Clients {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<u64, Client>> {
        self.serving.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn remove(&self, id: u64) {
        self.lock().remove(&id);
        self.gone.notify_all();
    }
}

struct Client {
    cancellation: Box<dyn StreamCancellation>,
    remote: bool,
}

/// Bookkeeping shared by the local and network accept loops.
#[derive(Clone, Default)]
pub(crate) struct Connections {
    clients: Arc<Clients>,
    workers: Arc<Mutex<Workers>>,
    next: Arc<AtomicU64>,
}

/// A panicked local client stops the server, as before; a panicked network
/// client is only logged, so the network can never stop the server.
#[derive(Default)]
struct Workers {
    local: Vec<JoinHandle<()>>,
    remote: Vec<JoinHandle<()>>,
}

impl Connections {
    pub(crate) fn spawn(
        &self,
        stream: Box<dyn ByteStream>,
        remote: bool,
        serve: impl FnOnce(Box<dyn ByteStream>) -> Result<(), WireError> + Send + 'static,
    ) -> io::Result<()> {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let cancellation = stream.cancellation()?;
        self.clients.lock().insert(
            id,
            Client {
                cancellation,
                remote,
            },
        );
        let clients = Arc::clone(&self.clients);
        if !remote {
            log::info!("client connected: {id}");
        }
        let worker = thread::Builder::new()
            .name(format!("client-{id}"))
            .spawn(move || {
                match serve(stream) {
                    // Network peers fail handshakes routinely; devices log after authenticating.
                    Err(error) if remote => log::debug!("network client {id}: {error}"),
                    Err(error) => log::error!("client {id}: {error}"),
                    Ok(()) => {}
                }
                clients.remove(id);
                if !remote {
                    log::info!("client disconnected: {id}");
                }
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                self.clients.remove(id);
                return Err(error);
            }
        };
        let mut workers = self.workers.lock().unwrap_or_else(PoisonError::into_inner);
        let workers = if remote {
            &mut workers.remote
        } else {
            &mut workers.local
        };
        workers.push(worker);
        let mut panicked = false;
        let mut index = 0;
        while index < workers.len() {
            if workers[index].is_finished() {
                panicked |= workers.swap_remove(index).join().is_err();
            } else {
                index += 1;
            }
        }
        match (panicked, remote) {
            (true, false) => Err(io::Error::other("client thread panicked")),
            (true, true) => {
                log::error!("network client thread panicked");
                Ok(())
            }
            (false, _) => Ok(()),
        }
    }

    /// Ends every network connection, including ones not yet authenticated.
    pub(crate) fn cancel_remote(&self) {
        for client in self.clients.lock().values().filter(|client| client.remote) {
            client.cancellation.cancel();
        }
    }

    fn cancel_all(&self) {
        for client in self.clients.lock().values() {
            client.cancellation.cancel();
        }
    }

    /// Waits until every connection has ended, or the timeout passes.
    fn drain(&self, timeout: Duration) {
        let serving = self.clients.lock();
        let _ = self
            .clients
            .gone
            .wait_timeout_while(serving, timeout, |serving| !serving.is_empty());
    }

    fn join(&self) -> io::Result<()> {
        let workers =
            std::mem::take(&mut *self.workers.lock().unwrap_or_else(PoisonError::into_inner));
        let mut result = Ok(());
        for worker in workers.local.into_iter().chain(workers.remote) {
            if worker.join().is_err() {
                result = Err(io::Error::other("client thread panicked"));
            }
        }
        result
    }
}

struct Socket {
    listener: Arc<UnixSocketListener>,
    path: PathBuf,
    identity: Mutex<Option<(u64, u64)>>,
}

impl Socket {
    fn bind(path: PathBuf) -> Result<Self, BindError> {
        validate_socket(&path)?;
        let listener = Arc::new(UnixSocketListener::bind(&path)?);
        let metadata = fs::symlink_metadata(&path)?;
        Ok(Self {
            listener,
            path,
            identity: Mutex::new(Some((metadata.dev(), metadata.ino()))),
        })
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        self.close();
    }
}

impl Socket {
    fn close(&self) {
        if let Err(error) = self.remove() {
            log::error!("socket cleanup failed: {error}");
        }
    }

    fn remove(&self) -> io::Result<()> {
        let mut owned = self.identity.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(identity) = owned.take() else {
            return Ok(());
        };
        let result = self.remove_owned(identity);
        self.listener.close();
        result
    }

    fn remove_owned(&self, identity: (u64, u64)) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let directory = fs::File::open(parent)?;
        directory.lock()?;
        self.listener.close();
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) if (metadata.dev(), metadata.ino()) == identity => {
                fs::remove_file(&self.path)
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

pub(crate) fn run(args: &Args) -> io::Result<()> {
    let mut signals = Signals::new([SIGTERM, SIGINT])?;
    let socket = match Socket::bind(args.socket.clone()) {
        Ok(socket) => Arc::new(socket),
        Err(BindError::InUse) => {
            writeln!(
                io::stdout(),
                "muxy-server already running at {}",
                args.socket.display()
            )?;
            return Ok(());
        }
        Err(error) => return Err(io::Error::other(error)),
    };
    let settings = settings_file::load(&args.settings)?;
    logging::init(&args.log)?;
    log::info!("server started: socket={}", args.socket.display());
    let (sender, events) = mpsc::channel();
    let wake = sender.clone();
    let directory = args
        .socket
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sessions");
    let legacy = crate::legacy::read(directory.parent().unwrap_or_else(|| Path::new(".")))?;
    let connections = Connections::default();
    let remote = Arc::new(RemoteListener::new(connections.clone())?);
    let registry = bootstrap_registry(args, settings, sender, &directory, legacy, &remote)?;
    remote.attach(&registry);
    let retention = retention::Retention::start(Arc::clone(&registry))?;
    registry.resume_remote_access();
    let stopping = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let requested_stop = Arc::clone(&stopping);
    let requested_socket = Arc::clone(&socket);
    let requests = thread::Builder::new()
        .name("server-events".into())
        .spawn(move || {
            if events.recv().is_ok() {
                requested_stop.store(true, Ordering::Release);
                requested_socket.close();
            }
        })?;
    let signal_handle = signals.handle();
    let closing_socket = Arc::clone(&socket);
    let stopped = Arc::clone(&stopping);
    let signal = thread::Builder::new()
        .name("server-signals".into())
        .spawn(move || {
            if signals.forever().next().is_some() {
                stopped.store(true, Ordering::Release);
                closing_socket.close();
            }
        });
    let result = match signal.as_ref() {
        Ok(_) => accept(&socket, &registry, &connections).or_else(|error| {
            if stopping.load(Ordering::Acquire) {
                Ok(())
            } else {
                Err(error)
            }
        }),
        Err(error) => Err(io::Error::other(error.to_string())),
    };
    socket.close();
    remote.shutdown();
    drop(retention);
    registry.shutdown();
    let _ = wake.send(ServerEvent::StopRequested);
    let requests_result = requests
        .join()
        .map_err(|_| io::Error::other("event thread panicked"));
    registry.wait_stopped();
    registry.close_connections();
    connections.drain(DRAIN_TIMEOUT);
    connections.cancel_all();
    let worker_result = connections.join();
    signal_handle.close();
    let signal_result = match signal {
        Ok(signal) => signal
            .join()
            .map_err(|_| io::Error::other("signal thread panicked")),
        Err(_) => Ok(()),
    };
    log::info!("server stopped");
    socket.remove()?;
    result
        .and(requests_result)
        .and(worker_result)
        .and(signal_result)
}

fn bootstrap_registry(
    args: &Args,
    settings: muxy_server::ServerSettings,
    sender: Sender<ServerEvent>,
    directory: &Path,
    legacy: muxy_server::LegacyImport,
    remote: &Arc<RemoteListener>,
) -> io::Result<Arc<Registry>> {
    let hooks =
        muxy_server::ShellIntegration::install(&directory.with_file_name("shell-integration"))?;
    let settings_path = args.settings.clone();
    let listener = Arc::clone(remote);
    Ok(Arc::new(
        Registry::persistent_with_import(settings, sender, directory, legacy)?
            .with_shell_integration(hooks)
            .with_settings_persistence(move |settings| {
                settings_file::save(&settings_path, settings)
            })
            .with_remote_access(RemoteAccess::open(directory.with_file_name("remote.json"))?)
            .with_remote_listener(move |listening| listener.apply(listening)),
    ))
}

fn accept(socket: &Socket, registry: &Arc<Registry>, connections: &Connections) -> io::Result<()> {
    loop {
        let stream = socket.listener.accept()?;
        let registry = Arc::clone(registry);
        connections.spawn(stream, false, move |stream| {
            connection::serve(stream, registry)
        })?;
    }
}

fn validate_socket(path: &Path) -> io::Result<()> {
    let bytes = path.as_os_str().as_encoded_bytes().len();
    let limit = if cfg!(target_os = "linux") { 108 } else { 104 };
    if bytes >= limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "socket path is {bytes} bytes; platform limit is {limit} bytes including the terminating NUL: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_socket_cleanup_does_not_remove_its_replacement() -> io::Result<()> {
        let directory =
            Path::new("/tmp").join(format!("muxy-socket-{}", muxy_protocol::OperationId::new()));
        fs::create_dir(&directory)?;
        let path = directory.join("server.sock");
        for _ in 0..32 {
            let previous = Socket::bind(path.clone()).map_err(io::Error::other)?;
            previous.listener.close();
            previous.remove()?;
            assert!(!path.exists());
            let replacement = Socket::bind(path.clone()).map_err(io::Error::other)?;
            previous.remove()?;
            drop(previous);
            assert!(std::os::unix::net::UnixStream::connect(&path).is_ok());
            drop(replacement);
        }
        fs::remove_dir(directory)
    }
}
