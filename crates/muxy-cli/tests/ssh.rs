//! Reaching a server on another computer over SSH. A fake ssh runs the remote
//! command on this computer, against the real `muxy` and `muxy-server`.

#[path = "support/ssh.rs"]
mod remote;
mod support;

use std::fs;
use std::os::unix::fs::symlink;
use std::process::{Command, Stdio};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::{Client, ClientError, ClientEvent, RemoteReason, Start};
use muxy_protocol::{ChannelId, ClientKind, SessionClient, SessionInfo, Size};
use remote::{FakeRemote, Result};

const TIMEOUT: Duration = Duration::from_secs(10);
const SIZE: Size = Size { cols: 80, rows: 24 };

fn refused(remote: &FakeRemote, start: Start) -> Result<(RemoteReason, String)> {
    let error = Client::connect_ssh(&remote.target()?, start)
        .err()
        .ok_or("connected unexpectedly")?;
    let message = error.to_string();
    match error {
        ClientError::Remote {
            reason,
            destination,
            ..
        } if destination == "box" => Ok((reason, message)),
        other => Err(format!("expected a remote error naming box, got {other:?}").into()),
    }
}

fn output_contains(
    client: &Client,
    events: &Receiver<ClientEvent>,
    channel: ChannelId,
    needle: &str,
) -> Result {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            ClientEvent::Frame {
                channel: framed,
                frame,
            } if framed == channel => {
                let text: String = frame
                    .rows
                    .iter()
                    .flat_map(|row| row.runs.iter().map(|run| run.text.as_str()))
                    .collect();
                if text.contains(needle) {
                    return Ok(());
                }
                client.ack(channel, frame.seq)?;
            }
            ClientEvent::Disconnected => return Err("the bridge disconnected".into()),
            _ => {}
        }
    }
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

#[test]
fn ssh_starts_the_remote_server_and_carries_a_terminal() -> Result {
    let remote = FakeRemote::new()?;
    assert!(!remote.socket().exists());
    let client = Client::connect_ssh(&remote.target()?, Start::IfNeeded)?;
    assert!(remote.socket().exists());
    let events = client.events().ok_or("events already taken")?;
    assert_eq!(client.list_sessions()?, vec![]);

    let session = client.create_session(&remote.home(), SIZE)?;
    let attachment = client.attach(session.id, SIZE)?;
    client.send_input(attachment.channel, b"echo bridged-$((40+2))\n")?;
    output_contains(&client, &events, attachment.channel, "bridged-42")?;
    client.end_session(session.id)?;
    Ok(())
}

#[test]
fn shell_noise_before_the_bridge_is_skipped() -> Result {
    let remote = FakeRemote::with_script("echo 'Welcome to box'; printf 'Last login: today'")?;
    Client::connect_ssh(&remote.target()?, Start::IfNeeded)?.ping()?;
    Ok(())
}

#[test]
fn a_refused_login_names_the_host_and_the_fix() -> Result {
    let remote =
        FakeRemote::with_script("echo 'dev@box: Permission denied (publickey).' >&2; exit 255")?;
    let (reason, message) = refused(&remote, Start::IfNeeded)?;
    assert_eq!(reason, RemoteReason::AuthenticationFailed);
    assert!(
        message.starts_with("SSH refused the login to box."),
        "{message}"
    );
    Ok(())
}

#[test]
fn muxy_is_found_in_the_installers_folder_or_reported_missing() -> Result {
    let remote = FakeRemote::with_script("export PATH=/usr/bin:/bin")?;
    let (reason, message) = refused(&remote, Start::IfNeeded)?;
    assert_eq!(reason, RemoteReason::NotInstalled);
    assert_eq!(
        message,
        "Muxy isn't installed on box (looked on PATH and in ~/.local/bin)."
    );

    let installed = remote.home().join(".local/bin");
    fs::create_dir_all(&installed)?;
    symlink(support::binary(), installed.join("muxy"))?;
    Client::connect_ssh(&remote.target()?, Start::IfNeeded)?.ping()?;
    Ok(())
}

#[test]
fn no_start_never_starts_a_server_but_uses_a_running_one() -> Result {
    let remote = FakeRemote::new()?;
    let (reason, message) = refused(&remote, Start::Never)?;
    assert_eq!(reason, RemoteReason::NotRunning);
    assert_eq!(message, "Muxy's server isn't running on box.");
    assert!(!remote.socket().exists());

    let started = Client::connect_ssh(&remote.target()?, Start::IfNeeded)?;
    let running = Client::connect_ssh(&remote.target()?, Start::Never)?;
    assert_eq!(running.server_info(), started.server_info());
    Ok(())
}

#[test]
fn dropping_the_client_ends_ssh_and_its_bridge_but_not_the_server() -> Result {
    let remote = FakeRemote::with_script(r#"echo $$ > "$HOME/ssh.pid""#)?;
    let client = Client::connect_ssh(&remote.target()?, Start::IfNeeded)?;
    let identity = client.identify(ClientKind::Cli)?;
    let session = client.create_session(&remote.home(), SIZE)?;
    client.attach(session.id, SIZE)?;
    let local = Client::connect(&remote.socket())?;
    assert_eq!(owner(&local, &session)?, Some(identity));
    let ssh = fs::read_to_string(remote.home().join("ssh.pid"))?;

    drop(client);
    // Killed and reaped, so nothing answers to its process ID.
    let signalled = Command::new("kill")
        .args(["-0", ssh.trim()])
        .stderr(Stdio::null())
        .status()?;
    assert!(!signalled.success(), "ssh {} is still running", ssh.trim());
    let deadline = Instant::now() + TIMEOUT;
    while owner(&local, &session)?.is_some() {
        assert!(
            Instant::now() < deadline,
            "the bridge kept its server connection"
        );
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(local.list_sessions()?, vec![session]);
    Ok(())
}

#[test]
fn the_bridge_alone_reports_a_missing_server_on_stderr_only() -> Result {
    let remote = FakeRemote::new()?;
    let output = remote
        .muxy(&["stdio", "--no-start"])
        .stdin(Stdio::null())
        .output()?;
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr)?,
        "muxy: connection failed: the server isn't running\n"
    );
    assert!(!remote.socket().exists());
    Ok(())
}

#[test]
fn the_bridge_alone_starts_the_server_announces_itself_and_ends_with_stdin() -> Result {
    let remote = FakeRemote::new()?;
    let output = remote.muxy(&["stdio"]).stdin(Stdio::null()).output()?;
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"MUXY-STDIO/1\n");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(remote.socket().exists());
    Ok(())
}
