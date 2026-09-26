use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use muxy_protocol::transport::{ByteStream, StreamCancellation, connect, tls};
use muxy_protocol::wire::{Decoder, Encoder, WireError};
use muxy_protocol::{
    CONTROL, ChannelId, ExitReason, Message, ReplyBody, RequestBody, RequestId, SUPPORTED,
    ServerPath, SessionId, Size,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(10);
const SIZE: Size = Size { cols: 80, rows: 24 };
static NEXT: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    directory: PathBuf,
    child: Option<Child>,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let directory = std::env::temp_dir().join(format!(
            "mx8-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory)?;
        Ok(Self {
            directory,
            child: None,
        })
    }

    fn socket(&self) -> PathBuf {
        self.directory.join("server.sock")
    }

    fn command(&self) -> Command {
        let mut command = Command::new(binary());
        command
            .env("MUXY_DIR", &self.directory)
            .env("MUXY_REMOTE_BIND", "127.0.0.1")
            .env("SHELL", "/bin/sh")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn start(&mut self) -> TestResult {
        let command = self.command();
        self.start_command(command, &self.socket())
    }

    fn start_command(&mut self, mut command: Command, socket: &Path) -> TestResult {
        self.child = Some(command.spawn()?);
        wait_until(|| {
            if !fs::symlink_metadata(socket).is_ok_and(|metadata| metadata.file_type().is_socket())
            {
                return Ok(false);
            }
            match UnixStream::connect(socket) {
                Ok(_) => Ok(true),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    Ok(false)
                }
                Err(error) => Err(error.into()),
            }
        })
    }

    fn signal(&self, signal: &str) -> TestResult {
        let child = self.child.as_ref().ok_or("server is not running")?;
        let output = Command::new("kill")
            .args([signal, &child.id().to_string()])
            .output()?;
        assert!(output.status.success(), "{output:?}");
        Ok(())
    }

    fn finish(&mut self) -> TestResult<Output> {
        wait_until(|| {
            Ok(self
                .child
                .as_mut()
                .ok_or("server is not running")?
                .try_wait()?
                .is_some())
        })?;
        Ok(self
            .child
            .take()
            .ok_or("server is not running")?
            .wait_with_output()?)
    }

    fn stop(&mut self, signal: &str) -> TestResult {
        self.signal(signal)?;
        let output = self.finish()?;
        assert!(output.status.success(), "{output:?}");
        Ok(())
    }

    fn output(&mut self, mut command: Command) -> TestResult<Output> {
        self.child = Some(command.spawn()?);
        self.finish()
    }

    fn log(&self) -> TestResult<String> {
        Ok(fs::read_to_string(self.directory.join("server.log"))?)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct Client {
    encoder: Encoder<Box<dyn Write + Send>>,
    incoming: Receiver<Result<(ChannelId, Message), WireError>>,
    cancellation: Box<dyn StreamCancellation>,
    next_request: u32,
}

impl Client {
    fn new(path: &Path) -> TestResult<Self> {
        Self::start(connect(path)?)
    }

    fn start(stream: Box<dyn ByteStream>) -> TestResult<Self> {
        let cancellation = stream.cancellation()?;
        let (read, write) = stream.split()?;
        let mut decoder = Decoder::new(read);
        let (sender, incoming) = mpsc::channel();
        thread::spawn(move || {
            loop {
                let received = decoder.next();
                let closed = received.is_err();
                if sender.send(received).is_err() || closed {
                    break;
                }
            }
        });
        let mut client = Self {
            encoder: Encoder::new(write),
            incoming,
            cancellation,
            next_request: 1,
        };
        client.encoder.send(
            CONTROL,
            &Message::Hello {
                versions: SUPPORTED.to_vec(),
            },
        )?;
        assert!(matches!(
            client.receive()?,
            (CONTROL, Message::HelloReply { .. })
        ));
        Ok(client)
    }

    fn receive(&self) -> TestResult<(ChannelId, Message)> {
        loop {
            let event = self.incoming.recv_timeout(TIMEOUT)??;
            if !matches!(
                event,
                (
                    _,
                    Message::Changed {
                        topic: muxy_protocol::Topic::Catalog,
                        ..
                    } | Message::Changed {
                        topic: muxy_protocol::Topic::Sessions,
                        ..
                    } | Message::Changed {
                        topic: muxy_protocol::Topic::RemoteAccess,
                        ..
                    }
                )
            ) {
                return Ok(event);
            }
        }
    }

    fn request(&mut self, body: RequestBody) -> TestResult<ReplyBody> {
        let id = RequestId(self.next_request);
        self.next_request += 1;
        self.encoder.send(CONTROL, &Message::Request { id, body })?;
        loop {
            match self.receive()? {
                (CONTROL, Message::Reply { id: received, body }) if id == received => {
                    return Ok(body);
                }
                (channel, Message::Frame(frame)) => self.encoder.send(
                    CONTROL,
                    &Message::FrameAck {
                        channel,
                        seq: frame.seq,
                    },
                )?,
                (
                    _,
                    Message::Metadata(_)
                    | Message::Changed {
                        topic: muxy_protocol::Topic::Catalog,
                        ..
                    }
                    | Message::Changed {
                        topic: muxy_protocol::Topic::Sessions,
                        ..
                    },
                ) => {}
                other => return Err(format!("unexpected reply: {other:?}").into()),
            }
        }
    }

    fn home(&mut self) -> TestResult<muxy_protocol::ProjectId> {
        match self.request(RequestBody::ReadCatalog {
            after: None,
            revision: None,
        })? {
            ReplyBody::Catalog(page) => Ok(page.home),
            other => Err(format!("expected catalog, got {other:?}").into()),
        }
    }

    fn create(&mut self, directory: &Path) -> TestResult<SessionId> {
        let project = self.home()?;
        match self.request(RequestBody::CreateSession {
            project,
            operation: muxy_protocol::OperationId::new(),
            directory: ServerPath(directory.as_os_str().as_encoded_bytes().to_vec()),
            size: SIZE,
        })? {
            ReplyBody::SessionCreated(info) => Ok(info.id),
            other => Err(format!("expected session, got {other:?}").into()),
        }
    }

    fn attach(&mut self, session: SessionId) -> TestResult<ChannelId> {
        match self.request(RequestBody::Attach {
            session,
            size: SIZE,
        })? {
            ReplyBody::Attached { snapshot, .. } => Ok(snapshot.channel),
            other => Err(format!("expected snapshot, got {other:?}").into()),
        }
    }

    fn wait_for_output(&mut self, expected: &str) -> TestResult {
        let mut rows = std::collections::BTreeMap::new();
        loop {
            let (channel, message) = self
                .receive()
                .map_err(|error| format!("waiting for {expected}: {error}; rows={rows:?}"))?;
            if let Message::Frame(frame) = message {
                self.encoder.send(
                    CONTROL,
                    &Message::FrameAck {
                        channel,
                        seq: frame.seq,
                    },
                )?;
                if frame.reset {
                    rows.clear();
                }
                rows.extend(frame.rows.iter().map(|row| {
                    (
                        row.index,
                        row.runs
                            .iter()
                            .map(|run| run.text.as_str())
                            .collect::<String>(),
                    )
                }));
                if rows.values().any(|row| row.trim() == expected) {
                    return Ok(());
                }
            }
        }
    }

    fn ended(&self, expected: &[SessionId], reason: ExitReason) -> TestResult {
        let mut remaining = expected.to_vec();
        while !remaining.is_empty() {
            match self.receive()? {
                (
                    CONTROL,
                    Message::SessionEnded {
                        session,
                        reason: received,
                    },
                ) => {
                    assert_eq!(received, reason);
                    assert!(remaining.contains(&session));
                    remaining.retain(|id| *id != session);
                }
                (_, Message::Frame(_) | Message::Metadata(_)) => {}
                other => return Err(format!("expected session end, got {other:?}").into()),
            }
        }
        Ok(())
    }

    fn closed(&self) -> TestResult {
        loop {
            match self.incoming.recv_timeout(TIMEOUT)? {
                Ok((
                    _,
                    Message::Changed {
                        topic: muxy_protocol::Topic::Catalog,
                        ..
                    }
                    | Message::Changed {
                        topic: muxy_protocol::Topic::Sessions,
                        ..
                    },
                )) => {}
                Err(WireError::Closed) => return Ok(()),
                other => return Err(format!("expected closed, got {other:?}").into()),
            }
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

fn wait_until(mut condition: impl FnMut() -> TestResult<bool>) -> TestResult {
    let deadline = Instant::now() + TIMEOUT;
    while !condition()? {
        if Instant::now() >= deadline {
            return Err("timed out".into());
        }
        thread::sleep(POLL);
    }
    Ok(())
}

#[test]
fn lifecycle_serves_shell_logs_and_stops_every_session() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let mut creator = Client::new(&fixture.socket())?;
    assert_eq!(creator.request(RequestBody::Ping)?, ReplyBody::Pong);
    let mut observer = Client::new(&fixture.socket())?;
    let first = creator.create(&fixture.directory)?;
    let second = creator.create(&fixture.directory)?;
    let channel = creator.attach(first)?;
    creator.encoder.send(
        channel,
        &Message::Input(b"printf '\\nphase-eight-ready\\n'\n".to_vec()),
    )?;
    creator.wait_for_output("phase-eight-ready")?;
    assert!(
        matches!(observer.request(RequestBody::ListSessions)?, ReplyBody::Sessions(sessions) if sessions.len() == 2)
    );
    let defaults = fs::read_to_string(fixture.directory.join("server.toml"))?;
    assert_eq!(
        defaults.trim(),
        "history_budget_bytes = 16777216\nshell_integration = true"
    );
    fixture.signal("-TERM")?;
    creator.ended(&[first, second], ExitReason::ServerStopped)?;
    observer.ended(&[first, second], ExitReason::ServerStopped)?;
    creator.closed()?;
    observer.closed()?;
    assert!(fixture.finish()?.status.success());
    assert!(!fixture.socket().exists());
    let log = fixture.log()?;
    for event in [
        "server started: socket=",
        "client connected:",
        "client disconnected:",
        "session created:",
        "session ended:",
        "ServerStopped",
        "server stopped",
    ] {
        assert!(log.contains(event), "missing {event}: {log}");
    }
    Ok(())
}

#[test]
fn second_instance_exits_successfully_and_preserves_the_first() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let mut client = Client::new(&fixture.socket())?;
    let mut second = Fixture::new()?;
    let output = second.output(fixture.command())?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?.trim(),
        format!(
            "muxy-server already running at {}",
            fixture.socket().display()
        )
    );
    assert_eq!(client.request(RequestBody::Ping)?, ReplyBody::Pong);
    fixture.stop("-INT")?;
    client.closed()?;
    assert!(!fixture.socket().exists());
    Ok(())
}

#[test]
fn shutdown_cancels_clients_that_never_finish_hello() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let mut stalled = UnixStream::connect(fixture.socket())?;
    stalled.set_read_timeout(Some(TIMEOUT))?;
    let _ready = Client::new(&fixture.socket())?;
    fixture.stop("-TERM")?;
    assert_eq!(stalled.read(&mut [0])?, 0);
    assert!(!fixture.socket().exists());
    Ok(())
}

#[test]
fn shutdown_interrupts_a_blocked_pty_write() -> TestResult {
    assert_shutdown_interrupts_input(
        b"stty -icanon -echo; printf '\\nnonreading-ready\\n'; exec sleep 3\n",
    )
}

#[test]
fn shutdown_does_not_wait_for_a_descendant_holding_the_pty() -> TestResult {
    assert_shutdown_interrupts_input(
        b"stty -icanon -echo; trap '' HUP; sleep 3 & printf '\\nnonreading-ready\\n'; wait\n",
    )
}

fn assert_shutdown_interrupts_input(command: &[u8]) -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let mut client = Client::new(&fixture.socket())?;
    let session = client.create(&fixture.directory)?;
    let channel = client.attach(session)?;
    client
        .encoder
        .send(channel, &Message::Input(command.to_vec()))?;
    client.wait_for_output("nonreading-ready")?;
    client
        .encoder
        .send(channel, &Message::Input(vec![b'x'; 65_536]))?;
    assert_eq!(client.request(RequestBody::Ping)?, ReplyBody::Pong);
    thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    fixture.signal("-TERM")?;
    client.ended(&[session], ExitReason::ServerStopped)?;
    client.closed()?;
    assert!(fixture.finish()?.status.success());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(!fixture.socket().exists());
    Ok(())
}

#[test]
fn shutdown_stops_a_session_after_its_output_closes() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let mut client = Client::new(&fixture.socket())?;
    let session = client.create(&fixture.directory)?;
    let channel = client.attach(session)?;
    client.encoder.send(
        channel,
        &Message::Input(b"exec </dev/null >/dev/null 2>&1; exec sleep 3\n".to_vec()),
    )?;
    assert_eq!(client.request(RequestBody::Ping)?, ReplyBody::Pong);
    thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    fixture.signal("-TERM")?;
    client.ended(&[session], ExitReason::ServerStopped)?;
    client.closed()?;
    assert!(fixture.finish()?.status.success());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(!fixture.socket().exists());
    Ok(())
}

