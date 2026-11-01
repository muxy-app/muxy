use std::sync::{Arc, mpsc};

use crate::{Registry, ServerSettings};

use super::Outbox;

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
    let phone = Arc::new(Outbox::for_device(
        Arc::clone(&registry.notifier),
        muxy_protocol::DeviceId::new(),
        muxy_protocol::DeviceKind::Phone,
    ));
    registry.notifier.register(&phone);
    let desktop = Arc::new(Outbox::new(Arc::clone(&registry.notifier)));
    desktop.identify(muxy_protocol::ClientKind::Desktop);
    registry.notifier.register(&desktop);
    assert!(!events.is_empty());
    assert!(registry.claim_activity(&phone, &events).is_empty());
    assert_eq!(registry.claim_activity(&desktop, &events), events);
}

#[test]
fn a_catalog_change_reaches_a_watching_connection_which_is_otherwise_quiet()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    use muxy_protocol::wire::{Decoder, Encoder, WireError};
    use muxy_protocol::{CONTROL, Message, ProjectMutation, ReplyBody, RequestBody, RequestId};

    let (sender, _requests) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), sender));
    let (client, server) = UnixStream::pair()?;
    let serving = Arc::clone(&registry);
    let connection = std::thread::spawn(move || super::serve(Box::new(server), serving));
    client.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut decoder = Decoder::new(client.try_clone()?);
    let mut encoder = Encoder::new(client.try_clone()?);
    encoder.send(
        CONTROL,
        &Message::Hello {
            versions: muxy_protocol::SUPPORTED.to_vec(),
        },
    )?;
    assert!(matches!(
        decoder.next()?,
        (CONTROL, Message::HelloReply { .. })
    ));
    let id = RequestId(1);
    encoder.send(
        CONTROL,
        &Message::Request {
            id,
            body: RequestBody::ReadCatalog {
                after: None,
                revision: None,
            },
        },
    )?;
    let home = match decoder.next()? {
        (
            CONTROL,
            Message::Reply {
                id: replied,
                body: ReplyBody::Catalog(page),
            },
        ) if replied == id => page.home,
        other => return Err(format!("expected the catalog, got {other:?}").into()),
    };

    let revision = registry.mutate_project(&muxy_protocol::ProjectIntent {
        operation: muxy_protocol::OperationId::new(),
        mutation: ProjectMutation::Patch {
            project: home,
            patch: muxy_protocol::ProjectPatch::Name("Renamed".into()),
        },
    })?;
    assert_eq!(
        decoder.next()?,
        (
            CONTROL,
            Message::Changed {
                topic: muxy_protocol::Topic::Catalog,
                revision
            }
        )
    );
    assert!(matches!(
        decoder.next()?,
        (
            CONTROL,
            Message::Changed {
                topic: muxy_protocol::Topic::Sessions,
                ..
            }
        )
    ));

    client.set_read_timeout(Some(Duration::from_millis(300)))?;
    match decoder.next() {
        Err(WireError::Io(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) => {}
        other => return Err(format!("an idle connection sent {other:?}").into()),
    }
    client.shutdown(std::net::Shutdown::Both)?;
    connection
        .join()
        .map_err(|_| "connection panicked")?
        .map_err(|error| error.to_string())?;
    Ok(())
}
