//! Local sockets: Unix-domain sockets on Unix, and `AF_UNIX` sockets on
//! Windows 10 1803 and later.

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, IoSlice, Read, Write};
use std::net::Shutdown;
use std::path::Path;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use crate::transport::{BindError, ByteStream, Listener, StreamCancellation};

/// A connected local socket.
#[cfg(unix)]
pub type LocalStream = std::os::unix::net::UnixStream;
/// A connected local socket.
#[cfg(windows)]
pub type LocalStream = uds_windows::UnixStream;

/// A listening local socket.
#[cfg(unix)]
pub type LocalListener = std::os::unix::net::UnixListener;
/// A listening local socket.
#[cfg(windows)]
pub type LocalListener = uds_windows::UnixListener;

#[derive(Debug)]
pub struct UnixSocketListener {
    socket: Mutex<Option<LocalListener>>,
    closed: Condvar,
}

impl UnixSocketListener {
    pub fn bind(path: impl AsRef<Path>) -> Result<Self, BindError> {
        let path = socket_address(path.as_ref());
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)?;
        let _lock = lock_socket_directory(parent)?;

        let socket = match LocalListener::bind(&path) {
            Ok(socket) => socket,
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                match LocalStream::connect(&path) {
                    Ok(_) => return Err(BindError::InUse),
                    Err(probe) if probe.kind() == io::ErrorKind::ConnectionRefused => {
                        if !is_socket(&path)? {
                            return Err(error.into());
                        }
                        fs::remove_file(&path)?;
                        LocalListener::bind(&path)?
                    }
                    Err(probe) => return Err(probe.into()),
                }
            }
            Err(error) => return Err(error.into()),
        };
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: Mutex::new(Some(socket)),
            closed: Condvar::new(),
        })
    }
}

impl Listener for UnixSocketListener {
    fn accept(&self) -> io::Result<Box<dyn ByteStream>> {
        let mut guard = self
            .socket
            .lock()
            .map_err(|_| io::Error::other("listener lock poisoned"))?;
        loop {
            let socket = guard
                .as_ref()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "listener is closed"))?;
            match socket.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false)?;
                    return Ok(Box::new(stream));
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    (guard, _) = self
                        .closed
                        .wait_timeout(guard, Duration::from_millis(10))
                        .map_err(|_| io::Error::other("listener lock poisoned"))?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn close(&self) {
        let mut guard = self
            .socket
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.take();
        self.closed.notify_all();
    }
}

pub fn connect(path: impl AsRef<Path>) -> io::Result<Box<dyn ByteStream>> {
    Ok(Box::new(connect_local(path)?))
}

/// Like [`connect`], but keeps the socket's type, so a caller can ask who is
/// at the other end before it carries any data.
pub fn connect_local(path: impl AsRef<Path>) -> io::Result<LocalStream> {
    let stream = LocalStream::connect(socket_address(path.as_ref()))?;
    stream.set_nonblocking(false)?;
    Ok(stream)
}

/// Held while a socket in `parent` is bound, replaced or removed, so those
/// steps exclude each other across processes.
#[derive(Debug)]
pub struct SocketDirectoryLock {
    _file: File,
}

/// Locks the folder that holds a socket. Windows cannot lock a directory, so
/// there it locks `<parent>\server.lock`.
pub fn lock_socket_directory(parent: &Path) -> io::Result<SocketDirectoryLock> {
    #[cfg(unix)]
    let file = File::open(parent)?;
    #[cfg(windows)]
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join("server.lock"))?;
    file.lock()?;
    Ok(SocketDirectoryLock { _file: file })
}

/// The process ID of whoever holds the other end of `stream`.
#[cfg(windows)]
pub fn peer_pid(stream: &LocalStream) -> io::Result<u32> {
    windows::peer_pid(stream)
}

/// Two connected sockets, for adapters that pump another transport into one
/// end while the other end serves as an ordinary byte stream. On Apple
/// platforms both get `SO_NOSIGPIPE`, which std doesn't set on socket pairs,
/// since an iOS host app does not ignore SIGPIPE the way Rust executables do.
///
/// Windows has no `socketpair`, so there the pair meets through a listener
/// with a random name in the user's temporary folder, which is removed
/// again before this returns.
pub fn socket_pair() -> io::Result<(LocalStream, LocalStream)> {
    #[cfg(unix)]
    {
        let (first, second) = LocalStream::pair()?;
        #[cfg(target_vendor = "apple")]
        for socket in [&first, &second] {
            rustix::net::sockopt::set_socket_nosigpipe(socket, true)?;
        }
        Ok((first, second))
    }
    #[cfg(windows)]
    {
        // Short, since Windows socket paths must fit in 108 bytes.
        let name = format!("muxy-{:016x}.sock", uuid::Uuid::new_v4().as_u64_pair().0);
        pair_at(&std::env::temp_dir().join(name), || Ok(()))
    }
}

/// Accepts only a connection from this process, so a stranger that connects
/// first is turned away. `after_bind` lets tests be that stranger.
#[cfg(windows)]
fn pair_at(
    path: &Path,
    after_bind: impl FnOnce() -> io::Result<()>,
) -> io::Result<(LocalStream, LocalStream)> {
    use std::time::Instant;

    const ACCEPT_TIMEOUT: Duration = Duration::from_secs(5);

    struct Remove<'a>(&'a Path);
    impl Drop for Remove<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0);
        }
    }

    let path = socket_address(path);
    let listener = LocalListener::bind(&path)?;
    let _remove = Remove(&path);
    listener.set_nonblocking(true)?;
    after_bind()?;
    let first = LocalStream::connect(&path)?;
    let deadline = Instant::now() + ACCEPT_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((second, _)) if peer_pid(&second)? == std::process::id() => {
                second.set_nonblocking(false)?;
                return Ok((first, second));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(error),
        }
    }
}

