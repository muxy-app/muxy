mod support;

use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::time::Duration;

use muxy_protocol::transport::{
    BindError, ByteStream, Listener, LocalListener, UnixSocketListener, connect, socket_pair,
};
use support::TempFolder;

type TestResult = Result<(), Box<dyn Error>>;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn live_socket_returns_in_use_and_remains_reachable() -> TestResult {
    let path = TempFolder::new()?;
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
fn sockets_work_under_a_non_ascii_folder_and_a_canonical_path() -> TestResult {
    let path = TempFolder::new()?;
    let folder = path.path.join("dév-日本");
    fs::create_dir(&folder)?;
    let socket = fs::canonicalize(&folder)?.join("socket");
    let listener = UnixSocketListener::bind(&socket)?;
    let (_, mut writer) = connect(&socket)?.split()?;
    let (mut reader, _) = listener.accept()?.split()?;
    writer.write_all(b"x")?;
    let mut byte = [0];
    reader.read_exact(&mut byte)?;
    assert_eq!(&byte, b"x");
    Ok(())
}

#[test]
fn bind_replaces_a_stale_socket() -> TestResult {
    let path = TempFolder::new()?;
    drop(LocalListener::bind(path.socket())?);
    assert!(path.socket().exists());

    let listener = UnixSocketListener::bind(path.socket())?;
    let _client = connect(path.socket())?;
    let _server = listener.accept()?;
    Ok(())
}

#[test]
fn bind_preserves_a_symlink_to_a_stale_socket() -> TestResult {
    let path = TempFolder::new()?;
    let target = path.path.join("target");
    drop(LocalListener::bind(&target)?);
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, path.socket())?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&target, path.socket())?;

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
    let (stream, mut peer) = socket_pair()?;
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
