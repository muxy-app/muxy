use std::io::{self, Read, Write};
use std::process::{Child, ChildStderr, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use muxy_core::process::ExitWatch;

use crate::transport::unix::{shut_down, socket_pair};
use crate::transport::{ByteStream, LocalStream, StreamCancellation};

const BUFFER: usize = 64 * 1024;

#[derive(Debug)]
pub struct ChildStream {
    socket: Option<LocalStream>,
    process: ChildProcess,
}

#[derive(Clone, Debug)]
pub struct ChildProcess {
    process: Arc<Process>,
}

#[derive(Debug)]
struct Process {
    child: Mutex<Child>,
}

impl ChildStream {
    pub fn spawn(mut command: Command) -> io::Result<Self> {
        let (socket, child_end) = socket_pair()?;
        #[cfg(unix)]
        {
            use std::os::fd::OwnedFd;
            command
                .stdin(Stdio::from(OwnedFd::from(child_end.try_clone()?)))
                .stdout(Stdio::from(OwnedFd::from(child_end)));
        }
        #[cfg(windows)]
        command.stdin(Stdio::piped()).stdout(Stdio::piped());
        let child = command.spawn()?;
        drop(command);
        let stream = Self {
            socket: Some(socket),
            process: ChildProcess {
                process: Arc::new(Process {
                    child: Mutex::new(child),
                }),
            },
        };
        #[cfg(windows)]
        stream.copy_pipes(child_end)?;
        Ok(stream)
    }

    #[cfg(windows)]
    fn copy_pipes(&self, child_end: LocalStream) -> io::Result<()> {
        let (stdin, stdout) = {
            let mut child = self.process.lock();
            (child.stdin.take(), child.stdout.take())
        };
        let (Some(mut stdin), Some(mut stdout)) = (stdin, stdout) else {
            return Err(io::Error::other(
                "the child's stdin and stdout are not piped",
            ));
        };
        let input = child_end.try_clone()?;
        thread::Builder::new()
            .name("child-stdout".into())
            .spawn(move || {
                let _ = io::copy(&mut stdout, &mut &child_end);
                let _ = child_end.shutdown(std::net::Shutdown::Write);
            })?;
        thread::Builder::new()
            .name("child-stdin".into())
            .spawn(move || {
                let _ = io::copy(&mut &input, &mut stdin);
            })?;
        Ok(())
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.process.lock().stderr.take()
    }

    pub fn process(&self) -> ChildProcess {
        self.process.clone()
    }

    fn socket(&self) -> io::Result<&LocalStream> {
        self.socket
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "stream already split"))
    }
}

impl ByteStream for ChildStream {
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
    pub fn wait_timeout(&self, timeout: Duration) -> io::Result<Option<ExitStatus>> {
        let mut exit = {
            let mut child = self.lock();
            if let Some(status) = child.try_wait()? {
                return Ok(Some(status));
            }
            ExitWatch::new(&child)?
        };
        exit.wait(Some(timeout))?;
        self.lock().try_wait()
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

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

struct ChildCancellation {
    socket: LocalStream,
    process: ChildProcess,
}

impl StreamCancellation for ChildCancellation {
    fn cancel(&self) {
        shut_down(&self.socket);
        self.process.kill();
    }
}

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