/// The path as a socket address. Windows takes plain paths there, so a
/// verbatim `\\?\C:\…` becomes `C:\…` and `\\?\UNC\…` becomes `\\…`.
fn socket_address(path: &Path) -> Cow<'_, Path> {
    #[cfg(windows)]
    if let Some(text) = path.to_str() {
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return Cow::Owned(format!(r"\\{rest}").into());
        }
        if let Some(rest) = text.strip_prefix(r"\\?\")
            && rest.as_bytes().get(1) == Some(&b':')
        {
            return Cow::Borrowed(Path::new(rest));
        }
    }
    Cow::Borrowed(path)
}

/// Whether `path` itself, not a link's target, is a socket.
fn is_socket(path: &Path) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        Ok(fs::symlink_metadata(path)?.file_type().is_socket())
    }
    #[cfg(windows)]
    {
        windows::is_socket(path)
    }
}

/// Shuts both directions down and wakes whatever is blocked on them.
pub(crate) fn shut_down(stream: &LocalStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let _ = stream.shutdown(Shutdown::Read);
    interrupt(stream);
}

/// Wakes reads and writes blocked on `socket` once it is shut down. Unix
/// wakes them on shutdown alone; Windows needs their I/O cancelled.
#[cfg(unix)]
pub(crate) fn interrupt<T>(_socket: &T) {}

/// Wakes reads and writes blocked on `socket` once it is shut down. Unix
/// wakes them on shutdown alone; Windows needs their I/O cancelled.
#[cfg(windows)]
pub(crate) fn interrupt(socket: &impl std::os::windows::io::AsRawSocket) {
    windows::interrupt(socket);
}

impl ByteStream for LocalStream {
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
        #[cfg(unix)]
        let reader = self;
        #[cfg(windows)]
        let reader = Box::new(SocketReader { stream: *self });
        Ok((reader, Box::new(writer)))
    }
}

#[derive(Debug)]
struct SocketCancellation {
    stream: LocalStream,
}

impl StreamCancellation for SocketCancellation {
    fn cancel(&self) {
        shut_down(&self.stream);
    }
}

/// Reads as Unix does once the socket is shut down: the end of the stream,
/// where Winsock reports the shutdown or the cancelled read as an error.
#[cfg(windows)]
#[derive(Debug)]
struct SocketReader {
    stream: LocalStream,
}

#[cfg(windows)]
impl Read for SocketReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        use windows_sys::Win32::Networking::WinSock::{WSAEINTR, WSAESHUTDOWN};
        match self.stream.read(buffer) {
            Err(error) if matches!(error.raw_os_error(), Some(WSAESHUTDOWN | WSAEINTR)) => Ok(0),
            result => result,
        }
    }
}

#[derive(Debug)]
struct SocketWriter {
    stream: LocalStream,
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

#[cfg(all(test, windows))]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};

    use super::*;

    const STRANGER: &str = "MUXY_TEST_STRANGER";

    /// Another process for the tests below: when one of them runs it, it
    /// connects to `$MUXY_TEST_STRANGER` and holds on until its stdin closes.
    #[test]
    fn stranger() -> io::Result<()> {
        let Some(path) = std::env::var_os(STRANGER) else {
            return Ok(());
        };
        let _stream = LocalStream::connect(path)?;
        writeln!(io::stdout(), "connected")?;
        io::stdin().read_to_end(&mut Vec::new())?;
        Ok(())
    }

    struct Stranger(Child);

    impl Stranger {
        fn connect(path: &Path) -> io::Result<Self> {
            let mut child = Command::new(std::env::current_exe()?)
                .args(["--exact", "transport::unix::tests::stranger", "--nocapture"])
                .env(STRANGER, path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;
            let stdout = child.stdout.take();
            let stranger = Self(child);
            for line in BufReader::new(stdout.ok_or(io::ErrorKind::BrokenPipe)?).lines() {
                if line?.contains("connected") {
                    return Ok(stranger);
                }
            }
            Err(io::Error::other("the stranger did not connect"))
        }
    }

    impl Drop for Stranger {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn socket_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("muxy-unit-{}-{name}.sock", std::process::id()))
    }

    #[test]
    fn peer_pid_names_the_process_at_the_other_end() -> io::Result<()> {
        let path = socket_path("peer");
        let _ = fs::remove_file(&path);
        let listener = LocalListener::bind(&path)?;
        let stranger = Stranger::connect(&path)?;
        let (accepted, _) = listener.accept()?;
        let result = peer_pid(&accepted);
        fs::remove_file(&path)?;
        assert_eq!(result?, stranger.0.id());
        Ok(())
    }

    #[test]
    fn socket_pair_turns_away_a_stranger_that_connects_first() -> io::Result<()> {
        let path = socket_path("pair");
        let _ = fs::remove_file(&path);
        let mut stranger = None;
        let (mut first, mut second) = pair_at(&path, || {
            stranger = Some(Stranger::connect(&path)?);
            Ok(())
        })?;
        assert!(stranger.is_some());
        assert!(!path.exists());
        assert_eq!(peer_pid(&second)?, std::process::id());
        first.write_all(b"x")?;
        let mut byte = [0];
        second.read_exact(&mut byte)?;
        assert_eq!(&byte, b"x");
        Ok(())
    }
}
