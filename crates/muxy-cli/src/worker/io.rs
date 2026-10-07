use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use muxy_app_core::PaneId;
use muxy_client::{Attachment, Client, ClientError, ClientEvent, RunGrid};
use muxy_protocol::{
    ActivitySnapshot, CatalogPage, ChannelId, ForegroundProcess, InputModes, MetadataEvent,
    MouseEvent, ProjectSession, ScreenFrame, SessionId, SessionMetadata, Size,
};

use crate::scroll::{HistoryRequest, Scroll};
use crate::state::{Result, State};

#[derive(Clone, Debug)]
pub(crate) struct View {
    pub session: SessionId,
    pub channel: Option<ChannelId>,
    pub grid: RunGrid,
    pub process: Option<ForegroundProcess>,
    pub input: InputModes,
    pub title: String,
    pub viewport: Size,
    pub ended: bool,
    pub scroll: Scroll,
}

impl View {
    pub(crate) fn attached(session: SessionId, viewport: Size, attachment: Attachment) -> Self {
        let mut grid = attachment.grid;
        grid.history.clear();
        grid.graphics = muxy_protocol::Graphics::default();
        Self {
            session,
            channel: Some(attachment.channel),
            grid,
            process: attachment.process,
            input: InputModes::default(),
            title: attachment.title,
            viewport,
            ended: false,
            scroll: Scroll::default(),
        }
    }

