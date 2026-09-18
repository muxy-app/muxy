use std::collections::HashMap;
use std::error::Error;
use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use muxy_protocol::{
    AttachSnapshot, ChannelId, ErrorCode, ExitReason, HistoryCursor, HistoryPage, InputModes,
    MetadataEvent, SavedScreen, ScreenFrame, SearchPage, SessionInfo, Size, TerminalColors,
};
use muxy_terminal::Terminal;
use muxy_terminal::pty::{ExitStatus, Pty, PtyEvent};

use crate::archive::{Archive, bound_history_page, history_range};
use crate::error::ServerError;
use crate::search::Search;
use crate::session::metadata::Metadata;
use crate::session::{AttachmentEvent, AttachmentId, SessionCommand, SessionHandle};

const TICK: Duration = Duration::from_millis(16);
const SYNC_TIMEOUT: Duration = Duration::from_secs(1);
const CHECKPOINT: Duration = Duration::from_secs(1);
const METADATA_POLL: Duration = Duration::from_millis(100);

type Fault = Box<dyn Error + Send + Sync>;

struct InputWrite {
    bytes: Vec<u8>,
    reply: Option<Sender<Result<(), ServerError>>>,
}

impl From<Vec<u8>> for InputWrite {
    fn from(bytes: Vec<u8>) -> Self {
        Self { bytes, reply: None }
    }
}

#[derive(Debug)]
pub(crate) enum OwnerEvent {
    Command(SessionCommand),
    Pty(PtyEvent),
    WriteFailed(io::Error),
}

enum Wake {
    Event(OwnerEvent),
    Tick,
    Checkpoint,
    Metadata,
    Orphaned,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum OutputState {
    Open,
    Closed,
}

struct Attachment {
    sink: Sender<AttachmentEvent>,
    seq: u64,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "Pending work flags and the application's cursor mode are independent states"
)]
struct Owner {
    info: SessionInfo,
    pty: Pty,
    terminal: Terminal,
    metadata: Metadata,
    progress: super::SharedProgress,
    shared_metadata: super::SharedMetadata,
    activity: std::sync::Arc<crate::activity::Activity>,
    detector: crate::detection::Detector,
    detection_dirty: bool,
    next_detection: Instant,
    detection_screen: String,
    detection_progress: String,
    size: Size,
    events: Receiver<OwnerEvent>,
    input: Sender<InputWrite>,
    attachments: HashMap<AttachmentId, Attachment>,
    output_pending: bool,
    resize_pending: bool,
    compress_pending: bool,
    history_total: u64,
    history_generation: u64,
    input_modes: InputModes,
    cursor_blinking: bool,
    links: Vec<muxy_protocol::LinkRow>,
    graphics: muxy_protocol::Graphics,
    prompts: Vec<u16>,
    frame_state: Option<(muxy_protocol::Cursor, muxy_protocol::Modes)>,
    next_tick: Option<Instant>,
    synchronized_since: Option<Instant>,
    output_state: OutputState,
    archive: Archive,
    next_checkpoint: Option<Instant>,
    next_metadata: Instant,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Session startup receives owned terminal resources and shared server services"
)]
pub(crate) fn start(
    info: SessionInfo,
    mut pty: Pty,
    size: Size,
    history_budget_bytes: usize,
    archive: Archive,
    colors: Option<TerminalColors>,
    activity: std::sync::Arc<crate::activity::Activity>,
    on_exit: impl FnOnce(ExitReason) + Send + 'static,
) -> Result<SessionHandle, ServerError> {
    crate::detection::prepare();
    let id = info.id.get();
    let (sender, receiver) = mpsc::channel();
    let (pty_sender, pty_receiver) = mpsc::channel();
    let (ready_sender, ready) = mpsc::channel();
    pty.start_reader(pty_sender)
        .map_err(ServerError::spawn_failed)?;

    let forward = sender.clone();
    thread::Builder::new()
        .name(format!("session-{id}-pty"))
        .spawn(move || {
            for event in pty_receiver {
                if forward.send(OwnerEvent::Pty(event)).is_err() {
                    return;
                }
            }
        })
        .map_err(ServerError::spawn_failed)?;

    let progress = super::SharedProgress::default();
    let session_progress = progress.clone();
    let shared_metadata =
        std::sync::Arc::new(std::sync::Mutex::new(muxy_protocol::SessionMetadata {
            title: String::new(),
            directory: info.directory.clone(),
            process: None,
        }));
    let owner_metadata = shared_metadata.clone();
    let session = info.clone();
    let failed = sender.clone();
    thread::Builder::new()
        .name(format!("session-{id}"))
        .spawn(move || {
            let terminal =
                match Terminal::new(size, history_budget_bytes).and_then(|mut terminal| {
                    if let Some(colors) = colors {
                        set_colors(&mut terminal, &colors)?;
                    }
                    Ok(terminal)
                }) {
                    Ok(terminal) => terminal,
                    Err(error) => {
                        let _ = pty.kill();
                        let _ = pty.wait();
                        let _ = ready_sender.send(Err(ServerError::spawn_failed(error)));
                        return;
                    }
                };
            let input = match start_input(&mut pty, failed) {
                Ok(input) => input,
                Err(error) => {
                    let _ = pty.kill();
                    let _ = pty.wait();
                    let _ = ready_sender.send(Err(error));
                    return;
                }
            };
            let _ = ready_sender.send(Ok(()));
            let owner = Owner {
                metadata: Metadata::new(session.directory.clone()),
                progress: session_progress,
                shared_metadata: owner_metadata,
                activity,
                detector: crate::detection::Detector::default(),
                detection_dirty: true,
                next_detection: Instant::now(),
                detection_screen: String::new(),
                detection_progress: String::new(),
                info: session,
                pty,
                terminal,
                size,
                events: receiver,
                input,
                attachments: HashMap::new(),
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
                synchronized_since: None,
                output_state: OutputState::Open,
                archive,
                next_checkpoint: Some(Instant::now() + CHECKPOINT),
                next_metadata: Instant::now(),
            };
            on_exit(owner.run());
        })
        .map_err(ServerError::spawn_failed)?;

    ready
        .recv()
        .map_err(|_| ServerError::spawn_failed("session thread stopped before it was ready"))??;
    Ok(SessionHandle::new(info, sender, progress, shared_metadata))
}

