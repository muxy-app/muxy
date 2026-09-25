use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};

use muxy_app_core::PaneId;
use muxy_client::{Attachment, Client, ClientEvent, RunGrid};
use muxy_protocol::{
    CatalogPage, ChannelId, ForegroundProcess, InputModes, MetadataEvent, ProjectSession,
    ScreenFrame, SessionId, Size,
};

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

#[derive(Default)]
pub(crate) struct Shared {
    pub client: Option<Client>,
    pub state: Option<State>,
    pub catalog: Option<CatalogPage>,
    pub sessions: Vec<ProjectSession>,
    pub sessions_dirty: bool,
    pub sessions_project: Option<muxy_protocol::ProjectId>,
    pub views: BTreeMap<PaneId, View>,
    pub message: String,
    pub refresh: bool,
    pub exit: Option<Result>,
    pub confirm: Option<PaneId>,
    pub focus: Option<ChannelId>,
    pub host_unfocused: bool,
    pub ended: BTreeSet<SessionId>,
    pending: BTreeMap<ChannelId, Pending>,
    last_channel: u32,
}

impl Shared {
    pub(crate) fn session_ended(&mut self, session: SessionId) {
        self.ended.insert(session);
        if let Some(state) = &mut self.state {
            state.close_sessions(&self.ended);
        }
        self.sessions.retain(|entry| entry.info.id != session);
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
            if self.confirm == Some(pane) {
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
        for view in self.views.values_mut() {
            view.channel = None;
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
                    ClientEvent::SessionMetadata { .. } | ClientEvent::ActivityChanged { .. } => {}
                    ClientEvent::Frame { channel, mut frame } => {
                        frame.graphics = None;
                        if let Some(view) = state
                            .views
                            .values_mut()
                            .find(|view| view.channel == Some(channel))
                        {
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
                    let Input::Bytes {
                        client,
                        channel,
                        bytes,
                    } = input
                    else {
                        if let Input::Flush(done) = input {
                            let _ = done.send(());
                        }
                        continue;
                    };
                    for chunk in bytes.chunks(muxy_protocol::MAX_INPUT) {
                        if let Err(error) = client.send_input(channel, chunk) {
                            lock(&shared).message = error.to_string();
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
