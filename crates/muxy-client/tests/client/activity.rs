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
fn activity_tracks_unattached_sessions_and_shares_history_reads_and_delivery() -> TestResult {
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
    assert!(first.client.activity()?.events[0].read);
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
    assert_eq!(first.client.activity()?.events, done.events);
    Ok(())
}

#[test]
fn agent_replacing_the_session_shell_is_detected() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let session = fixture.create(&first.client)?;
    let handle = fixture.registry.handle(session.id).ok_or("session")?;
    handle.send(muxy_server::SessionCommand::Input(
        b"exec /bin/bash -c 'exec -a codex /bin/cat'\n".to_vec(),
    ))?;
    wait_state(&first.client, session.id, AgentState::Idle)?;
    handle.send(muxy_server::SessionCommand::Input(
        b"\x1b]2;\xe2\xa0\x8b Fix tests\x07\n".to_vec(),
    ))?;
    wait_state(&first.client, session.id, AgentState::Working)?;
    first.client.end_session(session.id)?;
    Ok(())
}

#[test]
fn codex_finishes_while_its_hidden_terminal_keeps_animating() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.connect()?;
    let session = fixture.create(&first.client)?;
    let handle = fixture.registry.handle(session.id).ok_or("session")?;
    handle.send(muxy_server::SessionCommand::Input(
        b"stty -echo; /bin/bash -c 'exec -a codex /bin/cat'\n".to_vec(),
    ))?;
    wait_state(&first.client, session.id, AgentState::Idle)?;
    handle.send(muxy_server::SessionCommand::Input(
        b"\x1b]2;\xe2\xa0\x8b Fix tests\x07\n".to_vec(),
    ))?;
    wait_state(&first.client, session.id, AgentState::Working)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let output = thread::spawn(move || {
        let mut tick = 0;
        while !stopped.load(Ordering::Relaxed) {
            let screen = format!(
                "\x1b[2J\x1b[HAll tests passed.\r\n› Ask Codex to do anything {}⠈\r\n  gpt-6-astra low · ~/project\x1b]2;Fix tests\x07\n",
                " ".repeat(tick % 20)
            );
            if handle
                .send(muxy_server::SessionCommand::Input(screen.into_bytes()))
                .is_err()
            {
                break;
            }
            tick += 1;
            thread::sleep(Duration::from_millis(100));
        }
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    let result = (|| -> TestResult {
        while Instant::now() < deadline {
            let snapshot = first.client.activity()?;
            if snapshot
                .agents
                .iter()
                .any(|a| a.session == session.id && a.state == AgentState::Idle)
            {
                assert_eq!(
                    snapshot
                        .events
                        .iter()
                        .filter(|e| e.session == session.id
                            && e.kind == muxy_protocol::ActivityKind::Completed)
                        .count(),
                    1
                );
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err("Codex stayed working while its completed screen animated".into())
    })();
    stop.store(true, Ordering::Relaxed);
    output.join().map_err(|_| "output thread")?;
    first.client.end_session(session.id)?;
    result
}
