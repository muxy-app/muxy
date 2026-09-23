//! Shared protocol contracts, screen types, framing, and local transport.
//!
//! Client and server runtime policy, persistence, UI, and native terminal
//! execution stay with their owners.

mod build;
pub use build::{BuildInfo, COMPATIBILITY, ServerInfo};
mod control;
mod exec;
pub use exec::{ExecRequest, ExecResult, MAX_EXEC_OUTPUT};
mod ids;
mod message;
mod path;
mod samples;
mod screen;
mod session;
mod validate;
mod version;

pub use control::{
    ErrorCode, ErrorReply, ReplyBody, RequestBody, ServerSettingsDoc, TerminalColors,
};
pub use ids::{CONTROL, ChannelId, RequestId, SessionId};
pub use message::{ChannelKind, Message};
pub use path::ServerPath;
pub use screen::{
    Color, Cursor, CursorShape, Modes, Row, Run, ScreenFrame, Size, Style, Underline,
};
pub use session::{
    AttachSnapshot, ClientKind, ExitReason, ForegroundProcess, HistoryCursor, HistoryPage,
    InputModes, LinkRow, LinkSpan, MetadataEvent, Modifiers, MouseAction, MouseButton, MouseEvent,
    ProgressState, SavedScreen, ScrollDirection, SearchMatch, SearchPage, SearchSource,
    SessionClient, SessionInfo, SessionMetadata, SessionProgress, TerminalProgress,
};
pub use validate::{
    MAX_COLS, MAX_INPUT, MAX_LINK_SPANS, MAX_LINK_URI, MAX_ROWS, validate_input, validate_path,
    validate_search, validate_size, validate_versions,
};
pub use version::{SUPPORTED, V1, Version};

mod graphics;
pub use graphics::{
    CellSize, GraphicImage, GraphicPlacement, Graphics, MAX_GRAPHICS_BYTES, MAX_GRAPHICS_PLACEMENTS,
};

mod project;
pub use project::{
    CATALOG_PAGE_SIZE, CatalogPage, ClientId, MAX_PROJECT_LOGO_BYTES, MAX_PROJECTS, OperationId,
    ProjectDescriptor, ProjectId, ProjectIntent, ProjectKind, ProjectMutation, ProjectPatch,
    ProjectSession, ProjectSessions, ServerIdentity, SessionStatus, is_project_symbol,
};

pub mod wire;

pub mod transport;

mod git;
pub use git::{
    GitAction, GitBaseSwitch, GitBranch, GitChangesPreview, GitChecks, GitCommit, GitDiff,
    GitDiffKind, GitDiffRequest, GitDiffRow, GitFile, GitFileStatus, GitLineStat, GitMergeMethod,
    GitPreviewFile, GitPullRequest, GitPullRequestAction, GitPullRequestFilter, GitPushDestination,
    GitRawDiff, GitRef, GitRefKind, GitReply, GitRepoInfo, GitRequest, GitStatus, GitSummary,
    GitWorktree, WorktreeAction, WorktreeIntent, WorktreeRemoval,
};

mod activity;
pub use activity::{
    ACTIVITY_HISTORY_LIMIT, ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity,
    AgentProvider, AgentState,
};

mod files;
pub use files::{
    FileChanges, FileContent, FileEntry, FileInfo, FilesAction, FilesReply, FilesRequest,
    MAX_FILE_BYTES, MAX_FILE_CHANGES, MAX_FILE_ENTRIES, MAX_FILE_PATH_BYTES,
};
