//! What a phone can ask of a project's repository, mirroring `muxy_protocol::GitAction`.

use muxy_protocol::{self as protocol, OperationId, ProjectId};

use super::reply::{GitPushDestination, WorktreeRemoval};
use crate::records::server_path;

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum GitAction {
    Summary,
    Branches,
    Changes,
    SwitchBranch {
        name: String,
    },
    CreateBranch {
        name: String,
    },
    DeleteBranch {
        name: String,
    },
    Stage {
        paths: Vec<String>,
    },
    Unstage {
        paths: Vec<String>,
    },
    Discard {
        paths: Vec<String>,
    },
    Worktrees,
    InspectRemoval,
    Worktree {
        action: WorktreeAction,
    },
    /// Sends `GitChanged` when this project's repository changes, replacing
    /// the connection's previous Git watch.
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
    Diff {
        request: GitDiffRequest,
    },
    Init,
    Commit {
        message: String,
        stage_all: bool,
    },
    Push {
        set_upstream: bool,
    },
    Pull,
    Checkout {
        hash: String,
    },
    CherryPick {
        hash: String,
    },
    Revert {
        hash: String,
    },
    DeleteLocalBranch {
        name: String,
        force: bool,
    },
    DeleteRemoteBranch {
        name: String,
    },
    CreateTag {
        name: String,
        hash: String,
    },
    PullRequest {
        action: GitPullRequestAction,
    },
    /// Committed changes on this branch compared with an origin branch.
    BranchDiff {
        base: String,
        line_limit: Option<u32>,
    },
    /// Every working-tree change, without touching the index.
    ChangesPreview {
        line_limit: Option<u32>,
    },
    /// Stages everything and commits only while the tree still matches its preview.
    CommitAll {
        message: String,
        expected_head: Option<String>,
        expected_tree: String,
    },
    PublishBranch {
        branch: String,
        destination: GitPushDestination,
    },
    /// Switches to a merged pull request's base branch and fast-forwards it.
    SwitchToBase {
        branch: String,
    },
}

/// Run the adding actions in the parent project, and `Remove` in the
/// worktree's own project. `directory` is an absolute path on the computer. The
/// SDK chooses the new worktree project's id, and `GitReply::Project` returns it.
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum WorktreeAction {
    /// Checks out `branch`, creating it from `base` when one is given.
    Create {
        directory: String,
        branch: String,
        base: Option<String>,
    },
    Register {
        directory: String,
    },
    /// Removes the worktree that `InspectRemoval` described, if it is unchanged.
    Remove {
        expected: WorktreeRemoval,
    },
    CheckoutPullRequest {
        directory: String,
        number: u64,
    },
}

