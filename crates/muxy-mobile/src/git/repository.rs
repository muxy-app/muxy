use muxy_client::{Client, ClientError};
use muxy_protocol::{
    GitAction, GitDiffRequest, GitReply, GitRequest, ProjectId, ReplyBody, ServerPath,
};

use super::records::{
    GitBaseSwitch, GitBranch, GitChangesPreview, GitCommit, GitDiff, GitFile, GitPushDestination,
    GitRawDiff, GitRepoInfo, GitStatus, GitSummary, collect,
};
use crate::MobileError;
use crate::records::server_path;

/// Git in one project's repository. Every call talks to the server; make them
/// on a queue of their own, because a push or pull can take minutes.
#[derive(Debug, uniffi::Object)]
pub struct GitRepository {
    pub(super) client: Client,
    pub(super) project: ProjectId,
}

impl GitRepository {
    pub(crate) fn new(client: Client, project: ProjectId) -> Self {
        Self { client, project }
    }

    pub(super) fn request(&self, action: GitAction) -> Result<GitReply, MobileError> {
        self.request_in(self.project, action)
    }

    /// Runs `action` in another project, such as a worktree's parent.
    pub(super) fn request_in(
        &self,
        project: ProjectId,
        action: GitAction,
    ) -> Result<GitReply, MobileError> {
        Ok(self.client.git(GitRequest { project, action })?)
    }

    /// Runs an action whose only answer is `Done`.
    pub(super) fn run(&self, action: GitAction) -> Result<(), MobileError> {
        match self.request(action)? {
            GitReply::Done => Ok(()),
            other => Err(unexpected(other)),
        }
    }
}

#[uniffi::export]
impl GitRepository {
    /// `None` when the project's folder has no repository.
    pub fn summary(&self) -> Result<Option<GitSummary>, MobileError> {
        match self.request(GitAction::Summary)? {
            GitReply::Summary(summary) => Ok(summary.map(Into::into)),
            other => Err(unexpected(other)),
        }
    }

    /// The branch and its changes, and, when asked, its pull request, which
    /// takes longer because it asks GitHub.
    pub fn status(&self, include_pull_request: bool) -> Result<GitStatus, MobileError> {
        let action = GitAction::Status {
            local: !include_pull_request,
        };
        match self.request(action)? {
            GitReply::Status(status) => Ok((*status).into()),
            other => Err(unexpected(other)),
        }
    }

    pub fn changes(&self) -> Result<Vec<GitFile>, MobileError> {
        match self.request(GitAction::Changes)? {
            GitReply::Changes(files) => Ok(collect(files)),
            other => Err(unexpected(other)),
        }
    }

    pub fn branches(&self) -> Result<Vec<GitBranch>, MobileError> {
        match self.request(GitAction::Branches)? {
            GitReply::Branches(branches) => Ok(collect(branches)),
            other => Err(unexpected(other)),
        }
    }

    /// The branch names on `origin`, read over the network.
    pub fn remote_branches(&self) -> Result<Vec<String>, MobileError> {
        match self.request(GitAction::RemoteBranches)? {
            GitReply::RemoteBranches(branches) => Ok(branches),
            other => Err(unexpected(other)),
        }
    }

    pub fn info(&self) -> Result<GitRepoInfo, MobileError> {
        match self.request(GitAction::RepoInfo)? {
            GitReply::RepoInfo(info) => Ok(info.into()),
            other => Err(unexpected(other)),
        }
    }

    /// Commits, newest first. `max_count` is at most 1000.
    pub fn log(&self, max_count: u32, skip: u32) -> Result<Vec<GitCommit>, MobileError> {
        match self.request(GitAction::Log { max_count, skip })? {
            GitReply::Log(commits) => Ok(collect(commits)),
            other => Err(unexpected(other)),
        }
    }

    /// One file's changes as rows to draw, from the index when `staged`.
    /// `line_limit` shortens a long diff and sets `truncated`.
    pub fn diff(
        &self,
        path: String,
        staged: bool,
        line_limit: Option<u32>,
    ) -> Result<GitDiff, MobileError> {
        let request = GitDiffRequest {
            path: Some(server_path(path)),
            staged,
            raw: false,
            line_limit,
        };
        match self.request(GitAction::Diff(request))? {
            GitReply::Diff(diff) => Ok(diff.into()),
            other => Err(unexpected(other)),
        }
    }

    /// Git's diff text for one file, or for every tracked change when `path`
    /// is `None`.
    pub fn raw_diff(
        &self,
        path: Option<String>,
        staged: bool,
        line_limit: Option<u32>,
    ) -> Result<GitRawDiff, MobileError> {
        let request = GitDiffRequest {
            path: path.map(server_path),
            staged,
            raw: true,
            line_limit,
        };
        raw_diff(self.request(GitAction::Diff(request))?)
    }

    /// The commits on this branch compared with `origin/<base>`.
    pub fn branch_diff(
        &self,
        base: String,
        line_limit: Option<u32>,
    ) -> Result<GitRawDiff, MobileError> {
        raw_diff(self.request(GitAction::BranchDiff { base, line_limit })?)
    }

