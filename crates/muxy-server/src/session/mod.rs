mod metadata;
mod owner;

use std::sync::mpsc::Sender;

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
    WriteInput {
        bytes: Vec<u8>,
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
        sink: Sender<AttachmentEvent>,
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
        self.commands
            .send(OwnerEvent::Command(command))
            .map_err(|_| ServerError::unknown_session(self.info.id))
    }
}
