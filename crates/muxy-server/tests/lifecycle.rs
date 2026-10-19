use muxy_protocol::transport::LocalStream;
use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use muxy_protocol::transport::{ByteStream, StreamCancellation, connect};
use muxy_protocol::wire::{Decoder, Encoder, WireError};
use muxy_protocol::{
    CONTROL, ChannelId, ExitReason, Message, ReplyBody, RequestBody, RequestId, SUPPORTED,
    ServerPath, SessionId, Size,
};

mod support;

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
            .env("SHELL", support::shell())
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
            if !support::is_socket(socket) {
                return Ok(false);
            }
            match LocalStream::connect(socket) {
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

    /// Asks the server to stop as the system does. Windows has no signals,
    /// and its console events need a console the test shares with the
    /// server, so there the request arrives through the protocol.
    fn signal(&self, signal: &str) -> TestResult {
        let child = self.child.as_ref().ok_or("server is not running")?;
        if cfg!(windows) {
            let _ = (signal, child);
            let mut client = Client::new(&self.socket())?;
            assert_eq!(
                client.request(RequestBody::StopServer)?,
                ReplyBody::ServerStopping
            );
            return Ok(());
        }
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
    let command: &[u8] = if cfg!(windows) {
        b"echo phase-eight-^ready\r"
    } else {
        b"printf '\\nphase-eight-ready\\n'\n"
    };
    creator
        .encoder
        .send(channel, &Message::Input(command.to_vec()))?;
    creator.wait_for_output("phase-eight-ready")?;
    assert!(
        matches!(observer.request(RequestBody::ListSessions)?, ReplyBody::Sessions(sessions) if sessions.len() == 2)
    );
    fixture.signal("-TERM")?;
    creator.ended(&[first, second], ExitReason::ServerStopped)?;
    observer.ended(&[first, second], ExitReason::ServerStopped)?;
    creator.closed()?;
    observer.closed()?;
    assert!(fixture.finish()?.status.success());
    assert!(!fixture.socket().exists());
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
    let mut stalled = LocalStream::connect(fixture.socket())?;
    stalled.set_read_timeout(Some(TIMEOUT))?;
    let _ready = Client::new(&fixture.socket())?;
    fixture.stop("-TERM")?;
    assert_eq!(stalled.read(&mut [0])?, 0);
    assert!(!fixture.socket().exists());
    Ok(())
}

#[test]
fn shutdown_does_not_wait_for_a_descendant_holding_the_pty() -> TestResult {
    assert_shutdown_interrupts_input(if cfg!(windows) {
        b"start /b ping -n 10 127.0.0.1 >nul& echo nonreading-^ready& ping -n 10 127.0.0.1 >nul\r"
    } else {
        b"stty -icanon -echo; trap '' HUP; sleep 10 & printf '\\nnonreading-ready\\n'; wait\n"
    })
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
    assert!(started.elapsed() < Duration::from_secs(5));
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
fn default_directory_does_not_touch_other_channels() -> TestResult {
    let mut fixture = Fixture::new()?;
    let linux = cfg!(target_os = "linux");
    let support = if cfg!(windows) {
        fixture.directory.clone()
    } else {
        fixture.directory.join(if linux {
            ".local/state"
        } else {
            "Library/Application Support"
        })
    };
    let [development, beta, stable] = if linux {
        ["muxy-dev", "muxy-beta", "muxy"]
    } else {
        ["Muxy Dev", "Muxy Beta", "Muxy 2"]
    };
    let release = muxy_core::release::Version::current().filter(|_| !cfg!(debug_assertions));
    let current = match release.map(muxy_core::release::Version::channel) {
        None => development,
        Some(muxy_core::release::Channel::Beta) => beta,
        Some(muxy_core::release::Channel::Stable) => stable,
    };
    let others: Vec<&str> = ["Muxy", "Muxy Alpha", development, beta, stable]
        .into_iter()
        .filter(|name| *name != current)
        .collect();
    for name in &others {
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
        .env("LOCALAPPDATA", &fixture.directory)
        .arg("--socket")
        .arg(&socket);
    fixture.start_command(command, &socket)?;
    let mut client = Client::new(&socket)?;
    assert_eq!(client.request(RequestBody::Ping)?, ReplyBody::Pong);
    assert!(directory.join("server.toml").exists());
    assert!(directory.join("server.log").exists());
    for name in &others {
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

/// zsh's hooks; `powershell_marks_prompts_commands_failures_and_drive_folders`
/// covers `PowerShell`'s on Windows.
#[cfg(unix)]
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
    let shell = if cfg!(windows) {
        support::shell()
    } else {
        PathBuf::from("/bin/bash")
    };
    let settings = muxy_protocol::ServerSettingsDoc {
        default_shell: Some(support::server_path(&shell)?),
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
    assert!(saved.contains(if cfg!(windows) {
        "cmd.exe"
    } else {
        "/bin/bash"
    }));
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

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_muxy-server"))
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

#[test]
fn an_occupied_port_reports_failure_without_breaking_local_access() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.start()?;
    let occupied = std::net::TcpListener::bind("127.0.0.1:0")?;
    let mut local = Client::new(&fixture.socket())?;
    assert!(matches!(
        enable_mobile_access(&mut local, occupied.local_addr()?.port())?,
        muxy_protocol::ListenerStatus::Failed(_)
    ));
    assert_eq!(local.request(RequestBody::Ping)?, ReplyBody::Pong);
    fixture.stop("-TERM")
}
