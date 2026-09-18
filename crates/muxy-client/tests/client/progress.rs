use super::*;
use muxy_protocol::{ProgressState, SessionId, SessionProgress, TerminalProgress};

fn wait_progress(
    connection: &Connection,
    session: SessionId,
    expected: SessionProgress,
) -> TestResult {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match connection
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))?
        {
            ClientEvent::Progress {
                session: received,
                progress,
            } if received == session && progress == expected => return Ok(()),
            ClientEvent::Frame { channel, frame } => connection.client.ack(channel, frame.seq)?,
            ClientEvent::Progress { .. }
            | ClientEvent::SessionMetadata { .. }
            | ClientEvent::Metadata { .. }
            | ClientEvent::SessionsChanged { .. }
            | ClientEvent::CatalogChanged { .. } => {}
            other => return Err(format!("expected progress, got {other:?}").into()),
        }
    }
}

#[test]
fn progress_follows_open_pane_references_without_screen_attachments_and_restores() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let session = fixture.create(&first.client)?;
    first.client.sync_session_references(vec![session.id])?;
    let attached = first.client.attach(session.id, SIZE)?;
    let running = SessionProgress {
        progress: Some(TerminalProgress {
            state: ProgressState::Running,
            percent: Some(42),
        }),
        completed: 0,
    };
    first
        .client
        .send_input(attached.channel, b"printf '\\033]9;4;1;42\\007'\n")?;
    wait_progress(&first, session.id, running)?;
    let second = fixture.connect()?;
    second.client.sync_session_references(vec![session.id])?;
    wait_progress(&second, session.id, running)?;
    second.client.sync_session_references(vec![])?;
    second.client.sync_session_references(vec![session.id])?;
    wait_progress(&second, session.id, running)?;
    first.client.detach(attached.channel)?;
    let handle = fixture.registry.handle(session.id).ok_or("session")?;
    handle.send(muxy_server::SessionCommand::Input(
        b"printf '\\033]9;4;0\\007\\033]9;4;3\\007\\033]9;4;0\\007'\n".to_vec(),
    ))?;
    let completed = SessionProgress {
        progress: None,
        completed: 2,
    };
    wait_progress(&first, session.id, completed)?;
    wait_progress(&second, session.id, completed)?;
    let reconnected = fixture.connect()?;
    reconnected
        .client
        .sync_session_references(vec![session.id])?;
    wait_progress(&reconnected, session.id, completed)?;
    reconnected.client.sync_session_references(vec![])?;
    while reconnected.events.try_recv().is_ok() {}
    handle.send(muxy_server::SessionCommand::Input(
        b"printf '\\033]9;4;1;42\\007'\n".to_vec(),
    ))?;
    wait_progress(
        &first,
        session.id,
        SessionProgress {
            completed: 2,
            ..running
        },
    )?;
    assert!(
        !reconnected
            .events
            .try_iter()
            .any(|event| matches!(event, ClientEvent::Progress { .. }))
    );
    first.client.end_session(session.id)?;
    Ok(())
}
