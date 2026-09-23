use serde::{Deserialize, Serialize};

use crate::{ErrorCode, OperationId, ProjectDescriptor, ProjectId, ServerPath};

mod extension;
pub use extension::*;

/// Git operations always resolve their repository through a server project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitRequest {
    pub project: ProjectId,
    pub action: GitAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GitAction {
    Summary,
    Branches,
    Changes,
    SwitchBranch(String),
    CreateBranch(String),
    DeleteBranch(String),
    Stage(Vec<ServerPath>),
    Unstage(Vec<ServerPath>),
    Discard(Vec<ServerPath>),
    Worktrees,
    InspectRemoval,
    Worktree(WorktreeIntent),
    /// Replace this connection's filesystem watch with the requested project.
    Watch,
    Status {
        local: bool,
    },
    RepoInfo,
    RemoteBranches,
    Log {
        max_count: u32,
        skip: u32,
    },
    Diff(GitDiffRequest),
    Init,
    Commit {
        message: String,
        stage_all: bool,
    },
    Push {
        set_upstream: bool,
    },
    Pull,
    Checkout(String),
    CherryPick(String),
    Revert(String),
    DeleteLocalBranch {
        name: String,
        force: bool,
    },
    DeleteRemoteBranch(String),
    CreateTag {
        name: String,
        hash: String,
    },
    PullRequest(GitPullRequestAction),
    /// Preview committed changes on this branch relative to an origin branch.
    BranchDiff {
        base: String,
        line_limit: Option<u32>,
    },
    /// Snapshot every working-tree change without touching the index.
    ChangesPreview {
        line_limit: Option<u32>,
    },
    /// Stage every change and commit only while the tree still matches its preview.
    CommitAll {
        message: String,
        expected_head: Option<String>,
        expected_tree: String,
    },
    /// Push the current branch to the destination its preview reported.
    PublishBranch {
        branch: String,
        destination: GitPushDestination,
    },
    /// Switch to a merged pull request's base branch and fast-forward it.
    SwitchToBase(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorktreeIntent {
    pub operation: OperationId,
    pub action: WorktreeAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WorktreeAction {
    Create {
        project: ProjectId,
        directory: ServerPath,
        branch: String,
        /// None checks out an existing branch; Some creates a branch from this ref.
        base: Option<String>,
    },
    Register {
        project: ProjectId,
        directory: ServerPath,
    },
    Remove {
        expected: WorktreeRemoval,
    },
    CheckoutPullRequest {
        project: ProjectId,
        directory: ServerPath,
        number: u64,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitSummary {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub changed: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    pub checked_out: bool,
    pub default: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitFile {
    pub path: ServerPath,
    pub original_path: Option<ServerPath>,
    pub index: u8,
    pub worktree: u8,
    pub added: Option<u64>,
    pub removed: Option<u64>,
}

impl GitFile {
    pub fn untracked(&self) -> bool {
        self.index == b'?'
    }
    pub fn conflicted(&self) -> bool {
        self.index == b'U'
            || self.worktree == b'U'
            || matches!((self.index, self.worktree), (b'A', b'A') | (b'D', b'D'))
    }
    pub fn staged(&self) -> bool {
        !self.untracked() && self.index != b' '
    }
    pub fn unstaged(&self) -> bool {
        !self.untracked() && self.worktree != b' '
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent flags from Git worktree porcelain"
)]
pub struct GitWorktree {
    pub directory: ServerPath,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub primary: bool,
    pub locked: bool,
    pub bare: bool,
    pub detached: bool,
    pub prunable: bool,
    pub registered: Option<ProjectId>,
}

/// An inspection binds confirmation to the directory identity and current Git state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorktreeRemoval {
    pub directory: ServerPath,
    pub device: u64,
    pub inode: u64,
    pub dirty: bool,
    pub status: Vec<u8>,
    pub head: Option<String>,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitPushDestination {
    pub remote: String,
    pub branch: String,
}

/// Every change as it would be committed, computed without touching the index.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitChangesPreview {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub tree: String,
    pub diff: GitRawDiff,
    pub files: Vec<GitPreviewFile>,
    pub destination: Option<GitPushDestination>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitPreviewFile {
    pub path: ServerPath,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub untracked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GitBaseSwitch {
    Updated,
    /// The base branch is checked out by another worktree, so nothing changed.
    CheckedOutElsewhere(ServerPath),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GitReply {
    /// None means an existing directory without a Git repository.
    Summary(Option<GitSummary>),
    Branches(Vec<GitBranch>),
    Changes(Vec<GitFile>),
    Worktrees(Vec<GitWorktree>),
    Removal(WorktreeRemoval),
    Project(ProjectDescriptor),
    Done,
    Status(Box<GitStatus>),
    RepoInfo(GitRepoInfo),
    RemoteBranches(Vec<String>),
    Log(Vec<GitCommit>),
    RawDiff(GitRawDiff),
    Diff(GitDiff),
    Commit(String),
    PullRequest(Option<Box<GitPullRequest>>),
    PullRequestNumber(Option<u64>),
    PullRequests(Vec<GitPullRequest>),
    ChangesPreview(Box<GitChangesPreview>),
    BaseSwitch(GitBaseSwitch),
}

impl GitRequest {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        fn text(value: &str) -> Result<(), ErrorCode> {
            if value.is_empty()
                || value.len() > 1024
                || value.contains('\0')
                || value.starts_with('-')
            {
                Err(ErrorCode::BadRequest)
            } else {
                Ok(())
            }
        }
        fn path(value: &ServerPath) -> Result<(), ErrorCode> {
            if value.0.is_empty() || value.0.len() > 4096 || value.0.contains(&0) {
                Err(ErrorCode::BadPath)
            } else {
                Ok(())
            }
        }
        match &self.action {
            GitAction::SwitchBranch(s)
            | GitAction::CreateBranch(s)
            | GitAction::DeleteBranch(s)
            | GitAction::DeleteRemoteBranch(s)
            | GitAction::SwitchToBase(s)
            | GitAction::DeleteLocalBranch { name: s, .. } => text(s),
            GitAction::Checkout(hash) | GitAction::CherryPick(hash) | GitAction::Revert(hash) => {
                validate_hash(hash)
            }
            GitAction::CreateTag { name, hash } => {
                text(name)?;
                validate_hash(hash)
            }
            GitAction::Commit { message, .. } => validate_message(message, 64 * 1024),
            GitAction::Log { max_count, .. } if *max_count > 1000 => Err(ErrorCode::BadRequest),
            GitAction::Diff(request) => {
                if !request.raw && request.path.is_none() {
                    return Err(ErrorCode::BadRequest);
                }
                validate_line_limit(request.line_limit)?;
                request.path.as_ref().map_or(Ok(()), path)
            }
            GitAction::BranchDiff { base, line_limit } => {
                text(base)?;
                validate_line_limit(*line_limit)
            }
            GitAction::ChangesPreview { line_limit } => validate_line_limit(*line_limit),
            GitAction::CommitAll {
                message,
                expected_head,
                expected_tree,
            } => {
                validate_message(message, 64 * 1024)?;
                expected_head.as_deref().map_or(Ok(()), validate_hash)?;
                validate_hash(expected_tree)
            }
            GitAction::PublishBranch {
                branch,
                destination,
            } => {
                text(branch)?;
                text(&destination.remote)?;
                text(&destination.branch)
            }
            GitAction::PullRequest(action) => action.validate(),
            GitAction::Stage(paths) | GitAction::Unstage(paths) | GitAction::Discard(paths) => {
                if paths.len() > 4096 {
                    return Err(ErrorCode::BadRequest);
                }
                paths.iter().try_for_each(path)
            }
            GitAction::Worktree(intent) => match &intent.action {
                WorktreeAction::Create {
                    directory,
                    branch,
                    base,
                    ..
                } => {
                    path(directory)?;
                    text(branch)?;
                    base.as_deref().map_or(Ok(()), text)
                }
                WorktreeAction::Register { directory, .. } => path(directory),
                WorktreeAction::CheckoutPullRequest {
                    directory, number, ..
                } => {
                    validate_number(*number)?;
                    path(directory)
                }
                WorktreeAction::Remove { expected } => {
                    if expected.status.len() > 4 * 1024 * 1024 {
                        return Err(ErrorCode::BadRequest);
                    }
                    path(&expected.directory)
                }
            },
            _ => Ok(()),
        }
    }
}
