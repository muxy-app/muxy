#[cfg(unix)]
#[cfg_attr(
    not(any(target_os = "linux", target_os = "android")),
    allow(unsafe_code, reason = "getpeereid")
)]
mod credentials;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, IoSlice, Read, Write};
use std::net::Shutdown;
use std::path::Path;

use crate::transport::accepting::Accepting;
use crate::transport::{BindError, ByteStream, Listener, StreamCancellation};

#[cfg(unix)]
pub type LocalStream = std::os::unix::net::UnixStream;
#[cfg(windows)]
pub type LocalStream = uds_windows::UnixStream;

#[cfg(unix)]
pub type LocalListener = std::os::unix::net::UnixListener;
#[cfg(windows)]
pub type LocalListener = uds_windows::UnixListener;

#[cfg(unix)]
type Listening = LocalListener;
#[cfg(windows)]
type Listening = windows::Pollable;

#[derive(Debug)]
pub struct UnixSocketListener {
    socket: Accepting<Listening>,
}

impl UnixSocketListener {
    pub fn bind(path: impl AsRef<Path>) -> Result<Self, BindError> {
        let path = socket_address(path.as_ref());
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if !parent.try_exists()? {
            muxy_core::files::private_dir(parent)?;
        }
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
            socket: Accepting::new(Listening::from(socket))?,
        })
    }
}

impl Listener for UnixSocketListener {
    fn accept(&self) -> io::Result<Box<dyn ByteStream>> {
        let stream = loop {
            let stream = self.socket.accept(Listening::accept)?.0;
            if trusted_peer(&stream) {
                break stream;
            }
        };
        stream.set_nonblocking(false)?;
        Ok(Box::new(stream))
    }

    fn close(&self) {
        self.socket.close();
    }
}

pub fn connect(path: impl AsRef<Path>) -> io::Result<Box<dyn ByteStream>> {
    Ok(Box::new(connect_local(path)?))
}

pub fn connect_local(path: impl AsRef<Path>) -> io::Result<LocalStream> {
    let stream = LocalStream::connect(socket_address(path.as_ref()))?;
    stream.set_nonblocking(false)?;
    Ok(stream)
}

fn trusted_peer(stream: &LocalStream) -> bool {
    #[cfg(unix)]
    {
        credentials::peer_uid(stream).is_ok_and(|uid| uid == credentials::own_uid() || uid == 0)
    }
    #[cfg(windows)]
    {
        let _ = stream;
        true
    }
}

#[cfg(unix)]
pub fn peer_is_same_user(stream: &LocalStream) -> io::Result<bool> {
    Ok(credentials::peer_uid(stream)? == credentials::own_uid())
}

#[derive(Debug)]
pub struct SocketDirectoryLock {
    _file: File,
}

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

#[cfg(windows)]
pub fn peer_pid(stream: &LocalStream) -> io::Result<u32> {
    windows::peer_pid(stream)
}

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
        let name = format!("muxy-{:016x}.sock", uuid::Uuid::new_v4().as_u64_pair().0);
        pair_at(&std::env::temp_dir().join(name), || Ok(()))
    }
}

#[cfg(windows)]
fn pair_at(
    path: &Path,
    after_bind: impl FnOnce() -> io::Result<()>,
) -> io::Result<(LocalStream, LocalStream)> {
    use std::time::{Duration, Instant};

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

pub(crate) fn shut_down(stream: &LocalStream) {
    let _ = stream.shutdown(Shutdown::Write);
    let _ = stream.shutdown(Shutdown::Read);
    interrupt(stream);
}

#[cfg(unix)]
pub(crate) fn interrupt<T>(_socket: &T) {}

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

#[cfg(all(test, unix))]
mod unix_tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn a_missing_socket_folder_is_made_private_and_peers_are_this_user() -> io::Result<()> {
        let root = Path::new("/tmp").join(format!(
            "muxy-peer-{:x}",
            uuid::Uuid::new_v4().as_u64_pair().0
        ));
        let folder = root.join("a").join("b");
        let path = folder.join("s.sock");
        let result = (|| {
            let listener = UnixSocketListener::bind(&path).map_err(io::Error::other)?;
            assert_eq!(fs::metadata(&folder)?.permissions().mode() & 0o777, 0o700);
            let client = connect_local(&path)?;
            drop(listener.accept()?);
            assert!(peer_is_same_user(&client)?);
            Ok(())
        })();
        fs::remove_dir_all(&root)?;
        result
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::io::{BufRead, BufReader};
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};

    use super::*;

    const STRANGER: &str = "MUXY_TEST_STRANGER";

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