    /// Every change as `commit_all` would commit it, without staging anything.
    pub fn changes_preview(
        &self,
        line_limit: Option<u32>,
    ) -> Result<GitChangesPreview, MobileError> {
        match self.request(GitAction::ChangesPreview { line_limit })? {
            GitReply::ChangesPreview(preview) => Ok((*preview).into()),
            other => Err(unexpected(other)),
        }
    }

    /// Stages the files, or every change when `paths` is empty.
    pub fn stage(&self, paths: Vec<String>) -> Result<(), MobileError> {
        self.run(GitAction::Stage(server_paths(paths)))
    }

    /// Unstages the files, or everything when `paths` is empty.
    pub fn unstage(&self, paths: Vec<String>) -> Result<(), MobileError> {
        self.run(GitAction::Unstage(server_paths(paths)))
    }

    /// Throws away the files' unstaged changes and deletes the untracked ones.
    /// Unstage or resolve a file before discarding it. Does nothing when
    /// `paths` is empty.
    pub fn discard(&self, paths: Vec<String>) -> Result<(), MobileError> {
        self.run(GitAction::Discard(server_paths(paths)))
    }

    /// Returns the new commit's hash. `stage_all` stages every change first.
    pub fn commit(&self, message: String, stage_all: bool) -> Result<String, MobileError> {
        commit_hash(self.request(GitAction::Commit { message, stage_all })?)
    }

    /// Stages every change and commits only while it still matches the
    /// `changes_preview` that `expected_tree` came from.
    pub fn commit_all(
        &self,
        message: String,
        expected_head: Option<String>,
        expected_tree: String,
    ) -> Result<String, MobileError> {
        commit_hash(self.request(GitAction::CommitAll {
            message,
            expected_head,
            expected_tree,
        })?)
    }

    /// Creates a repository in the project's folder; call `watch` again after it.
    pub fn init_repository(&self) -> Result<(), MobileError> {
        self.run(GitAction::Init)
    }

    /// Sends `GitChanged` when this repository changes. A connection watches
    /// one repository, so this replaces the previous watch.
    pub fn watch(&self) -> Result<(), MobileError> {
        self.run(GitAction::Watch)
    }

    pub fn switch_branch(&self, name: String) -> Result<(), MobileError> {
        self.run(GitAction::SwitchBranch(name))
    }

    /// Creates the branch at the current commit and switches to it.
    pub fn create_branch(&self, name: String) -> Result<(), MobileError> {
        self.run(GitAction::CreateBranch(name))
    }

    /// `force` also deletes a branch whose commits aren't merged.
    pub fn delete_branch(&self, name: String, force: bool) -> Result<(), MobileError> {
        self.run(GitAction::DeleteLocalBranch { name, force })
    }

    pub fn delete_remote_branch(&self, name: String) -> Result<(), MobileError> {
        self.run(GitAction::DeleteRemoteBranch(name))
    }

    /// Checks out a commit without a branch.
    pub fn checkout_commit(&self, hash: String) -> Result<(), MobileError> {
        self.run(GitAction::Checkout(hash))
    }

    pub fn cherry_pick(&self, hash: String) -> Result<(), MobileError> {
        self.run(GitAction::CherryPick(hash))
    }

    /// Undoes the commit's changes in the working tree, without committing.
    pub fn revert(&self, hash: String) -> Result<(), MobileError> {
        self.run(GitAction::Revert(hash))
    }

    pub fn create_tag(&self, name: String, hash: String) -> Result<(), MobileError> {
        self.run(GitAction::CreateTag { name, hash })
    }

    /// Pushes the current branch. A branch without an upstream, or any branch
    /// when `set_upstream`, goes to `origin` and tracks it.
    pub fn push(&self, set_upstream: bool) -> Result<(), MobileError> {
        self.run(GitAction::Push { set_upstream })
    }

    pub fn pull(&self) -> Result<(), MobileError> {
        self.run(GitAction::Pull)
    }

    /// Pushes `branch` where `changes_preview` said it would go.
    pub fn publish_branch(
        &self,
        branch: String,
        destination: GitPushDestination,
    ) -> Result<(), MobileError> {
        self.run(GitAction::PublishBranch {
            branch,
            destination: destination.into(),
        })
    }

    /// Switches to a merged pull request's base branch and fast-forwards it.
    pub fn switch_to_base(&self, branch: String) -> Result<GitBaseSwitch, MobileError> {
        match self.request(GitAction::SwitchToBase(branch))? {
            GitReply::BaseSwitch(base_switch) => Ok(base_switch.into()),
            other => Err(unexpected(other)),
        }
    }
}

/// A reply of another kind than the request asked for.
pub(super) fn unexpected(reply: GitReply) -> MobileError {
    ClientError::UnexpectedReply(Box::new(ReplyBody::Git(reply))).into()
}

pub(super) fn raw_diff(reply: GitReply) -> Result<GitRawDiff, MobileError> {
    match reply {
        GitReply::RawDiff(diff) => Ok(diff.into()),
        other => Err(unexpected(other)),
    }
}

fn commit_hash(reply: GitReply) -> Result<String, MobileError> {
    match reply {
        GitReply::Commit(hash) => Ok(hash),
        other => Err(unexpected(other)),
    }
}

fn server_paths(paths: Vec<String>) -> Vec<ServerPath> {
    paths.into_iter().map(server_path).collect()
}
