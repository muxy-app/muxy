use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use muxy_protocol::{
    AttachSnapshot, ChannelId, ForegroundProcess, MetadataEvent, ServerPath, Size,
};
use muxy_server::{
    AttachmentEvent, AttachmentId, Registry, ServerSettings, SessionCommand, SessionHandle,
};

mod support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const SIZE: Size = Size { cols: 80, rows: 24 };
const TIMEOUT: Duration = Duration::from_secs(5);

struct Fixture {
    registry: Registry,
    handle: SessionHandle,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        Self::with_shell(&support::shell())
    }

    fn with_shell(shell: &Path) -> TestResult<Self> {
        let (events, _) = mpsc::channel();
        let registry = Registry::new(
            ServerSettings {
                default_shell: Some(shell.to_owned()),
                shell_integration: false,
                ..ServerSettings::default()
            },
            events,
        );
        let start = if cfg!(windows) {
            support::folder()
        } else {
            PathBuf::from("/")
        };
        let session = registry.create(&start, SIZE)?;
        let handle = registry.handle(session.id).ok_or("missing session")?;
        Ok(Self { registry, handle })
    }

    fn input(&self, bytes: &[u8]) -> TestResult {
        self.handle.send(SessionCommand::Input(bytes.to_vec()))?;
        Ok(())
    }

    fn attach(
        &self,
        id: u32,
    ) -> TestResult<(
        Receiver<AttachmentEvent>,
        AttachSnapshot,
        Option<ForegroundProcess>,
    )> {
        let (sink, events) = mpsc::channel();
        self.handle.send(SessionCommand::Attach {
            id: AttachmentId(u64::from(id)),
            channel: ChannelId(id),
            size: SIZE,
            sink: sink.into(),
        })?;
        match events.recv_timeout(TIMEOUT)? {
            AttachmentEvent::Snapshot { snapshot, process } => Ok((events, snapshot, process)),
            other => Err(format!("expected snapshot, got {other:?}").into()),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.registry.shutdown();
        let deadline = Instant::now() + TIMEOUT;
        while !self.registry.is_stopped() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn metadata(
    events: &Receiver<AttachmentEvent>,
    accept: impl Fn(&MetadataEvent) -> bool,
) -> TestResult<MetadataEvent> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            AttachmentEvent::Metadata(event) if accept(&event) => return Ok(event),
            AttachmentEvent::Frame(_) | AttachmentEvent::Metadata(_) => {}
            other => return Err(format!("unexpected session event: {other:?}").into()),
        }
    }
}

#[cfg(unix)]
#[test]
fn foreground_process_changes_while_silent_and_returns_to_the_spawned_shell() -> TestResult {
    let fixture = Fixture::new()?;
    let (events, _, _) = fixture.attach(1)?;
    fixture.input(b"stty -echo; PS1=''; sleep 30\n")?;
    metadata(
        &events,
        |event| matches!(event, MetadataEvent::ForegroundProcess { name, is_shell: false } if name == "sleep"),
    )?;
    let (_, _, process) = fixture.attach(2)?;
    assert_eq!(
        process,
        Some(ForegroundProcess {
            name: "sleep".into(),
            is_shell: false
        })
    );
    fixture.input(b"\x03")?;
    metadata(&events, |event| {
        matches!(
            event,
            MetadataEvent::ForegroundProcess { is_shell: true, .. }
        )
    })?;
    let (_, _, process) = fixture.attach(3)?;
    assert!(process.is_some_and(|process| process.is_shell));
    Ok(())
}

