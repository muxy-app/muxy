use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::{Registry, ServerSettings};

use super::{Outbox, serve_remote};

#[test]
fn a_silent_network_peer_is_closed_at_the_deadline() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, _events) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), sender));
    let admission = registry
        .remote
        .admit_with(Duration::from_millis(100))
        .ok_or("admission refused")?;
    let (mut peer, stream) = UnixStream::pair()?;
    let (_forward, events) = mpsc::channel();
    let started = Instant::now();
    let serving =
        thread::spawn(move || serve_remote(Box::new(stream), registry, events, admission));
    peer.set_read_timeout(Some(Duration::from_secs(5)))?;
    assert_eq!(peer.read(&mut [0; 1])?, 0);
    serving.join().map_err(|_| "serve panicked")??;
    assert!(started.elapsed() < Duration::from_secs(5));
    Ok(())
}

#[test]
fn a_phone_that_connects_first_never_receives_desktop_notifications() {
    let (sender, _events) = mpsc::channel();
    let registry = Registry::new(ServerSettings::default(), sender);
    let session = muxy_protocol::SessionId::new(1).expect("nonzero");
    registry.activity.update(
        muxy_protocol::AgentActivity {
            session,
            project: registry.home_project(),
            provider: muxy_protocol::AgentProvider::Claude,
            state: muxy_protocol::AgentState::Blocked,
        },
        false,
    );
    let events: Vec<_> = registry
        .activity
        .snapshot()
        .events
        .iter()
        .map(|event| event.id)
        .collect();
    let changes = Arc::clone(&registry.attachment_changes);
    let phone = Arc::new(Outbox::for_device(
        muxy_protocol::V1,
        Arc::clone(&changes),
        muxy_protocol::DeviceId::new(),
    ));
    registry.register_connection(&phone);
    let desktop = Arc::new(Outbox::new(muxy_protocol::V1, changes));
    desktop.identify(muxy_protocol::ClientKind::Desktop);
    registry.register_connection(&desktop);
    assert!(!events.is_empty());
    assert!(registry.claim_activity(&phone, &events).is_empty());
    assert_eq!(registry.claim_activity(&desktop, &events), events);
}
