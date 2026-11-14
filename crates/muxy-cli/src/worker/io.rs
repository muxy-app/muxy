use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use muxy_app_core::PaneId;
use muxy_client::{AttachedTerminal, Attachment, Client, ClientError, ClientEvent};
use muxy_protocol::{
    ActivitySnapshot, CatalogPage, ChannelId, ForegroundProcess, InputModes, MetadataEvent,
    MouseEvent, ProjectSession, SessionId, SessionMetadata, Size,
};

use crate::scroll::{HistoryRequest, Scroll};
use crate::state::{Result, State};

#[derive(Clone, Debug)]
pub(crate) struct View {
    pub session: SessionId,
    pub terminal: AttachedTerminal,
    /// False once the connection is gone; the last screen stays shown.
    pub live: bool,
    pub process: Option<ForegroundProcess>,
    pub input: InputModes,
    pub title: String,
    pub viewport: Size,
    pub ended: bool,
    pub scroll: Scroll,
}

impl View {
    pub(crate) fn attached(session: SessionId, viewport: Size, attachment: Attachment) -> Self {
        let mut terminal = attachment.terminal;
        terminal.grid.history.clear();
        terminal.grid.graphics = muxy_protocol::Graphics::default();
        Self {
            session,
            terminal,
            live: true,
            process: attachment.process,
            input: InputModes::default(),
            title: attachment.title,
            viewport,
            ended: false,
            scroll: Scroll::default(),
        }
    }

    pub(crate) fn channel(&self) -> Option<ChannelId> {
        self.live.then(|| self.terminal.channel())
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

/// Who waits for an attach that is under way.
pub(crate) enum Attaching {
    Pane {
        id: PaneId,
        viewport: Size,
    },
    /// Only what the terminal runs; the caller detaches it.
    Process(SyncSender<Option<ForegroundProcess>>),
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
    attaching: BTreeMap<u64, Attaching>,
    next_tag: u64,
    /// Channels up to this one were attached on this connection; the closed ones are acknowledged.
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
            .map(|(pane, view)| (*pane, view.channel()))
            .collect();
        for (pane, channel) in removed {
            self.views.remove(&pane);
            if channel.is_some() && self.focus == channel {
                self.focus = None;
            }
            if self.confirm.is_some_and(|closing| closing.pane() == pane) {
                self.confirm = None;
            }
        }
    }

    /// Registers an attach and returns its tag.
    pub(crate) fn start_attach(&mut self, attaching: Attaching) -> u64 {
        let tag = self.next_tag;
        self.next_tag = self.next_tag.wrapping_add(1);
        self.attaching.insert(tag, attaching);
        tag
    }

    pub(crate) fn cancel_attach(&mut self, tag: u64) {
        self.attaching.remove(&tag);
    }

    pub(crate) fn is_attaching(&self, pane: PaneId) -> bool {
        self.attaching
            .values()
            .any(|attaching| matches!(attaching, Attaching::Pane { id, .. } if *id == pane))
    }

    /// Shows an attached terminal, or returns the channel nobody waits for.
    fn attached(
        &mut self,
        tag: u64,
        session: SessionId,
        attachment: Attachment,
    ) -> Option<ChannelId> {
        let channel = attachment.terminal.channel();
        self.last_channel = self.last_channel.max(channel.0);
        match self.attaching.remove(&tag) {
            Some(Attaching::Pane { id, viewport }) => {
                if !self.ended.contains(&session) {
                    self.views
                        .insert(id, View::attached(session, viewport, attachment));
                }
                None
            }
            Some(Attaching::Process(reply)) => {
                let _ = reply.send(attachment.process);
                None
            }
            None => Some(channel),
        }
    }

    pub(crate) fn disconnected(&mut self) {
        self.client = None;
        self.sessions.clear();
        self.sessions_project = None;
        self.sessions_dirty = true;
        self.focus = None;
        self.attaching.clear();
        self.last_channel = 0;
        self.activity_dirty = true;
        for view in self.views.values_mut() {
            view.live = false;
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
                let mut stray = None;
                match event {
                    ClientEvent::Attached {
                        tag,
                        session,
                        attachment,
                    } => stray = state.attached(tag, session, *attachment),
                    ClientEvent::SessionMetadata { session, metadata } => {
                        state.metadata.insert(session, metadata);
                    }
                    ClientEvent::ActivityChanged { .. } => state.activity_dirty = true,
                    ClientEvent::Frame { channel, mut frame } => {
                        frame.graphics = None;
                        if let Some(view) = state
                            .views
                            .values_mut()
                            .find(|view| view.channel() == Some(channel))
                        {
                            if frame.reset {
                                view.scroll.bottom();
                            }
                            ack = Some((channel, view.terminal.apply(&frame)));
                        } else if channel.0 <= state.last_channel {
                            ack = Some((channel, frame.seq));
                        }
                    }
                    ClientEvent::Metadata { channel, event } => {
                        if let Some(view) = state
                            .views
                            .values_mut()
                            .find(|view| view.channel() == Some(channel))
                        {
                            view.metadata(event);
                        }
                    }
                    ClientEvent::FilesChanged { .. }
                    | ClientEvent::ClipboardWrite { .. }
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
                drop(state);
                if let Some(channel) = stray {
                    client.detach_async(channel).on_complete(drop);
                }
                if ack.is_some_and(|(channel, seq)| client.ack(channel, seq).is_err()) {
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
                            .find(|view| view.channel() == Some(fetch.channel))
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
                .find(|view| view.channel() == Some(channel))
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
