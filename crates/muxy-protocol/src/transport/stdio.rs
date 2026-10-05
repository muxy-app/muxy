//! A child process's stdin and stdout as a byte stream, and a relay that
//! carries a byte stream over any reader and writer, such as a process's own
//! stdin and stdout.

use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::transport::unix::socket_pair;
use crate::transport::{ByteStream, StreamCancellation};

const BUFFER: usize = 64 * 1024;
const EXIT_POLL: Duration = Duration::from_millis(10);

/// A child process's stdin and stdout as one byte stream, such as `ssh`
/// running `muxy stdio` on another computer.
///
/// Both are joined to one end of a Unix socket pair rather than to two pipes.
/// Cancelling can then wake blocked reads and writes at once, even while a
/// grandchild still holds the child's end.
#[derive(Debug)]
pub struct ChildStream {
    /// Taken by `split`; while it is here, dropping the stream kills the child.
    socket: Option<UnixStream>,
    process: ChildProcess,
}

/// The process behind a [`ChildStream`], kept to learn how it exited.
///
/// The child is killed and reaped once the stream is cancelled, or once
/// nothing refers to it any more.
#[derive(Clone, Debug)]
pub struct ChildProcess {
    process: Arc<Process>,
}

#[derive(Debug)]
struct Process {
    child: Mutex<Child>,
}

impl ChildStream {
    /// Starts `command` with its stdin and stdout joined to the stream. Its
    /// stderr stays as the command set it; take a piped one with
    /// [`ChildStream::take_stderr`]. The command is dropped once started:
    /// it holds a copy of the child's end, and the stream only sees the child
    /// finish once every copy of that end is closed.
    pub fn spawn(mut command: Command) -> io::Result<Self> {
        let (socket, child_end) = socket_pair()?;
        command
            .stdin(Stdio::from(OwnedFd::from(child_end.try_clone()?)))
            .stdout(Stdio::from(OwnedFd::from(child_end)));
        let child = command.spawn()?;
        drop(command);
        Ok(Self {
            socket: Some(socket),
            process: ChildProcess {
                process: Arc::new(Process {
                    child: Mutex::new(child),
                }),
            },
        })
    }

    /// The child's stderr, if the command piped it. Keep reading it, or the
    /// child blocks once the pipe is full.
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.process.lock().stderr.take()
    }

    pub fn process(&self) -> ChildProcess {
        self.process.clone()
    }

    fn socket(&self) -> io::Result<&UnixStream> {
        self.socket
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "stream already split"))
    }
}

impl ByteStream for ChildStream {
    /// Shuts the stream down, then kills and reaps the child.
    fn cancellation(&self) -> io::Result<Box<dyn StreamCancellation>> {
        Ok(Box::new(ChildCancellation {
            socket: self.socket()?.try_clone()?,
            process: self.process.clone(),
        }))
    }

    #[allow(clippy::type_complexity)]
    fn split(mut self: Box<Self>) -> io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
        let socket = self.socket()?.try_clone()?;
        let (reader, writer) = Box::new(socket).split()?;
        self.socket = None;
        Ok((
            Box::new(Half {
                inner: reader,
                _process: self.process.clone(),
            }),
            Box::new(Half {
                inner: writer,
                _process: self.process.clone(),
            }),
        ))
    }
}

impl Drop for ChildStream {
    fn drop(&mut self) {
        if self.socket.is_some() {
            self.process.kill();
        }
    }
}

impl ChildProcess {
    /// Waits up to `timeout` for the child to exit on its own; `None` if it
    /// is still running.
    pub fn wait_timeout(&self, timeout: Duration) -> io::Result<Option<ExitStatus>> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.lock().try_wait()? {
                return Ok(Some(status));
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            thread::sleep(left.min(EXIT_POLL));
        }
    }

    fn kill(&self) {
        kill(&mut self.lock());
    }

    fn lock(&self) -> MutexGuard<'_, Child> {
        self.process
            .child
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        kill(self.child.get_mut().unwrap_or_else(PoisonError::into_inner));
    }
}

/// Safe to repeat: std never signals a child it has already reaped, whose
/// process ID may belong to another process by now.
fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

struct ChildCancellation {
    socket: UnixStream,
    process: ChildProcess,
}

impl StreamCancellation for ChildCancellation {
    fn cancel(&self) {
        let _ = self.socket.shutdown(Shutdown::Both);
        self.process.kill();
    }
}

/// One half of a split [`ChildStream`]; the child lives at least as long.
struct Half<T> {
    inner: T,
    _process: ChildProcess,
}

impl<T: Read> Read for Half<T> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buffer)
    }
}

impl<T: Write> Write for Half<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Copies `input` into `stream` and `stream` into `output` until the stream
/// ends, writing and flushing every chunk as it arrives.
///
/// When `input` ends, the stream's write side closes and its remaining output
/// is still forwarded. Output that stops accepting bytes ends the relay
/// normally too. `input` is read on its own thread, which ends with `input`
/// or at its next chunk once the stream has closed.
pub fn relay(
    stream: Box<dyn ByteStream>,
    input: impl Read + Send + 'static,
    mut output: impl Write,
) -> io::Result<()> {
    let cancellation = stream.cancellation()?;
    let (mut reader, writer) = stream.split()?;
    thread::Builder::new()
        .name("stdio-relay".into())
        .spawn(move || forward_input(input, writer))?;
    let result = forward_output(&mut reader, &mut output);
    cancellation.cancel();
    result
}

/// Dropping the writer at the end closes the stream's write side.
fn forward_input(mut input: impl Read, mut writer: Box<dyn Write + Send>) {
    let mut buffer = vec![0; BUFFER];
    loop {
        match input.read(&mut buffer) {
            Ok(0) => return,
            Ok(read) => {
                if writer
                    .write_all(&buffer[..read])
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    return;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

fn forward_output(reader: &mut dyn Read, output: &mut impl Write) -> io::Result<()> {
    let mut buffer = vec![0; BUFFER];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        match output
            .write_all(&buffer[..read])
            .and_then(|()| output.flush())
        {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}