/// One file's diff when `path` is set; the whole diff needs `raw`.
#[derive(Clone, Debug, Default, Eq, PartialEq, uniffi::Record)]
pub struct GitDiffRequest {
    pub path: Option<String>,
    /// The index rather than the working tree.
    pub staged: bool,
    pub raw: bool,
    pub line_limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum GitPullRequestAction {
    Info,
    Number,
    Diff {
        number: u64,
        line_limit: Option<u32>,
    },
    List {
        filter: GitPullRequestFilter,
        limit: u32,
        checks: bool,
    },
    Create {
        title: String,
        body: String,
        base_branch: Option<String>,
        draft: bool,
    },
    /// Refuses the merge when the pull request's head moved past `expected_head`.
    Merge {
        number: u64,
        method: GitMergeMethod,
        delete_branch: bool,
        expected_head: Option<String>,
    },
    Close {
        number: u64,
    },
    Checkout {
        number: u64,
    },
    UpdateBranch {
        number: u64,
        expected_head: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum GitPullRequestFilter {
    Open,
    Closed,
    Merged,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum GitMergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl From<GitAction> for protocol::GitAction {
    fn from(action: GitAction) -> Self {
        let paths = |paths: Vec<String>| paths.into_iter().map(server_path).collect();
        match action {
            GitAction::Summary => Self::Summary,
            GitAction::Branches => Self::Branches,
            GitAction::Changes => Self::Changes,
            GitAction::SwitchBranch { name } => Self::SwitchBranch(name),
            GitAction::CreateBranch { name } => Self::CreateBranch(name),
            GitAction::DeleteBranch { name } => Self::DeleteBranch(name),
            GitAction::Stage { paths: values } => Self::Stage(paths(values)),
            GitAction::Unstage { paths: values } => Self::Unstage(paths(values)),
            GitAction::Discard { paths: values } => Self::Discard(paths(values)),
            GitAction::Worktrees => Self::Worktrees,
            GitAction::InspectRemoval => Self::InspectRemoval,
            GitAction::Worktree { action } => Self::Worktree(protocol::WorktreeIntent {
                operation: OperationId::new(),
                action: action.into(),
            }),
            GitAction::Watch => Self::Watch,
            GitAction::Status { local } => Self::Status { local },
            GitAction::RepoInfo => Self::RepoInfo,
            GitAction::RemoteBranches => Self::RemoteBranches,
            GitAction::Log { max_count, skip } => Self::Log { max_count, skip },
            GitAction::Diff { request } => Self::Diff(request.into()),
            GitAction::Init => Self::Init,
            GitAction::Commit { message, stage_all } => Self::Commit { message, stage_all },
            GitAction::Push { set_upstream } => Self::Push { set_upstream },
            GitAction::Pull => Self::Pull,
            GitAction::Checkout { hash } => Self::Checkout(hash),
            GitAction::CherryPick { hash } => Self::CherryPick(hash),
            GitAction::Revert { hash } => Self::Revert(hash),
            GitAction::DeleteLocalBranch { name, force } => Self::DeleteLocalBranch { name, force },
            GitAction::DeleteRemoteBranch { name } => Self::DeleteRemoteBranch(name),
            GitAction::CreateTag { name, hash } => Self::CreateTag { name, hash },
            GitAction::PullRequest { action } => Self::PullRequest(action.into()),
            GitAction::BranchDiff { base, line_limit } => Self::BranchDiff { base, line_limit },
            GitAction::ChangesPreview { line_limit } => Self::ChangesPreview { line_limit },
            GitAction::CommitAll {
                message,
                expected_head,
                expected_tree,
            } => Self::CommitAll {
                message,
                expected_head,
                expected_tree,
            },
            GitAction::PublishBranch {
                branch,
                destination,
            } => Self::PublishBranch {
                branch,
                destination: destination.into(),
            },
            GitAction::SwitchToBase { branch } => Self::SwitchToBase(branch),
        }
    }
}

impl From<WorktreeAction> for protocol::WorktreeAction {
    fn from(action: WorktreeAction) -> Self {
        match action {
            WorktreeAction::Create {
                directory,
                branch,
                base,
            } => Self::Create {
                project: ProjectId::new(),
                directory: server_path(directory),
                branch,
                base,
            },
            WorktreeAction::Register { directory } => Self::Register {
                project: ProjectId::new(),
                directory: server_path(directory),
            },
            WorktreeAction::Remove { expected } => Self::Remove {
                expected: expected.into(),
            },
            WorktreeAction::CheckoutPullRequest { directory, number } => {
                Self::CheckoutPullRequest {
                    project: ProjectId::new(),
                    directory: server_path(directory),
                    number,
                }
            }
        }
    }
}

impl From<GitDiffRequest> for protocol::GitDiffRequest {
    fn from(request: GitDiffRequest) -> Self {
        let GitDiffRequest {
            path,
            staged,
            raw,
            line_limit,
        } = request;
        Self {
            path: path.map(server_path),
            staged,
            raw,
            line_limit,
        }
    }
}

impl From<GitPullRequestAction> for protocol::GitPullRequestAction {
    fn from(action: GitPullRequestAction) -> Self {
        match action {
            GitPullRequestAction::Info => Self::Info,
            GitPullRequestAction::Number => Self::Number,
            GitPullRequestAction::Diff { number, line_limit } => Self::Diff { number, line_limit },
            GitPullRequestAction::List {
                filter,
                limit,
                checks,
            } => Self::List {
                filter: match filter {
                    GitPullRequestFilter::Open => protocol::GitPullRequestFilter::Open,
                    GitPullRequestFilter::Closed => protocol::GitPullRequestFilter::Closed,
                    GitPullRequestFilter::Merged => protocol::GitPullRequestFilter::Merged,
                    GitPullRequestFilter::All => protocol::GitPullRequestFilter::All,
                },
                limit,
                checks,
            },
            GitPullRequestAction::Create {
                title,
                body,
                base_branch,
                draft,
            } => Self::Create {
                title,
                body,
                base_branch,
                draft,
            },
            GitPullRequestAction::Merge {
                number,
                method,
                delete_branch,
                expected_head,
            } => Self::Merge {
                number,
                method: match method {
                    GitMergeMethod::Merge => protocol::GitMergeMethod::Merge,
                    GitMergeMethod::Squash => protocol::GitMergeMethod::Squash,
                    GitMergeMethod::Rebase => protocol::GitMergeMethod::Rebase,
                },
                delete_branch,
                expected_head,
            },
            GitPullRequestAction::Close { number } => Self::Close { number },
            GitPullRequestAction::Checkout { number } => Self::Checkout { number },
            GitPullRequestAction::UpdateBranch {
                number,
                expected_head,
            } => Self::UpdateBranch {
                number,
                expected_head,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_new_worktree_gets_its_own_project_and_operation() {
        let register = || {
            protocol::GitAction::from(GitAction::Worktree {
                action: WorktreeAction::Register {
                    directory: "/tmp/worktree".into(),
                },
            })
        };
        let intent = |action| match action {
            protocol::GitAction::Worktree(intent) => intent,
            other => panic!("expected a worktree intent, got {other:?}"),
        };
        let (first, second) = (intent(register()), intent(register()));
        let project = |intent: &protocol::WorktreeIntent| match &intent.action {
            protocol::WorktreeAction::Register { project, directory } => {
                assert_eq!(directory, &server_path("/tmp/worktree".into()));
                *project
            }
            other => panic!("expected a registration, got {other:?}"),
        };
        assert_ne!(project(&first), project(&second));
        assert_ne!(first.operation, second.operation);
    }
}
