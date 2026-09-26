use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::wire::cbor::open_enum;

use crate::{
    AttachSnapshot, ChannelId, ForegroundProcess, HistoryCursor, HistoryPage, SavedScreen,
    SearchPage, SearchSource, ServerPath, SessionId, SessionInfo, Size,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum RequestBody {
    #[n(0)]
    CancelCreation(#[n(0)] crate::OperationId),
    #[n(1)]
    ListSessions,
    #[n(2)]
    ReadCatalog {
        #[n(0)]
        after: Option<crate::ProjectId>,
        #[n(1)]
        revision: Option<u64>,
    },
    #[n(3)]
    MutateProject(#[n(0)] crate::ProjectIntent),
    #[n(4)]
    ListProjectSessions {
        #[n(0)]
        project: crate::ProjectId,
        #[n(1)]
        after: Option<SessionId>,
        #[n(2)]
        revision: Option<u64>,
    },
    #[n(5)]
    CreateSession {
        #[n(0)]
        project: crate::ProjectId,
        #[n(1)]
        operation: crate::OperationId,
        #[n(2)]
        directory: ServerPath,
        #[n(3)]
        size: Size,
    },
    #[n(6)]
    EndSession(#[n(0)] SessionId),
    #[n(7)]
    Attach {
        #[n(0)]
        session: SessionId,
        #[n(1)]
        size: Size,
    },
    #[n(8)]
    Detach(#[n(0)] ChannelId),
    #[n(9)]
    Resize {
        #[n(0)]
        channel: ChannelId,
        #[n(1)]
        size: Size,
    },
    #[n(10)]
    Ping,
    #[n(11)]
    ReadSavedScreen(#[n(0)] SessionId),
    #[n(12)]
    DiscardSession(#[n(0)] SessionId),
    #[n(13)]
    HistoryPage {
        #[n(0)]
        channel: ChannelId,
        #[n(1)]
        before: HistoryCursor,
        #[n(2)]
        max_rows: u16,
    },
    #[n(14)]
    SavedHistoryPage {
        #[n(0)]
        session: SessionId,
        #[n(1)]
        before: HistoryCursor,
        #[n(2)]
        max_rows: u16,
    },
    #[n(15)]
    Search {
        #[n(0)]
        source: SearchSource,
        #[n(1)]
        query: String,
        #[n(2)]
        ignore_case: bool,
        #[n(3)]
        before: HistoryCursor,
        #[n(4)]
        max_results: u16,
    },
    #[n(16)]
    SetTerminalColors(#[n(0)] TerminalColors),
    #[n(17)]
    ReadServerSettings,
    #[n(18)]
    WriteServerSettings(#[n(0)] ServerSettingsDoc),
    #[n(19)]
    StopServer,
    #[n(20)]
    StopServerIfIdle,
    #[n(21)]
    SyncSessionReferences {
        #[n(0)]
        owner: Option<crate::OperationId>,
        #[n(1)]
        revision: u64,
        #[n(2)]
        sessions: Vec<SessionId>,
    },
    #[n(22)]
    CloseSession {
        #[n(0)]
        session: SessionId,
        #[n(1)]
        operation: crate::OperationId,
    },
    #[n(23)]
    IdentifyClient(#[n(0)] crate::ClientKind),
    #[n(24)]
    Git(#[n(0)] crate::GitRequest),
    #[n(25)]
    ReadActivity,
    #[n(26)]
    AcknowledgeActivity(#[n(0)] Vec<u64>),
    #[n(27)]
    ClaimActivity(#[n(0)] Vec<u64>),
    #[n(28)]
    Files(#[n(0)] crate::FilesRequest),
    #[n(29)]
    WriteInput {
        #[n(0)]
        channel: ChannelId,
        #[n(1)]
        #[cbor(with = "crate::wire::cbor::bytes")]
        bytes: Vec<u8>,
    },
    #[n(30)]
    Exec(#[n(0)] crate::ExecRequest),
    #[n(31)]
    CancelExec(#[n(0)] u64),
    #[n(32)]
    Authenticate(#[n(0)] crate::DeviceCredential),
    #[n(33)]
    Pair(#[n(0)] crate::PairRequest),
    #[n(34)]
    ReadRemoteAccess,
    #[n(35)]
    WriteRemoteAccess(#[n(0)] crate::RemoteAccessSettings),
    #[n(36)]
    StartPairing,
    #[n(37)]
    CancelPairing,
    #[n(38)]
    RevokeDevice(#[n(0)] crate::DeviceId),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct TerminalColors {
    #[n(0)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub foreground: [u8; 3],
    #[n(1)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub background: [u8; 3],
    #[n(2)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub cursor: [u8; 3],
    #[n(3)]
    pub ansi: [[u8; 3]; 16],
    #[n(4)]
    pub palette: std::collections::BTreeMap<u8, [u8; 3]>,
    #[n(5)]
    pub cursor_style: Option<crate::CursorShape>,
    #[n(6)]
    pub cursor_blink: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum ReplyBody {
    #[n(0)]
    Sessions(#[n(0)] Vec<SessionInfo>),
    #[n(1)]
    Catalog(#[n(0)] crate::CatalogPage),
    #[n(2)]
    ProjectMutated {
        #[n(0)]
        revision: u64,
    },
    #[n(3)]
    ProjectSessions(#[n(0)] crate::ProjectSessions),
    #[n(4)]
    SessionCreated(#[n(0)] SessionInfo),
    #[n(5)]
    SessionEnded,
    #[n(6)]
    Detached,
    #[n(7)]
    Resized,
    #[n(8)]
    Pong,
    #[n(9)]
    Error(#[n(0)] ErrorReply),
    #[n(10)]
    SavedScreen(#[n(0)] SavedScreen),
    #[n(11)]
    SessionDiscarded,
    #[n(12)]
    CreationCancelled,
    #[n(13)]
    Attached {
        #[n(0)]
        snapshot: Box<AttachSnapshot>,
        #[n(1)]
        process: Option<ForegroundProcess>,
    },
    #[n(14)]
    HistoryPage(#[n(0)] HistoryPage),
    #[n(15)]
    SearchPage(#[n(0)] SearchPage),
    #[n(16)]
    TerminalColorsSet,
    #[n(17)]
    ServerSettings(#[n(0)] ServerSettingsDoc),
    #[n(18)]
    ServerSettingsWritten,
    #[n(19)]
    ServerStopping,
    #[n(20)]
    ServerBusy,
    #[n(21)]
    SessionReferencesSynced,
    #[n(22)]
    SessionClosed,
    #[n(23)]
    ClientIdentified(#[n(0)] crate::SessionClient),
    #[n(24)]
    Git(#[n(0)] crate::GitReply),
    #[n(25)]
    Activity(#[n(0)] crate::ActivitySnapshot),
    #[n(26)]
    ActivityAcknowledged,
    #[n(27)]
    ActivityClaimed(#[n(0)] Vec<u64>),
    #[n(28)]
    Files(#[n(0)] crate::FilesReply),
    #[n(29)]
    InputWritten,
    #[n(30)]
    Exec(#[n(0)] crate::ExecResult),
    #[n(31)]
    ExecCancelled,
    #[n(32)]
    Authenticated,
    #[n(33)]
    Paired(#[n(0)] crate::Paired),
    #[n(34)]
    RemoteAccess(#[n(0)] crate::RemoteAccessState),
    #[n(35)]
    Pairing(#[n(0)] crate::PairingOffer),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ErrorReply {
    #[n(0)]
    pub code: ErrorCode,
    #[n(1)]
    pub message: String,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum ErrorCode {
        UnknownProject = 0,
        CatalogChanged = 1,
        UnknownSession = 2,
        UnknownChannel = 3,
        BadSize = 4,
        BadPath = 5,
        SpawnFailed = 6,
        BadRequest = 7,
        SavedContentUnavailable = 8,
        StaleHistoryCursor = 9,
        HistoryUnavailable = 10,
        PersistenceFailed = 11,
        Unauthorized = 12,
        /// The other side's build doesn't support this request or can't read its reply.
        Unsupported = 13,
    }
}

/// Server-owned configuration, independent of its storage format.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ServerSettingsDoc {
    #[n(0)]
    pub default_shell: Option<ServerPath>,
    #[n(1)]
    pub history_budget_bytes: u64,
    #[n(2)]
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
