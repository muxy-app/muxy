use super::*;

fn wait_metadata(
    connection: &Connection,
    session: muxy_protocol::SessionId,
    accept: impl Fn(&muxy_protocol::SessionMetadata) -> bool,
) -> TestResult<muxy_protocol::SessionMetadata> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match connection
            .events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))?
        {
            ClientEvent::SessionMetadata {
                session: received,
                metadata,
            } if received == session && accept(&metadata) => return Ok(metadata),
            ClientEvent::Disconnected => return Err("disconnected".into()),
            ClientEvent::Frame { .. } => {
                return Err("hidden session must not require screen frames".into());
            }
            _ => {}
        }
    }
}

fn wait_title(
    connection: &Connection,
    session: muxy_protocol::SessionId,
    title: &str,
) -> TestResult {
    wait_metadata(connection, session, |metadata| metadata.title == title).map(|_| ())
}

#[test]
fn hidden_session_titles_update_clear_and_reconnect_without_attachments() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let session = fixture.create(&first.client)?;
    first.client.sync_session_references(vec![session.id])?;
    let handle = fixture.registry.handle(session.id).ok_or("session")?;
    for title in ["Working on tests", "Finished tests", ""] {
        handle.send(muxy_server::SessionCommand::Input(
            format!("printf '\\033]2;{title}\\007'\n").into_bytes(),
        ))?;
        wait_title(&first, session.id, title)?;
    }

    handle.send(muxy_server::SessionCommand::Input(
        b"cd /tmp; sleep 30\n".to_vec(),
    ))?;
    wait_metadata(&first, session.id, |metadata| {
        metadata.title.is_empty()
            && metadata.directory.0.ends_with(b"/tmp")
            && metadata
                .process
                .as_ref()
                .is_some_and(|process| process.name == "sleep" && !process.is_shell)
    })?;
    handle.send(muxy_server::SessionCommand::Input(vec![3]))?;
    wait_metadata(&first, session.id, |metadata| {
        metadata
            .process
            .as_ref()
            .is_some_and(|process| process.is_shell)
    })?;
    handle.send(muxy_server::SessionCommand::Input(
        b"printf '\\033]2;Latest title\\007'\n".to_vec(),
    ))?;
    wait_title(&first, session.id, "Latest title")?;
    let second = fixture.connect()?;
    second.client.sync_session_references(vec![session.id])?;
    wait_title(&second, session.id, "Latest title")?;
    first.client.end_session(session.id)?;
    Ok(())
}
