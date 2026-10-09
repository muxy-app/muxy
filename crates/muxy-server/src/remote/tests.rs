use std::collections::HashSet;

use super::*;

fn listening() -> RemoteAccess {
    let mut remote = RemoteAccess::memory();
    remote.set_listener(Box::new(|listening| {
        if listening.is_some() {
            ListenerStatus::Listening
        } else {
            ListenerStatus::Disabled
        }
    }));
    remote
}

fn enabled() -> Result<RemoteAccess, ServerError> {
    let remote = listening();
    remote.configure(RemoteAccessSettings {
        enabled: true,
        port: 7419,
    })?;
    Ok(remote)
}

fn request(secret: [u8; 16]) -> PairRequest {
    PairRequest {
        secret,
        name: " Phone ".into(),
    }
}

fn server() -> ServerIdentity {
    ServerIdentity::from_u128(1)
}

/// Starts pairing, falling back to a fixed offer where the machine has no network.
fn offer(remote: &RemoteAccess, owner: ClientId) -> [u8; 16] {
    if let Ok(offer) = remote.start_pairing(owner, &[]) {
        return offer.invite.secret;
    }
    let secret = [9; 16];
    remote.lock().offer = Some(Offer {
        secret,
        expires_at: now() + PAIRING_SECONDS,
        owner,
    });
    secret
}

#[test]
fn enabling_creates_one_identity_and_reports_the_listener() -> Result<(), ServerError> {
    let remote = enabled()?;
    let first = remote.lock().stored.identity.clone();
    assert!(first.is_some());
    remote.configure(RemoteAccessSettings {
        enabled: true,
        port: 7420,
    })?;
    assert_eq!(remote.lock().stored.identity, first);
    let state = remote.state(&HashSet::new());
    assert_eq!(state.status, ListenerStatus::Listening);
    assert_eq!(state.settings.port, 7420);
    Ok(())
}

#[test]
fn wrong_secrets_leave_the_offer_usable_but_expiry_ends_it() -> Result<(), ServerError> {
    let remote = enabled()?;
    let secret = offer(&remote, ClientId::new());
    for _ in 0..10 {
        assert!(remote.pair(&request([0; 16]), server()).is_none());
    }
    let expired = now() + PAIRING_SECONDS + 1;
    assert!(
        remote
            .pair_at(&request(secret), server(), expired)
            .is_none()
    );
    assert!(remote.pair(&request(secret), server()).is_none());
    Ok(())
}

#[test]
fn an_offer_ends_with_the_connection_that_created_it() -> Result<(), ServerError> {
    let remote = enabled()?;
    let owner = ClientId::new();
    let secret = offer(&remote, owner);
    remote.release_pairing(ClientId::new());
    assert!(remote.state(&HashSet::new()).pairing_expires_at.is_some());
    remote.release_pairing(owner);
    assert!(remote.pair(&request(secret), server()).is_none());
    Ok(())
}

#[test]
fn pairing_needs_access_on_and_listening() -> Result<(), ServerError> {
    let remote = RemoteAccess::memory();
    assert!(remote.start_pairing(ClientId::new(), &[]).is_err());
    remote.configure(RemoteAccessSettings {
        enabled: true,
        port: 7419,
    })?;
    assert!(matches!(
        remote.state(&HashSet::new()).status,
        ListenerStatus::Failed(_)
    ));
    assert!(remote.start_pairing(ClientId::new(), &[]).is_err());
    Ok(())
}

#[test]
fn unauthenticated_admissions_are_capped_and_released() {
    let remote = RemoteAccess::memory();
    let admitted: Vec<_> = (0..MAX_UNAUTHENTICATED)
        .map_while(|_| remote.admit())
        .collect();
    assert_eq!(admitted.len(), MAX_UNAUTHENTICATED);
    assert!(remote.admit().is_none());
    drop(admitted);
    assert!(remote.admit().is_some());
}

#[test]
fn turning_access_off_keeps_an_unreadable_store_for_recovery()
-> Result<(), Box<dyn std::error::Error>> {
    let directory =
        std::env::temp_dir().join(format!("muxy-remote-{}", muxy_protocol::OperationId::new()));
    std::fs::create_dir(&directory)?;
    let path = directory.join("remote.json");
    std::fs::write(&path, "damaged")?;
    let remote = RemoteAccess::open(path.clone());
    assert!(matches!(
        remote.state(&HashSet::new()).status,
        ListenerStatus::Failed(_)
    ));
    remote.configure(RemoteAccessSettings {
        enabled: false,
        port: 7419,
    })?;
    assert_eq!(std::fs::read_to_string(&path)?, "damaged");
    remote.configure(RemoteAccessSettings {
        enabled: true,
        port: 7419,
    })?;
    assert!(store::load(&path)?.identity.is_some());
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
