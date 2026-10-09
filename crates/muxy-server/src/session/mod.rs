mod metadata;
mod owner;

use std::fmt;
use std::sync::mpsc::{SendError, Sender};

use muxy_protocol::{
    AttachSnapshot, ChannelId, ExitReason, ForegroundProcess, HistoryCursor, HistoryPage,
    MetadataEvent, MouseEvent, ScreenFrame, SearchPage, SessionId, SessionInfo, Size,
    TerminalColors,
};

use crate::error::ServerError;
use owner::OwnerEvent;

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
        snapshot: AttachSnapshot,
        process: Option<ForegroundProcess>,
    },
    Metadata(MetadataEvent),
    Frame(ScreenFrame),
    Resized(ScreenFrame),
    Ended(ExitReason),
}

/// Where a session delivers one attachment's events.
pub struct AttachmentSink(Target);

enum Target {
    Channel(Sender<AttachmentEvent>),
    Outbox(crate::connection::OutboxSink),
}

impl AttachmentSink {
    pub(crate) fn outbox(sink: crate::connection::OutboxSink) -> Self {
        Self(Target::Outbox(sink))
    }

    /// Returns false once nobody receives this attachment's events.
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

pub(super) type SharedMetadata = std::sync::Arc<std::sync::Mutex<muxy_protocol::SessionMetadata>>;

pub(super) type SharedProgress = std::sync::Arc<std::sync::Mutex<muxy_protocol::SessionProgress>>;

#[derive(Clone, Debug)]
pub struct SessionHandle {
    info: SessionInfo,
    progress: SharedProgress,
    metadata: SharedMetadata,
    commands: Sender<OwnerEvent>,
}

impl SessionHandle {
    pub(crate) fn new(
        info: SessionInfo,
        commands: Sender<OwnerEvent>,
        progress: SharedProgress,
        metadata: SharedMetadata,
    ) -> Self {
        Self {
            info,
            progress,
            metadata,
            commands,
        }
    }

    pub(crate) fn metadata(&self) -> muxy_protocol::SessionMetadata {
        self.metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn progress(&self) -> muxy_protocol::SessionProgress {
        *self
            .progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn id(&self) -> SessionId {
        self.info.id
    }

    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    pub fn send(&self, command: SessionCommand) -> Result<(), ServerError> {
        self.try_send(command)
            .map_err(|_| ServerError::unknown_session(self.info.id))
    }

    /// Hands an undelivered command back, so the caller decides where its sink drops.
    pub(crate) fn try_send(&self, command: SessionCommand) -> Result<(), SendError<OwnerEvent>> {
        self.commands.send(OwnerEvent::Command(command))
    }

    /// A handle whose commands arrive on the returned receiver instead of a session thread.
    #[cfg(test)]
    pub(crate) fn fake() -> (Self, std::sync::mpsc::Receiver<OwnerEvent>) {
        let directory = muxy_protocol::ServerPath(b"/tmp".to_vec());
        let (commands, received) = std::sync::mpsc::channel();
        let handle = Self::new(
            SessionInfo {
                project: muxy_protocol::ProjectId::from_u128(1),
                id: SessionId::from(std::num::NonZeroU64::MIN),
                directory: directory.clone(),
            },
            commands,
            SharedProgress::default(),
            std::sync::Arc::new(std::sync::Mutex::new(muxy_protocol::SessionMetadata {
                title: String::new(),
                directory,
                process: None,
            })),
        );
        (handle, received)
    }
}