#[test]
fn malformed_settings_name_the_key_and_cleanup_the_socket() -> TestResult {
    let mut fixture = Fixture::new()?;
    let path = fixture.directory.join("server.toml");
    fs::write(&path, "unexpected_key = true\n")?;
    let output = fixture.output(fixture.command())?;
    assert!(!output.status.success());
    assert!(String::from_utf8(output.stderr)?.contains("unexpected_key"));
    assert_eq!(fs::read_to_string(path)?, "unexpected_key = true\n");
    assert!(!fixture.socket().exists());
    Ok(())
}

#[test]
fn custom_paths_and_settings_are_used_without_default_directory() -> TestResult {
    let mut fixture = Fixture::new()?;
    let socket = fixture.directory.join("custom.sock");
    let settings = fixture.directory.join("custom.toml");
    let log = fixture.directory.join("custom.log");
    fs::write(
        &settings,
        "default_shell = '/missing/muxy-shell'\nhistory_budget_bytes = 4096\n",
    )?;
    let mut command = fixture.command();
    command
        .env_remove("MUXY_DIR")
        .env_remove("HOME")
        .arg("--socket")
        .arg(&socket)
        .arg("--settings")
        .arg(&settings)
        .arg("--log")
        .arg(&log);
    fixture.start_command(command, &socket)?;
    let mut client = Client::new(&socket)?;
    let project = client.home()?;
    assert!(matches!(client.request(RequestBody::CreateSession {
        project, operation: muxy_protocol::OperationId::new(),
        directory: ServerPath(fixture.directory.as_os_str().as_encoded_bytes().to_vec()),
        size: SIZE,
    })?, ReplyBody::Error(error) if error.code == muxy_protocol::ErrorCode::SpawnFailed));
    fixture.stop("-INT")?;
    assert!(fs::read_to_string(log)?.contains("server started:"));
    assert!(!socket.exists());
    assert!(!fixture.directory.join("server.toml").exists());
    Ok(())
}

