mod metadata;
mod owner;

use std::fmt;
use std::sync::mpsc::{SendError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use muxy_protocol::{
    AttachSnapshot, ChannelId, ExitReason, ForegroundProcess, HistoryCursor, HistoryPage, Message,
    MetadataEvent, MouseEvent, ProjectId, ScreenFrame, SearchPage, SessionId, SessionInfo,
    SessionMetadata, SessionProgress, Size, TerminalColors,
};

use crate::connection::Connection;
use crate::error::ServerError;
use crate::notifier::Notifier;
pub(crate) use owner::OwnerEvent;

pub(crate) use owner::start;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AttachmentId(pub u64);

#[derive(Debug)]
pub enum SessionCommand {
    Input(Vec<u8>),
    TerminalInput(muxy_protocol::TerminalInput),
    WriteInput {
        bytes: Vec<u8>,
        reply: Sender<Result<(), ServerError>>,
    },
    ClearScreen {
        reply: Sender<Result<(), ServerError>>,
    },
    Mouse(MouseEvent),
    CellSize(muxy_protocol::CellSize),
    Resize(Size),
    SetColors(TerminalColors),
    ResizeAttachment {
        id: AttachmentId,
        size: Size,
    },
    Attach {
        id: AttachmentId,
        channel: ChannelId,
        size: Size,
        sink: AttachmentSink,
    },
    AttachWithoutResize {
        id: AttachmentId,
        channel: ChannelId,
        sink: AttachmentSink,
    },
    Detach(AttachmentId),
    HistoryPage {
        before: HistoryCursor,
        max_rows: u16,
        reply: Sender<Result<HistoryPage, ServerError>>,
    },
    ImageFrames {
        image: u32,
        keys: Vec<u64>,
        reply: Sender<Result<Vec<muxy_protocol::ImageFrame>, ServerError>>,
    },
    End,
    Search {
        query: String,
        ignore_case: bool,
        before: HistoryCursor,
        max_results: u16,
        reply: Sender<Result<SearchPage, ServerError>>,
    },
    Stop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachmentEvent {
    Snapshot {
        snapshot: Box<AttachSnapshot>,
        process: Option<ForegroundProcess>,
    },
    Metadata(MetadataEvent),
    Frame(ScreenFrame),
    Resized(ScreenFrame),
    Ended(ExitReason),
}

pub struct AttachmentSink(Target);

enum Target {
    Channel(Sender<AttachmentEvent>),
    Outbox(crate::connection::OutboxSink),
}

impl AttachmentSink {
    pub(crate) fn outbox(sink: crate::connection::OutboxSink) -> Self {
        Self(Target::Outbox(sink))
    }

    pub(crate) fn shares_images(&self) -> bool {
        match &self.0 {
            Target::Channel(_) => false,
            Target::Outbox(outbox) => outbox.shares_images(),
        }
    }

    pub(crate) fn send(&mut self, event: AttachmentEvent) -> bool {
        match &mut self.0 {
            Target::Channel(sender) => sender.send(event).is_ok(),
            Target::Outbox(outbox) => outbox.send(event),
        }
    }
}

impl From<Sender<AttachmentEvent>> for AttachmentSink {
    fn from(sender: Sender<AttachmentEvent>) -> Self {
        Self(Target::Channel(sender))
    }
}

impl fmt::Debug for AttachmentSink {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Target::Channel(_) => formatter.write_str("AttachmentSink::Channel"),
            Target::Outbox(_) => formatter.write_str("AttachmentSink::Outbox"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Published {
    session: SessionId,
    project: Mutex<ProjectId>,
    state: Mutex<(SessionProgress, SessionMetadata)>,
    notifier: Arc<Notifier>,
}

impl Published {
    pub(crate) fn new(info: &SessionInfo, notifier: Arc<Notifier>) -> Self {
        Self {
            session: info.id,
            project: Mutex::new(info.project),
            state: Mutex::new((
                SessionProgress::default(),
                SessionMetadata {
                    title: String::new(),
                    directory: info.directory.clone(),
                    process: None,
                },
            )),
            notifier,
        }
    }

    pub(crate) fn project(&self) -> ProjectId {
        *self.project.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set_project(&self, project: ProjectId) {
        *self.project.lock().unwrap_or_else(PoisonError::into_inner) = project;
    }

    pub(crate) fn set_progress(&self, progress: SessionProgress) {
        let mut state = self.lock();
        if state.0 != progress {
            state.0 = progress;
            self.notifier.broadcast(&Message::Progress {
                session: self.session,
                progress,
            });
        }
    }

    #[cfg(windows)]
    fn metadata(&self) -> SessionMetadata {
        self.lock().1.clone()
    }

    pub(crate) fn set_metadata(&self, metadata: SessionMetadata) {
        let mut state = self.lock();
        if state.1 != metadata {
            state.1.clone_from(&metadata);
            self.notifier.broadcast(&Message::SessionMetadata {
                session: self.session,
                metadata,
            });
        }
    }

    pub(crate) fn introduce(&self, connection: &Connection) {
        let state = self.lock();
        connection.push_control(Message::SessionMetadata {
            session: self.session,
            metadata: state.1.clone(),
        });
        if state.0 != SessionProgress::default() {
            connection.push_control(Message::Progress {
                session: self.session,
                progress: state.0,
            });
        }
    }

    fn lock(&self) -> MutexGuard<'_, (SessionProgress, SessionMetadata)> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Clone, Debug)]
pub struct SessionHandle {
    info: SessionInfo,
    published: Arc<Published>,
    commands: Sender<OwnerEvent>,
}

impl SessionHandle {
    pub(crate) fn new(
        info: SessionInfo,
        commands: Sender<OwnerEvent>,
        published: Arc<Published>,
    ) -> Self {
        Self {
            info,
            published,
            commands,
        }
    }

    pub(crate) fn introduce(&self, connection: &Connection) {
        self.published.introduce(connection);
    }

    pub fn id(&self) -> SessionId {
        self.info.id
    }

    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            project: self.published.project(),
            ..self.info.clone()
        }
    }

    #[cfg(windows)]
    pub(crate) fn metadata(&self) -> SessionMetadata {
        self.published.metadata()
    }

    pub(crate) fn move_to(&self, project: ProjectId) {
        self.published.set_project(project);
    }

    pub fn send(&self, command: SessionCommand) -> Result<(), ServerError> {
        self.try_send(command)
            .map_err(|_| ServerError::unknown_session(self.info.id))
    }

    pub(crate) fn try_send(&self, command: SessionCommand) -> Result<(), SendError<OwnerEvent>> {
        self.commands.send(OwnerEvent::Command(command))
    }

    #[cfg(test)]
    pub(crate) fn fake() -> (Self, std::sync::mpsc::Receiver<OwnerEvent>) {
        let info = SessionInfo {
            project: ProjectId::from_u128(1),
            id: SessionId::from(std::num::NonZeroU64::MIN),
            directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
        };
        let (commands, received) = std::sync::mpsc::channel();
        let published = Arc::new(Published::new(&info, Arc::default()));
        (Self::new(info, commands, published), received)
    }
}