fn set_colors(
    terminal: &mut Terminal,
    colors: &TerminalColors,
) -> Result<(), muxy_terminal::TerminalError> {
    terminal.set_defaults(colors)
}

fn start_input(
    pty: &mut Pty,
    events: Sender<OwnerEvent>,
) -> Result<Sender<InputWrite>, ServerError> {
    let mut writer = pty.take_writer().map_err(ServerError::spawn_failed)?;
    let (input, pending) = mpsc::channel::<InputWrite>();
    thread::Builder::new()
        .name(format!("session-{}-input", pty.child_pid()))
        .spawn(move || {
            for input in pending {
                let result = writer.write_all(&input.bytes);
                if let Some(reply) = input.reply {
                    let acknowledgement = result.as_ref().copied().map_err(|error| {
                        ServerError::new(
                            ErrorCode::BadRequest,
                            format!("terminal input write failed: {error}"),
                        )
                    });
                    let _ = reply.send(acknowledgement);
                }
                if let Err(error) = result {
                    let _ = events.send(OwnerEvent::WriteFailed(error));
                    break;
                }
            }
        })
        .map_err(ServerError::spawn_failed)?;
    Ok(input)
}

impl Owner {
    fn run(mut self) -> ExitReason {
        let reason = self.serve().unwrap_or_else(|_| {
            let _ = self.pty.kill();
            self.pty.wait().map_or(ExitReason::Ended, exit_reason)
        });
        self.drain_output();
        self.update_metadata();
        self.activity.remove(self.info.id);
        self.checkpoint(Some(reason));
        for attachment in self.attachments.values() {
            let _ = attachment.sink.send(AttachmentEvent::Ended(reason));
        }
        reason
    }