    fn metadata(&mut self, event: MetadataEvent) {
        match event {
            MetadataEvent::Title(title) => self.title = title,
            MetadataEvent::InputModes(input) => self.input = input,
            MetadataEvent::ForegroundProcess { name, is_shell } => {
                self.process = Some(ForegroundProcess { name, is_shell });
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct Pending {
    frame: Option<ScreenFrame>,
    metadata: Vec<MetadataEvent>,
}

/// What a close that needs confirming would end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Closing {
    Pane(PaneId),
    /// The tab holding this pane.
    Tab(PaneId),
}

impl Closing {
    pub(crate) fn pane(self) -> PaneId {
        match self {
            Self::Pane(pane) | Self::Tab(pane) => pane,
        }
    }
}

#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Shared {
    /// The server, as named to the user.
    pub server: String,
    pub client: Option<Client>,
    pub state: Option<State>,
    pub catalog: Option<CatalogPage>,
    pub sessions: Vec<ProjectSession>,
    pub sessions_dirty: bool,
    pub sessions_project: Option<muxy_protocol::ProjectId>,
    pub views: BTreeMap<PaneId, View>,
    pub message: String,
    /// Counts messages, so the same text said again is shown again.
    pub message_id: u64,
    pub refresh: bool,
    pub exit: Option<Result>,
    pub confirm: Option<Closing>,
    pub focus: Option<ChannelId>,
    pub host_unfocused: bool,
    pub ended: BTreeSet<SessionId>,
    /// Titles and programs of the terminals in this layout, shown or not.
    pub metadata: BTreeMap<SessionId, SessionMetadata>,
    pub activity: ActivitySnapshot,
    pub activity_dirty: bool,
    pending: BTreeMap<ChannelId, Pending>,
    last_channel: u32,
}

impl Shared {
    /// Shows `text` to the user, replacing the last message.
    pub(crate) fn say(&mut self, text: impl Into<String>) {
        self.message = text.into();
        self.message_id = self.message_id.wrapping_add(1);
    }

    pub(crate) fn session_ended(&mut self, session: SessionId) {
        self.ended.insert(session);
        if let Some(state) = &mut self.state {
            state.close_sessions(&self.ended);
        }
        self.sessions.retain(|entry| entry.info.id != session);
        self.metadata.remove(&session);
        let removed: Vec<_> = self
            .views
            .iter()
            .filter(|(_, view)| view.session == session)
            .map(|(pane, view)| (*pane, view.channel))
            .collect();
        for (pane, channel) in removed {
            self.views.remove(&pane);
            if let Some(channel) = channel {
                self.retire(channel);
                if self.focus == Some(channel) {
                    self.focus = None;
                }
            }
            if self.confirm.is_some_and(|closing| closing.pane() == pane) {
                self.confirm = None;
            }
        }
    }

    pub(crate) fn retire(&mut self, channel: ChannelId) {
        self.last_channel = self.last_channel.max(channel.0);
        self.pending.remove(&channel);
    }

    pub(crate) fn insert(&mut self, id: PaneId, mut view: View) -> Option<(ChannelId, u64)> {
        if self.ended.contains(&view.session) {
            if let Some(channel) = view.channel {
                self.retire(channel);
            }
            return None;
        }
        let mut ack = None;
        if let Some(channel) = view.channel {
            self.last_channel = self.last_channel.max(channel.0);
            if let Some(pending) = self.pending.remove(&channel) {
                for event in pending.metadata {
                    view.metadata(event);
                }
                if let Some(frame) = pending.frame {
                    view.grid.apply(&frame);
                    ack = Some((channel, frame.seq));
                }
            }
        }
        self.views.insert(id, view);
        ack
    }

    pub(crate) fn disconnected(&mut self) {
        self.client = None;
        self.sessions.clear();
        self.sessions_project = None;
        self.sessions_dirty = true;
        self.focus = None;
        self.pending.clear();
        self.last_channel = 0;
        self.activity_dirty = true;
        for view in self.views.values_mut() {
            view.channel = None;
            view.scroll.bottom();
        }
    }
}

pub(crate) fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn reader(client: Client, shared: Arc<Mutex<Shared>>) -> Result<JoinHandle<()>> {
    let events = client.events().ok_or("Connection events already taken")?;
    thread::Builder::new()
        .name("muxy-tui-events".into())
        .spawn(move || {
            while let Ok(event) = events.recv() {
                let mut state = lock(&shared);
                let mut ack = None;
                match event {
                    ClientEvent::SessionMetadata { session, metadata } => {
                        state.metadata.insert(session, metadata);
                    }
                    ClientEvent::ActivityChanged { .. } => state.activity_dirty = true,
                    ClientEvent::Frame { channel, mut frame } => {
                        frame.graphics = None;
                        if let Some(view) = state
                            .views
                            .values_mut()
                            .find(|view| view.channel == Some(channel))
                        {
                            if frame.reset {
                                view.scroll.bottom();
                            }
                            view.grid.apply(&frame);
                            ack = Some((channel, frame.seq));
                        } else if channel.0 > state.last_channel {
                            state.pending.entry(channel).or_default().frame = Some(frame);
                        } else {
                            ack = Some((channel, frame.seq));
                        }
                    }
                    ClientEvent::Metadata { channel, event } => {
                        if let Some(view) = state
                            .views
                            .values_mut()
                            .find(|view| view.channel == Some(channel))
                        {
                            view.metadata(event);
                        } else if channel.0 > state.last_channel
                            && matches!(
                                event,
                                MetadataEvent::Title(_)
                                    | MetadataEvent::InputModes(_)
                                    | MetadataEvent::ForegroundProcess { .. }
                            )
                        {
                            let pending = state.pending.entry(channel).or_default();
                            pending.metadata.retain(|old| {
                                std::mem::discriminant(old) != std::mem::discriminant(&event)
                            });
                            pending.metadata.push(event);
                        }
                    }
                    ClientEvent::FilesChanged { .. }
                    | ClientEvent::GitChanged { .. }
                    | ClientEvent::RemoteAccessChanged { .. }
                    | ClientEvent::Progress { .. } => (),
                    ClientEvent::CatalogChanged { .. } => state.refresh = true,
                    ClientEvent::SessionsChanged { .. } => state.sessions_dirty = true,
                    ClientEvent::SessionEnded { session, .. } => {
                        state.session_ended(session);
                        state.refresh = true;
                    }
                    ClientEvent::Disconnected | ClientEvent::ServerRestarting => {
                        state.disconnected();
                        break;
                    }
                }
                let invalid = state.pending.len() > 16;
                drop(state);
                if invalid || ack.is_some_and(|(channel, seq)| client.ack(channel, seq).is_err()) {
                    client.disconnect();
                    break;
                }
            }
        })
        .map_err(|error| error.to_string())
}

enum Input {
    Bytes {
        client: Client,
        channel: ChannelId,
        bytes: Vec<u8>,
    },
    Mouse {
        client: Client,
        channel: ChannelId,
        event: MouseEvent,
    },
    Flush(SyncSender<()>),
}

pub(crate) struct InputWriter {
    sender: Option<SyncSender<Input>>,
    queued: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}

impl InputWriter {
    pub(crate) fn new(shared: Arc<Mutex<Shared>>) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Input>(1024);
        let queued = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&queued);
        let worker = thread::Builder::new()
            .name("muxy-tui-input".into())
            .spawn(move || {
                while let Ok(input) = receiver.recv() {
                    let (client, channel, bytes) = match input {
                        Input::Bytes {
                            client,
                            channel,
                            bytes,
                        } => (client, channel, bytes),
                        Input::Mouse {
                            client,
                            channel,
                            event,
                        } => {
                            if let Err(error) = client.send_mouse(channel, event) {
                                lock(&shared).say(error.to_string());
                            }
                            continue;
                        }
                        Input::Flush(done) => {
                            let _ = done.send(());
                            continue;
                        }
                    };
                    for chunk in bytes.chunks(muxy_protocol::MAX_INPUT) {
                        if let Err(error) = client.send_input(channel, chunk) {
                            lock(&shared).say(error.to_string());
                            break;
                        }
                    }
                    count.fetch_sub(bytes.len(), Ordering::AcqRel);
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            sender: Some(sender),
            queued,
            worker: Some(worker),
        })
    }

    pub(crate) fn send(&self, client: Client, channel: ChannelId, bytes: Vec<u8>) -> Result {
        let length = bytes.len();
        self.queued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count
                    .checked_add(length)
                    .filter(|count| *count <= 16 * muxy_protocol::MAX_INPUT)
            })
            .map_err(|_| "Input buffer is full; wait before sending more text")?;
        if self
            .sender
            .as_ref()
            .ok_or("Input writer stopped")?
            .try_send(Input::Bytes {
                client,
                channel,
                bytes,
            })
            .is_err()
        {
            self.queued.fetch_sub(length, Ordering::AcqRel);
            return Err("Input buffer is full; wait before sending more text".into());
        }
        Ok(())
    }

    /// Queues `event` behind any typing, so clicks and keys keep their order.
    pub(crate) fn mouse(&self, client: Client, channel: ChannelId, event: MouseEvent) -> Result {
        self.sender
            .as_ref()
            .ok_or("Input writer stopped")?
            .try_send(Input::Mouse {
                client,
                channel,
                event,
            })
            .map_err(|_| "Input buffer is full; wait before sending more input".into())
    }

    pub(crate) fn flush(&self) -> Result {
        let (done, finished) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or("Input writer stopped")?
            .try_send(Input::Flush(done))
            .map_err(|_| "Input is busy; wait before changing panes")?;
        finished
            .recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| "Input writer did not finish pending text".into())
    }
}

