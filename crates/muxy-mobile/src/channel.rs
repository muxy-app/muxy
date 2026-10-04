//! Connections through an SSH channel that the app opens with its own SSH
//! client. The channel runs the bridge, `muxy stdio`, on the computer, so its
//! server sees an ordinary local connection.
//!
//! One end of a socket pair is the connection's stream. The app writes what
//! the channel receives into the other end, and a pump thread hands what the
//! SDK writes on to the app.

use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use muxy_client::bridge::{self, BridgeExit, Start};
use muxy_client::{Client, ClientError};
use muxy_protocol::transport::socket_pair;

use crate::MobileError;

/// Covers the remote shell starting and the bridge starting the server.
const READY_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a channel that ended early gets to report how.
const EXIT_TIMEOUT: Duration = Duration::from_secs(1);
/// How much of the channel's error output is kept for messages.
const STDERR_TAIL: usize = 8 * 1024;
const BUFFER: usize = 64 * 1024;

/// The command an SSH exec channel runs to reach the computer's server.
#[uniffi::export]
pub fn bridge_command() -> String {
    bridge::command(Start::IfNeeded).into()
}

// A callback interface: a foreign trait's generated Kotlin class would also
// get `close()` from `AutoCloseable`, and the two would clash.
/// The app's SSH channel, which the SDK writes to from an SDK thread.
#[uniffi::export(callback_interface)]
pub trait ChannelWriter: Send + Sync {
    /// Writes to the channel's stdin, in order, and may block while the
    /// channel is full. Throw once the channel can't take more.
    fn write(&self, bytes: Vec<u8>) -> Result<(), MobileError>;
    /// Closes the channel. Called once, when the connection ends or fails to
    /// start.
    fn close(&self) -> Result<(), MobileError>;
}

/// An SSH exec channel running `bridge_command()`, which the app feeds with
/// what the channel receives. Use a new one for each connection.
#[derive(uniffi::Object)]
pub struct BridgeChannel {
    /// The app's end: received bytes go in, and the SDK's bytes come out.
    socket: UnixStream,
    /// The SDK's end, until a connection takes it.
    stream: Mutex<Option<UnixStream>>,
    writer: Arc<dyn ChannelWriter>,
    ending: Ending,
}

impl fmt::Debug for BridgeChannel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BridgeChannel")
            .finish_non_exhaustive()
    }
}

#[uniffi::export]
impl BridgeChannel {
    #[uniffi::constructor]
    pub fn new(writer: Box<dyn ChannelWriter>) -> Result<Arc<Self>, MobileError> {
        let (socket, stream) = socket_pair().map_err(ClientError::Io)?;
        Ok(Arc::new(Self {
            socket,
            stream: Mutex::new(Some(stream)),
            writer: Arc::from(writer),
            ending: Ending::default(),
        }))
    }

    /// Passes on what the channel's stdout received, in order. It blocks
    /// while the SDK catches up, so call it from one background thread,
    /// never the main thread or the SSH library's event loop.
    pub fn receive(&self, bytes: Vec<u8>) {
        // Fails only once the connection is gone and nothing reads any more.
        let _ = (&self.socket).write_all(&bytes);
    }

    /// Passes on what the channel's stderr received; its end explains a
    /// connection that fails.
    pub fn receive_error(&self, bytes: Vec<u8>) {
        self.ending.push(&bytes);
    }

    /// The channel closed, or the app gave up on it. Call it after the last
    /// output, with the exit status if the server sent one. It also releases
    /// a `receive` that is waiting.
    pub fn finish(&self, exit_status: Option<i32>) {
        self.ending.finish(exit_status);
        let _ = self.socket.shutdown(Shutdown::Write);
    }
}

impl BridgeChannel {
    /// Connects once the bridge is ready, handing what the SDK writes to the
    /// app until the connection ends.
    pub(crate) fn connect(&self, host: &str) -> Result<Client, ClientError> {
        let stream = self
            .stream
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or_else(|| {
                io::Error::other("this channel was already used; open one for each connection")
            })?;
        let writer = Arc::clone(&self.writer);
        let pumping = self.socket.try_clone().and_then(|outgoing| {
            thread::Builder::new()
                .name("muxy-mobile-channel".into())
                .spawn(move || pump(outgoing, writer.as_ref()))
        });
        if let Err(error) = pumping {
            let _ = self.writer.close();
            return Err(error.into());
        }
        Client::connect_bridge(Box::new(stream), host, READY_TIMEOUT, || {
            self.ending.wait(EXIT_TIMEOUT)
        })
    }
}

/// Hands what the SDK writes to the channel until either side ends, then
/// closes the channel.
fn pump(mut socket: UnixStream, writer: &dyn ChannelWriter) {
    let mut buffer = vec![0; BUFFER];
    loop {
        match socket.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                if writer.write(buffer[..read].to_vec()).is_err() {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    // The SDK sees the connection end, and later output is dropped.
    let _ = socket.shutdown(Shutdown::Both);
    let _ = writer.close();
}

/// How the channel ended, to explain a connection that failed before the
/// bridge was ready.
#[derive(Default)]
struct Ending {
    state: Mutex<EndingState>,
    finished: Condvar,
}

#[derive(Default)]
struct EndingState {
    stderr: VecDeque<u8>,
    status: Option<i32>,
    finished: bool,
}

impl Ending {
    fn push(&self, bytes: &[u8]) {
        let mut state = self.lock();
        state.stderr.extend(bytes);
        let excess = state.stderr.len().saturating_sub(STDERR_TAIL);
        state.stderr.drain(..excess);
    }

    fn finish(&self, status: Option<i32>) {
        let mut state = self.lock();
        if !state.finished {
            state.status = status;
            state.finished = true;
        }
        drop(state);
        self.finished.notify_all();
    }

    /// Waits up to `timeout` for the channel to finish, then tells how it ended.
    fn wait(&self, timeout: Duration) -> BridgeExit {
        let (mut state, _) = self
            .finished
            .wait_timeout_while(self.lock(), timeout, |state| !state.finished)
            .unwrap_or_else(PoisonError::into_inner);
        BridgeExit {
            status: state.status,
            stderr: String::from_utf8_lossy(state.stderr.make_contiguous()).into_owned(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, EndingState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bridge_command_is_the_one_every_client_runs() {
        assert_eq!(bridge_command(), bridge::command(Start::IfNeeded));
    }

    #[test]
    fn the_ending_keeps_the_first_status_and_the_end_of_the_errors() {
        let ending = Ending::default();
        ending.push(&[b'a'; STDERR_TAIL]);
        ending.push(b"sh: 1: exec: muxy: not found\n");
        ending.finish(Some(127));
        ending.finish(None);
        let exit = ending.wait(Duration::ZERO);
        assert_eq!(exit.status, Some(127));
        assert_eq!(exit.stderr.len(), STDERR_TAIL);
        assert!(exit.stderr.ends_with("muxy: not found\n"));
    }

    #[test]
    fn an_unfinished_ending_reports_what_it_has_after_the_wait() {
        let ending = Ending::default();
        ending.push(b"still logging in\n");
        assert_eq!(
            ending.wait(Duration::from_millis(10)),
            BridgeExit {
                status: None,
                stderr: "still logging in\n".into(),
            }
        );
    }
}