#[test]
fn default_directory_does_not_touch_other_channels() -> TestResult {
    let mut fixture = Fixture::new()?;
    let linux = cfg!(target_os = "linux");
    let support = fixture.directory.join(if linux {
        ".local/state"
    } else {
        "Library/Application Support"
    });
    let (development, beta) = if linux {
        ("muxy-dev", "muxy-beta")
    } else {
        ("Muxy Dev", "Muxy Beta")
    };
    // Packaged-runtime verification runs a release server from a debug test harness.
    let development_build = match std::env::var("MUXY_TEST_SERVER_PROFILE").as_deref() {
        Ok("beta") => false,
        Ok("dev") => true,
        Ok(_) => return Err("invalid MUXY_TEST_SERVER_PROFILE".into()),
        Err(_) => cfg!(debug_assertions) || env!("CARGO_PKG_VERSION") == "2.0.0-beta-0",
    };
    let (current, other) = if development_build {
        (development, beta)
    } else {
        (beta, development)
    };
    for name in ["Muxy", "Muxy Alpha", other] {
        let directory = support.join(name);
        fs::create_dir_all(&directory)?;
        fs::write(directory.join("server.toml"), "settings must not be read")?;
    }
    let directory = support.join(current);
    let socket = fixture.socket();
    let mut command = fixture.command();
    command
        .env_remove("MUXY_DIR")
        .env_remove("XDG_STATE_HOME")
        .env("HOME", &fixture.directory)
        .arg("--socket")
        .arg(&socket);
    fixture.start_command(command, &socket)?;
    let mut client = Client::new(&socket)?;
    assert_eq!(client.request(RequestBody::Ping)?, ReplyBody::Pong);
    assert!(directory.join("server.toml").exists());
    assert!(directory.join("server.log").exists());
    for name in ["Muxy", "Muxy Alpha", other] {
        let directory = support.join(name);
        assert_eq!(
            fs::read_to_string(directory.join("server.toml"))?,
            "settings must not be read"
        );
        assert!(!directory.join("server.log").exists());
        assert!(!directory.join("server.sock").exists());
    }
    fixture.stop("-TERM")?;
    Ok(())
}

