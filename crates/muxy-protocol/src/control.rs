use serde::{Deserialize, Serialize};

use crate::{
    AttachSnapshot, ChannelId, ForegroundProcess, HistoryCursor, HistoryPage, SavedScreen,
    SearchPage, SearchSource, ServerPath, SessionId, SessionInfo, Size,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RequestBody {
    CancelCreation(crate::OperationId),
    ListSessions,
    ReadCatalog {
        after: Option<crate::ProjectId>,
        revision: Option<u64>,
    },
    MutateProject(crate::ProjectIntent),
    ListProjectSessions {
        project: crate::ProjectId,
        after: Option<SessionId>,
        revision: Option<u64>,
    },
    CreateSession {
        project: crate::ProjectId,
        operation: crate::OperationId,
        directory: ServerPath,
        size: Size,
    },
    EndSession(SessionId),
    Attach {
        session: SessionId,
        size: Size,
    },
    Detach(ChannelId),
    Resize {
        channel: ChannelId,
        size: Size,
    },
    Ping,
    ReadSavedScreen(SessionId),
    DiscardSession(SessionId),
    HistoryPage {
        channel: ChannelId,
        before: HistoryCursor,
        max_rows: u16,
    },
    SavedHistoryPage {
        session: SessionId,
        before: HistoryCursor,
        max_rows: u16,
    },
    Search {
        source: SearchSource,
        query: String,
        ignore_case: bool,
        before: HistoryCursor,
        max_results: u16,
    },
    SetTerminalColors(TerminalColors),
    ReadServerSettings,
    WriteServerSettings(ServerSettingsDoc),
    StopServer,
    StopServerIfIdle,
    SyncSessionReferences {
        owner: Option<crate::OperationId>,
        revision: u64,
        sessions: Vec<SessionId>,
    },
    CloseSession {
        session: SessionId,
        operation: crate::OperationId,
    },
    IdentifyClient(crate::ClientKind),
    Git(crate::GitRequest),
    ReadActivity,
    AcknowledgeActivity(Vec<u64>),
    ClaimActivity(Vec<u64>),
    Files(crate::FilesRequest),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TerminalColors {
    pub foreground: [u8; 3],
    pub background: [u8; 3],
    pub cursor: [u8; 3],
    pub ansi: [[u8; 3]; 16],
    pub palette: std::collections::BTreeMap<u8, [u8; 3]>,
    pub cursor_style: Option<crate::CursorShape>,
    pub cursor_blink: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ReplyBody {
    Sessions(Vec<SessionInfo>),
    Catalog(crate::CatalogPage),
    ProjectMutated {
        revision: u64,
    },
    ProjectSessions(crate::ProjectSessions),
    SessionCreated(SessionInfo),
    SessionEnded,
    Detached,
    Resized,
    Pong,
    Error(ErrorReply),
    SavedScreen(SavedScreen),
    SessionDiscarded,
    CreationCancelled,
    Attached {
        snapshot: Box<AttachSnapshot>,
        process: Option<ForegroundProcess>,
    },
    HistoryPage(HistoryPage),
    SearchPage(SearchPage),
    TerminalColorsSet,
    ServerSettings(ServerSettingsDoc),
    ServerSettingsWritten,
    ServerStopping,
    ServerBusy,
    SessionReferencesSynced,
    SessionClosed,
    ClientIdentified(crate::SessionClient),
    Git(crate::GitReply),
    Activity(crate::ActivitySnapshot),
    ActivityAcknowledged,
    ActivityClaimed(Vec<u64>),
    Files(crate::FilesReply),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ErrorReply {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ErrorCode {
    UnknownProject,
    CatalogChanged,
    UnknownSession,
    UnknownChannel,
    BadSize,
    BadPath,
    SpawnFailed,
    BadRequest,
    SavedContentUnavailable,
    StaleHistoryCursor,
    HistoryUnavailable,
    PersistenceFailed,
}

/// Server-owned configuration, independent of its storage format.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerSettingsDoc {
    pub default_shell: Option<ServerPath>,
    pub history_budget_bytes: u64,
    pub shell_integration: bool,
}

impl ServerSettingsDoc {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.history_budget_bytes > 64 * 1024 * 1024 * 1024 {
            return Err(ErrorCode::BadRequest);
        }
        if let Some(shell) = &self.default_shell {
            crate::validate_path(shell)?;
            if !shell.0.starts_with(b"/") || shell.0.contains(&0) {
                return Err(ErrorCode::BadPath);
            }
        }
        Ok(())
    }
}
