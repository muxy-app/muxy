use std::net::{SocketAddr, TcpListener};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::{Client, ClientError, ClientEvent, RemoteEndpoint};
use muxy_protocol::transport::Listener;
use muxy_protocol::transport::tls::TlsListener;
use muxy_protocol::{ErrorCode, ListenerStatus, PairingOffer, RemoteAccessSettings, Size};
use muxy_server::connection::{serve, serve_remote};
use muxy_server::{Registry, ServerEvent, ServerSettings};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const TIMEOUT: Duration = Duration::from_secs(10);
const SIZE: Size = Size { cols: 40, rows: 5 };

/// An in-process server whose network listener is a real TLS listener on loopback.
struct Server {
    registry: Arc<Registry>,
    _events: mpsc::Receiver<ServerEvent>,
}

impl Server {
    fn new() -> Self {
        let (sender, events) = mpsc::channel();
        let slot = Arc::new(OnceLock::<Weak<Registry>>::new());
        let owner = Arc::clone(&slot);
        let running = Mutex::new(None::<Arc<TlsListener>>);
        let registry = Arc::new(
            Registry::new(
                ServerSettings {
                    default_shell: Some(PathBuf::from("/bin/sh")),
                    ..ServerSettings::default()
                },
                sender,
            )
            .with_remote_listener(move |listening| {
                let mut running = running.lock().unwrap_or_else(PoisonError::into_inner);
                if let Some(previous) = running.take() {
                    previous.close();
                }
                let Some((port, identity)) = listening else {
                    return ListenerStatus::Disabled;
                };
                match TlsListener::bind(SocketAddr::from(([127, 0, 0, 1], port)), &identity) {
                    Ok(listener) => {
                        let listener = Arc::new(listener);
                        *running = Some(Arc::clone(&listener));
                        let registry = owner.get().cloned().unwrap_or_default();
                        thread::spawn(move || accept(&listener, &registry));
                        ListenerStatus::Listening
                    }
                    Err(error) => ListenerStatus::Failed(error.to_string()),
                }
            }),
        );
        let _ = slot.set(Arc::downgrade(&registry));
        Self {
            registry,
            _events: events,
        }
    }

    fn local(&self) -> TestResult<Client> {
        let (client, server) = UnixStream::pair()?;
        let registry = Arc::clone(&self.registry);
        thread::spawn(move || {
            let (_keep, events) = mpsc::channel();
            let _ = serve(Box::new(server), registry, events);
        });
        Ok(Client::from_stream(Box::new(client))?)
    }

    /// Enables mobile access and returns the local client with a fresh offer.
    fn pairing(&self) -> TestResult<(Client, PairingOffer)> {
        let local = self.local()?;
        let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let state = local.write_remote_access(RemoteAccessSettings {
            enabled: true,
            port,
        })?;
        assert_eq!(state.status, ListenerStatus::Listening);
        let mut offer = local.start_pairing()?;
        offer.invite.hosts = vec!["127.0.0.1".into()];
        Ok((local, offer))
    }
}

fn accept(listener: &TlsListener, registry: &Weak<Registry>) {
    while let Ok(stream) = listener.accept() {
        let Some(registry) = registry.upgrade() else {
            return;
        };
        let Some(admission) = registry.admit_remote() else {
            continue;
        };
        thread::spawn(move || {
            let (_keep, events) = mpsc::channel();
            let _ = serve_remote(stream, registry, events, admission);
        });
    }
}

#[test]
fn a_phone_pairs_reconnects_and_types_into_a_shell() -> TestResult {
    let server = Server::new();
    let (_local, offer) = server.pairing()?;
    let (phone, paired) = Client::pair(&offer.invite, "Phone")?;
    drop(phone);
    let phone = Client::connect_remote(&RemoteEndpoint::from(&offer.invite), paired.credential)?;
    let events = phone.events().ok_or("events already taken")?;
    assert_eq!(phone.catalog()?.server, paired.server);
    let session = phone.create_session(Path::new("/tmp"), SIZE)?;
    let attachment = phone.attach(session.id, SIZE)?;
    phone.send_input(attachment.channel, b"echo phone-$((40+2))\n")?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            ClientEvent::Frame { channel, frame } if channel == attachment.channel => {
                let text: String = frame
                    .rows
                    .iter()
                    .flat_map(|row| row.runs.iter().map(|run| run.text.as_str()))
                    .collect();
                if text.contains("phone-42") {
                    break;
                }
                phone.ack(channel, frame.seq)?;
            }
            ClientEvent::Disconnected => return Err("phone disconnected".into()),
            _ => {}
        }
    }
    phone.end_session(session.id)?;
    Ok(())
}

#[test]
fn a_revoked_device_is_unauthorized() -> TestResult {
    let server = Server::new();
    let (local, offer) = server.pairing()?;
    let (phone, paired) = Client::pair(&offer.invite, "Phone")?;
    local.revoke_device(paired.credential.device)?;
    let deadline = Instant::now() + TIMEOUT;
    while phone.is_connected() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!phone.is_connected());
    match Client::connect_remote(&RemoteEndpoint::from(&offer.invite), paired.credential) {
        Err(ClientError::Server(error)) => assert_eq!(error.code, ErrorCode::Unauthorized),
        other => return Err(format!("expected unauthorized, got {other:?}").into()),
    }
    Ok(())
}

#[test]
fn an_unreachable_host_falls_back_and_a_foreign_certificate_is_a_mismatch() -> TestResult {
    let server = Server::new();
    let (_local, offer) = server.pairing()?;
    let mut endpoint = RemoteEndpoint::from(&offer.invite);
    endpoint.hosts.insert(0, "::1".into());
    let (_phone, paired) = Client::pair(&offer.invite, "Phone")?;
    Client::connect_remote(&endpoint, paired.credential)?;
    endpoint.fingerprint = [0; 32];
    assert!(matches!(
        Client::connect_remote(&endpoint, paired.credential),
        Err(ClientError::IdentityMismatch)
    ));
    Ok(())
}

#[test]
fn local_watchers_hear_when_a_phone_connects() -> TestResult {
    let server = Server::new();
    let (local, offer) = server.pairing()?;
    let events = local.events().ok_or("events already taken")?;
    let before = local.read_remote_access()?.revision;
    let (_phone, _) = Client::pair(&offer.invite, "Phone")?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            ClientEvent::RemoteAccessChanged { revision } if revision > before => break,
            ClientEvent::Disconnected => return Err("local client disconnected".into()),
            _ => {}
        }
    }
    let state = local.read_remote_access()?;
    assert_eq!(state.devices.len(), 1);
    assert!(state.devices[0].connected);
    Ok(())
}
