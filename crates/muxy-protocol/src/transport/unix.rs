use std::fs;
use std::io::{self, IoSlice, Read, Write};
use std::net::Shutdown;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use crate::transport::accepting::Accepting;
use crate::transport::{BindError, ByteStream, Listener, StreamCancellation};

#[derive(Debug)]
pub struct UnixSocketListener {
    socket: Accepting<UnixListener>,
}

impl UnixSocketListener {
    pub fn bind(path: impl AsRef<Path>) -> Result<Self, BindError> {
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let directory = fs::File::open(parent)?;
        directory.lock()?;

        let socket = match UnixListener::bind(path) {
            Ok(socket) => socket,
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                match UnixStream::connect(path) {
                    Ok(_) => return Err(BindError::InUse),
                    Err(probe) if probe.kind() == io::ErrorKind::ConnectionRefused => {
                        if !fs::symlink_metadata(path)?.file_type().is_socket() {
                            return Err(error.into());
                        }
                        fs::remove_file(path)?;
                        UnixListener::bind(path)?
                    }
                    Err(probe) => return Err(probe.into()),
                }
            }
            Err(error) => return Err(error.into()),
        };
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: Accepting::new(socket)?,
        })
    }
}

impl Listener for UnixSocketListener {
    fn accept(&self) -> io::Result<Box<dyn ByteStream>> {
        let stream = self.socket.accept(UnixListener::accept)?.0;
        stream.set_nonblocking(false)?;
        Ok(Box::new(stream))
    }

    fn close(&self) {
        self.socket.close();
    }
}

pub fn connect(path: impl AsRef<Path>) -> io::Result<Box<dyn ByteStream>> {
    let stream = UnixStream::connect(path)?;
    stream.set_nonblocking(false)?;
    Ok(Box::new(stream))
}

/// Two connected sockets, for adapters that pump another transport into one
/// end while the other end serves as an ordinary byte stream. On Apple
/// platforms both get `SO_NOSIGPIPE`, which std doesn't set on socket pairs,
/// since an iOS host app does not ignore SIGPIPE the way Rust executables do.
pub fn socket_pair() -> io::Result<(UnixStream, UnixStream)> {
    let (first, second) = UnixStream::pair()?;
    #[cfg(target_vendor = "apple")]
    for socket in [&first, &second] {
        rustix::net::sockopt::set_socket_nosigpipe(socket, true)?;
    }
    Ok((first, second))
}

impl ByteStream for UnixStream {
    fn cancellation(&self) -> io::Result<Box<dyn StreamCancellation>> {
        Ok(Box::new(SocketCancellation {
            stream: self.try_clone()?,
        }))
    }

    #[allow(clippy::type_complexity)]
    fn split(self: Box<Self>) -> io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
        let writer = SocketWriter {
            stream: self.try_clone()?,
        };
        Ok((self, Box::new(writer)))
    }
}

#[derive(Debug)]
struct SocketCancellation {
    stream: UnixStream,
}

impl StreamCancellation for SocketCancellation {
    fn cancel(&self) {
        let _ = self.stream.shutdown(Shutdown::Write);
        let _ = self.stream.shutdown(Shutdown::Read);
    }
}

#[derive(Debug)]
struct SocketWriter {
    stream: UnixStream,
}

impl Write for SocketWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream.write(bytes)
    }

    fn write_vectored(&mut self, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
        self.stream.write_vectored(buffers)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl Drop for SocketWriter {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Write);
    }
}
