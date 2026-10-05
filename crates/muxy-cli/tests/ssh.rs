//! Reaching a server on another computer over SSH. A fake ssh runs the remote
//! command on this computer, against the real `muxy` and `muxy-server`.

#[path = "support/ssh.rs"]
mod remote;
mod support;

use std::fs;
use std::net::TcpListener;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::{Client, ClientError, ClientEvent, RemoteReason, Start};
use muxy_protocol::{ChannelId, ClientKind, SessionClient, SessionInfo, Size};
use remote::{FakeRemote, Result};
use serde_json::Value;

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

/// `muxy --host box` on this computer, with `local` as its own profile.
fn host_command(remote: &FakeRemote, local: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(support::binary());
    command
        .args(["--host", "box"])
        .args(args)
        .env("MUXY_SSH", remote.ssh())
        .env("MUXY_DIR", local)
        .env_remove("MUXY_SERVER_BIN")
        .env_remove("MUXY_PANE_ID")
        .stdin(Stdio::null());
    command
}

/// Runs `muxy --host box` on this computer, with `local` as its own profile.
fn host(remote: &FakeRemote, local: &Path, args: &[&str]) -> Result<Output> {
    Ok(host_command(remote, local, args).output()?)
}

fn succeeded(output: Output) -> Result<String> {
    assert!(output.status.success(), "{output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

fn failed(output: Output) -> Result<String> {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    Ok(String::from_utf8(output.stderr)?)
}

#[test]
fn host_runs_server_commands_on_the_other_computer_only() -> Result {
    let remote = FakeRemote::new()?;
    let local = tempfile::tempdir()?;
    let muxy = |args: &[&str]| host(&remote, local.path(), args);

    for command in [&["server", "status"][..], &["server", "stop"]] {
        assert_eq!(
            failed(muxy(command)?)?,
            "muxy: Muxy's server isn't running on box.\n"
        );
    }
    assert!(!remote.socket().exists());
    let info: Value = serde_json::from_str(&succeeded(muxy(&["server", "start"])?)?)?;
    assert!(info["build"]["version"].is_string(), "{info}");
    succeeded(muxy(&["server", "status"])?)?;

    let folder = remote.home().join("app");
    fs::create_dir(&folder)?;
    let folder = folder.to_str().ok_or("folder")?;
    let added: Value = serde_json::from_str(&succeeded(muxy(&[
        "project", "add", folder, "--name", "App", "--json",
    ])?)?)?;
    let projects: Value = serde_json::from_str(&succeeded(muxy(&["project", "list", "--json"])?)?)?;
    assert!(
        projects
            .as_array()
            .ok_or("projects")?
            .iter()
            .any(|project| project["id"] == added["id"] && project["directory"] == folder),
        "{projects}"
    );
    assert_eq!(
        failed(muxy(&["project", "add", "app"])?)?,
        "muxy: app: with --host, directories must be absolute paths on that computer\n"
    );
    assert!(failed(muxy(&["project", "add", "/missing/app"])?)?.contains("does not exist"));

    let created: Value =
        serde_json::from_str(&succeeded(muxy(&["session", "create", "App", "--json"])?)?)?;
    assert_eq!(created["directory"], folder);
    let session = created["id"].as_str().ok_or("session ID")?;
    succeeded(muxy(&[
        "session",
        "send",
        session,
        "echo remote-$((40+2))",
    ])?)?;
    succeeded(muxy(&["session", "send-keys", session, "Enter"])?)?;
    let deadline = Instant::now() + TIMEOUT;
    while !succeeded(muxy(&["session", "read-screen", session])?)?.contains("remote-42") {
        assert!(
            Instant::now() < deadline,
            "the remote terminal never answered"
        );
        thread::sleep(Duration::from_millis(50));
    }

    assert!(succeeded(muxy(&["mobile"])?)?.contains("Mobile access: off"));
    let port = TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port()
        .to_string();
    assert!(
        succeeded(muxy(&["mobile", "enable", "--port", &port])?)?
            .contains(&format!("listening on port {port}"))
    );

    assert_eq!(succeeded(muxy(&["server", "stop", "--force"])?)?, "ok\n");
    let deadline = Instant::now() + TIMEOUT;
    while remote.socket().exists() {
        assert!(Instant::now() < deadline, "the remote server kept running");
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(fs::read_dir(local.path())?.count(), 0);
    Ok(())
}

#[test]
fn host_problems_print_one_line_and_never_reach_the_bridge_through_ssh() -> Result {
    let local = tempfile::tempdir()?;
    let refused =
        FakeRemote::with_script("echo 'dev@box: Permission denied (publickey).' >&2; exit 255")?;
    let message = failed(host(&refused, local.path(), &["session", "list"])?)?;
    assert!(
        message.starts_with("muxy: SSH refused the login to box."),
        "{message}"
    );
    assert_eq!(message.lines().count(), 1, "{message}");
    let missing = FakeRemote::with_script("export PATH=/usr/bin:/bin")?;
    assert_eq!(
        failed(host(&missing, local.path(), &["project", "list"])?)?,
        "muxy: Muxy isn't installed on box (looked on PATH and in ~/.local/bin).\n"
    );

    let vanished = FakeRemote::with_script("printf 'MUXY-STDIO/1\\n'; exit 0")?;
    assert_eq!(
        failed(host(&vanished, local.path(), &["session", "list"])?)?,
        "muxy: box: disconnected from server\n"
    );

    let watched = FakeRemote::with_script(r#"touch "$HOME/reached""#)?;
    for args in [&["stdio"][..], &["--version"]] {
        let message = failed(host(&watched, local.path(), args)?)?;
        assert!(
            message.starts_with("muxy: --host can't be combined with"),
            "{message}"
        );
    }
    assert!(!watched.home().join("reached").exists());
    Ok(())
}

#[test]
fn a_pairing_code_from_another_computer_lists_the_given_address_first() -> Result {
    use std::io::{BufRead, BufReader, Read};

    let remote = FakeRemote::new()?;
    let local = tempfile::tempdir()?;
    let port = TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port()
        .to_string();
    succeeded(host(
        &remote,
        local.path(),
        &["mobile", "enable", "--port", &port],
    )?)?;
    let mut pairing = host_command(
        &remote,
        local.path(),
        &["mobile", "pair", "--address", "box.example.com"],
    )
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
    let mut output = BufReader::new(pairing.stdout.take().ok_or("no stdout")?);
    let (sender, lines) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        while output.read_line(&mut line).is_ok_and(|read| read > 0) {
            if sender.send(std::mem::take(&mut line)).is_err() {
                break;
            }
        }
    });
    let link = loop {
        let line = lines.recv_timeout(TIMEOUT)?;
        if line.starts_with("muxy://pair?") {
            break line.trim().to_owned();
        }
    };
    let mut invite = muxy_protocol::PairingInvite::parse_link(&link)
        .map_err(|code| format!("invalid link {link}: {code:?}"))?;
    assert_eq!(invite.hosts[0], "box.example.com", "{link}");

    invite.hosts = vec!["127.0.0.1".into()];
    let (_phone, _) = Client::pair(&invite, "Test phone")?;
    let status = pairing.wait()?;
    let mut rest = String::new();
    while let Ok(line) = lines.recv_timeout(Duration::from_secs(1)) {
        rest.push_str(&line);
    }
    let mut errors = String::new();
    pairing
        .stderr
        .take()
        .ok_or("no stderr")?
        .read_to_string(&mut errors)?;
    assert!(status.success(), "{rest}{errors}");
    assert!(rest.contains("Paired Test phone."), "{rest}");
    Ok(())
}
