use std::sync::Arc;

use muxy_protocol::{ProjectId, SessionId};
use muxy_terminal::pty::SpawnRequest;

use super::*;

struct Fixture {
    owner: Owner,
    frames: Receiver<AttachmentEvent>,
    _events: Sender<OwnerEvent>,
    _input: Receiver<InputWrite>,
}

impl Fixture {
    fn new() -> Result<Self, Fault> {
        let size = Size { cols: 40, rows: 4 };
        let directory = std::env::temp_dir();
        let info = SessionInfo {
            project: ProjectId::new(),
            id: SessionId::new(1).ok_or("invalid session id")?,
            directory: (&directory).into(),
        };
        let notifier = Arc::new(crate::notifier::Notifier::default());
        let published = Arc::new(crate::session::Published::new(&info, notifier.clone()));
        let terminal = Terminal::new(size, 1024 * 1024)?;
        let pty = Pty::spawn(SpawnRequest {
            program: "/bin/cat".into(),
            args: Vec::new(),
            cwd: directory,
            env: Vec::new(),
            size: size.into(),
        })?;
        let (events, receiver) = mpsc::channel();
        let (input, pending) = mpsc::channel();
        let (sink, frames) = mpsc::channel();
        let owner = Owner {
            metadata: Metadata::new(info.directory.clone()),
            info,
            pty,
            terminal,
            published,
            activity: Arc::new(crate::activity::Activity::new(notifier)),
            detector: crate::detection::Detector::default(),
            detection_dirty: false,
            next_detection: Instant::now(),
            detection_screen: String::new(),
            detection_progress: String::new(),
            program_group: None,
            program_reports: 0,
            seen_reports: 0,
            size,
            events: receiver,
            input,
            attachments: HashMap::from([(
                AttachmentId(1),
                Attachment {
                    sink: sink.into(),
                    seq: 1,
                },
            )]),
            output_pending: false,
            resize_pending: false,
            compress_pending: false,
            history_total: 0,
            history_generation: 0,
            input_modes: InputModes::default(),
            cursor_blinking: true,
            links: Vec::new(),
            graphics: muxy_protocol::Graphics::default(),
            prompts: Vec::new(),
            frame_state: None,
            next_tick: None,
            tick_deferred: false,
            synchronized_since: None,
            output_state: OutputState::Open,
            archive: Archive::memory(1024 * 1024),
            next_checkpoint: None,
            next_metadata: Instant::now() + Duration::from_secs(10),
        };
        Ok(Self {
            owner,
            frames,
            _events: events,
            _input: pending,
        })
    }

    fn frames(&self) -> Vec<ScreenFrame> {
        self.frames
            .try_iter()
            .filter_map(|event| match event {
                AttachmentEvent::Frame(frame) => Some(frame),
                _ => None,
            })
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.owner.pty.kill();
        let _ = self.owner.pty.wait();
    }
}

#[test]
fn deferred_refresh_wakes_when_synchronized_output_finishes() -> Result<(), Fault> {
    let mut fixture = Fixture::new()?;
    fixture.owner.feed(b"\x1b[?2026hfirst")?;
    let since = fixture
        .owner
        .synchronized_since
        .ok_or("missing sync start")?;
    fixture.owner.tick()?;
    assert!(fixture.frames().is_empty());
    assert!(fixture.owner.tick_deferred);
    assert_eq!(fixture.owner.next_tick, Some(since + SYNC_TIMEOUT));

    fixture.owner.feed(b" second\x1b[?2026")?;
    assert_eq!(fixture.owner.next_tick, Some(since + SYNC_TIMEOUT));
    assert!(fixture.frames().is_empty());
    fixture.owner.feed(b"l")?;
    assert!(
        fixture
            .owner
            .next_tick
            .is_some_and(|tick| tick <= Instant::now())
    );
    assert!(matches!(fixture.owner.next_wake(), Wake::Tick));
    fixture.owner.tick()?;
    let frames = fixture.frames();
    assert_eq!(frames.len(), 1);
    let text: String = frames[0].rows[0]
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(text, "first second");
    assert!(!fixture.owner.tick_deferred);
    assert!(fixture.owner.synchronized_since.is_none());
    Ok(())
}

#[test]
fn synchronized_frames_do_not_bypass_the_refresh_interval() -> Result<(), Fault> {
    let mut fixture = Fixture::new()?;
    fixture.owner.feed(b"\x1b[?2026hfirst\x1b[?2026l")?;
    fixture.owner.tick()?;
    assert_eq!(fixture.frames().len(), 1);
    let deadline = fixture.owner.next_tick.ok_or("missing next tick")?;

    for _ in 0..10 {
        fixture.owner.feed(b"\x1b[?2026h\rnext")?;
        fixture.owner.feed(b"\x1b[?2026l")?;
        assert_eq!(fixture.owner.next_tick, Some(deadline));
        assert!(!fixture.owner.tick_deferred);
        assert!(fixture.frames().is_empty());
    }
    fixture.owner.tick()?;
    assert_eq!(fixture.frames().len(), 1);
    Ok(())
}

#[test]
fn partial_output_does_not_postpone_the_synchronization_timeout() -> Result<(), Fault> {
    let mut fixture = Fixture::new()?;
    fixture.owner.feed(b"\x1b[?2026hfirst")?;
    fixture.owner.tick()?;
    let since = fixture.owner.synchronized_since;
    let deadline = fixture.owner.next_tick;
    fixture.owner.feed(b" second")?;
    assert_eq!(fixture.owner.synchronized_since, since);
    assert_eq!(fixture.owner.next_tick, deadline);
    assert!(fixture.frames().is_empty());
    Ok(())
}

#[test]
fn unfinished_synchronized_output_flushes_at_the_timeout() -> Result<(), Fault> {
    let mut fixture = Fixture::new()?;
    fixture.owner.feed(b"\x1b[?2026hfirst")?;
    fixture.owner.tick()?;
    fixture.owner.synchronized_since = Some(
        Instant::now()
            .checked_sub(SYNC_TIMEOUT)
            .ok_or("clock is earlier than the synchronization timeout")?,
    );
    fixture.owner.tick()?;
    assert!(!fixture.owner.terminal.synchronized_output()?);
    assert!(!fixture.owner.tick_deferred);
    assert!(fixture.owner.synchronized_since.is_none());
    assert_eq!(fixture.frames().len(), 1);
    assert!(fixture.owner.next_tick.is_some());
    Ok(())
}

#[test]
fn ordinary_output_keeps_a_frame_budget_and_stops_ticking_when_idle() -> Result<(), Fault> {
    let mut fixture = Fixture::new()?;
    fixture.owner.feed(b"first")?;
    let before = Instant::now();
    fixture.owner.tick()?;
    let after = Instant::now();
    let deadline = fixture.owner.next_tick.ok_or("missing next tick")?;
    assert!((before + TICK..=after + TICK).contains(&deadline));
    assert_eq!(fixture.frames().len(), 1);
    assert!(fixture.owner.compress_pending);

    fixture.owner.feed(b" second")?;
    assert_eq!(fixture.owner.next_tick, Some(deadline));
    fixture.owner.tick()?;
    assert_eq!(fixture.frames().len(), 1);
    fixture.owner.tick()?;
    assert!(!fixture.owner.has_pending_work());
    assert!(fixture.owner.next_tick.is_none());
    assert!(fixture.frames().is_empty());
    Ok(())
}
