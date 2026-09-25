use std::collections::HashSet;
use std::sync::{Arc, Mutex};

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
    if let Ok(offer) = remote.start_pairing(owner) {
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
fn pairing_is_single_use_and_trims_the_device_name() -> Result<(), ServerError> {
    let remote = enabled()?;
    let secret = offer(&remote, ClientId::new());
    assert!(remote.pair(&request([0; 16]), server()).is_none());
    let paired = remote
        .pair(&request(secret), server())
        .ok_or_else(|| unavailable("pairing failed"))?;
    assert_eq!(paired.server, server());
    assert!(remote.pair(&request(secret), server()).is_none());
    let state = remote.state(&HashSet::from([paired.credential.device]));
    assert_eq!(state.devices.len(), 1);
    assert_eq!(state.devices[0].name, "Phone");
    assert!(state.devices[0].connected);
    assert_eq!(state.pairing_expires_at, None);
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
fn every_authentication_failure_looks_the_same() -> Result<(), ServerError> {
    let remote = enabled()?;
    let secret = offer(&remote, ClientId::new());
    let paired = remote
        .pair(&request(secret), server())
        .ok_or_else(|| unavailable("pairing failed"))?;
    let credential = paired.credential;
    assert_eq!(remote.authenticate(&credential), Some(credential.device));
    let wrong_token = DeviceCredential {
        token: [0; 32],
        ..credential
    };
    let unknown = DeviceCredential {
        device: DeviceId::new(),
        ..credential
    };
    assert_eq!(remote.authenticate(&wrong_token), None);
    assert_eq!(remote.authenticate(&unknown), None);
    remote.configure(RemoteAccessSettings {
        enabled: false,
        port: 7419,
    })?;
    assert_eq!(remote.authenticate(&credential), None);
    assert!(!remote.authorized(credential.device));
    Ok(())
}

#[test]
fn revoked_devices_are_no_longer_authorized() -> Result<(), ServerError> {
    let remote = enabled()?;
    let secret = offer(&remote, ClientId::new());
    let paired = remote
        .pair(&request(secret), server())
        .ok_or_else(|| unavailable("pairing failed"))?;
    assert!(remote.authorized(paired.credential.device));
    remote.revoke(paired.credential.device)?;
    remote.revoke(paired.credential.device)?;
    assert!(!remote.authorized(paired.credential.device));
    assert_eq!(remote.authenticate(&paired.credential), None);
    Ok(())
}

#[test]
fn pairing_needs_access_on_and_listening() -> Result<(), ServerError> {
    let remote = RemoteAccess::memory();
    assert!(remote.start_pairing(ClientId::new()).is_err());
    remote.configure(RemoteAccessSettings {
        enabled: true,
        port: 7419,
    })?;
    assert!(matches!(
        remote.state(&HashSet::new()).status,
        ListenerStatus::Failed(_)
    ));
    assert!(remote.start_pairing(ClientId::new()).is_err());
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
fn listener_changes_are_applied_without_holding_the_state_lock() -> Result<(), ServerError> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&calls);
    let mut remote = RemoteAccess::memory();
    remote.set_listener(Box::new(move |listening| {
        let port = listening.map(|(port, _)| port);
        recorded
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(port);
        ListenerStatus::Listening
    }));
    let remote = Arc::new(remote);
    remote.configure(RemoteAccessSettings {
        enabled: true,
        port: 7419,
    })?;
    remote.configure(RemoteAccessSettings {
        enabled: false,
        port: 7419,
    })?;
    assert_eq!(
        *calls.lock().unwrap_or_else(PoisonError::into_inner),
        vec![Some(7419), None]
    );
    Ok(())
}

#[test]
fn pairing_is_refused_once_the_device_limit_is_reached() -> Result<(), ServerError> {
    let remote = enabled()?;
    remote.lock().stored.devices = (0..MAX_DEVICES)
        .map(|index| Device {
            id: DeviceId::new(),
            name: format!("Phone {index}"),
            token_hash: [0; 32],
            paired_at: 1,
            last_seen: None,
        })
        .collect();
    let error = remote
        .start_pairing(ClientId::new())
        .err()
        .ok_or_else(|| unavailable("pairing started"))?;
    assert!(error.message().contains("Revoke"), "{error}");
    Ok(())
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