#[test]
fn socket_length_and_invalid_arguments_fail_clearly() -> TestResult {
    let mut fixture = Fixture::new()?;
    for (arguments, expected) in [
        (
            vec![
                "--socket".to_owned(),
                "a".repeat(if cfg!(target_os = "linux") { 108 } else { 104 }),
            ],
            "platform limit",
        ),
        (vec!["--socket".to_owned()], "requires a path"),
        (vec!["--stdio".to_owned()], "unknown argument: --stdio"),
        (
            vec!["--log".to_owned(), "--settings".to_owned()],
            "requires a path",
        ),
        (
            vec![
                "--log".to_owned(),
                "a".to_owned(),
                "--log".to_owned(),
                "b".to_owned(),
            ],
            "duplicate argument",
        ),
    ] {
        let mut command = fixture.command();
        command.args(arguments);
        let output = fixture.output(command)?;
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr)?.contains(expected));
    }
    Ok(())
}

#[test]
fn fatal_protocol_errors_are_logged_before_and_after_hello() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let socket = UnixStream::connect(fixture.socket())?;
    socket.set_read_timeout(Some(TIMEOUT))?;
    Encoder::new(socket.try_clone()?).send(CONTROL, &Message::VersionUnsupported)?;
    assert!(matches!(
        Decoder::new(socket).next()?,
        (CONTROL, Message::Fatal(_))
    ));
    let mut client = Client::new(&fixture.socket())?;
    client.encoder.send(CONTROL, &Message::VersionUnsupported)?;
    assert!(matches!(client.receive()?, (CONTROL, Message::Fatal(_))));
    client.closed()?;
    fixture.stop("-TERM")?;
    let log = fixture.log()?;
    assert!(log.contains("fatal protocol error: expected Hello on control"));
    assert!(log.contains("fatal protocol error: misplaced client message"));
    Ok(())
}

