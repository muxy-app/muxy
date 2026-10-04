//! `muxy --host`: the TUI on the server of another computer, reached through
//! a fake ssh that runs the bridge on this computer.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::{Duration, Instant};

use muxy_client::{Client, Start};
use muxy_protocol::{SessionClient, SessionInfo};

use super::fixture::{Fixture, Result, Tui};
use super::remote::FakeRemote;

fn remote_tui<'a>(fixture: &'a Fixture, remote: &FakeRemote) -> Result<Tui<'a>> {
    let ssh = remote.ssh().display().to_string();
    Tui::launch(fixture, &[("MUXY_SSH", &ssh)], &["--host", "box"])
}

fn owner(client: &Client, session: &SessionInfo) -> Result<Option<SessionClient>> {
    Ok(client
        .project_sessions(session.project, None, None)?
        .sessions
        .into_iter()
        .find(|listed| listed.info.id == session.id)
        .ok_or("the session is gone")?
        .owner)
}

/// Ends the bridge, which the fake ssh became, as a dropped connection would.
fn kill_bridge(remote: &FakeRemote) -> Result<String> {
    let bridge = fs::read_to_string(remote.home().join("bridge.pid"))?;
    let status = Command::new("kill")
        .args(["-KILL", bridge.trim()])
        .status()?;
    assert!(status.success());
    Ok(bridge)
}

#[test]
fn the_tui_keeps_a_remote_servers_layout_apart_and_detaching_leaves_it_running() -> Result {
    let fixture = Fixture::new()?;
    let remote = FakeRemote::new()?;
    let mut tui = remote_tui(&fixture, &remote)?;
    tui.ready()?;
    tui.write(b"echo \"REMOTE_HOME=$HOME\"\r")?;
    tui.output(&format!("REMOTE_HOME={}", remote.home().display()))?;
    let client = Client::connect_ssh(&remote.target()?, Start::Never)?;
    let layouts = fixture
        .directory
        .path()
        .join("servers")
        .join(client.catalog()?.server.to_string());
    tui.wait(|_| Ok(layouts.join("tui-state.json").is_file()))?;
    assert_eq!(fs::metadata(&layouts)?.permissions().mode() & 0o777, 0o700);
    assert!(!fixture.directory.path().join("tui-state.json").exists());
    tui.detach()?;
    assert!(
        remote
            .muxy(&["server", "status"])
            .output()?
            .status
            .success()
    );

    let sessions: Vec<_> = client
        .list_sessions()?
        .iter()
        .map(|session| session.id)
        .collect();
    assert_eq!(sessions.len(), 1);
    let mut restored = remote_tui(&fixture, &remote)?;
    restored.ready()?;
    let restored_sessions: Vec<_> = client
        .list_sessions()?
        .iter()
        .map(|session| session.id)
        .collect();
    assert_eq!(restored_sessions, sessions);
    restored.detach()?;
    assert!(!fixture.directory.path().join("server.sock").exists());
    Ok(())
}

#[test]
fn a_refused_login_is_reported_before_the_tui_takes_over_the_terminal() -> Result {
    let fixture = Fixture::new()?;
    let remote =
        FakeRemote::with_script("echo 'dev@box: Permission denied (publickey).' >&2; exit 255")?;
    let mut tui = remote_tui(&fixture, &remote)?;
    assert_eq!(tui.exit()?.code, Some(1));
    assert!(!tui.raw.windows(8).any(|bytes| bytes == b"\x1b[?1049h"));
    let text = tui.text()?.join("\n");
    assert!(
        text.contains("muxy: SSH refused the login to box."),
        "{text}"
    );
    tui.assert_restored()?;
    assert!(!fixture.directory.path().join("servers").exists());
    Ok(())
}

