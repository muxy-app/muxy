use std::sync::{Arc, mpsc};

use crate::{Registry, ServerSettings};

use super::Connection;

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
    let phone = Arc::new(Connection::for_device(
        Arc::clone(&registry.notifier),
        muxy_protocol::DeviceId::new(),
        muxy_protocol::DeviceKind::Phone,
    ));
    registry.notifier.register(&phone);
    let desktop = Arc::new(Connection::new(Arc::clone(&registry.notifier)));
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

#[test]
fn every_session_ended_reaches_connections_before_they_close_at_shutdown()
-> Result<(), Box<dyn std::error::Error>> {
    let (sender, _requests) = mpsc::channel();
    let registry = Registry::new(
        ServerSettings {
            default_shell: Some("/bin/sh".into()),
            ..ServerSettings::default()
        },
        sender,
    );
    let connection = Arc::new(Connection::new(Arc::clone(&registry.notifier)));
    registry.notifier.register(&connection);
    let size = muxy_protocol::Size { cols: 80, rows: 24 };
    let mut sessions: Vec<_> = (0..8)
        .map(|_| {
            registry
                .create(std::path::Path::new("/tmp"), size)
                .map(|info| info.id)
        })
        .collect::<Result<_, _>>()?;
    registry.shutdown();
    registry.wait_stopped();
    registry.close_connections();
    let mut ended: Vec<_> = connection
        .take_control()
        .into_iter()
        .filter_map(|message| match message {
            muxy_protocol::Message::SessionEnded { session, .. } => Some(session),
            _ => None,
        })
        .collect();
    sessions.sort();
    ended.sort();
    assert_eq!(ended, sessions);
    Ok(())
}

#[test]
fn connections_of_one_owner_share_its_references_until_the_last_one_closes()
-> Result<(), Box<dyn std::error::Error>> {
    use muxy_protocol::{Message, OperationId};

    let (sender, _requests) = mpsc::channel();
    let registry = Registry::new(
        ServerSettings {
            default_shell: Some("/bin/sh".into()),
            ..ServerSettings::default()
        },
        sender,
    );
    let connect = || {
        let connection = Arc::new(Connection::new(Arc::clone(&registry.notifier)));
        registry.notifier.register(&connection);
        connection
    };
    let introduced = |connection: &Connection| {
        connection
            .take_control()
            .iter()
            .filter(|message| matches!(message, Message::SessionMetadata { .. }))
            .count()
    };
    let session = registry
        .create(
            std::path::Path::new("/tmp"),
            muxy_protocol::Size { cols: 80, rows: 24 },
        )?
        .id;
    let owner = Some(OperationId::new());
    let (first, second) = (connect(), connect());

    registry.set_references(&first, owner, 2, &[session]);
    registry.set_references(&second, owner, 1, &[]);
    assert!(first.references(session, false) && second.references(session, false));
    assert_eq!((introduced(&first), introduced(&second)), (1, 1));

    registry.set_references(&first, owner, 3, &[]);
    registry.set_references(&first, owner, 4, &[session]);
    assert_eq!((introduced(&first), introduced(&second)), (1, 1));
    registry.set_references(&second, owner, 5, &[]);
    assert!(!first.references(session, false));

    let other = Some(OperationId::new());
    registry.set_references(&second, other, 1, &[session]);
    assert!(second.references(session, false) && !first.references(session, false));

    for connection in [&first, &second] {
        connection.close();
        registry.connection_closed(connection);
    }
    let returning = connect();
    registry.set_references(&returning, owner, 1, &[session]);
    assert!(returning.references(session, false));
    registry.shutdown();
    registry.wait_stopped();
    Ok(())
}