#[test]
fn shell_hooks_are_socket_relative_and_the_setting_controls_new_shells() -> TestResult {
    let mut fixture = Fixture::new()?;
    let home = fixture.directory.join("home");
    fs::create_dir(&home)?;
    fs::write(home.join(".zshenv"), "skip_global_compinit=1\n")?;
    fs::write(home.join(".zshrc"), "PS1='muxy-lifecycle> '\n")?;
    for enabled in [true, false] {
        fs::write(
            fixture.directory.join("server.toml"),
            format!("default_shell = '/bin/zsh'\nshell_integration = {enabled}\n"),
        )?;
        let mut command = fixture.command();
        command.env("HOME", &home).env("ZDOTDIR", &home);
        fixture.start_command(command, &fixture.socket())?;
        let mut client = Client::new(&fixture.socket())?;
        let session = client.create(&home)?;
        let mut observed = false;
        let mut screen = String::new();
        wait_until(|| {
            let ReplyBody::Attached { snapshot, .. } = client.request(RequestBody::Attach {
                session,
                size: SIZE,
            })?
            else {
                return Err("missing snapshot".into());
            };
            screen = snapshot
                .rows
                .iter()
                .map(|row| {
                    row.runs
                        .iter()
                        .map(|run| run.text.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            let ready = screen.contains("muxy-lifecycle>");
            observed = !snapshot.prompts.is_empty();
            client.request(RequestBody::Detach(snapshot.channel))?;
            Ok(ready)
        })
        .map_err(|error| {
            format!("shell readiness (integration={enabled}): {error}; screen={screen:?}")
        })?;
        assert_eq!(observed, enabled);
        assert!(
            fixture
                .directory
                .join("shell-integration/zsh/.zshenv")
                .is_file()
        );
        assert!(
            fixture
                .directory
                .join("shell-integration/fish/vendor_conf.d/muxy.fish")
                .is_file()
        );
        fixture.stop("-TERM")?;
    }
    Ok(())
}

#[test]
fn settings_persist_and_protocol_stop_gracefully_ends_sessions_before_restart() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let mut client = Client::new(&fixture.socket())?;
    let session = client.create(&fixture.directory)?;
    let settings = muxy_protocol::ServerSettingsDoc {
        default_shell: Some(ServerPath(b"/bin/bash".to_vec())),
        history_budget_bytes: 8 * 1024 * 1024,
        shell_integration: false,
    };
    assert_eq!(
        client.request(RequestBody::WriteServerSettings(settings.clone()))?,
        ReplyBody::ServerSettingsWritten
    );
    assert_eq!(
        client.request(RequestBody::ReadServerSettings)?,
        ReplyBody::ServerSettings(settings.clone())
    );
    assert_eq!(
        client.request(RequestBody::StopServer)?,
        ReplyBody::ServerStopping
    );
    assert!(fixture.finish()?.status.success());
    assert!(!fixture.socket().exists());
    let saved = fs::read_to_string(fixture.directory.join("server.toml"))?;
    assert!(saved.contains("/bin/bash"));
    assert!(saved.contains("shell_integration = false"));
    drop(client);
    fixture.start()?;
    let mut client = Client::new(&fixture.socket())?;
    assert_eq!(
        client.request(RequestBody::ReadServerSettings)?,
        ReplyBody::ServerSettings(settings)
    );
    assert_eq!(
        client.request(RequestBody::ListSessions)?,
        ReplyBody::Sessions(vec![])
    );
    assert!(matches!(
        client.request(RequestBody::ReadSavedScreen(session))?,
        ReplyBody::SavedScreen(_)
    ));
    assert_eq!(
        client.request(RequestBody::StopServer)?,
        ReplyBody::ServerStopping
    );
    assert!(fixture.finish()?.status.success());
    Ok(())
}

#[test]
fn build_info_has_no_server_or_storage_side_effects() -> TestResult {
    let fixture = Fixture::new()?;
    let output = Command::new(binary())
        .env("MUXY_DIR", &fixture.directory)
        .arg("--build-info")
        .output()?;
    assert!(output.status.success());
    let build: muxy_protocol::BuildInfo = serde_json::from_slice(&output.stdout)?;
    assert_eq!(build, muxy_protocol::BuildInfo::current());
    assert_eq!(fs::read_dir(&fixture.directory)?.count(), 0);
    Ok(())
}

#[test]
fn replacing_the_binary_and_reconnecting_preserves_the_shell_process() -> TestResult {
    let mut fixture = Fixture::new()?;
    let executable = fixture.directory.join("muxy-server");
    copy_executable(&binary(), &executable)?;
    let mut command = Command::new(&executable);
    command
        .env("MUXY_DIR", &fixture.directory)
        .env("SHELL", "/bin/sh");
    fixture.start_command(command, &fixture.socket())?;
    let server_pid = fixture.child.as_ref().ok_or("server")?.id();
    let mut client = Client::new(&fixture.socket())?;
    let session = client.create(&fixture.directory)?;
    let channel = client.attach(session)?;
    client
        .encoder
        .send(channel, &Message::Input(b"echo $$ > before.pid\n".to_vec()))?;
    wait_until(|| Ok(fixture.directory.join("before.pid").exists()))?;
    let pid = fs::read(fixture.directory.join("before.pid"))?;
    drop(client);
    fs::rename(&executable, fixture.directory.join("previous-server"))?;
    copy_executable(&binary(), &executable)?;
    let mut client = Client::new(&fixture.socket())?;
    let channel = client.attach(session)?;
    client
        .encoder
        .send(channel, &Message::Input(b"echo $$ > after.pid\n".to_vec()))?;
    wait_until(|| Ok(fixture.directory.join("after.pid").exists()))?;
    assert_eq!(fs::read(fixture.directory.join("after.pid"))?, pid);
    assert_eq!(fixture.child.as_ref().ok_or("server")?.id(), server_pid);
    assert_eq!(
        client.request(RequestBody::StopServerIfIdle)?,
        ReplyBody::ServerBusy
    );
    drop(client);
    fixture.stop("-TERM")?;
    Ok(())
}

#[test]
fn idle_update_notifies_every_client_before_disconnecting() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let mut updater = Client::new(&fixture.socket())?;
    let observer = Client::new(&fixture.socket())?;
    assert_eq!(
        updater.request(RequestBody::StopServerIfIdle)?,
        ReplyBody::ServerStopping
    );
    assert_eq!(observer.receive()?, (CONTROL, Message::ServerRestarting));
    observer.closed()?;
    assert!(fixture.finish()?.status.success());
    Ok(())
}

fn copy_executable(source: &Path, destination: &Path) -> TestResult {
    // A separate process prevents concurrent test forks inheriting a writable
    // descriptor and temporarily blocking Linux exec with ETXTBSY.
    let output = Command::new("cp").args([source, destination]).output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(())
}

fn binary() -> PathBuf {
    std::env::var_os("MUXY_TEST_SERVER").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_muxy-server")),
        PathBuf::from,
    )
}