#[test]
fn the_tui_reconnects_after_its_bridge_dies_and_the_pane_resumes() -> Result {
    let fixture = Fixture::new()?;
    let remote = FakeRemote::with_script(r#"echo $$ > "$HOME/bridge.pid""#)?;
    let mut tui = remote_tui(&fixture, &remote)?;
    tui.ready()?;
    let client = Client::connect(&remote.socket())?;
    let session = client.list_sessions()?.remove(0);
    let first = owner(&client, &session)?;
    assert!(first.is_some());
    kill_bridge(&remote)?;
    tui.wait(|_| Ok(owner(&client, &session)?.is_some_and(|now| Some(now) != first)))?;
    let mut typed: Option<Instant> = None;
    tui.wait(|tui| {
        if tui
            .text()?
            .iter()
            .any(|row| row.contains("RESUMED_AFTER") && !row.contains("printf"))
        {
            return Ok(true);
        }
        // Keys typed in the moment before the pane is attached again are
        // dropped, so type until it answers.
        if typed.is_none_or(|at| at.elapsed() > Duration::from_secs(1)) {
            tui.write(b"printf '\\nRESUMED_%s\\n' AFTER\r")?;
            typed = Some(Instant::now());
        }
        Ok(false)
    })?;
    tui.detach()?;
    Ok(())
}

#[test]
fn detach_works_while_a_reconnect_waits_on_an_unresponsive_host() -> Result {
    let fixture = Fixture::new()?;
    let remote = FakeRemote::with_script(
        r#"echo $$ > "$HOME/bridge.pid"; if [ -e "$HOME/hang" ]; then exec sleep 30; fi"#,
    )?;
    let mut tui = remote_tui(&fixture, &remote)?;
    tui.ready()?;
    fs::write(remote.home().join("hang"), "")?;
    kill_bridge(&remote)?;
    tui.output("Connecting to box…")?;
    let detaching = Instant::now();
    tui.detach()?;
    assert!(
        detaching.elapsed() < Duration::from_secs(5),
        "detach waited {:?}",
        detaching.elapsed()
    );
    Ok(())
}

#[test]
fn a_reconnect_refused_by_ssh_ends_the_tui_instead_of_retrying() -> Result {
    let fixture = Fixture::new()?;
    let remote = FakeRemote::with_script(
        r#"echo $$ > "$HOME/bridge.pid"; echo >> "$HOME/attempts"
        if [ -e "$HOME/refuse" ]; then echo 'dev@box: Permission denied (publickey).' >&2; exit 255; fi"#,
    )?;
    let mut tui = remote_tui(&fixture, &remote)?;
    tui.ready()?;
    fs::write(remote.home().join("refuse"), "")?;
    kill_bridge(&remote)?;
    assert_eq!(tui.exit()?.code, Some(1));
    tui.assert_restored()?;
    let text = tui.text()?.join("\n");
    assert!(
        text.contains("muxy: SSH refused the login to box."),
        "{text}"
    );
    let attempts = fs::read_to_string(remote.home().join("attempts"))?;
    assert_eq!(attempts.lines().count(), 2);
    Ok(())
}

#[test]
fn a_different_server_answering_after_a_reconnect_ends_the_tui_with_advice() -> Result {
    let fixture = Fixture::new()?;
    let elsewhere = FakeRemote::new()?;
    let remote = FakeRemote::with_script(
        r#"echo $$ > "$HOME/bridge.pid"; if [ -e "$HOME/moved" ]; then MUXY_DIR=$(cat "$HOME/moved"); fi"#,
    )?;
    let mut tui = remote_tui(&fixture, &remote)?;
    tui.ready()?;
    fs::write(
        remote.home().join("moved"),
        elsewhere.profile().display().to_string(),
    )?;
    kill_bridge(&remote)?;
    assert_eq!(tui.exit()?.code, Some(1));
    tui.assert_restored()?;
    let text = tui.text()?.join("\n");
    assert!(
        text.contains(
            "muxy: A different Muxy server now runs on box. Run muxy --host box again to use it."
        ),
        "{text}"
    );
    assert!(elsewhere.socket().exists());
    Ok(())
}
