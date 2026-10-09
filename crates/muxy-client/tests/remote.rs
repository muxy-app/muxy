use std::net::{SocketAddr, TcpListener};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak, mpsc};
use std::thread;

use muxy_client::{Client, ClientError, RemoteEndpoint};
use muxy_protocol::transport::Listener;
use muxy_protocol::transport::tls::TlsListener;
use muxy_protocol::{ListenerStatus, PairingOffer, RemoteAccessSettings};
use muxy_server::connection::{serve, serve_remote};
use muxy_server::{Registry, ServerEvent, ServerSettings};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

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
