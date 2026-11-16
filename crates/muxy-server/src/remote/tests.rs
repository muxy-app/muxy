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
        kind: DeviceKind::Phone,
    }
}

fn server() -> ServerIdentity {
    ServerIdentity::from_u128(1)
}

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
fn an_unreadable_store_is_moved_aside_and_access_starts_off()
-> Result<(), Box<dyn std::error::Error>> {
    let directory =
        std::env::temp_dir().join(format!("muxy-remote-{}", muxy_protocol::OperationId::new()));
    std::fs::create_dir(&directory)?;
    let path = directory.join("remote.json");
    std::fs::write(&path, "damaged")?;
    let remote = RemoteAccess::open(path.clone())?;
    assert_eq!(
        remote.state(&HashSet::new()).status,
        ListenerStatus::Disabled
    );
    let kept = remote
        .unreadable()
        .map(|file| PathBuf::from(file.kept_as.clone()))
        .ok_or("the unreadable store is not reported")?;
    assert_eq!(std::fs::read_to_string(kept)?, "damaged");
    assert!(!path.exists());
    remote.configure(RemoteAccessSettings {
        enabled: true,
        port: 7419,
    })?;
    assert!(store::load(&path)?.identity.is_some());
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn a_computer_keeps_its_kind_and_unknown_kinds_pair_as_phones() -> Result<(), ServerError> {
    let remote = enabled()?;
    let mut kinds = Vec::new();
    for kind in [DeviceKind::Computer, DeviceKind::Unrecognized(9)] {
        let secret = offer(&remote, ClientId::new());
        let paired = remote
            .pair(
                &PairRequest {
                    kind,
                    ..request(secret)
                },
                server(),
            )
            .ok_or_else(|| ServerError::new(ErrorCode::BadRequest, "not paired"))?;
        let authenticated = remote.authenticate(&paired.credential);
        kinds.push(authenticated.map(|(_, kind)| kind));
    }
    assert_eq!(kinds, [Some(DeviceKind::Computer), Some(DeviceKind::Phone)]);
    let listed: Vec<_> = remote
        .state(&HashSet::new())
        .devices
        .iter()
        .map(|device| device.kind)
        .collect();
    assert_eq!(listed, [DeviceKind::Computer, DeviceKind::Phone]);
    Ok(())
}

#[test]
fn nothing_is_saved_once_access_is_closed() -> Result<(), ServerError> {
    let remote = enabled()?;
    remote.close();
    assert!(remote.set_push_notifications(false).is_err());
    assert!(remote.push_notifications());
    Ok(())
}

fn block_saves(remote: &mut RemoteAccess) -> io::Result<PathBuf> {
    let directory =
        std::env::temp_dir().join(format!("muxy-remote-{}", muxy_protocol::OperationId::new()));
    let path = directory.join("remote.json");
    std::fs::create_dir_all(&path)?;
    remote.store = Some(path);
    Ok(directory)
}

#[test]
fn failed_configuration_preserves_identity_listener_and_pairing_offer()
-> Result<(), Box<dyn std::error::Error>> {
    let mut remote = listening();
    let directory = block_saves(&mut remote)?;
    let path = directory.join("remote.json");
    let before = remote.state(&HashSet::new());
    let settings = RemoteAccessSettings {
        enabled: true,
        port: 7420,
    };
    let error = remote
        .configure(settings)
        .err()
        .ok_or("configuration succeeded")?;
    assert_eq!(error.code(), ErrorCode::PersistenceFailed);
    assert!(
        error
            .message()
            .starts_with("could not save device access: ")
    );
    assert_eq!(remote.state(&HashSet::new()), before);
    assert!(remote.lock().stored.identity.is_none());

    std::fs::remove_dir(&path)?;
    remote.configure(settings)?;
    let identity = remote.lock().stored.identity.clone();
    assert_eq!(store::load(&path)?.identity, identity);
    let secret = offer(&remote, ClientId::new());
    let before = remote.state(&HashSet::new());
    std::fs::remove_file(&path)?;
    std::fs::create_dir(&path)?;
    assert!(
        remote
            .configure(RemoteAccessSettings {
                enabled: false,
                ..settings
            })
            .is_err()
    );
    assert_eq!(remote.state(&HashSet::new()), before);
    assert_eq!(remote.lock().stored.identity, identity);
    assert_eq!(
        remote.lock().offer.as_ref().map(|offer| offer.secret),
        Some(secret)
    );
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn failed_pairing_keeps_the_offer_and_a_retry_saves_the_credential()
-> Result<(), Box<dyn std::error::Error>> {
    let mut remote = enabled()?;
    let secret = offer(&remote, ClientId::new());
    let before = remote.state(&HashSet::new());
    let directory = block_saves(&mut remote)?;
    let path = directory.join("remote.json");
    assert!(remote.pair(&request(secret), server()).is_none());
    assert_eq!(remote.state(&HashSet::new()), before);
    assert_eq!(
        remote.lock().offer.as_ref().map(|offer| offer.secret),
        Some(secret)
    );

    std::fs::remove_dir(&path)?;
    let paired = remote
        .pair(&request(secret), server())
        .ok_or("pairing failed")?;
    let saved = store::load(&path)?;
    assert_eq!(saved.devices.len(), 1);
    assert_eq!(saved.devices[0].id, paired.credential.device);
    assert_eq!(saved.devices[0].token_hash, hash(&paired.credential.token));
    assert_eq!(
        remote.authenticate(&paired.credential),
        Some((paired.credential.device, DeviceKind::Phone))
    );
    assert!(remote.pair(&request(secret), server()).is_none());
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn failed_device_saves_keep_authorization_and_the_previous_last_seen()
-> Result<(), Box<dyn std::error::Error>> {
    let mut remote = enabled()?;
    let secret = offer(&remote, ClientId::new());
    let paired = remote
        .pair(&request(secret), server())
        .ok_or("pairing failed")?;
    remote.lock().stored.devices[0].last_seen = Some(1);
    let before = remote.state(&HashSet::new());
    let directory = block_saves(&mut remote)?;
    let error = remote
        .revoke(paired.credential.device)
        .err()
        .ok_or("revocation succeeded")?;
    assert_eq!(error.code(), ErrorCode::PersistenceFailed);
    assert_eq!(remote.state(&HashSet::new()), before);
    assert!(remote.authorized(paired.credential.device));
    assert_eq!(
        remote.authenticate(&paired.credential),
        Some((paired.credential.device, DeviceKind::Phone))
    );
    assert_eq!(
        remote.state(&HashSet::new()),
        RemoteAccessState {
            revision: before.revision + 1,
            ..before
        }
    );

    let path = directory.join("remote.json");
    std::fs::remove_dir(&path)?;
    remote.revoke(paired.credential.device)?;
    assert!(!remote.authorized(paired.credential.device));
    assert!(remote.authenticate(&paired.credential).is_none());
    assert!(store::load(&path)?.devices.is_empty());
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn failed_push_saves_keep_registration_secrets_and_retention()
-> Result<(), Box<dyn std::error::Error>> {
    let mut remote = enabled()?;
    let secret = offer(&remote, ClientId::new());
    let paired = remote
        .pair(&request(secret), server())
        .ok_or("pairing failed")?;
    let device = paired.credential.device;
    let active = PushRecord {
        token: muxy_protocol::PushToken {
            platform: muxy_protocol::PushPlatform::Apns,
            token: "ab".repeat(32),
            environment: Some(muxy_protocol::PushEnvironment::Sandbox),
        },
        key: [3; 32],
        relay: Some(
            Credentials::new(
                "01jc8q4m6x2v7s0t3n5r9k1b2d".into(),
                "q5XxAbCdEfGhIjKlMnOpQrStUvWxYz0123456789-_A".into(),
            )
            .ok_or("invalid test credentials")?,
        ),
        status: PushStatus::Active,
    };
    assert!(remote.replace_push(device, None, Some(active.clone()))?);
    let before = remote.state(&HashSet::new());
    let directory = block_saves(&mut remote)?;
    let error = remote
        .set_push_notifications(false)
        .err()
        .ok_or("push was disabled")?;
    assert_eq!(error.code(), ErrorCode::PersistenceFailed);
    assert_eq!(remote.push_record(device), Some(active.clone()));
    assert_eq!(remote.state(&HashSet::new()), before);
    let refused = PushRecord {
        status: PushStatus::Refused,
        ..active.clone()
    };
    assert!(
        remote
            .replace_push(device, Some(&active), Some(refused))
            .is_err()
    );
    assert!(remote.replace_push(device, Some(&active), None).is_err());
    assert_eq!(remote.push_record(device), Some(active.clone()));
    assert_eq!(remote.state(&HashSet::new()), before);

    let path = directory.join("remote.json");
    std::fs::remove_dir(&path)?;
    let switch = remote.set_push_notifications(false)?;
    assert_eq!(switch.deleted, active.relay.into_iter().collect::<Vec<_>>());
    let paused = remote.push_record(device);
    assert_eq!(store::load(&path)?.devices[0].push, paused);
    let before = remote.state(&HashSet::new());
    let until = before.push_kept_until.ok_or("missing retention deadline")?;
    std::fs::remove_file(&path)?;
    std::fs::create_dir(&path)?;
    assert!(remote.expire_paused_push(until).is_err());
    assert!(remote.set_push_notifications(true).is_err());
    assert_eq!(remote.push_record(device), paused);
    assert_eq!(remote.state(&HashSet::new()), before);

    std::fs::remove_dir(&path)?;
    assert!(remote.expire_paused_push(until)?);
    assert_eq!(remote.push_record(device), None);
    assert_eq!(store::load(&path)?.devices[0].push, None);
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
