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
