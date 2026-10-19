use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};
use std::{env, fs, process, thread};

#[cfg(unix)]
use muxy_protocol::CellSize;
use muxy_protocol::{AttachSnapshot, ChannelId, ExitReason, Row, ScreenFrame, SessionInfo, Size};
use muxy_server::{
    AttachmentEvent, AttachmentId, Registry, ServerEvent, ServerSettings, SessionCommand,
    SessionHandle,
};

mod support;

type TestResult = Result<(), Box<dyn Error>>;

const TIMEOUT: Duration = Duration::from_secs(10);
const SIZE: Size = Size { cols: 80, rows: 24 };

struct Fixture {
    registry: Registry,
    events: Receiver<ServerEvent>,
    directory: PathBuf,
}

#[cfg(unix)]
#[test]
fn end_kills_a_running_program() -> TestResult {
    let fixture = fixture("end")?;
    let (info, handle) = session(&fixture)?;
    let (events, _) = attach(&handle, 1, SIZE)?;

    handle.send(SessionCommand::Input(b"cat\n".to_vec()))?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            AttachmentEvent::Metadata(muxy_protocol::MetadataEvent::ForegroundProcess {
                name,
                is_shell: false,
            }) if name == "cat" => break,
            AttachmentEvent::Frame(_) | AttachmentEvent::Metadata(_) => {}
            other => return Err(format!("waiting for cat: {other:?}").into()),
        }
    }
    handle.send(SessionCommand::Input(b"ping\n".to_vec()))?;
    wait_for_text(&events, "ping\nping")?;

    fixture.registry.end(info.id)?;

    assert_eq!(wait_for_ended(&events)?, ExitReason::Ended);
    assert_eq!(
        fixture.events.recv_timeout(TIMEOUT)?,
        ServerEvent::SessionEnded {
            id: info.id,
            reason: ExitReason::Ended
        }
    );
    assert!(fixture.registry.list().is_empty());
    fixture.finish()
}

/// Windows consoles report sizes in cells only.
#[cfg(unix)]
#[test]
fn programs_read_the_pane_size_in_pixels_from_the_pty() -> TestResult {
    const PROBE: &str = concat!(
        "python3 -c 'import fcntl,struct,termios;",
        "print(\"pixels %dx%d\"%struct.unpack(\"HHHH\",fcntl.ioctl(1,termios.TIOCGWINSZ,bytes(8)))[2:])'\n",
    );
    let fixture = fixture("pixels")?;
    let (_, handle) = session(&fixture)?;
    let (events, _) = attach(&handle, 1, SIZE)?;

    handle.send(SessionCommand::CellSize(CellSize {
        width: 9,
        height: 21,
    }))?;
    handle.send(SessionCommand::Input(PROBE.as_bytes().to_vec()))?;
    wait_for_text(&events, "pixels 720x504")?;

    handle.send(SessionCommand::Resize(Size {
        cols: 100,
        rows: 30,
    }))?;
    handle.send(SessionCommand::Input(PROBE.as_bytes().to_vec()))?;
    wait_for_text(&events, "pixels 900x630")?;

    handle.send(SessionCommand::CellSize(CellSize {
        width: 10,
        height: 20,
    }))?;
    handle.send(SessionCommand::Input(PROBE.as_bytes().to_vec()))?;
    wait_for_text(&events, "pixels 1000x600")?;
    fixture.finish()
}

fn fixture(name: &str) -> Result<Fixture, Box<dyn Error>> {
    let directory = env::temp_dir().join(format!("muxy-server-core-{}-{name}", process::id()));
    fs::create_dir_all(&directory)?;
    let (sender, events) = channel();
    let settings = ServerSettings {
        default_shell: Some(support::shell()),
        ..ServerSettings::default()
    };
    Ok(Fixture {
        registry: Registry::new(settings, sender),
        events,
        directory,
    })
}

impl Fixture {
    fn finish(self) -> TestResult {
        for session in self.registry.list() {
            self.registry.end(session.id)?;
        }
        let deadline = Instant::now() + TIMEOUT;
        while !self.registry.list().is_empty() {
            if Instant::now() > deadline {
                return Err("sessions did not end".into());
            }
            thread::sleep(Duration::from_millis(20));
        }
        fs::remove_dir(&self.directory)?;
        Ok(())
    }
}

