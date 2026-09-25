use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use muxy_protocol::ListenerStatus;
use muxy_protocol::transport::Listener;
use muxy_protocol::transport::tls::{TlsIdentity, TlsListener};
use muxy_server::{Registry, connection};

use crate::run::Connections;

const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);
const ERROR_REPORT_INTERVAL: Duration = Duration::from_secs(60);

/// Owns the network listener for paired devices; the registry decides when it runs.
pub(crate) struct RemoteListener {
    connections: Connections,
    address: Ipv4Addr,
    registry: OnceLock<Weak<Registry>>,
    running: Mutex<Running>,
}

#[derive(Default)]
struct Running {
    listener: Option<Arc<TlsListener>>,
    thread: Option<JoinHandle<()>>,
    shut_down: bool,
}

impl RemoteListener {
    pub(crate) fn new(connections: Connections) -> io::Result<Self> {
        Ok(Self {
            connections,
            address: bind_address()?,
            registry: OnceLock::new(),
            running: Mutex::default(),
        })
    }

    pub(crate) fn attach(&self, registry: &Arc<Registry>) {
        let _ = self.registry.set(Arc::downgrade(registry));
    }

    /// Replaces any running listener; `None` also ends every network connection.
    pub(crate) fn apply(&self, listening: Option<(u16, TlsIdentity)>) -> ListenerStatus {
        let mut running = self.lock();
        stop(&mut running);
        if running.shut_down {
            return ListenerStatus::Disabled;
        }
        let Some((port, identity)) = listening else {
            self.connections.cancel_remote();
            return ListenerStatus::Disabled;
        };
        let listener = match TlsListener::bind(SocketAddr::from((self.address, port)), &identity) {
            Ok(listener) => Arc::new(listener),
            Err(error) => {
                log::error!("mobile access cannot listen on port {port}: {error}");
                return ListenerStatus::Failed(error.to_string());
            }
        };
        let accepting = Arc::clone(&listener);
        let registry = self.registry.get().cloned().unwrap_or_default();
        let connections = self.connections.clone();
        match thread::Builder::new()
            .name("remote-accept".into())
            .spawn(move || accept(&accepting, &registry, &connections))
        {
            Ok(thread) => {
                log::info!("mobile access listening on port {port}");
                running.listener = Some(listener);
                running.thread = Some(thread);
                ListenerStatus::Listening
            }
            Err(error) => {
                listener.close();
                ListenerStatus::Failed(error.to_string())
            }
        }
    }

    /// Stops listening for good; later changes are refused while the server exits.
    pub(crate) fn shutdown(&self) {
        let mut running = self.lock();
        running.shut_down = true;
        stop(&mut running);
    }

    fn lock(&self) -> MutexGuard<'_, Running> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn stop(running: &mut Running) {
    if let Some(listener) = running.listener.take() {
        listener.close();
    }
    if let Some(thread) = running.thread.take() {
        let _ = thread.join();
    }
}

fn accept(listener: &TlsListener, registry: &Weak<Registry>, connections: &Connections) {
    let mut reported: Option<Instant> = None;
    loop {
        let stream = match listener.accept() {
            Ok(stream) => stream,
            Err(error) if error.kind() == io::ErrorKind::NotConnected => return,
            Err(error) => {
                if reported.is_none_or(|last| last.elapsed() >= ERROR_REPORT_INTERVAL) {
                    log::warn!("mobile access accept failed: {error}");
                    reported = Some(Instant::now());
                }
                thread::sleep(ACCEPT_BACKOFF);
                continue;
            }
        };
        let Some(registry) = registry.upgrade() else {
            return;
        };
        // Too many unauthenticated peers: dropping the stream refuses this one.
        let Some(admission) = registry.admit_remote() else {
            continue;
        };
        if let Err(error) = connections.spawn(stream, true, move |stream, events| {
            connection::serve_remote(stream, registry, events, admission)
        }) {
            log::error!("mobile access connection failed: {error}");
        }
    }
}

/// Loopback in tests keeps developer runs off the network and the firewall prompt.
fn bind_address() -> io::Result<Ipv4Addr> {
    match std::env::var_os("MUXY_REMOTE_BIND") {
        None => Ok(Ipv4Addr::UNSPECIFIED),
        Some(address) => address
            .to_str()
            .and_then(|address| address.parse().ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "MUXY_REMOTE_BIND must be an IPv4 address",
                )
            }),
    }
}
