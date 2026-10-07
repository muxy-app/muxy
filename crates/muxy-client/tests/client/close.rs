use super::*;
use muxy_protocol::OperationId;
use std::sync::Barrier;

#[test]
fn close_preserves_another_clients_hidden_pane_until_its_final_close() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let second = fixture.connect()?;
    let session = fixture.create(&first.client)?.id;
    first.client.attach(session, SIZE)?;
    second.client.sync_session_references(vec![session])?;
    first.client.close_session(session, OperationId::new())?;
    assert_eq!(fixture.registry.list().len(), 1);
    let mut attached = second.client.attach(session, SIZE)?;
    second
        .client
        .send_input(attached.channel, b"printf 'SURVIVED_CLOSE\n'\r")?;
    second.frame_containing(&mut attached, "SURVIVED_CLOSE")?;
    second.client.sync_session_references(vec![])?;
    second.client.close_session(session, OperationId::new())?;
    assert!(fixture.registry.list().is_empty());
    assert!(
        second
            .client
            .project_sessions(fixture.registry.home_project(), None, None)?
            .sessions
            .is_empty()
    );
    Ok(())
}

#[test]
fn remaining_local_panes_keep_the_session_and_detach_never_ends_it() -> TestResult {
    let fixture = Fixture::new()?;
    let connection = fixture.connect()?;
    let session = fixture.create(&connection.client)?.id;
    let first = connection.client.attach(session, SIZE)?;
    connection.client.sync_session_references(vec![session])?;
    connection.client.detach(first.channel)?;
    connection
        .client
        .close_session(session, OperationId::new())?;
    assert_eq!(fixture.registry.list().len(), 1);
    connection.client.sync_session_references(vec![])?;
    connection.disconnect()?;
    assert_eq!(fixture.registry.list().len(), 1);
    let reopened = fixture.connect()?;
    reopened.client.close_session(session, OperationId::new())?;
    assert!(fixture.registry.list().is_empty());
    Ok(())
}

#[test]
fn retrying_a_shared_close_after_reconnect_cannot_kill_the_surviving_session() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let second = fixture.connect()?;
    let session = fixture.create(&first.client)?.id;
    let operation = OperationId::new();
    let attachment = second.client.attach(session, SIZE)?;
    first.client.close_session(session, operation)?;
    first.disconnect()?;
    second.client.detach(attachment.channel)?;
    let retry = fixture.connect()?;
    retry.client.close_session(session, operation)?;
    assert_eq!(fixture.registry.list().len(), 1);
    let other = fixture.create(&retry.client)?.id;
    assert!(
        matches!(retry.client.close_session(other, operation), Err(ClientError::Server(error)) if error.code == ErrorCode::BadRequest)
    );
    retry.client.close_session(session, OperationId::new())?;
    assert_eq!(fixture.registry.list()[0].id, other);
    Ok(())
}

#[test]
fn simultaneous_final_closes_are_serialized() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let second = fixture.connect()?;
    for _ in 0..8 {
        let session = fixture.create(&first.client)?.id;
        first.client.attach(session, SIZE)?;
        second.client.attach(session, SIZE)?;
        let barrier = Arc::new(Barrier::new(2));
        let client = first.client.clone();
        let ready = Arc::clone(&barrier);
        let thread = thread::spawn(move || {
            ready.wait();
            client.close_session(session, OperationId::new())
        });
        barrier.wait();
        second.client.close_session(session, OperationId::new())?;
        thread.join().map_err(|_| "close thread panicked")??;
        assert!(fixture.registry.list().is_empty());
    }
    Ok(())
}

#[test]
fn shared_close_receipt_survives_a_server_restart() -> TestResult {
    let mut fixture = Fixture::with_storage("", true)?;
    let first = fixture.connect()?;
    let second = fixture.connect()?;
    let session = fixture.create(&first.client)?.id;
    second.client.sync_session_references(vec![session])?;
    let operation = OperationId::new();
    first.client.close_session(session, operation)?;
    first.disconnect()?;
    second.disconnect()?;
    fixture.registry.end(session)?;
    let (events, _) = mpsc::channel();
    let previous = std::mem::replace(
        &mut fixture.registry,
        Arc::new(Registry::new(ServerSettings::default(), events)),
    );
    drop(previous);
    let (events, _) = mpsc::channel();
    fixture.registry = Arc::new(Registry::persistent(
        ServerSettings::default(),
        events,
        &fixture.directory.join("sessions"),
    )?);
    let retry = fixture.connect()?;
    retry.client.close_session(session, operation)?;
    assert_eq!(
        retry
            .client
            .project_sessions(fixture.registry.home_project(), None, None)?
            .sessions
            .len(),
        1
    );
    retry.client.close_session(session, OperationId::new())?;
    assert!(
        retry
            .client
            .project_sessions(fixture.registry.home_project(), None, None)?
            .sessions
            .is_empty()
    );
    Ok(())
}

#[test]
fn shared_layout_revisions_retire_old_attachments_without_reviving_closed_tabs() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let second = fixture.connect()?;
    let session = fixture.create(&first.client)?.id;
    let owner = OperationId::new();
    first
        .client
        .sync_layout_references(Some(owner), 1, vec![session])?;
    second
        .client
        .sync_layout_references(Some(owner), 1, vec![session])?;
    first.client.attach(session, SIZE)?;
    second.client.attach(session, SIZE)?;
    first
        .client
        .sync_layout_references(Some(owner), 2, vec![])?;
    second
        .client
        .sync_layout_references(Some(owner), 1, vec![session])?;
    assert!(
        matches!(second.client.attach(session, SIZE), Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownSession)
    );
    first.client.close_session(session, OperationId::new())?;
    assert!(fixture.registry.list().is_empty());
    Ok(())
}

#[test]
fn close_preserves_shared_creation_while_cancelling_an_unclaimed_creation() -> TestResult {
    let fixture = Fixture::new()?;
    let desktop = fixture.connect()?;
    let other = fixture.connect()?;
    let project = fixture.registry.home_project();
    let session = desktop.client.create_project_session(
        project,
        OperationId::new(),
        &fixture.directory,
        SIZE,
    )?;
    desktop.client.attach(session.id, SIZE)?;
    other.client.sync_session_references(vec![session.id])?;
    let attached = other.client.attach(session.id, SIZE)?;
    let pending = OperationId::new();
    desktop
        .client
        .create_project_session(project, pending, &fixture.directory, SIZE)?;
    desktop.client.cancel_creation(pending)?;
    desktop.client.sync_session_references(vec![])?;
    desktop
        .client
        .close_session(session.id, OperationId::new())?;
    assert_eq!(fixture.registry.list(), vec![session.clone()]);
    other
        .client
        .write_input(attached.channel, b"printf 'LAYOUT_SURVIVED\\n'\r".to_vec())?;
    other.client.sync_session_references(vec![])?;
    other.client.close_session(session.id, OperationId::new())?;
    assert!(fixture.registry.list().is_empty());
    Ok(())
}