    fn serve(&mut self) -> Result<ExitReason, Fault> {
        loop {
            match self.next_wake() {
                Wake::Event(OwnerEvent::Pty(PtyEvent::Output(bytes))) => self.feed(&bytes)?,
                Wake::Event(OwnerEvent::Pty(PtyEvent::Closed)) => {
                    self.output_state = OutputState::Closed;
                }
                Wake::Event(OwnerEvent::WriteFailed(error)) => return Err(error.into()),
                Wake::Event(OwnerEvent::Command(SessionCommand::Input(bytes))) => {
                    self.input.send(bytes.into())?;
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::WriteInput { bytes, reply })) => {
                    self.input.send(InputWrite {
                        bytes,
                        reply: Some(reply),
                    })?;
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::CellSize(cell))) => {
                    self.terminal.set_cell_size(cell)?;
                    self.output_pending = true;
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::Mouse(event))) => {
                    let bytes = self.terminal.encode_mouse(&event)?;
                    if !bytes.is_empty() {
                        self.input.send(bytes.into())?;
                    }
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::SetColors(colors))) => {
                    set_colors(&mut self.terminal, &colors)?;
                    self.output_pending = true;
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::Resize(size))) => {
                    self.resize(size)?;
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::ResizeAttachment { id, size })) => {
                    self.resize(size)?;
                    self.broadcast_frame(Some(id))?;
                    self.output_pending = false;
                    self.resize_pending = false;
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::Attach {
                    id,
                    channel,
                    size,
                    sink,
                })) => self.attach(id, channel, size, sink)?,
                Wake::Event(OwnerEvent::Command(SessionCommand::HistoryPage {
                    before,
                    max_rows,
                    reply,
                })) => {
                    let _ = reply.send(self.history_page(before, max_rows));
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::Detach(id))) => {
                    self.attachments.remove(&id);
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::Search {
                    query,
                    ignore_case,
                    before,
                    max_results,
                    reply,
                })) => {
                    let _ = reply.send(self.search(&query, ignore_case, before, max_results));
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::End)) => {
                    return self.terminate(ExitReason::Ended);
                }
                Wake::Event(OwnerEvent::Command(SessionCommand::Stop)) => {
                    return self.terminate(ExitReason::ServerStopped);
                }
                Wake::Tick => self.tick()?,
                Wake::Checkpoint => self.checkpoint(None),
                Wake::Metadata => self.update_metadata(),
                Wake::Orphaned => return self.terminate(ExitReason::ServerStopped),
            }
            if self.output_state == OutputState::Closed
                && let Some(status) = self.pty.try_wait()
            {
                return Ok(exit_reason(status));
            }
            if self.next_tick.is_none() && self.has_pending_work() {
                self.next_tick = Some(Instant::now() + TICK);
            }
        }
    }

    fn next_wake(&self) -> Wake {
        let now = Instant::now();
        if self.next_tick.is_some_and(|tick| tick <= now) {
            return Wake::Tick;
        }
        if self
            .next_checkpoint
            .is_some_and(|checkpoint| checkpoint <= now)
        {
            return Wake::Checkpoint;
        }
        if self.next_metadata <= now {
            return Wake::Metadata;
        }
        let deadline = self
            .next_tick
            .unwrap_or(self.next_metadata)
            .min(self.next_checkpoint.unwrap_or(self.next_metadata))
            .min(self.next_metadata);
        let received = self
            .events
            .recv_timeout(deadline.saturating_duration_since(now));
        match received {
            Ok(event) => Wake::Event(event),
            Err(RecvTimeoutError::Timeout) if Some(deadline) == self.next_checkpoint => {
                Wake::Checkpoint
            }
            Err(RecvTimeoutError::Timeout) if deadline == self.next_metadata => Wake::Metadata,
            Err(RecvTimeoutError::Timeout) => Wake::Tick,
            Err(RecvTimeoutError::Disconnected) => Wake::Orphaned,
        }
    }

    fn has_pending_work(&self) -> bool {
        self.output_pending
            || self.resize_pending
            || self.compress_pending
            || self.output_state == OutputState::Closed
    }

    fn feed(&mut self, bytes: &[u8]) -> Result<(), Fault> {
        self.detection_dirty = true;
        self.terminal.feed(bytes);
        if self.terminal.synchronized_output()? {
            self.synchronized_since.get_or_insert_with(Instant::now);
        } else {
            self.synchronized_since = None;
        }
        let answers = self.terminal.take_pty_output();
        if !answers.is_empty() {
            self.input.send(answers.into())?;
        }
        self.output_pending = true;
        self.compress_pending = true;
        self.next_checkpoint
            .get_or_insert_with(|| Instant::now() + CHECKPOINT);
        Ok(())
    }

    fn resize(&mut self, size: Size) -> Result<(), Fault> {
        self.pty.resize(size.into())?;
        self.terminal.resize(size)?;
        self.size = size;
        self.resize_pending = true;
        self.next_checkpoint
            .get_or_insert_with(|| Instant::now() + CHECKPOINT);
        Ok(())
    }

    fn attach(
        &mut self,
        id: AttachmentId,
        channel: ChannelId,
        size: Size,
        sink: Sender<AttachmentEvent>,
    ) -> Result<(), Fault> {
        if self.attachments.is_empty() && size != self.size {
            self.resize(size)?;
        }
        self.update_metadata();
        self.update_cursor_blinking()?;
        // Establish one common baseline before adding an attachment. Existing
        // clients receive pending changes before the new client's snapshot.
        if self.attachments.is_empty() {
            self.terminal.take_changed_rows()?;
            self.links = self.terminal.screen_links();
            self.prompts = self.terminal.screen_prompts()?;
            self.frame_state = Some((self.terminal.cursor()?, self.terminal.modes()?));
        } else {
            self.broadcast_frame(None)?;
        }
        self.output_pending = false;
        self.resize_pending = false;
        let history = self.history_page(HistoryCursor(0), 200)?;
        let screen = history
            .screen
            .ok_or_else(|| io::Error::other("fresh history has no screen"))?;
        let snapshot = AttachSnapshot {
            graphics: screen.graphics,
            prompts: history.prompts,
            channel,
            size: self.size,
            rows: screen.rows,
            cursor: screen.cursor,
            modes: self.terminal.modes()?,
            title: self.metadata.title.clone(),
            directory: self.metadata.directory.clone(),
            history: history.rows,
            history_cursor: history.next,
            history_total: history.total_rows,
        };
        if sink
            .send(AttachmentEvent::Snapshot {
                snapshot,
                process: self.metadata.process.clone(),
            })
            .is_ok()
        {
            let _ = sink.send(AttachmentEvent::Metadata(MetadataEvent::InputModes(
                self.terminal.input_modes()?,
            )));
            let _ = sink.send(AttachmentEvent::Metadata(MetadataEvent::CursorBlinking(
                self.cursor_blinking,
            )));
            let _ = sink.send(AttachmentEvent::Metadata(MetadataEvent::Links {
                seq: 0,
                rows: self.links.clone(),
            }));
            self.attachments.insert(id, Attachment { sink, seq: 1 });
        }
        Ok(())
    }

    fn history_page(
        &mut self,
        before: HistoryCursor,
        max_rows: u16,
    ) -> Result<HistoryPage, ServerError> {
        let terminal_error = |error| {
            ServerError::new(
                ErrorCode::HistoryUnavailable,
                format!("terminal history: {error}"),
            )
        };
        let generation = self.terminal.history_generation().map_err(terminal_error)?;
        let total = self.terminal.history_rows().map_err(terminal_error)?;
        let (range, next) = history_range(self.info.id, generation, before, max_rows, total)?;
        self.compress_pending = true;
        let (history, mut prompts) = self
            .terminal
            .history_with_prompts(range)
            .map_err(terminal_error)?;
        let screen = if before.0 == 0 {
            prompts.extend(
                self.terminal
                    .screen_prompts()
                    .map_err(terminal_error)?
                    .into_iter()
                    .filter_map(|row| u16::try_from(history.len() + usize::from(row)).ok()),
            );
            Some(SavedScreen {
                graphics: self.terminal.graphics().map_err(terminal_error)?,
                size: self.size,
                rows: self.terminal.screen().map_err(terminal_error)?,
                cursor: self.terminal.cursor().map_err(terminal_error)?,
                reason: None,
            })
        } else {
            None
        };
        bound_history_page(
            self.info.id,
            generation,
            before,
            HistoryPage {
                prompts,
                rows: history,
                next,
                total_rows: total as u64,
                screen,
            },
        )
    }

    fn update_cursor_blinking(&mut self) -> Result<(), Fault> {
        let blinking = self.terminal.cursor_blinking()?;
        if blinking != self.cursor_blinking {
            self.cursor_blinking = blinking;
            self.attachments.retain(|_, attachment| {
                attachment
                    .sink
                    .send(AttachmentEvent::Metadata(MetadataEvent::CursorBlinking(
                        blinking,
                    )))
                    .is_ok()
            });
        }
        Ok(())
    }

    fn tick(&mut self) -> Result<(), Fault> {
        self.next_tick = None;
        if self.terminal.synchronized_output()? {
            let since = self.synchronized_since.get_or_insert_with(Instant::now);
            if since.elapsed() < SYNC_TIMEOUT {
                return Ok(());
            }
            self.terminal.end_synchronized_output()?;
        }
        self.synchronized_since = None;
        self.update_metadata();
        let modes = self.terminal.input_modes()?;
        if modes != self.input_modes {
            self.input_modes = modes;
            self.attachments.retain(|_, attachment| {
                attachment
                    .sink
                    .send(AttachmentEvent::Metadata(MetadataEvent::InputModes(modes)))
                    .is_ok()
            });
        }
        self.update_cursor_blinking()?;
        if self.output_pending || self.resize_pending {
            self.broadcast_frame(None)?;
            self.output_pending = false;
            self.resize_pending = false;
        } else if self.compress_pending {
            self.terminal.compress_idle()?;
            self.compress_pending = false;
        }
        Ok(())
    }

    fn search(
        &mut self,
        query: &str,
        ignore_case: bool,
        before: HistoryCursor,
        max_results: u16,
    ) -> Result<SearchPage, ServerError> {
        let failed =
            |error| ServerError::new(ErrorCode::HistoryUnavailable, format!("search: {error}"));
        let history_rows = self.terminal.history_rows().map_err(failed)?;
        let search = Search {
            session: self.info.id,
            generation: self.terminal.history_generation().map_err(failed)?,
            query,
            ignore_case,
            before,
            max_results,
            history_rows,
            screen_rows: usize::from(self.size.rows),
        };
        let range = search.range()?;
        self.compress_pending = true;
        let history = self
            .terminal
            .history(range.start.min(history_rows)..range.end.min(history_rows))
            .map_err(failed)?;
        let screen = if range.end > history_rows {
            self.terminal.screen().map_err(failed)?
        } else {
            Vec::new()
        };
        search.scan(|index| {
            Ok(if index < history_rows {
                &history[index - range.start].runs
            } else {
                &screen[index - history_rows].runs
            })
        })
    }

    fn update_metadata(&mut self) {
        let terminal = self.terminal.take_events();
        for event in &terminal {
            if let muxy_terminal::TerminalEvent::Progress(progress) = event {
                use muxy_protocol::ProgressState;
                self.detection_progress = progress.progress.map_or_else(
                    || "4;0;0".into(),
                    |p| match p.state {
                        ProgressState::Running => {
                            format!("4;1;{}", p.percent.map_or(-1, i16::from))
                        }
                        ProgressState::Indeterminate => "4;1;-1".into(),
                        ProgressState::Error => "4;2".into(),
                        ProgressState::Paused => "4;4".into(),
                    },
                );
                *self
                    .progress
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = *progress;
            }
        }
        let events = self.metadata.update(&self.pty, terminal);
        if events.iter().any(|event| {
            matches!(
                event,
                MetadataEvent::Title(_)
                    | MetadataEvent::Directory(_)
                    | MetadataEvent::ForegroundProcess { .. }
            )
        }) {
            *self
                .shared_metadata
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                muxy_protocol::SessionMetadata {
                    title: self.metadata.title.clone(),
                    directory: self.metadata.directory.clone(),
                    process: self.metadata.process.clone(),
                };
        }
        for event in events {
            self.attachments.retain(|_, attachment| {
                attachment
                    .sink
                    .send(AttachmentEvent::Metadata(event.clone()))
                    .is_ok()
            });
        }
        if self.metadata.agent != self.detector.provider {
            self.detection_dirty = true;
            self.detection_progress.clear();
        }
        let now = Instant::now();
        if now >= self.next_detection {
            self.next_detection = now + METADATA_POLL;
            if self.metadata.agent.is_some() {
                if self.detection_dirty && !self.terminal.synchronized_output().unwrap_or(true) {
                    if let Ok(screen) = self.terminal.detection_text() {
                        self.detection_screen = screen;
                    }
                    self.detection_dirty = false;
                }
                let completed = self.detector.update(
                    self.metadata.agent,
                    self.detection_screen.clone(),
                    &self.metadata.title,
                    &self.detection_progress,
                    now,
                );
                if let Some(provider) = self.detector.provider {
                    self.activity.update(
                        muxy_protocol::AgentActivity {
                            session: self.info.id,
                            project: self.info.project,
                            provider,
                            state: self.detector.state,
                        },
                        completed,
                    );
                }
            } else {
                self.detector = crate::detection::Detector::default();
                self.detection_screen.clear();
                self.activity.remove(self.info.id);
            }
        }
        self.next_metadata = Instant::now()
            + if self.metadata.agent.is_some() {
                METADATA_POLL
            } else {
                Duration::from_secs(1)
            };
    }

    fn broadcast_frame(&mut self, resized: Option<AttachmentId>) -> Result<(), Fault> {
        if self.attachments.is_empty() {
            return Ok(());
        }
        let total_rows = self.terminal.history_rows()? as u64;
        let generation = self.terminal.history_generation()?;
        if total_rows != self.history_total || generation != self.history_generation {
            self.history_total = total_rows;
            self.history_generation = generation;
            self.attachments.retain(|_, attachment| {
                attachment
                    .sink
                    .send(AttachmentEvent::Metadata(MetadataEvent::History {
                        total_rows,
                    }))
                    .is_ok()
            });
        }
        let graphics = self.terminal.graphics()?;
        let graphics_changed = graphics != self.graphics || self.resize_pending;
        self.graphics = graphics;
        let frame = ScreenFrame {
            graphics: graphics_changed.then(|| self.graphics.clone()),
            seq: 0,
            reset: self.resize_pending,
            rows: self.terminal.take_changed_rows()?,
            cursor: self.terminal.cursor()?,
            modes: self.terminal.modes()?,
        };
        let links = self.terminal.screen_links();
        let prompts = self.terminal.screen_prompts()?;
        let prompts_changed = self.prompts != prompts;
        self.prompts = prompts;
        let links_changed = self.links != links;
        self.links = links;
        let state = (frame.cursor, frame.modes);
        if !graphics_changed
            && !links_changed
            && !prompts_changed
            && !frame.reset
            && frame.rows.is_empty()
            && self.frame_state == Some(state)
        {
            return Ok(());
        }
        self.frame_state = Some(state);
        self.attachments.retain(|id, attachment| {
            let frame = ScreenFrame {
                seq: attachment.seq,
                ..frame.clone()
            };
            if prompts_changed || frame.reset {
                let _ =
                    attachment
                        .sink
                        .send(AttachmentEvent::Metadata(MetadataEvent::ScreenPrompts {
                            seq: frame.seq,
                            rows: self.prompts.clone(),
                        }));
            }
            if links_changed || frame.reset {
                let _ = attachment
                    .sink
                    .send(AttachmentEvent::Metadata(MetadataEvent::Links {
                        seq: frame.seq,
                        rows: self.links.clone(),
                    }));
            }
            attachment.seq += 1;
            let event = if resized == Some(*id) {
                AttachmentEvent::Resized(frame)
            } else {
                AttachmentEvent::Frame(frame)
            };
            attachment.sink.send(event).is_ok()
        });
        Ok(())
    }

    fn terminate(&mut self, reason: ExitReason) -> Result<ExitReason, Fault> {
        self.pty.kill()?;
        self.pty.wait()?;
        Ok(reason)
    }

    fn drain_output(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while self.output_state == OutputState::Open {
            match self
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(OwnerEvent::Pty(PtyEvent::Output(bytes))) => {
                    self.terminal.feed(&bytes);
                    self.terminal.take_pty_output();
                }
                Ok(OwnerEvent::Pty(PtyEvent::Closed)) => self.output_state = OutputState::Closed,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }

    fn checkpoint(&mut self, reason: Option<ExitReason>) {
        self.next_checkpoint = None;
        self.compress_pending = true;
        let result = self
            .terminal
            .archive()
            .map_err(io::Error::other)
            .and_then(|terminal| self.archive.save(self.info.id, terminal, reason));
        if let Err(error) = result {
            log::error!(
                "session {} could not save terminal content: {error}",
                self.info.id.get()
            );
        }
    }
}