#[test]
fn local_apps_learn_which_saved_files_the_server_could_not_read()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::time::Duration;

    use muxy_protocol::wire::{Decoder, Encoder};
    use muxy_protocol::{CONTROL, Message};

    let directory = std::env::temp_dir().join(format!(
        "muxy-unreadable-{}",
        muxy_protocol::OperationId::new()
    ));
    let sessions = directory.join("sessions");
    std::fs::create_dir_all(&sessions)?;
    std::fs::write(sessions.join("catalog.json"), "damaged {")?;
    std::fs::write(directory.join("remote.json"), "damaged {")?;
    let (sender, _requests) = mpsc::channel();
    let registry = Arc::new(
        Registry::persistent(ServerSettings::default(), sender, &sessions)?
            .with_remote_access(crate::RemoteAccess::open(directory.join("remote.json"))?),
    );
    let (client, server) = UnixStream::pair()?;
    let serving = Arc::clone(&registry);
    let connection = std::thread::spawn(move || super::serve(Box::new(server), serving));
    client.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut decoder = Decoder::new(client.try_clone()?);
    Encoder::new(client.try_clone()?).send(
        CONTROL,
        &Message::Hello {
            versions: muxy_protocol::SUPPORTED.to_vec(),
        },
    )?;
    let (CONTROL, Message::HelloReply { server, .. }) = decoder.next()? else {
        return Err("expected HelloReply".into());
    };
    let mut kept: Vec<_> = server
        .unreadable
        .iter()
        .map(|file| PathBuf::from(file.kept_as.clone()))
        .collect();
    kept.sort();
    assert_eq!(kept.len(), 2);
    assert!(kept[0].starts_with(&directory) && kept[1].starts_with(&sessions));
    for path in kept {
        assert_eq!(std::fs::read_to_string(path)?, "damaged {");
    }
    client.shutdown(std::net::Shutdown::Both)?;
    let _ = connection.join();
    drop(registry);
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

type Wire = (
    muxy_protocol::wire::Encoder<std::os::unix::net::UnixStream>,
    muxy_protocol::wire::Decoder<std::os::unix::net::UnixStream>,
);

type Serving = std::thread::JoinHandle<Result<(), muxy_protocol::wire::WireError>>;

/// A connection to `registry` that has finished its hello.
fn greeted(
    registry: &Arc<Registry>,
    client: &std::os::unix::net::UnixStream,
    server: std::os::unix::net::UnixStream,
) -> Result<(Wire, Serving), Box<dyn std::error::Error>> {
    use muxy_protocol::wire::{Decoder, Encoder};
    use muxy_protocol::{CONTROL, Message};

    let serving = Arc::clone(registry);
    let connection = std::thread::spawn(move || super::serve(Box::new(server), serving));
    client.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
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
    Ok(((encoder, decoder), connection))
}

/// Lists `project`'s sessions and returns the first name among them.
fn listed_name(
    (encoder, decoder): &mut Wire,
    project: muxy_protocol::ProjectId,
    id: u32,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    use muxy_protocol::{CONTROL, Message, ReplyBody, RequestBody, RequestId};

    encoder.send(
        CONTROL,
        &Message::Request {
            id: RequestId(id),
            body: RequestBody::ListProjectSessions {
                project,
                after: None,
                revision: None,
            },
        },
    )?;
    loop {
        match decoder.next()? {
            (
                CONTROL,
                Message::Reply {
                    body: ReplyBody::ProjectSessions(page),
                    ..
                },
            ) => return Ok(page.sessions.into_iter().find_map(|listed| listed.name)),
            (CONTROL, Message::Changed { .. }) => {}
            other => return Err(format!("expected sessions, got {other:?}").into()),
        }
    }
}

#[test]
fn a_session_name_reaches_listing_clients_with_a_sessions_change()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::net::UnixStream;

    use muxy_protocol::{CONTROL, Message, ReplyBody, RequestBody, RequestId, SessionId, Topic};

    let (sender, _requests) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), sender));
    let project = registry.home_project();
    let session = SessionId::new(1).expect("nonzero");
    registry.catalog.reserve(
        muxy_protocol::OperationId::new(),
        &muxy_protocol::SessionInfo {
            project,
            id: session,
            directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
        },
    )?;
    let (client, server) = UnixStream::pair()?;
    let (mut wire, connection) = greeted(&registry, &client, server)?;
    assert_eq!(listed_name(&mut wire, project, 1)?, None);

    wire.0.send(
        CONTROL,
        &Message::Request {
            id: RequestId(2),
            body: RequestBody::SetSessionName {
                session,
                name: Some("API server".into()),
            },
        },
    )?;
    let (mut replied, mut changed) = (false, false);
    while !(replied && changed) {
        match wire.1.next()? {
            (
                CONTROL,
                Message::Reply {
                    body: ReplyBody::SessionNameSet,
                    ..
                },
            ) => replied = true,
            (
                CONTROL,
                Message::Changed {
                    topic: Topic::Sessions,
                    ..
                },
            ) => changed = true,
            (CONTROL, Message::Changed { .. }) => {}
            other => return Err(format!("expected the name to be set, got {other:?}").into()),
        }
    }
    assert_eq!(
        listed_name(&mut wire, project, 3)?.as_deref(),
        Some("API server")
    );

    client.shutdown(std::net::Shutdown::Both)?;
    connection
        .join()
        .map_err(|_| "connection panicked")?
        .map_err(|error| error.to_string())?;
    Ok(())
}
