//! The generic bridge connection, against an in-process server behind a fake
//! bridge that prints shell noise and the ready line, then relays.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::bridge::BridgeExit;
use muxy_client::{Client, ClientError, RemoteReason};
use muxy_protocol::transport::relay;
use muxy_protocol::wire::{Decoder, Encoder, WireError};
use muxy_protocol::{CONTROL, Message, Size};
use muxy_server::connection::serve;
use muxy_server::{Registry, ServerSettings};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const TIMEOUT: Duration = Duration::from_secs(10);

/// Returns the client's end of a bridge that prints `printed` first.
fn bridge(printed: &'static [u8]) -> TestResult<UnixStream> {
    let (client, mut remote) = UnixStream::pair()?;
    let (bridge, server) = UnixStream::pair()?;
    let (events, receiver) = mpsc::channel();
    let registry = Arc::new(Registry::new(
        ServerSettings {
            default_shell: Some(PathBuf::from("/bin/sh")),
            ..ServerSettings::default()
        },
        events,
    ));
    thread::spawn(move || {
        let (_keep, events) = mpsc::channel();
        let _ = serve(Box::new(server), registry, events);
        drop(receiver);
    });
    thread::spawn(move || -> std::io::Result<()> {
        remote.write_all(printed)?;
        remote.write_all(b"MUXY-STDIO/1\n")?;
        relay(Box::new(bridge), remote.try_clone()?, remote)
    });
    Ok(client)
}

fn connect(stream: UnixStream, exit: BridgeExit) -> Result<Client, ClientError> {
    Client::connect_bridge(Box::new(stream), "box", TIMEOUT, || exit)
}

fn remote_error(result: Result<Client, ClientError>) -> TestResult<(RemoteReason, String)> {
    let error = result.err().ok_or("connected unexpectedly")?;
    let message = error.to_string();
    match error {
        ClientError::Remote {
            reason,
            destination,
            ..
        } if destination == "box" => Ok((reason, message)),
        other => Err(format!("expected a remote error naming box, got {other:?}").into()),
    }
}

#[test]
fn a_bridge_behind_shell_noise_carries_the_protocol() -> TestResult {
    let client = connect(
        bridge(b"Welcome to box!\nLast login: today\n")?,
        BridgeExit::default(),
    )?;
    client.ping()?;
    assert_eq!(client.list_sessions()?, vec![]);
    let session = client.create_session(Path::new("/tmp"), Size { cols: 40, rows: 5 })?;
    assert_eq!(client.list_sessions()?, vec![session.clone()]);
    client.end_session(session.id)?;
    Ok(())
}

#[test]
fn a_transport_that_ends_early_is_explained_by_its_exit() -> TestResult {
    let (client, mut remote) = UnixStream::pair()?;
    remote.write_all(b"Welcome\n")?;
    drop(remote);
    let (reason, message) = remote_error(connect(
        client,
        BridgeExit {
            status: Some(127),
            stderr: "sh: 1: exec: muxy: not found\n".into(),
        },
    ))?;
    assert_eq!(reason, RemoteReason::NotInstalled);
    assert_eq!(
        message,
        "Muxy isn't installed on box (looked on PATH and in ~/.local/bin)."
    );
    Ok(())
}

#[test]
fn a_bridge_of_another_version_is_incompatible_rather_than_a_hang() -> TestResult {
    let (client, mut remote) = UnixStream::pair()?;
    remote.set_read_timeout(Some(TIMEOUT))?;
    remote.write_all(b"MUXY-STDIO/2\n")?;
    let (reason, message) = remote_error(connect(client, BridgeExit::default()))?;
    assert_eq!(reason, RemoteReason::Incompatible);
    assert!(message.contains("bridge version 2"), "{message}");
    assert_eq!(remote.read(&mut [0; 64])?, 0);
    Ok(())
}

#[test]
fn a_remote_server_without_a_shared_version_is_named_as_incompatible() -> TestResult {
    let (client, remote) = UnixStream::pair()?;
    thread::spawn(move || -> Result<(), WireError> {
        let mut writer = remote.try_clone()?;
        writer.write_all(b"MUXY-STDIO/1\n")?;
        Decoder::new(remote).next()?;
        Encoder::new(writer).send(CONTROL, &Message::VersionUnsupported)
    });
    let (reason, message) = remote_error(connect(client, BridgeExit::default()))?;
    assert_eq!(reason, RemoteReason::Incompatible);
    assert!(
        message.starts_with("Muxy on box can't accept connections from this version of Muxy (its running server is a different version)."),
        "{message}"
    );
    Ok(())
}

#[test]
fn a_silent_transport_times_out_and_is_shut_down() -> TestResult {
    let (client, mut remote) = UnixStream::pair()?;
    remote.set_read_timeout(Some(TIMEOUT))?;
    remote.write_all(b"still logging in")?;
    let start = Instant::now();
    let (reason, message) = remote_error(Client::connect_bridge(
        Box::new(client),
        "box",
        Duration::from_millis(200),
        || BridgeExit {
            status: None,
            stderr: "debug: waiting\n".into(),
        },
    ))?;
    assert!(start.elapsed() < TIMEOUT);
    assert_eq!(reason, RemoteReason::Timeout);
    assert_eq!(message, "Timed out connecting to box: debug: waiting");
    assert_eq!(remote.read(&mut [0; 64])?, 0);
    Ok(())
}
