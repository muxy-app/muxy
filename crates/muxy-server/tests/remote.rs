use std::error::Error;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use muxy_protocol::wire::{Decoder, Encoder, WireError};
use muxy_protocol::{
    CONTROL, ChannelId, ClientKind, DeviceCredential, DeviceId, ErrorCode, ListenerStatus, Message,
    OperationId, PairRequest, Paired, RemoteAccessSettings, RemoteAccessState, ReplyBody,
    RequestBody, RequestId, SUPPORTED, ServerPath, Size,
};
use muxy_server::connection::{serve, serve_remote};
use muxy_server::{Registry, ServerEvent, ServerSettings};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const TIMEOUT: Duration = Duration::from_secs(10);

struct Fixture {
    registry: Arc<Registry>,
    subscribers: Arc<Mutex<Vec<Sender<ServerEvent>>>>,
    running: Arc<AtomicBool>,
}

struct Client {
    socket: UnixStream,
    encoder: Encoder<UnixStream>,
    incoming: Receiver<Result<(ChannelId, Message), WireError>>,
    next_request: u32,
}

impl Fixture {
    fn new() -> Self {
        let (sender, events) = mpsc::channel();
        let registry = Arc::new(
            Registry::new(
                ServerSettings {
                    default_shell: Some(PathBuf::from("/bin/sh")),
                    ..ServerSettings::default()
                },
                sender,
            )
            .with_remote_listener(|listening| {
                if listening.is_some() {
                    ListenerStatus::Listening
                } else {
                    ListenerStatus::Disabled
                }
            }),
        );
        let subscribers = Arc::new(Mutex::new(Vec::<Sender<ServerEvent>>::new()));
        let sinks = Arc::clone(&subscribers);
        let running = Arc::new(AtomicBool::new(true));
        let active = Arc::clone(&running);
        thread::spawn(move || {
            while active.load(Ordering::Relaxed) {
                match events.recv_timeout(Duration::from_millis(100)) {
                    Ok(event) => {
                        if let Ok(mut sinks) = sinks.lock() {
                            sinks.retain(|sink| sink.send(event.clone()).is_ok());
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Self {
            registry,
            subscribers,
            running,
        }
    }

    fn connect(&self, remote: bool) -> TestResult<Client> {
        let (socket, server) = UnixStream::pair()?;
        let (sender, events) = mpsc::channel();
        self.subscribers
            .lock()
            .map_err(|_| "subscriber lock poisoned")?
            .push(sender);
        let registry = Arc::clone(&self.registry);
        let admission = if remote {
            Some(registry.admit_remote().ok_or("admission refused")?)
        } else {
            None
        };
        thread::spawn(move || {
            let _ = match admission {
                Some(admission) => serve_remote(Box::new(server), registry, events, admission),
                None => serve(Box::new(server), registry, events),
            };
        });
        let mut decoder = Decoder::new(socket.try_clone()?);
        let (sender, incoming) = mpsc::channel();
        thread::spawn(move || {
            loop {
                let result = decoder.next();
                let ended = result.is_err();
                if sender.send(result).is_err() || ended {
                    break;
                }
            }
        });
        let mut client = Client {
            encoder: Encoder::new(socket.try_clone()?),
            socket,
            incoming,
            next_request: 1,
        };
        client.encoder.send(
            CONTROL,
            &Message::Hello {
                versions: SUPPORTED.to_vec(),
                compatibility: muxy_protocol::COMPATIBILITY,
            },
        )?;
        match client.receive()? {
            (CONTROL, Message::HelloReply { .. }) => Ok(client),
            other => Err(format!("expected hello reply, got {other:?}").into()),
        }
    }

    fn local(&self) -> TestResult<Client> {
        self.connect(false)
    }

    fn enabled(&self) -> TestResult<Client> {
        let mut local = self.local()?;
        let state = remote_access(local.request(RequestBody::WriteRemoteAccess(
            RemoteAccessSettings {
                enabled: true,
                port: 7419,
            },
        ))?)?;
        assert_eq!(state.status, ListenerStatus::Listening);
        Ok(local)
    }

    fn pair(&self, local: &mut Client) -> TestResult<(Paired, Client)> {
        let offer = match local.request(RequestBody::StartPairing)? {
            ReplyBody::Pairing(offer) => offer,
            other => return Err(format!("expected pairing, got {other:?}").into()),
        };
        let mut phone = self.connect(true)?;
        match phone.request(RequestBody::Pair(PairRequest {
            secret: offer.invite.secret,
            name: "Phone".into(),
        }))? {
            ReplyBody::Paired(paired) => Ok((paired, phone)),
            other => Err(format!("expected paired, got {other:?}").into()),
        }
    }

    fn authenticated(&self, credential: DeviceCredential) -> TestResult<Client> {
        let mut phone = self.connect(true)?;
        match phone.request(RequestBody::Authenticate(credential))? {
            ReplyBody::Authenticated => Ok(phone),
            other => Err(format!("expected authenticated, got {other:?}").into()),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Client {
    fn receive(&self) -> TestResult<(ChannelId, Message)> {
        Ok(self.incoming.recv_timeout(TIMEOUT)??)
    }

    fn request(&mut self, body: RequestBody) -> TestResult<ReplyBody> {
        let id = RequestId(self.next_request);
        self.next_request += 1;
        self.encoder.send(CONTROL, &Message::Request { id, body })?;
        loop {
            match self.receive()? {
                (CONTROL, Message::Reply { id: received, body }) if received == id => {
                    return Ok(body);
                }
                (CONTROL, Message::RemoteAccessChanged { .. }) => {}
                other => return Err(format!("unexpected message: {other:?}").into()),
            }
        }
    }

    fn closed(&self) -> TestResult {
        loop {
            match self.incoming.recv_timeout(TIMEOUT)? {
                Err(WireError::Closed) => return Ok(()),
                Ok(_) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(Shutdown::Both);
    }
}

fn remote_access(reply: ReplyBody) -> TestResult<RemoteAccessState> {
    match reply {
        ReplyBody::RemoteAccess(state) => Ok(state),
        other => Err(format!("expected remote access, got {other:?}").into()),
    }
}

fn unauthorized(reply: &ReplyBody) -> Option<&str> {
    match reply {
        ReplyBody::Error(error) if error.code == ErrorCode::Unauthorized => {
            Some(error.message.as_str())
        }
        _ => None,
    }
}

#[test]
fn requests_before_authentication_are_refused_and_closed() -> TestResult {
    let fixture = Fixture::new();
    let mut phone = fixture.connect(true)?;
    let reply = phone.request(RequestBody::ReadCatalog {
        after: None,
        revision: None,
    })?;
    assert!(unauthorized(&reply).is_some(), "{reply:?}");
    phone.closed()
}

#[test]
fn a_paired_device_reconnects_and_uses_a_terminal() -> TestResult {
    let fixture = Fixture::new();
    let mut local = fixture.enabled()?;
    let (paired, phone) = fixture.pair(&mut local)?;
    drop(phone);
    let mut phone = fixture.authenticated(paired.credential)?;
    let size = Size { cols: 40, rows: 5 };
    let session = match phone.request(RequestBody::CreateSession {
        project: fixture.registry.home_project(),
        operation: OperationId::new(),
        directory: ServerPath(b"/tmp".to_vec()),
        size,
    })? {
        ReplyBody::SessionCreated(session) => session,
        other => return Err(format!("expected session, got {other:?}").into()),
    };
    let channel = match phone.request(RequestBody::Attach {
        session: session.id,
        size,
    })? {
        ReplyBody::Attached { snapshot, .. } => snapshot.channel,
        other => return Err(format!("expected attached, got {other:?}").into()),
    };
    phone.encoder.send(
        channel,
        &Message::Input(b"echo remote-$((40+2))\n".to_vec()),
    )?;
    loop {
        match phone.receive()? {
            (received, Message::Frame(frame)) if received == channel => {
                let text: String = frame
                    .rows
                    .iter()
                    .flat_map(|row| row.runs.iter().map(|run| run.text.as_str()))
                    .collect();
                if text.contains("remote-42") {
                    break;
                }
                phone.encoder.send(
                    CONTROL,
                    &Message::FrameAck {
                        channel,
                        seq: frame.seq,
                    },
                )?;
            }
            (_, Message::Metadata(_)) | (CONTROL, Message::RemoteAccessChanged { .. }) => {}
            other => return Err(format!("unexpected message: {other:?}").into()),
        }
    }
    fixture.registry.end(session.id)?;
    Ok(())
}

#[test]
fn unknown_devices_and_wrong_tokens_are_indistinguishable() -> TestResult {
    let fixture = Fixture::new();
    let mut local = fixture.enabled()?;
    let (paired, _phone) = fixture.pair(&mut local)?;
    let mut replies = Vec::new();
    for credential in [
        DeviceCredential {
            device: DeviceId::new(),
            ..paired.credential
        },
        DeviceCredential {
            token: [0; 32],
            ..paired.credential
        },
    ] {
        let mut phone = fixture.connect(true)?;
        replies.push(phone.request(RequestBody::Authenticate(credential))?);
        phone.closed()?;
    }
    assert!(unauthorized(&replies[0]).is_some());
    assert_eq!(replies[0], replies[1]);
    Ok(())
}

#[test]
fn large_frames_before_authentication_are_rejected() -> TestResult {
    let fixture = Fixture::new();
    let mut phone = fixture.connect(true)?;
    // The server stops reading at the header, so the rest of the write may fail.
    let _ = phone
        .encoder
        .send(CONTROL, &Message::Input(vec![0; 64 * 1024]));
    assert!(matches!(phone.receive()?, (CONTROL, Message::Fatal(_))));
    phone.closed()
}

#[test]
fn a_pairing_secret_works_once() -> TestResult {
    let fixture = Fixture::new();
    let mut local = fixture.enabled()?;
    let offer = match local.request(RequestBody::StartPairing)? {
        ReplyBody::Pairing(offer) => offer,
        other => return Err(format!("expected pairing, got {other:?}").into()),
    };
    let pair = PairRequest {
        secret: offer.invite.secret,
        name: "Phone".into(),
    };
    let mut first = fixture.connect(true)?;
    assert!(matches!(
        first.request(RequestBody::Pair(pair.clone()))?,
        ReplyBody::Paired(_)
    ));
    let mut second = fixture.connect(true)?;
    assert!(unauthorized(&second.request(RequestBody::Pair(pair))?).is_some());
    second.closed()
}

#[test]
fn paired_devices_cannot_manage_access_or_the_server() -> TestResult {
    let fixture = Fixture::new();
    let mut local = fixture.enabled()?;
    let (_, mut phone) = fixture.pair(&mut local)?;
    for body in [
        RequestBody::ReadRemoteAccess,
        RequestBody::WriteRemoteAccess(RemoteAccessSettings::default()),
        RequestBody::StartPairing,
        RequestBody::CancelPairing,
        RequestBody::RevokeDevice(DeviceId::new()),
        RequestBody::StopServer,
        RequestBody::StopServerIfIdle,
        RequestBody::WriteServerSettings(fixture.registry.settings().document()),
        RequestBody::Exec(muxy_protocol::ExecRequest {
            job: 1,
            project: fixture.registry.home_project(),
            argv: vec!["true".into()],
            shell: None,
            cwd: None,
            env: std::collections::BTreeMap::new(),
            stdin: Vec::new(),
            timeout_ms: 1000,
        }),
        RequestBody::CancelExec(1),
        RequestBody::IdentifyClient(ClientKind::Desktop),
    ] {
        let reply = phone.request(body.clone())?;
        assert!(unauthorized(&reply).is_some(), "{body:?}: {reply:?}");
    }
    match phone.request(RequestBody::IdentifyClient(ClientKind::Mobile))? {
        ReplyBody::ClientIdentified(client) => assert_eq!(client.kind, ClientKind::Mobile),
        other => return Err(format!("expected identified, got {other:?}").into()),
    }
    assert!(matches!(
        phone.request(RequestBody::ReadCatalog {
            after: None,
            revision: None
        })?,
        ReplyBody::Catalog(_)
    ));
    match local.request(RequestBody::IdentifyClient(ClientKind::Mobile))? {
        ReplyBody::Error(error) => assert_eq!(error.code, ErrorCode::BadRequest),
        other => return Err(format!("local client claimed mobile: {other:?}").into()),
    }
    Ok(())
}

#[test]
fn revoking_closes_only_that_device_and_disabling_closes_all() -> TestResult {
    let fixture = Fixture::new();
    let mut local = fixture.enabled()?;
    let (first, first_phone) = fixture.pair(&mut local)?;
    let (_, mut second_phone) = fixture.pair(&mut local)?;
    let state = remote_access(local.request(RequestBody::ReadRemoteAccess)?)?;
    assert_eq!(state.devices.len(), 2);
    assert!(state.devices.iter().all(|device| device.connected));
    let state = remote_access(local.request(RequestBody::RevokeDevice(first.credential.device))?)?;
    assert_eq!(state.devices.len(), 1);
    first_phone.closed()?;
    assert_eq!(second_phone.request(RequestBody::Ping)?, ReplyBody::Pong);
    local.request(RequestBody::WriteRemoteAccess(RemoteAccessSettings {
        enabled: false,
        port: 7419,
    }))?;
    second_phone.closed()?;
    let refused = fixture.connect(true)?;
    drop(refused);
    Ok(())
}

#[test]
fn a_revoke_racing_authentication_never_leaves_the_device_connected() -> TestResult {
    let fixture = Arc::new(Fixture::new());
    let mut local = fixture.enabled()?;
    for _ in 0..20 {
        let (paired, phone) = fixture.pair(&mut local)?;
        drop(phone);
        let racing = Arc::clone(&fixture);
        let credential = paired.credential;
        let phone = thread::spawn(move || -> Result<(), String> {
            let mut phone = racing.connect(true).map_err(|error| error.to_string())?;
            // Either reply is fine; a request cut off by the revoke has already seen the close.
            if phone.request(RequestBody::Authenticate(credential)).is_ok() {
                phone.closed().map_err(|error| error.to_string())?;
            }
            Ok(())
        });
        local.request(RequestBody::RevokeDevice(credential.device))?;
        phone.join().map_err(|_| "phone thread panicked")??;
    }
    Ok(())
}

#[test]
fn local_watchers_hear_about_pairing_and_connections() -> TestResult {
    let fixture = Fixture::new();
    let mut local = fixture.enabled()?;
    let mut watcher = fixture.local()?;
    let before = remote_access(watcher.request(RequestBody::ReadRemoteAccess)?)?;
    let (_, _phone) = fixture.pair(&mut local)?;
    loop {
        match watcher.receive()? {
            (CONTROL, Message::RemoteAccessChanged { revision }) if revision > before.revision => {
                break;
            }
            (CONTROL, Message::RemoteAccessChanged { .. }) => {}
            other => return Err(format!("unexpected message: {other:?}").into()),
        }
    }
    let after = remote_access(watcher.request(RequestBody::ReadRemoteAccess)?)?;
    assert_eq!(after.devices.len(), 1);
    assert!(after.devices[0].connected);
    Ok(())
}