impl Drop for InputWriter {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Fetch {
    client: Client,
    channel: ChannelId,
    request: HistoryRequest,
}

/// Pages scrollback in from the server off the input and request threads, so
/// scrolling never delays typing or layout changes.
pub(crate) struct Fetcher {
    sender: Option<SyncSender<Fetch>>,
    worker: Option<JoinHandle<()>>,
}

impl Fetcher {
    pub(crate) fn new(shared: Arc<Mutex<Shared>>) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Fetch>(64);
        let worker = thread::Builder::new()
            .name("muxy-tui-history".into())
            .spawn(move || {
                while let Ok(fetch) = receiver.recv() {
                    let mut next = Some(fetch);
                    while let Some(fetch) = next.take() {
                        let result = fetch.client.history_page(
                            fetch.channel,
                            fetch.request.before,
                            fetch.request.max_rows,
                        );
                        let mut state = lock(&shared);
                        let Some(view) = state
                            .views
                            .values_mut()
                            .find(|view| view.channel == Some(fetch.channel))
                        else {
                            continue;
                        };
                        let height = usize::from(view.viewport.rows);
                        next = view
                            .scroll
                            .receive(fetch.request, result, height)
                            .map(|request| Fetch { request, ..fetch });
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
        })
    }

    /// Asks for `request` for the view on `channel`, or tells the view it
    /// failed so it can stop waiting.
    pub(crate) fn fetch(&self, shared: &mut Shared, channel: ChannelId, request: HistoryRequest) {
        let sent = shared.client.clone().is_some_and(|client| {
            self.sender.as_ref().is_some_and(|sender| {
                sender
                    .try_send(Fetch {
                        client,
                        channel,
                        request,
                    })
                    .is_ok()
            })
        });
        if !sent
            && let Some(view) = shared
                .views
                .values_mut()
                .find(|view| view.channel == Some(channel))
        {
            let height = usize::from(view.viewport.rows);
            view.scroll
                .receive(request, Err(ClientError::Disconnected), height);
        }
    }
}

impl Drop for Fetcher {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
