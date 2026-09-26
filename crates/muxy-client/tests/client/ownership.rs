use super::*;
use muxy_protocol::{ClientKind, OperationId, ProjectSession, SessionClient, SessionId};

fn listing(
    connection: &Connection,
    session: &muxy_protocol::SessionInfo,
) -> TestResult<ProjectSession> {
    Ok(connection
        .client
        .project_sessions(session.project, None, None)?
        .sessions
        .into_iter()
        .find(|entry| entry.info.id == session.id)
        .ok_or("session missing")?)
}

#[test]
fn creator_owns_before_subscribing_and_hidden_panes_keep_their_position() -> TestResult {
    let fixture = Fixture::new()?;
    let desktop = fixture.connect()?;
    let tui = fixture.connect()?;
    let observer = fixture.connect()?;
    let desktop_id = desktop.client.identify(ClientKind::Desktop)?;
    let tui_id = tui.client.identify(ClientKind::Tui)?;
    let operation = OperationId::new();
    let session = desktop.client.create_project_session(
        fixture.registry.home_project(),
        operation,
        &fixture.directory,
        SIZE,
    )?;
    assert_eq!(listing(&observer, &session)?.owner, Some(desktop_id));
    assert!(listing(&desktop, &session)?.attached);
    assert!(!listing(&observer, &session)?.attached);
    assert!(
        desktop
            .client
            .available_project_sessions(session.project)?
            .sessions
            .is_empty()
    );
    let tui_attachment = tui.client.attach(session.id, SIZE)?;
    let first = desktop.client.attach(session.id, SIZE)?;
    let duplicate = desktop.client.attach(session.id, SIZE)?;
    desktop.client.sync_session_references(vec![session.id])?;
    desktop.client.detach(first.channel)?;
    desktop.client.detach(duplicate.channel)?;
    assert_eq!(listing(&tui, &session)?.owner, Some(desktop_id));
    let before = desktop
        .client
        .project_sessions(session.project, None, None)?
        .revision;
    desktop
        .client
        .sync_session_references(vec![session.id, session.id])?;
    desktop
        .client
        .create_project_session(session.project, operation, &fixture.directory, SIZE)?;
    assert_eq!(
        desktop
            .client
            .project_sessions(session.project, None, None)?
            .revision,
        before
    );
    desktop.client.sync_session_references(vec![])?;
    assert_eq!(listing(&observer, &session)?.owner, Some(tui_id));
    assert!(!listing(&desktop, &session)?.attached);
    let available = desktop
        .client
        .available_project_sessions(session.project)?
        .sessions;
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].owner, Some(tui_id));
    let reattached = desktop.client.attach(session.id, SIZE)?;
    assert_eq!(listing(&observer, &session)?.owner, Some(tui_id));
    tui.client.detach(tui_attachment.channel)?;
    assert_eq!(listing(&observer, &session)?.owner, Some(desktop_id));
    desktop.client.detach(reattached.channel)?;
    assert_eq!(listing(&observer, &session)?.owner, None);
    assert_eq!(fixture.registry.list().len(), 1);
    Ok(())
}

#[test]
fn shared_layout_instances_are_distinct_and_disconnect_promotes_the_next_client() -> TestResult {
    let fixture = Fixture::new()?;
    let desktop = fixture.connect()?;
    let first = fixture.connect()?;
    let second = fixture.connect()?;
    let observer = fixture.connect()?;
    let first_id = first.client.identify(ClientKind::Tui)?;
    let second_id = second.client.identify(ClientKind::Tui)?;
    let desktop_id = desktop.client.identify(ClientKind::Desktop)?;
    assert_ne!(first_id.id, second_id.id);
    let session = fixture.create(&first.client)?;
    let layout = OperationId::new();
    first
        .client
        .sync_layout_references(Some(layout), 1, vec![session.id])?;
    second
        .client
        .sync_layout_references(Some(layout), 1, vec![session.id])?;
    desktop.client.sync_session_references(vec![session.id])?;
    assert_eq!(listing(&observer, &session)?.owner, Some(first_id));
    first.client.disconnect();
    first.finished.recv_timeout(TIMEOUT)??;
    assert_eq!(listing(&observer, &session)?.owner, Some(second_id));
    second.client.disconnect();
    second.finished.recv_timeout(TIMEOUT)??;
    assert_eq!(listing(&observer, &session)?.owner, Some(desktop_id));
    desktop.client.disconnect();
    desktop.finished.recv_timeout(TIMEOUT)??;
    assert_eq!(listing(&observer, &session)?.owner, None);
    assert!(!fixture.registry.list().is_empty());
    Ok(())
}

