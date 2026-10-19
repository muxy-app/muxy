mod support;

use std::error::Error;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Barrier};
use std::time::Duration;
use std::{fs, thread};

use muxy_protocol::transport::{
    BindError, ByteStream, Listener, LocalListener, UnixSocketListener, connect, socket_pair,
};
use support::TempFolder;

type TestResult = Result<(), Box<dyn Error>>;

const TIMEOUT: Duration = Duration::from_secs(5);
const BLOCKED_INTERVAL: Duration = Duration::from_millis(50);

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
    // On Windows this is a verbatim `\\?\C:\…` path.
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
fn concurrent_stale_binds_keep_exactly_one_live_listener() -> TestResult {
    let path = TempFolder::new()?;
    for _ in 0..16 {
        drop(LocalListener::bind(path.socket())?);
        let start = Arc::new(Barrier::new(8));
        let done = Arc::new(Barrier::new(8));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let socket = path.socket();
            let start = Arc::clone(&start);
            let done = Arc::clone(&done);
            workers.push(thread::spawn(move || {
                start.wait();
                let result = UnixSocketListener::bind(socket);
                done.wait();
                result
            }));
        }

        let results = workers
            .into_iter()
            .map(|worker| worker.join().map_err(|_| "bind thread panicked"))
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(
            results
                .iter()
                .all(|result| matches!(result, Ok(_) | Err(BindError::InUse)))
        );
        drop(results);
        fs::remove_file(path.socket())?;
    }
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
fn accepted_stream_blocks_until_bytes_arrive() -> TestResult {
    let path = TempFolder::new()?;
    let listener = UnixSocketListener::bind(path.socket())?;
    let (_, mut writer) = connect(path.socket())?.split()?;
    let (mut reader, _server_writer) = listener.accept()?.split()?;
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _ = ready_sender.send(());
        let mut byte = [0];
        let result = reader.read_exact(&mut byte).map(|()| byte);
        let _ = sender.send(result);
    });

    ready_receiver.recv_timeout(TIMEOUT)?;
    assert!(matches!(
        receiver.recv_timeout(BLOCKED_INTERVAL),
        Err(RecvTimeoutError::Timeout)
    ));
    writer.write_all(b"x")?;
    assert_eq!(receiver.recv_timeout(TIMEOUT)??, *b"x");
    worker.join().map_err(|_| "reader thread panicked")?;
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

#[test]
fn close_wakes_pending_accepts_and_rejects_future_accepts() -> TestResult {
    let path = TempFolder::new()?;
    let listener = Arc::new(UnixSocketListener::bind(path.socket())?);
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let mut workers = Vec::new();
    for _ in 0..2 {
        let listener = Arc::clone(&listener);
        let ready_sender = ready_sender.clone();
        let sender = sender.clone();
        workers.push(thread::spawn(move || {
            let _ = ready_sender.send(());
            let _ = sender.send(listener.accept().err().map(|error| error.kind()));
        }));
    }

    for _ in 0..2 {
        ready_receiver.recv_timeout(TIMEOUT)?;
    }
    assert!(matches!(
        receiver.recv_timeout(BLOCKED_INTERVAL),
        Err(RecvTimeoutError::Timeout)
    ));
    listener.close();
    listener.close();

    for worker in workers {
        assert_eq!(
            receiver.recv_timeout(TIMEOUT)?,
            Some(io::ErrorKind::NotConnected)
        );
        worker.join().map_err(|_| "accept thread panicked")?;
    }
    assert_eq!(
        listener.accept().err().map(|error| error.kind()),
        Some(io::ErrorKind::NotConnected)
    );
    assert!(connect(path.socket()).is_err());
    let _replacement = UnixSocketListener::bind(path.socket())?;
    Ok(())
}

#[test]
fn cancellation_wakes_a_blocked_reader_and_closes_both_directions() -> TestResult {
    let (stream, mut peer) = socket_pair()?;
    peer.set_read_timeout(Some(TIMEOUT))?;
    let cancellation = stream.cancellation()?;
    let (mut reader, mut writer) = Box::new(stream).split()?;
    let (ready, started) = mpsc::channel();
    let (sender, finished) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _ = ready.send(());
        let _ = sender.send(reader.read(&mut [0]));
    });
    started.recv_timeout(TIMEOUT)?;
    assert!(matches!(
        finished.recv_timeout(BLOCKED_INTERVAL),
        Err(RecvTimeoutError::Timeout)
    ));
    cancellation.cancel();
    cancellation.cancel();
    assert_eq!(finished.recv_timeout(TIMEOUT)??, 0);
    worker.join().map_err(|_| "reader panicked")?;
    assert!(writer.write_all(b"closed").is_err());
    assert_eq!(peer.read(&mut [0])?, 0);
    Ok(())
}

#[test]
fn cancellation_wakes_a_blocked_writer_after_peer_write_shutdown() -> TestResult {
    let (stream, peer) = socket_pair()?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    let cancellation = stream.cancellation()?;
    let (mut reader, mut writer) = Box::new(stream).split()?;
    peer.shutdown(Shutdown::Write)?;
    assert_eq!(reader.read(&mut [0])?, 0);
    let (ready, started) = mpsc::channel();
    let (sender, finished) = mpsc::channel();
    let worker = thread::spawn(move || {
        let _ = ready.send(());
        // Windows takes one write of any size into an empty buffer, so keep
        // writing until a write blocks.
        let chunk = vec![0; 64 * 1024];
        let _ = sender.send(loop {
            if let Err(error) = writer.write_all(&chunk) {
                break Err::<(), _>(error);
            }
        });
    });
    started.recv_timeout(TIMEOUT)?;
    assert!(matches!(
        finished.recv_timeout(BLOCKED_INTERVAL),
        Err(RecvTimeoutError::Timeout)
    ));
    cancellation.cancel();
    let result = finished.recv_timeout(TIMEOUT);
    drop(peer);
    worker.join().map_err(|_| "writer panicked")?;
    assert!(result?.is_err());
    Ok(())
}
