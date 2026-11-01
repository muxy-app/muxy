use super::{Creation, GitReceipt, Membership, Receipt, State, expiry};
use muxy_protocol::{
    ErrorCode, ErrorReply, GitReply, OperationId, ProjectDescriptor, ProjectId, ProjectIntent,
    ProjectKind, ProjectMutation, ProjectPatch, ServerIdentity, ServerPath, SessionId, SessionInfo,
    SessionStatus, WorktreeAction, WorktreeHook, WorktreeIntent, WorktreeOptions, WorktreeRemoval,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::sync::Arc;

const UNKNOWN_VARIANT: u32 = u32::MAX;

macro_rules! open_unit_enum {
    (
        $(#[$meta:meta])*
        enum $stored:ident for $protocol:ident { $($variant:ident,)* }
    ) => {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize)]
        $(#[$meta])*
        enum $stored {
            $($variant,)*
            #[serde(other)]
            Unknown,
        }

        impl From<$protocol> for $stored {
            fn from(value: $protocol) -> Self {
                match value {
                    $($protocol::$variant => Self::$variant,)*
                    $protocol::Unrecognized(_) => Self::Unknown,
                }
            }
        }

        impl From<$stored> for $protocol {
            fn from(value: $stored) -> Self {
                match value {
                    $($stored::$variant => Self::$variant,)*
                    $stored::Unknown => Self::Unrecognized(UNKNOWN_VARIANT),
                }
            }
        }
    };
}

open_unit_enum! {
    #[serde(rename_all = "snake_case")]
    enum StoredKind for ProjectKind {
        Worktree,
    }
}

open_unit_enum! {
    enum StoredStatus for SessionStatus {
        Starting,
        Live,
        Ended,
        Unavailable,
    }
}

open_unit_enum! {
    enum StoredErrorCode for ErrorCode {
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
        Unauthorized,
        Unsupported,
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct StoredState {
    #[serde(default, deserialize_with = "readable_records")]
    git: BTreeMap<OperationId, StoredGitReceipt>,
    version: u32,
    server: ServerIdentity,
    home: ProjectId,
    legacy_home: Option<ProjectId>,
    revision: u64,
    projects: BTreeMap<ProjectId, StoredProject>,
    deleting: BTreeSet<ProjectId>,
    discarding: BTreeSet<SessionId>,
    cancelled: BTreeSet<OperationId>,
    sessions: BTreeMap<SessionId, StoredMembership>,
    creations: BTreeMap<OperationId, StoredCreation>,
    #[serde(deserialize_with = "readable_records")]
    receipts: BTreeMap<OperationId, StoredReceipt>,
    #[serde(default)]
    closes: BTreeMap<OperationId, (SessionId, bool)>,
    #[serde(default)]
    finished: expiry::Finished,
}

fn readable_records<'de, D, T>(deserializer: D) -> Result<BTreeMap<OperationId, T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let records = BTreeMap::<OperationId, Box<RawValue>>::deserialize(deserializer)?;
    Ok(records
        .into_iter()
        .filter_map(
            |(operation, record)| match serde_json::from_str(record.get()) {
                Ok(record) => Some((operation, record)),
                Err(error) => {
                    log::warn!(
                        "skipping saved operation {operation} this build cannot read: {error}"
                    );
                    None
                }
            },
        )
        .collect())
}

#[derive(Serialize, Deserialize)]
struct StoredProject {
    id: ProjectId,
    home: bool,
    directory: ServerPath,
    name: String,
    icon: Option<String>,
    #[serde(default)]
    logo: Option<Arc<[u8]>>,
    color: String,
    kind: Option<StoredKind>,
    parent_id: Option<ProjectId>,
}

#[derive(Serialize, Deserialize)]
struct StoredSessionInfo {
    project: ProjectId,
    id: SessionId,
    directory: ServerPath,
}

#[derive(Serialize, Deserialize)]
struct StoredError {
    code: StoredErrorCode,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct StoredCreation {
    info: StoredSessionInfo,
    error: Option<StoredError>,
}

#[derive(Serialize, Deserialize)]
struct StoredMembership {
    info: StoredSessionInfo,
    status: StoredStatus,
    #[serde(default)]
    ended_at: Option<u64>,
}

#[derive(Serialize, Deserialize)]
struct StoredReceipt {
    intent: StoredProjectIntent,
    error: Option<StoredError>,
    complete: bool,
}

#[derive(Serialize, Deserialize)]
struct StoredProjectIntent {
    operation: OperationId,
    mutation: StoredMutation,
}

#[derive(Serialize, Deserialize)]
enum StoredMutation {
    Create(StoredProject),
    Patch {
        project: ProjectId,
        patch: StoredPatch,
    },
    Delete(ProjectId),
    PruneWorktree(ProjectId),
}

#[derive(Serialize, Deserialize)]
enum StoredPatch {
    Name(String),
    Icon(Option<String>),
    Color(String),
    Logo(Option<Arc<[u8]>>),
}

#[derive(Serialize, Deserialize)]
struct StoredGitReceipt {
    owner: ProjectId,
    intent: StoredWorktreeIntent,
    project: StoredProject,
    device: u64,
    inode: u64,
    applied: bool,
    #[serde(default)]
    hooks_started: bool,
    #[serde(default)]
    hooks_finished: bool,
    #[serde(default)]
    hook_error: Option<String>,
    #[serde(default)]
    failed: Option<StoredError>,
    reply: Option<StoredGitReply>,
}

#[derive(Serialize, Deserialize)]
struct StoredWorktreeIntent {
    operation: OperationId,
    action: StoredWorktreeAction,
    #[serde(default)]
    options: Option<StoredWorktreeOptions>,
}

#[derive(Serialize, Deserialize)]
enum StoredWorktreeAction {
    Create {
        project: ProjectId,
        directory: ServerPath,
        branch: String,
        base: Option<String>,
    },
    Register {
        project: ProjectId,
        directory: ServerPath,
    },
    Remove {
        expected: StoredWorktreeRemoval,
    },
    CheckoutPullRequest {
        project: ProjectId,
        directory: ServerPath,
        number: u64,
    },
}

#[derive(Serialize, Deserialize)]
struct StoredWorktreeRemoval {
    directory: ServerPath,
    device: u64,
    inode: u64,
    dirty: bool,
    status: Vec<u8>,
    head: Option<String>,
    branch: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct StoredWorktreeOptions {
    name: Option<String>,
    hooks: Option<Vec<StoredWorktreeHook>>,
}

#[derive(Serialize, Deserialize)]
struct StoredWorktreeHook {
    command: String,
    name: Option<String>,
    project: bool,
}

#[derive(Serialize, Deserialize)]
enum StoredGitReply {
    Project(StoredProject),
    Done,
    WorktreeSetupFailed {
        project: StoredProject,
        message: String,
    },
}

impl TryFrom<&State> for StoredState {
    type Error = io::Error;

    fn try_from(state: &State) -> io::Result<Self> {
        Ok(Self {
            git: state
                .git
                .iter()
                .map(|(operation, receipt)| Ok((*operation, receipt.try_into()?)))
                .collect::<io::Result<_>>()?,
            version: state.version,
            server: state.server,
            home: state.home,
            legacy_home: state.legacy_home,
            revision: state.revision,
            projects: state
                .projects
                .iter()
                .map(|(id, project)| (*id, project.into()))
                .collect(),
            deleting: state.deleting.clone(),
            discarding: state.discarding.clone(),
            cancelled: state.cancelled.clone(),
            sessions: state
                .sessions
                .iter()
                .map(|(id, membership)| (*id, membership.into()))
                .collect(),
            creations: state
                .creations
                .iter()
                .map(|(operation, creation)| (*operation, creation.into()))
                .collect(),
            receipts: state
                .receipts
                .iter()
                .map(|(operation, receipt)| (*operation, receipt.into()))
                .collect(),
            closes: state.closes.clone(),
            finished: state.finished.clone(),
        })
    }
}

impl From<StoredState> for State {
    fn from(stored: StoredState) -> Self {
        Self {
            git: stored
                .git
                .into_iter()
                .map(|(operation, receipt)| (operation, receipt.into()))
                .collect(),
            version: stored.version,
            server: stored.server,
            home: stored.home,
            legacy_home: stored.legacy_home,
            revision: stored.revision,
            projects: stored
                .projects
                .into_iter()
                .map(|(id, project)| (id, project.into()))
                .collect(),
            deleting: stored.deleting,
            discarding: stored.discarding,
            cancelled: stored.cancelled,
            sessions: stored
                .sessions
                .into_iter()
                .map(|(id, membership)| (id, membership.into()))
                .collect(),
            creations: stored
                .creations
                .into_iter()
                .map(|(operation, creation)| (operation, creation.into()))
                .collect(),
            receipts: stored
                .receipts
                .into_iter()
                .map(|(operation, receipt)| (operation, receipt.into()))
                .collect(),
            closes: stored.closes,
            finished: stored.finished,
        }
    }
}

impl From<&ProjectDescriptor> for StoredProject {
    fn from(project: &ProjectDescriptor) -> Self {
        Self {
            id: project.id,
            home: project.home,
            directory: project.directory.clone(),
            name: project.name.clone(),
            icon: project.icon.clone(),
            logo: project.logo.clone(),
            color: project.color.clone(),
            kind: project.kind.map(StoredKind::from),
            parent_id: project.parent_id,
        }
    }
}

impl From<StoredProject> for ProjectDescriptor {
    fn from(project: StoredProject) -> Self {
        Self {
            id: project.id,
            home: project.home,
            directory: project.directory,
            name: project.name,
            icon: project.icon,
            logo: project.logo,
            color: project.color,
            kind: project.kind.map(ProjectKind::from),
            parent_id: project.parent_id,
        }
    }
}

impl From<&SessionInfo> for StoredSessionInfo {
    fn from(info: &SessionInfo) -> Self {
        Self {
            project: info.project,
            id: info.id,
            directory: info.directory.clone(),
        }
    }
}

impl From<StoredSessionInfo> for SessionInfo {
    fn from(info: StoredSessionInfo) -> Self {
        Self {
            project: info.project,
            id: info.id,
            directory: info.directory,
        }
    }
}

impl From<&ErrorReply> for StoredError {
    fn from(error: &ErrorReply) -> Self {
        Self {
            code: error.code.into(),
            message: error.message.clone(),
        }
    }
}

impl From<StoredError> for ErrorReply {
    fn from(error: StoredError) -> Self {
        Self {
            code: error.code.into(),
            message: error.message,
        }
    }
}

impl From<&Creation> for StoredCreation {
    fn from(creation: &Creation) -> Self {
        Self {
            info: (&creation.info).into(),
            error: creation.error.as_ref().map(StoredError::from),
        }
    }
}

impl From<StoredCreation> for Creation {
    fn from(creation: StoredCreation) -> Self {
        Self {
            info: creation.info.into(),
            error: creation.error.map(ErrorReply::from),
        }
    }
}

impl From<&Membership> for StoredMembership {
    fn from(membership: &Membership) -> Self {
        Self {
            info: (&membership.info).into(),
            status: membership.status.into(),
            ended_at: membership.ended_at,
        }
    }
}

impl From<StoredMembership> for Membership {
    fn from(membership: StoredMembership) -> Self {
        Self {
            info: membership.info.into(),
            status: membership.status.into(),
            ended_at: membership.ended_at,
        }
    }
}

impl From<&Receipt> for StoredReceipt {
    fn from(receipt: &Receipt) -> Self {
        Self {
            intent: StoredProjectIntent {
                operation: receipt.intent.operation,
                mutation: (&receipt.intent.mutation).into(),
            },
            error: receipt.error.as_ref().map(StoredError::from),
            complete: receipt.complete,
        }
    }
}

impl From<StoredReceipt> for Receipt {
    fn from(receipt: StoredReceipt) -> Self {
        Self {
            intent: ProjectIntent {
                operation: receipt.intent.operation,
                mutation: receipt.intent.mutation.into(),
            },
            error: receipt.error.map(ErrorReply::from),
            complete: receipt.complete,
        }
    }
}

impl From<&ProjectMutation> for StoredMutation {
    fn from(mutation: &ProjectMutation) -> Self {
        match mutation {
            ProjectMutation::Create(project) => Self::Create(project.into()),
            ProjectMutation::Patch { project, patch } => Self::Patch {
                project: *project,
                patch: match patch {
                    ProjectPatch::Name(name) => StoredPatch::Name(name.clone()),
                    ProjectPatch::Icon(icon) => StoredPatch::Icon(icon.clone()),
                    ProjectPatch::Color(color) => StoredPatch::Color(color.clone()),
                    ProjectPatch::Logo(logo) => StoredPatch::Logo(logo.clone()),
                },
            },
            ProjectMutation::Delete(project) => Self::Delete(*project),
            ProjectMutation::PruneWorktree(project) => Self::PruneWorktree(*project),
        }
    }
}

impl From<StoredMutation> for ProjectMutation {
    fn from(mutation: StoredMutation) -> Self {
        match mutation {
            StoredMutation::Create(project) => Self::Create(project.into()),
            StoredMutation::Patch { project, patch } => Self::Patch {
                project,
                patch: match patch {
                    StoredPatch::Name(name) => ProjectPatch::Name(name),
                    StoredPatch::Icon(icon) => ProjectPatch::Icon(icon),
                    StoredPatch::Color(color) => ProjectPatch::Color(color),
                    StoredPatch::Logo(logo) => ProjectPatch::Logo(logo),
                },
            },
            StoredMutation::Delete(project) => Self::Delete(project),
            StoredMutation::PruneWorktree(project) => Self::PruneWorktree(project),
        }
    }
}

impl TryFrom<&GitReceipt> for StoredGitReceipt {
    type Error = io::Error;

    fn try_from(receipt: &GitReceipt) -> io::Result<Self> {
        Ok(Self {
            owner: receipt.owner,
            intent: (&receipt.intent).into(),
            project: (&receipt.project).into(),
            device: receipt.device,
            inode: receipt.inode,
            applied: receipt.applied,
            hooks_started: receipt.hooks_started,
            hooks_finished: receipt.hooks_finished,
            hook_error: receipt.hook_error.clone(),
            failed: receipt.failed.as_ref().map(StoredError::from),
            reply: receipt
                .reply
                .as_ref()
                .map(StoredGitReply::try_from)
                .transpose()?,
        })
    }
}

impl From<StoredGitReceipt> for GitReceipt {
    fn from(receipt: StoredGitReceipt) -> Self {
        Self {
            owner: receipt.owner,
            intent: receipt.intent.into(),
            project: receipt.project.into(),
            device: receipt.device,
            inode: receipt.inode,
            applied: receipt.applied,
            hooks_started: receipt.hooks_started,
            hooks_finished: receipt.hooks_finished,
            hook_error: receipt.hook_error,
            failed: receipt.failed.map(ErrorReply::from),
            reply: receipt.reply.map(GitReply::from),
        }
    }
}

impl From<&WorktreeIntent> for StoredWorktreeIntent {
    fn from(intent: &WorktreeIntent) -> Self {
        Self {
            operation: intent.operation,
            action: (&intent.action).into(),
            options: intent
                .options
                .as_ref()
                .map(|options| StoredWorktreeOptions {
                    name: options.name.clone(),
                    hooks: options.hooks.as_ref().map(|hooks| {
                        hooks
                            .iter()
                            .map(|hook| StoredWorktreeHook {
                                command: hook.command.clone(),
                                name: hook.name.clone(),
                                project: hook.project,
                            })
                            .collect()
                    }),
                }),
        }
    }
}

impl From<StoredWorktreeIntent> for WorktreeIntent {
    fn from(intent: StoredWorktreeIntent) -> Self {
        Self {
            operation: intent.operation,
            action: intent.action.into(),
            options: intent.options.map(|options| WorktreeOptions {
                name: options.name,
                hooks: options.hooks.map(|hooks| {
                    hooks
                        .into_iter()
                        .map(|hook| WorktreeHook {
                            command: hook.command,
                            name: hook.name,
                            project: hook.project,
                        })
                        .collect()
                }),
            }),
        }
    }
}

impl From<&WorktreeAction> for StoredWorktreeAction {
    fn from(action: &WorktreeAction) -> Self {
        match action {
            WorktreeAction::Create {
                project,
                directory,
                branch,
                base,
            } => Self::Create {
                project: *project,
                directory: directory.clone(),
                branch: branch.clone(),
                base: base.clone(),
            },
            WorktreeAction::Register { project, directory } => Self::Register {
                project: *project,
                directory: directory.clone(),
            },
            WorktreeAction::Remove { expected } => Self::Remove {
                expected: StoredWorktreeRemoval {
                    directory: expected.directory.clone(),
                    device: expected.device,
                    inode: expected.inode,
                    dirty: expected.dirty,
                    status: expected.status.clone(),
                    head: expected.head.clone(),
                    branch: expected.branch.clone(),
                },
            },
            WorktreeAction::CheckoutPullRequest {
                project,
                directory,
                number,
            } => Self::CheckoutPullRequest {
                project: *project,
                directory: directory.clone(),
                number: *number,
            },
        }
    }
}

impl From<StoredWorktreeAction> for WorktreeAction {
    fn from(action: StoredWorktreeAction) -> Self {
        match action {
            StoredWorktreeAction::Create {
                project,
                directory,
                branch,
                base,
            } => Self::Create {
                project,
                directory,
                branch,
                base,
            },
            StoredWorktreeAction::Register { project, directory } => {
                Self::Register { project, directory }
            }
            StoredWorktreeAction::Remove { expected } => Self::Remove {
                expected: WorktreeRemoval {
                    directory: expected.directory,
                    device: expected.device,
                    inode: expected.inode,
                    dirty: expected.dirty,
                    status: expected.status,
                    head: expected.head,
                    branch: expected.branch,
                },
            },
            StoredWorktreeAction::CheckoutPullRequest {
                project,
                directory,
                number,
            } => Self::CheckoutPullRequest {
                project,
                directory,
                number,
            },
        }
    }
}

impl TryFrom<&GitReply> for StoredGitReply {
    type Error = io::Error;

    fn try_from(reply: &GitReply) -> io::Result<Self> {
        match reply {
            GitReply::Project(project) => Ok(Self::Project(project.into())),
            GitReply::Done => Ok(Self::Done),
            GitReply::WorktreeSetupFailed { project, message } => Ok(Self::WorktreeSetupFailed {
                project: project.into(),
                message: message.clone(),
            }),
            _ => Err(io::Error::other("only worktree replies are saved")),
        }
    }
}

impl From<StoredGitReply> for GitReply {
    fn from(reply: StoredGitReply) -> Self {
        match reply {
            StoredGitReply::Project(project) => Self::Project(project.into()),
            StoredGitReply::Done => Self::Done,
            StoredGitReply::WorktreeSetupFailed { project, message } => Self::WorktreeSetupFailed {
                project: project.into(),
                message,
            },
        }
    }
}