fn exit_reason(status: ExitStatus) -> ExitReason {
    match (status.code, status.signal) {
        (Some(code), _) => ExitReason::Exited(code),
        (None, Some(signal)) => ExitReason::Signaled(signal),
        (None, None) => ExitReason::Ended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::{ServerPath, SessionId};
    use muxy_terminal::pty::SpawnRequest;
    use std::num::NonZeroU64;
    use std::os::unix::ffi::OsStrExt;

    fn owner() -> Result<Owner, Fault> {
        let size = Size { cols: 20, rows: 3 };
        let cwd = std::env::temp_dir();
        let mut pty = Pty::spawn(SpawnRequest {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "exit 0".into()],
            cwd: cwd.clone(),
            env: vec![],
            size: size.into(),
        })?;
        pty.wait()?;
        let directory = ServerPath(cwd.as_os_str().as_bytes().to_vec());
        Ok(Owner {
            graphics: muxy_protocol::Graphics::default(),
            info: SessionInfo {
                project: muxy_protocol::ProjectId::from_u128(1),
                id: SessionId::from(NonZeroU64::MIN),
                directory: directory.clone(),
            },
            pty,
            terminal: Terminal::new(size, 1024)?,
            metadata: Metadata::new(directory.clone()),
            progress: crate::session::SharedProgress::default(),
            shared_metadata: std::sync::Arc::new(std::sync::Mutex::new(
                muxy_protocol::SessionMetadata {
                    title: String::new(),
                    directory,
                    process: None,
                },
            )),
            activity: std::sync::Arc::default(),
            detector: crate::detection::Detector::default(),
            detection_dirty: true,
            next_detection: Instant::now(),
            detection_screen: String::new(),
            detection_progress: String::new(),
            size,
            events: mpsc::channel().1,
            input: mpsc::channel().0,
            attachments: HashMap::new(),
            output_pending: false,
            resize_pending: false,
            compress_pending: false,
            history_total: 0,
            history_generation: 0,
            input_modes: InputModes::default(),
            cursor_blinking: true,
            links: Vec::new(),
            prompts: Vec::new(),
            frame_state: None,
            next_tick: None,
            synchronized_since: None,
            output_state: OutputState::Closed,
            archive: Archive::memory(1024),
            next_checkpoint: None,
            next_metadata: Instant::now(),
        })
    }

    fn blink_modes(events: &Receiver<AttachmentEvent>) -> Vec<bool> {
        events
            .try_iter()
            .filter_map(|event| match event {
                AttachmentEvent::Metadata(MetadataEvent::CursorBlinking(blinking)) => {
                    Some(blinking)
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn synchronized_frames_wait_for_completion_and_recover_after_timeout() -> Result<(), Fault> {
        let mut owner = owner()?;
        let (sink, events) = mpsc::channel();
        owner.attach(AttachmentId(1), ChannelId(1), owner.size, sink)?;
        events.try_iter().for_each(drop);
        owner.feed(b"\x1b[?2026hpartial")?;
        owner.tick()?;
        assert!(
            !events
                .try_iter()
                .any(|event| matches!(event, AttachmentEvent::Frame(_)))
        );
        assert!(owner.output_pending);
        owner.feed(b" complete\x1b[?2026l")?;
        owner.tick()?;
        assert!(
            events
                .try_iter()
                .any(|event| matches!(event, AttachmentEvent::Frame(_)))
        );
        owner.feed(b"\x1b[?2026hstalled")?;
        owner.synchronized_since = Some(Instant::now().checked_sub(SYNC_TIMEOUT).unwrap());
        owner.tick()?;
        assert!(!owner.terminal.synchronized_output()?);
        assert!(
            events
                .try_iter()
                .any(|event| matches!(event, AttachmentEvent::Frame(_)))
        );
        Ok(())
    }

    #[test]
    fn prompt_marks_are_atomic_on_attach_and_history_reads_schedule_recompression()
    -> Result<(), Fault> {
        let mut owner = owner()?;
        for _ in 0..10 {
            owner.terminal.feed(
                b"\x1b]133;A\x07$ \x1b]133;B\x07echo hi\r\n\x1b]133;C\x07hi\r\n\x1b]133;D;0\x07",
            );
        }
        owner.terminal.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07");
        let (sink, events) = mpsc::channel();
        owner.attach(AttachmentId(1), ChannelId(1), owner.size, sink)?;
        let AttachmentEvent::Snapshot { snapshot, .. } = events.recv()? else {
            return Err("expected snapshot".into());
        };
        assert_eq!(snapshot.prompts.len(), 11);
        assert_eq!(
            snapshot.prompts.last().copied().map(usize::from),
            Some(snapshot.history.len() + 2)
        );
        owner.compress_pending = false;
        let page = owner.history_page(HistoryCursor(0), 5)?;
        assert!(owner.compress_pending);
        assert_eq!(page.prompts, [1, 3, 5, 7]);
        owner.tick()?;
        assert!(!owner.compress_pending);
        owner.terminal.feed(b"\x1b[2J\x1b[H");
        owner.broadcast_frame(None)?;
        let events: Vec<_> = events.try_iter().collect();
        assert!(events.iter().any(|event| matches!(event, AttachmentEvent::Metadata(MetadataEvent::ScreenPrompts { seq: 1, rows }) if rows.is_empty())));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, AttachmentEvent::Frame(frame) if frame.seq == 1))
        );
        Ok(())
    }

    #[test]
    fn attaching_between_cursor_mode_changes_keeps_every_attachment_current() -> Result<(), Fault> {
        for initial in [true, false] {
            let mut owner = owner()?;
            let sequence = |blinking| {
                if blinking {
                    &b"\x1b[1 q"[..]
                } else {
                    &b"\x1b[2 q"[..]
                }
            };
            owner.feed(sequence(initial))?;
            owner.tick()?;
            let (first, first_events) = mpsc::channel();
            owner.attach(AttachmentId(1), ChannelId(1), owner.size, first)?;
            assert_eq!(blink_modes(&first_events), [initial]);

            owner.feed(sequence(!initial))?;
            let (second, second_events) = mpsc::channel();
            owner.attach(AttachmentId(2), ChannelId(2), owner.size, second)?;
            owner.feed(sequence(initial))?;
            owner.tick()?;

            assert_eq!(blink_modes(&second_events), [!initial, initial]);
            assert_eq!(blink_modes(&first_events), [!initial, initial]);
        }
        Ok(())
    }
    #[test]
    fn new_attachment_receives_its_own_baseline_correction() -> Result<(), Fault> {
        let mut owner = owner()?;
        owner.feed(b"\x1b[1;1HA")?;
        let (first, first_events) = mpsc::channel();
        owner.attach(AttachmentId(1), ChannelId(1), owner.size, first)?;
        owner.tick()?;
        first_events.try_iter().for_each(drop);
        owner.feed(b"\x1b[1;1HB")?;
        let (second, second_events) = mpsc::channel();
        owner.attach(AttachmentId(2), ChannelId(2), owner.size, second)?;
        let mut displayed = match second_events.recv()? {
            AttachmentEvent::Snapshot { snapshot, .. } => snapshot.rows,
            other => return Err(format!("expected snapshot, got {other:?}").into()),
        };
        owner.feed(b"\x1b[1;1HA")?;
        owner.tick()?;
        for event in second_events.try_iter() {
            if let AttachmentEvent::Frame(frame) = event {
                for row in frame.rows {
                    let index = usize::from(row.index);
                    displayed[index] = row;
                }
            }
        }
        let current = owner.terminal.screen()?;
        assert_eq!(
            displayed, current,
            "each attachment must converge to the server screen"
        );
        Ok(())
    }

    #[test]
    fn replaced_history_invalidates_attached_caches_without_a_screen_frame() -> Result<(), Fault> {
        let mut owner = owner()?;
        let burst = |label: &str| format!("{}\x1b[2J\x1b[H", format!("{label}\r\n").repeat(10));
        owner.feed(burst("old").as_bytes())?;
        let (first, _first_events) = mpsc::channel();
        owner.attach(AttachmentId(1), ChannelId(1), owner.size, first)?;
        let (second, second_events) = mpsc::channel();
        owner.attach(AttachmentId(2), ChannelId(2), owner.size, second)?;
        let previous = match second_events.recv()? {
            AttachmentEvent::Snapshot { snapshot, .. } => snapshot,
            other => return Err(format!("expected snapshot, got {other:?}").into()),
        };
        second_events.try_iter().for_each(drop);

        owner.feed(b"\x1b[3J")?;
        owner.feed(burst("new").as_bytes())?;
        owner.tick()?;
        let current = owner.history_page(HistoryCursor(0), 200)?;
        assert_eq!(previous.history_total, current.total_rows);
        assert_ne!(previous.history, current.rows);
        let events: Vec<_> = second_events.try_iter().collect();
        assert!(matches!(
            events.as_slice(),
            [AttachmentEvent::Metadata(MetadataEvent::History { total_rows })]
                if *total_rows == current.total_rows
        ));
        Ok(())
    }

    #[test]
    fn unchanged_output_emits_no_frame() -> Result<(), Fault> {
        let mut owner = owner()?;
        owner.feed(b"\x1b[1;1HA")?;
        let (sink, events) = mpsc::channel();
        owner.attach(AttachmentId(1), ChannelId(1), owner.size, sink)?;
        owner.tick()?;
        events.try_iter().for_each(drop);
        owner.feed(b"\x1b[1;1HA")?;
        owner.tick()?;
        let frames: Vec<_> = events
            .try_iter()
            .filter_map(|event| match event {
                AttachmentEvent::Frame(frame) => Some(frame),
                _ => None,
            })
            .collect();
        assert!(
            frames.is_empty(),
            "unchanged screen, cursor and modes should not emit a frame"
        );
        Ok(())
    }

    #[test]
    fn cursor_only_mode_only_and_reset_frames_are_not_suppressed() -> Result<(), Fault> {
        let mut owner = owner()?;
        let (sink, events) = mpsc::channel();
        owner.attach(AttachmentId(1), ChannelId(1), owner.size, sink)?;
        events.try_iter().for_each(drop);
        for sequence in [b"\x1b[1;2H".as_slice(), b"\x1b[?2004h", b"\x1b[?1h"] {
            owner.feed(sequence)?;
            owner.tick()?;
            let frames: Vec<_> = events
                .try_iter()
                .filter_map(|event| match event {
                    AttachmentEvent::Frame(frame) => Some(frame),
                    _ => None,
                })
                .collect();
            assert_eq!(frames.len(), 1);
            assert!(frames[0].rows.is_empty());
        }
        owner.resize(Size {
            cols: owner.size.cols + 1,
            ..owner.size
        })?;
        owner.tick()?;
        assert!(events.try_iter().any(|event| matches!(event, AttachmentEvent::Frame(frame) if frame.reset && !frame.rows.is_empty())));
        Ok(())
    }

    #[test]
    fn repeated_identical_output_has_no_frame_cost() -> Result<(), Fault> {
        let mut owner = owner()?;
        let (sink, events) = mpsc::channel();
        owner.feed(b"\x1b[1;1HA")?;
        owner.attach(AttachmentId(1), ChannelId(1), owner.size, sink)?;
        events.try_iter().for_each(drop);
        let mut frames = 0;
        for _ in 0..10_000 {
            owner.feed(b"\x1b[1;1HA")?;
            owner.tick()?;
            frames += events
                .try_iter()
                .filter(|event| matches!(event, AttachmentEvent::Frame(_)))
                .count();
        }
        assert_eq!(frames, 0);
        Ok(())
    }
}