fn session(fixture: &Fixture) -> Result<(SessionInfo, SessionHandle), Box<dyn Error>> {
    let info = fixture.registry.create(&fixture.directory, SIZE)?;
    let handle = fixture
        .registry
        .handle(info.id)
        .ok_or("created session has no handle")?;
    Ok((info, handle))
}

fn attach(
    handle: &SessionHandle,
    id: u64,
    size: Size,
) -> Result<(Receiver<AttachmentEvent>, AttachSnapshot), Box<dyn Error>> {
    let (sink, events) = channel();
    handle.send(SessionCommand::Attach {
        id: AttachmentId(id),
        channel: ChannelId(u32::try_from(id)?),
        size,
        sink: sink.into(),
    })?;
    match events.recv_timeout(TIMEOUT)? {
        AttachmentEvent::Snapshot { snapshot, .. } => Ok((events, snapshot)),
        other => Err(format!("expected a snapshot, got {other:?}").into()),
    }
}

fn wait_for_text(
    events: &Receiver<AttachmentEvent>,
    needle: &str,
) -> Result<ScreenFrame, Box<dyn Error>> {
    let mut rows = std::collections::BTreeMap::new();
    wait_for_frame(events, |frame| {
        if frame.reset {
            rows.clear();
        }
        rows.extend(frame.rows.iter().map(|row| (row.index, row.clone())));
        text(&rows.values().cloned().collect::<Vec<_>>()).contains(needle)
    })
}

fn wait_for_frame(
    events: &Receiver<AttachmentEvent>,
    mut accept: impl FnMut(&ScreenFrame) -> bool,
) -> Result<ScreenFrame, Box<dyn Error>> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(remaining)? {
            AttachmentEvent::Frame(frame) if accept(&frame) => return Ok(frame),
            AttachmentEvent::Frame(_) | AttachmentEvent::Metadata(_) => {}
            other => return Err(format!("expected a frame, got {other:?}").into()),
        }
    }
}

fn wait_for_ended(events: &Receiver<AttachmentEvent>) -> Result<ExitReason, Box<dyn Error>> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(remaining)? {
            AttachmentEvent::Ended(reason) => return Ok(reason),
            AttachmentEvent::Frame(_)
            | AttachmentEvent::Resized(_)
            | AttachmentEvent::Metadata(_) => {}
            AttachmentEvent::Snapshot { .. } => {
                return Err("expected a frame or ended, got a snapshot".into());
            }
        }
    }
}

fn text(rows: &[Row]) -> String {
    let lines: Vec<String> = rows
        .iter()
        .map(|row| row.runs.iter().map(|run| run.text.as_str()).collect())
        .collect();
    lines.join("\n")
}

#[cfg(windows)]
#[test]
fn windows_end_kills_a_running_program_and_everything_the_session_started() -> TestResult {
    let fixture = fixture("end")?;
    let (info, handle) = session(&fixture)?;
    let (events, _) = attach(&handle, 1, SIZE)?;
    // The background program writes a line a second while it runs.
    let output = fixture.directory.join("background.txt");
    // Process IDs repeat, so an earlier run's fixture folder may remain.
    let _ = fs::remove_file(&output);
    handle.send(SessionCommand::Input(
        format!("start /b ping -n 60 127.0.0.1 > {}\r", output.display()).into_bytes(),
    ))?;
    handle.send(SessionCommand::Input(b"ping -n 60 127.0.0.1\r".to_vec()))?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            AttachmentEvent::Metadata(muxy_protocol::MetadataEvent::ForegroundProcess {
                name,
                is_shell: false,
            }) if name.eq_ignore_ascii_case("ping") => break,
            AttachmentEvent::Frame(_) | AttachmentEvent::Metadata(_) => {}
            other => return Err(format!("waiting for ping: {other:?}").into()),
        }
    }
    wait_for_text(&events, "127.0.0.1")?;
    while fs::metadata(&output).map_or(0, |metadata| metadata.len()) < 100 {
        if Instant::now() > deadline {
            return Err("the background program wrote nothing".into());
        }
        thread::sleep(Duration::from_millis(50));
    }

    fixture.registry.end(info.id)?;

    assert_eq!(wait_for_ended(&events)?, ExitReason::Ended);
    assert_eq!(
        fixture.events.recv_timeout(TIMEOUT)?,
        ServerEvent::SessionEnded {
            id: info.id,
            reason: ExitReason::Ended
        }
    );
    thread::sleep(Duration::from_secs(1));
    let ended = fs::metadata(&output)?.len();
    thread::sleep(Duration::from_secs(3));
    assert_eq!(
        fs::metadata(&output)?.len(),
        ended,
        "the session's background program outlived it"
    );
    fs::remove_file(&output)?;
    assert!(fixture.registry.list().is_empty());
    fixture.finish()
}

