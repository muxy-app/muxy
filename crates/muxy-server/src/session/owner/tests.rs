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
            program: if cfg!(windows) {
                crate::test_support::shell()
            } else {
                "/bin/cat".into()
            },
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
            shell: Shell {
                pty,
                #[cfg(windows)]
                job: crate::platform::windows::Job::killed_on_drop()?,
            },
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
                    shares: false,
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
            full_graphics: muxy_protocol::Graphics::default(),
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
        let _ = self.owner.shell.pty.kill();
        let _ = self.owner.shell.pty.wait();
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

#[test]
fn attachments_that_share_images_get_them_whole_and_others_get_them_fitted() -> Result<(), Fault> {
    let mut fixture = Fixture::new()?;
    let (sharing, shared_frames) = mpsc::channel();
    fixture.owner.attachments.insert(
        AttachmentId(2),
        Attachment {
            sink: sharing.into(),
            seq: 1,
            shares: true,
        },
    );
    fixture
        .owner
        .terminal
        .set_cell_size(muxy_protocol::CellSize {
            width: 16,
            height: 34,
        })?;
    let (width, height) = (2400_u32, 1600_u32);
    let pixels = vec![90_u8; (width * height * 4) as usize];
    let encoded = muxy_core::base64::encode(&pixels);
    let chunks: Vec<&[u8]> = encoded.as_bytes().chunks(4096).collect();
    for (index, chunk) in chunks.iter().enumerate() {
        let keys = if index == 0 {
            format!("a=T,f=32,s={width},v={height},C=1,")
        } else {
            String::new()
        };
        let more = u8::from(index + 1 < chunks.len());
        fixture
            .owner
            .terminal
            .feed(format!("\x1b_G{keys}q=2,m={more};").as_bytes());
        fixture.owner.terminal.feed(chunk);
        fixture.owner.terminal.feed(b"\x1b\\");
    }
    fixture.owner.broadcast_frame(None)?;
    let fitted = fixture.frames().pop().and_then(|frame| frame.graphics);
    let fitted = fitted.ok_or("no fitted images")?;
    assert!(fitted.images[0].width < width);
    assert!(fitted.images[0].rgba.len() <= muxy_protocol::MAX_GRAPHICS_BYTES);
    let whole = shared_frames.try_iter().find_map(|event| match event {
        AttachmentEvent::Frame(frame) => frame.graphics,
        _ => None,
    });
    let whole = whole.ok_or("no whole images")?;
    assert_eq!(
        (whole.images[0].width, whole.images[0].height),
        (width, height)
    );
    assert_eq!(whole.images[0].rgba.len(), pixels.len());
    assert_eq!(whole.images[0].generation, fitted.images[0].generation);

    fixture.owner.broadcast_frame(None)?;
    assert!(
        fixture
            .frames()
            .iter()
            .all(|frame| frame.graphics.is_none())
    );
    assert!(shared_frames.try_iter().all(|event| match event {
        AttachmentEvent::Frame(frame) => frame.graphics.is_none(),
        _ => true,
    }));

    fixture.owner.detach(AttachmentId(2));
    assert!(fixture.owner.full_graphics.images.is_empty());
    let relisted = fixture.owner.terminal.full_graphics()?;
    assert_ne!(
        relisted.images[0].rgba.as_ptr(),
        whole.images[0].rgba.as_ptr()
    );
    Ok(())
}

#[test]
fn an_app_that_attaches_is_told_when_images_go_back_to_what_was_last_sent() -> Result<(), Fault> {
    let mut fixture = Fixture::new()?;
    fixture.owner.attachments.clear();
    fixture
        .owner
        .terminal
        .feed(b"\x1b_Ga=T,i=4,f=32,s=1,v=1,c=2,r=1,q=2;/wAA/w==\x1b\\");
    let (sink, events) = mpsc::channel();
    fixture.owner.attach(
        AttachmentId(3),
        ChannelId(3),
        fixture.owner.size,
        sink.into(),
    )?;
    let attached = events.try_iter().find_map(|event| match event {
        AttachmentEvent::Snapshot { snapshot, .. } => Some(snapshot.graphics),
        _ => None,
    });
    assert_eq!(attached.ok_or("no snapshot")?.images.len(), 1);
    fixture.owner.terminal.feed(b"\x1b_Ga=d,q=2\x1b\\");
    fixture.owner.broadcast_frame(None)?;
    let cleared = events.try_iter().find_map(|event| match event {
        AttachmentEvent::Frame(frame) => frame.graphics,
        _ => None,
    });
    assert!(
        cleared
            .ok_or("the cleared images were not sent")?
            .images
            .is_empty()
    );
    Ok(())
}
