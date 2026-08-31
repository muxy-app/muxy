pub mod support;

use muxy_proto::session::codec::{
    FrameHeader, FrameKind, HEADER_BYTES, decode_header, decode_structured, encode_frame,
    encode_structured,
};
use muxy_proto::session::{
    BuildMode, ClientKind, Hello, MAX_CONTROL_CONNECTIONS, MAX_STRUCTURED_FRAME_BYTES,
    ProtocolVersion, VersionMismatch,
};
use muxy_session::{SessionClient, current_build_mode};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::{Duration, Instant};
use support::{TestDaemon, spawn_isolated_daemon};

#[test]
fn build_mode_mismatch_and_oversized_first_frame_fail_without_harming_daemon() {
    let daemon = TestDaemon::start();
    let mismatched = match current_build_mode() {
        BuildMode::Development => BuildMode::Production,
        BuildMode::Production => BuildMode::Development,
    };
    let error = match SessionClient::connect(&daemon.socket, mismatched) {
        Ok(_) => panic!("mismatched build mode connected"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);

    let mut hostile = UnixStream::connect(&daemon.socket).unwrap();
    hostile
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let header = FrameHeader {
        version: ProtocolVersion::CURRENT,
        kind: FrameKind::Hello,
        flags: 0,
        request_id: 0,
        payload_len: (MAX_STRUCTURED_FRAME_BYTES + 1) as u32,
    };
    hostile.write_all(&header.encode()).unwrap();
    let mut byte = [0u8; 1];
    match hostile.read(&mut byte) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
            ) => {}
        result => panic!("oversized frame connection remained usable: {result:?}"),
    }

    let mut valid = daemon.client();
    valid.ping().unwrap();
    drop(valid);
    daemon.finish(false);
}

#[test]
fn protocol_major_mismatch_is_explicit_and_daemon_remains_available() {
    let daemon = TestDaemon::start();
    let mut stream = UnixStream::connect(&daemon.socket).unwrap();
    let hello = Hello {
        version: ProtocolVersion {
            major: ProtocolVersion::CURRENT.major + 1,
            minor: 0,
        },
        client_kind: ClientKind::Control,
        process_id: std::process::id(),
        nonce: [7; 32],
    };
    let payload = encode_structured(&hello).unwrap();
    let frame = encode_frame(FrameKind::Hello, 0, &payload).unwrap();
    stream.write_all(&frame).unwrap();
    let mut header_bytes = [0; HEADER_BYTES];
    stream.read_exact(&mut header_bytes).unwrap();
    let header = decode_header(&header_bytes).unwrap();
    assert_eq!(header.kind, FrameKind::VersionMismatch);
    let mut payload = vec![0; header.payload_len as usize];
    stream.read_exact(&mut payload).unwrap();
    let mismatch: VersionMismatch = decode_structured(&payload).unwrap();
    assert_eq!(mismatch.received, hello.version);
    assert_eq!(mismatch.supported, ProtocolVersion::CURRENT);
    drop(stream);
    let mut client = daemon.client();
    client.ping().unwrap();
    drop(client);
    daemon.finish(false);
}

#[test]
fn singleton_failure_never_unlinks_the_live_socket() {
    let daemon = TestDaemon::start();
    let metadata = std::fs::symlink_metadata(&daemon.socket).unwrap();
    let mut second = spawn_isolated_daemon(&daemon.socket);
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = second.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "second daemon did not fail");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(!status.success());
    let after = std::fs::symlink_metadata(&daemon.socket).unwrap();
    assert_eq!(
        std::os::unix::fs::MetadataExt::dev(&metadata),
        std::os::unix::fs::MetadataExt::dev(&after)
    );
    assert_eq!(
        std::os::unix::fs::MetadataExt::ino(&metadata),
        std::os::unix::fs::MetadataExt::ino(&after)
    );
    let mut client = daemon.client();
    client.ping().unwrap();
    drop(client);
    daemon.finish(false);
}

#[test]
fn incomplete_handshakes_time_out_and_release_daemon_capacity() {
    let daemon = TestDaemon::start();
    daemon.release_startup_client();
    let mut stalled = Vec::new();
    for _ in 0..(MAX_CONTROL_CONNECTIONS * 2) {
        let mut stream = UnixStream::connect(&daemon.socket).unwrap();
        stream.write_all(b"M").unwrap();
        stalled.push(stream);
    }
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Ok(mut client) = SessionClient::connect(&daemon.socket, current_build_mode())
            && client.ping().is_ok()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "incomplete handshakes did not release daemon capacity"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(stalled);
    daemon.finish(false);
}

#[test]
fn stalled_server_cannot_hold_a_client_handshake_indefinitely() {
    let root = tempfile::Builder::new()
        .prefix("p8-isolated-test-")
        .tempdir_in("/tmp")
        .unwrap();
    let socket = root.path().join("stalled.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        std::thread::sleep(Duration::from_secs(4));
        drop(stream);
    });

    let started = Instant::now();
    let error = match SessionClient::connect(&socket, current_build_mode()) {
        Ok(_) => panic!("stalled server completed the handshake"),
        Err(error) => error,
    };
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
    assert!(started.elapsed() < Duration::from_secs(4));
    server.join().unwrap();
}

#[test]
fn control_connection_limit_is_independent_and_recovers_capacity() {
    let daemon = TestDaemon::start();
    let mut clients = Vec::new();
    for _ in 0..MAX_CONTROL_CONNECTIONS {
        let mut client = daemon.client();
        client.ping().unwrap();
        clients.push(client);
    }
    match SessionClient::connect(&daemon.socket, current_build_mode()) {
        Ok(mut excess) => assert!(excess.ping().is_err()),
        Err(error) => assert!(matches!(
            error.kind(),
            std::io::ErrorKind::InvalidInput
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::UnexpectedEof
        )),
    }
    clients.pop();

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(mut client) = SessionClient::connect(&daemon.socket, current_build_mode())
            && client.ping().is_ok()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "control capacity was not released"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(clients);
    daemon.finish(false);
}