fn free_port() -> TestResult<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?
        .local_addr()?
        .port())
}

fn enable_mobile_access(
    client: &mut Client,
    port: u16,
) -> TestResult<muxy_protocol::ListenerStatus> {
    match client.request(RequestBody::WriteRemoteAccess(
        muxy_protocol::RemoteAccessSettings {
            enabled: true,
            port,
        },
    ))? {
        ReplyBody::RemoteAccess(state) => Ok(state.status),
        other => Err(format!("expected remote access, got {other:?}").into()),
    }
}

/// Retries while the listener comes up after a start or restart.
fn connect_phone(port: u16, fingerprint: [u8; 32]) -> TestResult<Client> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match tls::connect("127.0.0.1", port, fingerprint, TIMEOUT) {
            Ok(stream) => return Client::start(stream),
            Err(error) if Instant::now() < deadline => {
                if error.kind() != std::io::ErrorKind::ConnectionRefused {
                    return Err(error.into());
                }
                thread::sleep(POLL);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn pair_phone(
    local: &mut Client,
    port: u16,
) -> TestResult<(muxy_protocol::Paired, [u8; 32], Client)> {
    let offer = match local.request(RequestBody::StartPairing)? {
        ReplyBody::Pairing(offer) => offer,
        other => return Err(format!("expected pairing, got {other:?}").into()),
    };
    let mut phone = connect_phone(port, offer.invite.fingerprint)?;
    match phone.request(RequestBody::Pair(muxy_protocol::PairRequest {
        secret: offer.invite.secret,
        name: "Test phone".into(),
    }))? {
        ReplyBody::Paired(paired) => Ok((paired, offer.invite.fingerprint, phone)),
        other => Err(format!("expected paired, got {other:?}").into()),
    }
}

#[test]
fn a_paired_phone_reconnects_over_tls_after_a_restart() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let port = free_port()?;
    let mut local = Client::new(&fixture.socket())?;
    assert_eq!(
        enable_mobile_access(&mut local, port)?,
        muxy_protocol::ListenerStatus::Listening
    );
    let (paired, fingerprint, mut phone) = pair_phone(&mut local, port)?;
    assert!(matches!(
        phone.request(RequestBody::ReadCatalog {
            after: None,
            revision: None
        })?,
        ReplyBody::Catalog(_)
    ));
    drop(phone);
    assert_eq!(
        local.request(RequestBody::StopServer)?,
        ReplyBody::ServerStopping
    );
    assert!(fixture.finish()?.status.success());
    drop(local);
    let store = fixture.directory.join("remote.json");
    let token = paired
        .credential
        .token
        .iter()
        .fold(String::new(), |mut text, byte| {
            use std::fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        });
    let saved = fs::read_to_string(&store)?;
    assert!(saved.contains("Test phone"));
    assert!(!saved.contains(&token), "the raw device token was stored");
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&store)?.permissions().mode() & 0o777, 0o600);
    }
    fixture.start()?;
    let mut phone = connect_phone(port, fingerprint)?;
    assert_eq!(
        phone.request(RequestBody::Authenticate(paired.credential))?,
        ReplyBody::Authenticated
    );
    let mut local = Client::new(&fixture.socket())?;
    match local.request(RequestBody::ReadRemoteAccess)? {
        ReplyBody::RemoteAccess(state) => {
            assert_eq!(state.devices.len(), 1);
            assert!(state.devices[0].connected);
        }
        other => return Err(format!("expected remote access, got {other:?}").into()),
    }
    let stopping = Instant::now();
    fixture.stop("-TERM")?;
    assert!(stopping.elapsed() < TIMEOUT);
    Ok(())
}

#[test]
fn an_occupied_port_reports_failure_and_disabling_drops_phones() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let occupied = std::net::TcpListener::bind("127.0.0.1:0")?;
    let mut local = Client::new(&fixture.socket())?;
    assert!(matches!(
        enable_mobile_access(&mut local, occupied.local_addr()?.port())?,
        muxy_protocol::ListenerStatus::Failed(_)
    ));
    assert_eq!(local.request(RequestBody::Ping)?, ReplyBody::Pong);
    let port = free_port()?;
    assert_eq!(
        enable_mobile_access(&mut local, port)?,
        muxy_protocol::ListenerStatus::Listening
    );
    let (_, _, phone) = pair_phone(&mut local, port)?;
    local.request(RequestBody::WriteRemoteAccess(
        muxy_protocol::RemoteAccessSettings {
            enabled: false,
            port,
        },
    ))?;
    loop {
        match phone.incoming.recv_timeout(TIMEOUT)? {
            Err(WireError::Closed) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
    }
    assert!(tls::connect("127.0.0.1", port, [0; 32], TIMEOUT).is_err());
    fixture.stop("-TERM")
}
