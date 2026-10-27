use std::error::Error;
use std::io::{self, Read, Write};
use std::os::unix::fs::symlink;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use std::{env, fs, process};

use muxy_protocol::transport::{BindError, ByteStream, Listener, UnixSocketListener, connect};

type TestResult = Result<(), Box<dyn Error>>;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn live_socket_returns_in_use_and_remains_reachable() -> TestResult {
    let path = SocketPath::new()?;
    let listener = UnixSocketListener::bind(path.socket())?;
    let result = UnixSocketListener::bind(path.socket());

    assert!(matches!(result, Err(BindError::InUse)), "{result:?}");
    let (mut probe_reader, _) = listener.accept()?.split()?;
    assert_eq!(probe_reader.read(&mut [0])?, 0);

    let (_, mut client_writer) = connect(path.socket())?.split()?;
    let (mut server_reader, _) = listener.accept()?.split()?;
    client_writer.write_all(b"x")?;
    let mut byte = [0];
    server_reader.read_exact(&mut byte)?;
    assert_eq!(&byte, b"x");
    Ok(())
}

#[test]
fn bind_replaces_a_stale_socket() -> TestResult {
    let path = SocketPath::new()?;
    drop(UnixListener::bind(path.socket())?);
    assert!(path.socket().exists());

    let listener = UnixSocketListener::bind(path.socket())?;
    let _client = connect(path.socket())?;
    let _server = listener.accept()?;
    Ok(())
}

#[test]
fn bind_preserves_a_symlink_to_a_stale_socket() -> TestResult {
    let path = SocketPath::new()?;
    let target = path.directory.join("target");
    drop(UnixListener::bind(&target)?);
    symlink(&target, path.socket())?;

    assert!(matches!(
        UnixSocketListener::bind(path.socket()),
        Err(BindError::Io(_))
    ));
    assert_eq!(fs::read_link(path.socket())?, target);
    assert!(target.exists());
    Ok(())
}

#[test]
fn dropping_writer_sends_eof_while_reader_still_receives() -> TestResult {
    let (stream, mut peer) = UnixStream::pair()?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    peer.set_read_timeout(Some(TIMEOUT))?;
    let (mut reader, mut writer) = Box::new(stream).split()?;

    writer.write_all(b"last bytes")?;
    drop(writer);
    let mut output = Vec::new();
    peer.read_to_end(&mut output)?;
    assert_eq!(output, b"last bytes");

    peer.write_all(b"reply")?;
    let mut reply = [0; 5];
    reader.read_exact(&mut reply)?;
    assert_eq!(&reply, b"reply");
    Ok(())
}

struct SocketPath {
    directory: PathBuf,
}

impl SocketPath {
    fn new() -> io::Result<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let directory = env::temp_dir().join(format!(
            "muxy-t-{}-{}",
            process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory)?;
        Ok(Self { directory })
    }

    fn socket(&self) -> PathBuf {
        self.directory.join("socket")
    }
}

impl Drop for SocketPath {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
