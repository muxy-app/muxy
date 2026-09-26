use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ErrorCode, OperationId, ProjectDescriptor, ProjectId, ServerPath};

mod extension;
pub use extension::*;

/// Git operations always resolve their repository through a server project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitRequest {
    #[n(0)]
    pub project: ProjectId,
    #[n(1)]
    pub action: GitAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum GitAction {
    #[n(0)]
    Summary,
    #[n(1)]
    Branches,
    #[n(2)]
    Changes,
    #[n(3)]
    SwitchBranch(#[n(0)] String),
    #[n(4)]
    CreateBranch(#[n(0)] String),
    #[n(5)]
    DeleteBranch(#[n(0)] String),
    #[n(6)]
    Stage(#[n(0)] Vec<ServerPath>),
    #[n(7)]
    Unstage(#[n(0)] Vec<ServerPath>),
    #[n(8)]
    Discard(#[n(0)] Vec<ServerPath>),
    #[n(9)]
    Worktrees,
    #[n(10)]
    InspectRemoval,
    #[n(11)]
    Worktree(#[n(0)] WorktreeIntent),
    /// Replace this connection's filesystem watch with the requested project.
    #[n(12)]
    Watch,
    #[n(13)]
    Status {
        #[n(0)]
        local: bool,
    },
    #[n(14)]
    RepoInfo,
    #[n(15)]
    RemoteBranches,
    #[n(16)]
    Log {
        #[n(0)]
        max_count: u32,
        #[n(1)]
        skip: u32,
    },
    #[n(17)]
    Diff(#[n(0)] GitDiffRequest),
    #[n(18)]
    Init,
    #[n(19)]
    Commit {
        #[n(0)]
        message: String,
        #[n(1)]
        stage_all: bool,
    },
    #[n(20)]
    Push {
        #[n(0)]
        set_upstream: bool,
    },
    #[n(21)]
    Pull,
    #[n(22)]
    Checkout(#[n(0)] String),
    #[n(23)]
    CherryPick(#[n(0)] String),
    #[n(24)]
    Revert(#[n(0)] String),
    #[n(25)]
    DeleteLocalBranch {
        #[n(0)]
        name: String,
        #[n(1)]
        force: bool,
    },
    #[n(26)]
    DeleteRemoteBranch(#[n(0)] String),
    #[n(27)]
    CreateTag {
        #[n(0)]
        name: String,
        #[n(1)]
        hash: String,
    },
    #[n(28)]
    PullRequest(#[n(0)] GitPullRequestAction),
    /// Preview committed changes on this branch relative to an origin branch.
    #[n(29)]
    BranchDiff {
        #[n(0)]
        base: String,
        #[n(1)]
        line_limit: Option<u32>,
    },
    /// Snapshot every working-tree change without touching the index.
    #[n(30)]
    ChangesPreview {
        #[n(0)]
        line_limit: Option<u32>,
    },
    /// Stage every change and commit only while the tree still matches its preview.
    #[n(31)]
    CommitAll {
        #[n(0)]
        message: String,
        #[n(1)]
        expected_head: Option<String>,
        #[n(2)]
        expected_tree: String,
    },
    /// Push the current branch to the destination its preview reported.
    #[n(32)]
    PublishBranch {
        #[n(0)]
        branch: String,
        #[n(1)]
        destination: GitPushDestination,
    },
    /// Switch to a merged pull request's base branch and fast-forward it.
    #[n(33)]
    SwitchToBase(#[n(0)] String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct WorktreeIntent {
    #[n(0)]
    pub operation: OperationId,
    #[n(1)]
    pub action: WorktreeAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum WorktreeAction {
    #[n(0)]
    Create {
        #[n(0)]
        project: ProjectId,
        #[n(1)]
        directory: ServerPath,
        #[n(2)]
        branch: String,
        /// None checks out an existing branch; Some creates a branch from this ref.
        #[n(3)]
        base: Option<String>,
    },
    #[n(1)]
    Register {
        #[n(0)]
        project: ProjectId,
        #[n(1)]
        directory: ServerPath,
    },
    #[n(2)]
    Remove {
        #[n(0)]
        expected: WorktreeRemoval,
    },
    #[n(3)]
    CheckoutPullRequest {
        #[n(0)]
        project: ProjectId,
        #[n(1)]
        directory: ServerPath,
        #[n(2)]
        number: u64,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitSummary {
    #[n(0)]
    pub branch: Option<String>,
    #[n(1)]
    pub head: Option<String>,
    #[n(2)]
    pub upstream: Option<String>,
    #[n(3)]
    pub ahead: u64,
    #[n(4)]
    pub behind: u64,
    #[n(5)]
    pub changed: u32,
    #[n(6)]
    pub staged: u32,
    #[n(7)]
    pub unstaged: u32,
    #[n(8)]
    pub untracked: u32,
    #[n(9)]
    pub conflicted: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitBranch {
    #[n(0)]
    pub name: String,
    #[n(1)]
    pub current: bool,
    #[n(2)]
    pub checked_out: bool,
    #[n(3)]
    pub default: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitFile {
    #[n(0)]
    pub path: ServerPath,
    #[n(1)]
    pub original_path: Option<ServerPath>,
    #[n(2)]
    pub index: u8,
    #[n(3)]
    pub worktree: u8,
    #[n(4)]
    pub added: Option<u64>,
    #[n(5)]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent flags from Git worktree porcelain"
)]
pub struct GitWorktree {
    #[n(0)]
    pub directory: ServerPath,
    #[n(1)]
    pub head: Option<String>,
    #[n(2)]
    pub branch: Option<String>,
    #[n(3)]
    pub primary: bool,
    #[n(4)]
    pub locked: bool,
    #[n(5)]
    pub bare: bool,
    #[n(6)]
    pub detached: bool,
    #[n(7)]
    pub prunable: bool,
    #[n(8)]
    pub registered: Option<ProjectId>,
}

/// An inspection binds confirmation to the directory identity and current Git state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct WorktreeRemoval {
    #[n(0)]
    pub directory: ServerPath,
    #[n(1)]
    pub device: u64,
    #[n(2)]
    pub inode: u64,
    #[n(3)]
    pub dirty: bool,
    #[n(4)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub status: Vec<u8>,
    #[n(5)]
    pub head: Option<String>,
    #[n(6)]
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitPushDestination {
    #[n(0)]
    pub remote: String,
    #[n(1)]
    pub branch: String,
}

/// Every change as it would be committed, computed without touching the index.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitChangesPreview {
    #[n(0)]
    pub branch: Option<String>,
    #[n(1)]
    pub head: Option<String>,
    #[n(2)]
    pub tree: String,
    #[n(3)]
    pub diff: GitRawDiff,
    #[n(4)]
    pub files: Vec<GitPreviewFile>,
    #[n(5)]
    pub destination: Option<GitPushDestination>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitPreviewFile {
    #[n(0)]
    pub path: ServerPath,
    #[n(1)]
    pub added: Option<u64>,
    #[n(2)]
    pub removed: Option<u64>,
    #[n(3)]
    pub untracked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum GitBaseSwitch {
    #[n(0)]
    Updated,
    /// The base branch is checked out by another worktree, so nothing changed.
    #[n(1)]
    CheckedOutElsewhere(#[n(0)] ServerPath),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum GitReply {
    /// None means an existing directory without a Git repository.
    #[n(0)]
    Summary(#[n(0)] Option<GitSummary>),
    #[n(1)]
    Branches(#[n(0)] Vec<GitBranch>),
    #[n(2)]
    Changes(#[n(0)] Vec<GitFile>),
    #[n(3)]
    Worktrees(#[n(0)] Vec<GitWorktree>),
    #[n(4)]
    Removal(#[n(0)] WorktreeRemoval),
    #[n(5)]
    Project(#[n(0)] ProjectDescriptor),
    #[n(6)]
    Done,
    #[n(7)]
    Status(#[n(0)] Box<GitStatus>),
    #[n(8)]
    RepoInfo(#[n(0)] GitRepoInfo),
    #[n(9)]
    RemoteBranches(#[n(0)] Vec<String>),
    #[n(10)]
    Log(#[n(0)] Vec<GitCommit>),
    #[n(11)]
    RawDiff(#[n(0)] GitRawDiff),
    #[n(12)]
    Diff(#[n(0)] GitDiff),
    #[n(13)]
    Commit(#[n(0)] String),
    #[n(14)]
    PullRequest(#[n(0)] Option<Box<GitPullRequest>>),
    #[n(15)]
    PullRequestNumber(#[n(0)] Option<u64>),
    #[n(16)]
    PullRequests(#[n(0)] Vec<GitPullRequest>),
    #[n(17)]
    ChangesPreview(#[n(0)] Box<GitChangesPreview>),
    #[n(18)]
    BaseSwitch(#[n(0)] GitBaseSwitch),
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