/// Windows reports no program's working directory, so the server resolves a
/// session's folder before starting it, as Unix reports the shell's.
#[cfg(windows)]
#[test]
fn windows_sessions_start_in_and_report_their_resolved_folder() -> TestResult {
    const WIDE: Size = Size {
        cols: 400,
        rows: 24,
    };
    let fixture = fixture("resolved")?;
    let real = fixture.directory.join("real");
    let junction = fixture.directory.join("junction");
    // Process IDs repeat, so an earlier run's fixture folder may remain.
    let _ = fs::remove_dir(&junction);
    fs::create_dir_all(&real)?;
    let linked = process::Command::new("cmd")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(&real)
        .stdout(process::Stdio::null())
        .status()?;
    assert!(linked.success());
    let requested = PathBuf::from(junction.to_string_lossy().replace('\\', "/"));
    let resolved = muxy_core::files::canonicalize(&real)?;

    let info = fixture.registry.create(&requested, WIDE)?;
    assert_eq!(info.directory, support::server_path(&resolved)?);
    let handle = fixture
        .registry
        .handle(info.id)
        .ok_or("created session has no handle")?;
    let (events, snapshot) = attach(&handle, 1, WIDE)?;
    assert_eq!(snapshot.directory, info.directory);
    handle.send(SessionCommand::Input(b"cd\r".to_vec()))?;
    wait_for_text(&events, &format!("{}>", resolved.display()))?;

    fixture.registry.end(info.id)?;
    assert_eq!(wait_for_ended(&events)?, ExitReason::Ended);
    let deadline = Instant::now() + TIMEOUT;
    while !fixture.registry.list().is_empty() {
        if Instant::now() > deadline {
            return Err("the session did not end".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
    fs::remove_dir(&junction)?;
    fs::remove_dir(&real)?;
    fixture.finish()
}

/// Changes the user's `PATH` in the registry for a moment, so it runs only
/// in the test VM: `cargo test -- --ignored`.
#[cfg(windows)]
#[test]
#[ignore = "changes the user's PATH in the registry"]
fn windows_a_path_change_in_the_registry_reaches_new_sessions() -> TestResult {
    let query = process::Command::new("reg")
        .args(["query", r"HKCU\Environment", "/v", "Path"])
        .output()?;
    let original = String::from_utf8_lossy(&query.stdout)
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("Path").map(|rest| {
                rest.trim_start()
                    .split_once(char::is_whitespace)
                    .map_or("", |(_, value)| value)
                    .trim()
                    .to_owned()
            })
        });
    let marker = fixture_marker();
    let value = match &original {
        Some(path) if !path.is_empty() => format!("{path};{marker}"),
        _ => marker.clone(),
    };
    let set = |value: &str| {
        process::Command::new("reg")
            .args([
                "add",
                r"HKCU\Environment",
                "/v",
                "Path",
                "/t",
                "REG_EXPAND_SZ",
                "/d",
                value,
                "/f",
            ])
            .output()
    };
    assert!(set(&value)?.status.success());
    let result = (|| -> TestResult {
        let fixture = fixture("registry-path")?;
        let (_, handle) = session(&fixture)?;
        let (events, _) = attach(&handle, 1, SIZE)?;
        handle.send(SessionCommand::Input(b"echo [%PATH%]\r".to_vec()))?;
        wait_for_text(&events, &format!("{marker}]"))?;
        fixture.finish()
    })();
    match original {
        Some(path) => assert!(set(&path)?.status.success()),
        None => {
            process::Command::new("reg")
                .args(["delete", r"HKCU\Environment", "/v", "Path", "/f"])
                .output()?;
        }
    }
    result
}

#[cfg(windows)]
fn fixture_marker() -> String {
    format!(r"C:\muxy-path-{}", process::id())
}