#[cfg(unix)]
#[test]
fn terminal_directory_and_bell_events_are_delivered_without_replaying_bells_on_attach() -> TestResult
{
    let fixture = Fixture::new()?;
    let (events, _, _) = fixture.attach(1)?;
    fixture.input(b"printf '\\033]7;file:///tmp/a%%20b\\007'\n")?;
    assert_eq!(
        metadata(
            &events,
            |event| matches!(event, MetadataEvent::Directory(path) if path.0 == b"/tmp/a b")
        )?,
        MetadataEvent::Directory(ServerPath(b"/tmp/a b".to_vec()))
    );
    fixture.input(b"printf '\\007'\n")?;
    metadata(&events, |event| matches!(event, MetadataEvent::Bell))?;
    let (second, snapshot, _) = fixture.attach(2)?;
    assert_eq!(snapshot.directory.0, b"/tmp/a b");
    assert!(
        second
            .try_iter()
            .all(|event| !matches!(event, AttachmentEvent::Metadata(MetadataEvent::Bell)))
    );
    Ok(())
}

/// Windows has no foreground process group, so a program started in the
/// background counts as the foreground while it runs.
#[cfg(unix)]
#[test]
fn background_work_alone_keeps_shell_foreground_metadata() -> TestResult {
    for shell in ["/bin/sh", "/bin/zsh"] {
        let fixture = Fixture::with_shell(Path::new(shell))?;
        let (events, _, _) = fixture.attach(1)?;
        fixture.input(b"sleep 5 &\ncd /tmp\n")?;
        metadata(
            &events,
            |event| matches!(event, MetadataEvent::Directory(path) if path.0.ends_with(b"/tmp")),
        )?;
        // A shell hook, such as zoxide's on `cd`, can briefly run in the
        // foreground; the background job, which outlives this wait, never may.
        let deadline = Instant::now() + Duration::from_secs(3);
        for id in 2.. {
            let (_, _, process) = fixture.attach(id)?;
            if process.as_ref().is_some_and(|process| process.is_shell) {
                break;
            }
            assert!(Instant::now() < deadline, "{shell}: {process:?}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_foreground_program_is_the_newest_the_shell_started() -> TestResult {
    let fixture = Fixture::new()?;
    let (events, _, _) = fixture.attach(1)?;
    fixture.input(b"ping -n 30 127.0.0.1\r")?;
    let is_ping = |name: &str| name.eq_ignore_ascii_case("ping");
    metadata(
        &events,
        |event| matches!(event, MetadataEvent::ForegroundProcess { name, is_shell: false } if is_ping(name)),
    )?;
    let (_, _, process) = fixture.attach(2)?;
    assert!(process.is_some_and(|process| is_ping(&process.name) && !process.is_shell));
    fixture.input(b"\x03")?;
    metadata(&events, |event| {
        matches!(
            event,
            MetadataEvent::ForegroundProcess { is_shell: true, .. }
        )
    })?;
    let (_, _, process) = fixture.attach(3)?;
    assert!(
        process.is_some_and(|process| process.is_shell && process.name.eq_ignore_ascii_case("cmd"))
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_drive_folders_and_bells_are_delivered_without_replaying_bells_on_attach() -> TestResult {
    let system = std::env::var_os("SystemRoot").ok_or("SystemRoot is unset")?;
    let powershell = Path::new(&system).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    let fixture = Fixture::with_shell(&powershell)?;
    let (events, _, _) = fixture.attach(1)?;
    fixture.input(b"[Console]::Write(\"$([char]27)]7;file:///C:/a%20b$([char]7)\")\r")?;
    let expected = ServerPath(br"C:\a b".to_vec());
    assert_eq!(
        metadata(
            &events,
            |event| matches!(event, MetadataEvent::Directory(path) if *path == expected)
        )?,
        MetadataEvent::Directory(expected.clone())
    );
    fixture.input(b"[Console]::Write([char]7)\r")?;
    metadata(&events, |event| matches!(event, MetadataEvent::Bell))?;
    let (second, snapshot, _) = fixture.attach(2)?;
    assert_eq!(snapshot.directory, expected);
    assert!(
        second
            .try_iter()
            .all(|event| !matches!(event, AttachmentEvent::Metadata(MetadataEvent::Bell)))
    );
    Ok(())
}