#[test]
fn shared_layout_updates_do_not_attach_instances_that_have_not_opened_the_session() -> TestResult {
    let fixture = Fixture::new()?;
    let desktop = fixture.connect()?;
    let first = fixture.connect()?;
    let second = fixture.connect()?;
    let first_id = first.client.identify(ClientKind::Tui)?;
    let second_id = second.client.identify(ClientKind::Tui)?;
    let session = fixture.create(&desktop.client)?;
    let layout = OperationId::new();
    first
        .client
        .sync_layout_references(Some(layout), 1, vec![])?;
    second
        .client
        .sync_layout_references(Some(layout), 1, vec![])?;
    first
        .client
        .sync_layout_references(Some(layout), 2, vec![session.id])?;
    assert!(listing(&first, &session)?.attached);
    assert!(!listing(&second, &session)?.attached);
    assert_eq!(
        second
            .client
            .available_project_sessions(session.project)?
            .sessions
            .len(),
        1
    );
    desktop.client.sync_session_references(vec![])?;
    assert_eq!(listing(&second, &session)?.owner, Some(first_id));
    first.client.disconnect();
    first.finished.recv_timeout(TIMEOUT)??;
    assert_eq!(listing(&second, &session)?.owner, None);
    second
        .client
        .sync_layout_references(Some(layout), 2, vec![session.id])?;
    assert_eq!(listing(&second, &session)?.owner, Some(second_id));
    second
        .client
        .sync_layout_references(Some(layout), 3, vec![])?;
    second
        .client
        .sync_layout_references(Some(layout), 2, vec![session.id])?;
    assert!(!listing(&second, &session)?.attached);
    assert_eq!(listing(&second, &session)?.owner, None);
    Ok(())
}

#[test]
fn attachment_changes_invalidate_session_pages_and_notify_listing_watchers() -> TestResult {
    let fixture = Fixture::new()?;
    let creator = fixture.connect()?;
    let watcher = fixture.connect()?;
    let session = fixture.create(&creator.client)?;
    let catalog_revision = watcher.client.catalog()?.revision;
    let page = watcher
        .client
        .project_sessions(session.project, None, None)?;
    creator.client.sync_session_references(vec![])?;
    assert!(
        matches!(watcher.client.project_sessions(session.project, None, Some(page.revision)),
        Err(ClientError::Server(error)) if error.code == ErrorCode::CatalogChanged)
    );
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let event = watcher
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
        if matches!(event, ClientEvent::SessionsChanged { revision } if revision > page.revision) {
            break;
        }
    }
    assert_eq!(watcher.client.catalog()?.revision, catalog_revision);
    assert_eq!(listing(&watcher, &session)?.owner, None);
    let id = watcher.client.identify(ClientKind::Desktop)?;
    let a = watcher.client.attach(session.id, SIZE)?;
    let b = watcher.client.attach(session.id, SIZE)?;
    watcher.client.detach(a.channel)?;
    assert_eq!(listing(&creator, &session)?.owner, Some(id));
    watcher.client.detach(b.channel)?;
    assert_eq!(listing(&creator, &session)?.owner, None);
    Ok(())
}

#[test]
fn availability_search_skips_attached_pages_and_restarts_after_invalidation() -> TestResult {
    use muxy_protocol::{
        ProjectId, ProjectSessions, ReplyBody, RequestBody, ServerPath, SessionInfo, SessionStatus,
    };
    let project = ProjectId::new();
    let owner = SessionClient {
        kind: ClientKind::Tui,
        ..SessionClient::default()
    };
    let (socket, server) = UnixStream::pair()?;
    let serving = thread::spawn(move || -> Result<(), String> {
        let mut decoder = Decoder::new(server.try_clone().map_err(|e| e.to_string())?);
        let mut encoder = Encoder::new(server);
        assert!(matches!(
            decoder.next().map_err(|e| e.to_string())?.1,
            Message::Hello { .. }
        ));
        encoder
            .send(
                CONTROL,
                &Message::HelloReply {
                    versions: muxy_protocol::SUPPORTED.to_vec(),
                    server: muxy_protocol::ServerInfo::current(),
                    features: Vec::new(),
                },
            )
            .map_err(|e| e.to_string())?;
        for (index, expected_after) in [None, SessionId::new(128), None, SessionId::new(128)]
            .into_iter()
            .enumerate()
        {
            let (_, message) = decoder.next().map_err(|e| e.to_string())?;
            let Message::Request {
                id,
                body:
                    RequestBody::ListProjectSessions {
                        project: received,
                        after,
                        revision,
                    },
            } = message
            else {
                return Err("expected listing".into());
            };
            assert_eq!(received, project);
            assert_eq!(after, expected_after);
            assert_eq!(revision, after.map(|_| if index == 1 { 1 } else { 2 }));
            let body = if index == 1 {
                ReplyBody::Error(muxy_protocol::ErrorReply {
                    code: ErrorCode::CatalogChanged,
                    message: "changed".into(),
                })
            } else {
                let attached = after.is_none();
                let range = if attached { 1..=128 } else { 129..=130 };
                ReplyBody::ProjectSessions(ProjectSessions {
                    revision: if index == 0 { 1 } else { 2 },
                    sessions: range
                        .map(|id| ProjectSession {
                            info: SessionInfo {
                                id: SessionId::new(id).expect("id"),
                                project,
                                directory: ServerPath(b"/tmp".to_vec()),
                            },
                            status: SessionStatus::Live,
                            attached,
                            owner: (id != 130).then_some(owner),
                        })
                        .collect(),
                    next: attached.then(|| SessionId::new(128).expect("id")),
                })
            };
            encoder
                .send(CONTROL, &Message::Reply { id, body })
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    });
    let client = Client::from_stream(Box::new(socket))?;
    let page = client.available_project_sessions(project)?;
    assert_eq!(page.revision, 2);
    assert_eq!(
        page.sessions
            .iter()
            .map(|entry| entry.info.id.get())
            .collect::<Vec<_>>(),
        [129, 130]
    );
    assert_eq!(page.sessions[0].owner, Some(owner));
    assert_eq!(page.sessions[1].owner, None);
    assert!(page.next.is_none());
    serving.join().map_err(|_| "server panicked")??;
    Ok(())
}
