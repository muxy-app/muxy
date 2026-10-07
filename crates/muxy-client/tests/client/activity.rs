use super::*;
use muxy_protocol::{AgentState, ClientKind, SessionId};

fn wait_state(
    client: &Client,
    session: SessionId,
    expected: AgentState,
) -> TestResult<muxy_protocol::ActivitySnapshot> {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        let snapshot = client.activity()?;
        if snapshot
            .agents
            .iter()
            .any(|agent| agent.session == session && agent.state == expected)
        {
            return Ok(snapshot);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!("agent did not reach {expected:?}: {:?}", client.activity()?).into())
}

#[test]
fn activity_tracks_unattached_sessions_and_clears_acknowledged_or_ended_events() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let second = fixture.connect()?;
    first.client.identify(ClientKind::Desktop)?;
    second.client.identify(ClientKind::Desktop)?;
    let session = fixture.create(&first.client)?;
    let handle = fixture.registry.handle(session.id).ok_or("session")?;
    // A real foreground process with a provider argv0; no screen attachments or pane references.
    handle.send(muxy_server::SessionCommand::Input(
        b"/bin/bash -c 'exec -a codex /bin/cat'\n".to_vec(),
    ))?;
    wait_state(&first.client, session.id, AgentState::Idle)?;
    handle.send(muxy_server::SessionCommand::Input(
        b"\x1b]2;\xe2\xa0\x8b Fix tests\x07\n".to_vec(),
    ))?;
    wait_state(&first.client, session.id, AgentState::Working)?;
    handle.send(muxy_server::SessionCommand::Input(
        b"\x1b]2;Action Required\x07\n".to_vec(),
    ))?;
    let blocked = wait_state(&first.client, session.id, AgentState::Blocked)?;
    let attention = blocked.events.first().ok_or("attention event")?.id;
    assert_eq!(second.client.activity()?.events, blocked.events);
    assert!(second.client.claim_activity(vec![attention])?.is_empty());
    assert_eq!(
        first.client.claim_activity(vec![attention])?,
        vec![attention]
    );
    assert!(first.client.claim_activity(vec![attention])?.is_empty());
    second.client.acknowledge_activity(vec![attention])?;
    assert!(first.client.activity()?.events.is_empty());
    assert_eq!(
        first.client.activity()?.agents[0].state,
        AgentState::Blocked
    );
    handle.send(muxy_server::SessionCommand::Input(
        b"\x1b]2;\xe2\xa0\x8b Fix tests\x07\n".to_vec(),
    ))?;
    wait_state(&first.client, session.id, AgentState::Working)?;
    handle.send(muxy_server::SessionCommand::Input(
        b"\x1b]2;Fix tests\x07\n".to_vec(),
    ))?;
    let done = wait_state(&first.client, session.id, AgentState::Idle)?;
    assert_eq!(done.events[0].kind, muxy_protocol::ActivityKind::Completed);
    let reconnected = fixture.connect()?;
    assert_eq!(reconnected.client.activity()?.events, done.events);
    first.client.end_session(session.id)?;
    let deadline = Instant::now() + TIMEOUT;
    while !first.client.activity()?.agents.is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(first.client.activity()?.agents.is_empty());
    assert!(first.client.activity()?.events.is_empty());
    Ok(())
}
