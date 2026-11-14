use muxy_client::Client;
use muxy_protocol::{ProjectId, ServerPath};

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
}

#[uniffi::export]
impl GitRepository {
    /// `None` when the project's folder has no repository.
    pub fn summary(&self) -> Result<Option<GitSummary>, MobileError> {
        Ok(self.client.git_summary(self.project)?.map(Into::into))
    }

    /// The branch and its changes, and, when asked, its pull request, which
    /// takes longer because it asks GitHub.
    pub fn status(&self, include_pull_request: bool) -> Result<GitStatus, MobileError> {
        Ok(self
            .client
            .git_status(self.project, !include_pull_request, false)?
            .into())
    }

    pub fn changes(&self) -> Result<Vec<GitFile>, MobileError> {
        Ok(collect(self.client.git_changes(self.project)?))
    }

    pub fn branches(&self) -> Result<Vec<GitBranch>, MobileError> {
        Ok(collect(self.client.git_branches(self.project)?))
    }

    /// The branch names on `origin`, read over the network.
    pub fn remote_branches(&self) -> Result<Vec<String>, MobileError> {
        Ok(self.client.git_remote_branches(self.project)?)
    }

    pub fn info(&self) -> Result<GitRepoInfo, MobileError> {
        Ok(self.client.git_repo_info(self.project)?.into())
    }

    /// Commits, newest first. `max_count` is at most 1000.
    pub fn log(&self, max_count: u32, skip: u32) -> Result<Vec<GitCommit>, MobileError> {
        Ok(collect(self.client.git_log(
            self.project,
            max_count,
            skip,
        )?))
    }

    /// One file's changes as rows to draw, from the index when `staged`.
    /// `line_limit` shortens a long diff and sets `truncated`.
    pub fn diff(
        &self,
        path: String,
        staged: bool,
        line_limit: Option<u32>,
    ) -> Result<GitDiff, MobileError> {
        Ok(self
            .client
            .git_diff(self.project, server_path(path), staged, line_limit)?
            .into())
    }

    /// Git's diff text for one file, or for every tracked change when `path`
    /// is `None`.
    pub fn raw_diff(
        &self,
        path: Option<String>,
        staged: bool,
        line_limit: Option<u32>,
    ) -> Result<GitRawDiff, MobileError> {
        Ok(self
            .client
            .git_raw_diff(self.project, path.map(server_path), staged, line_limit)?
            .into())
    }

    /// The commits on this branch compared with `origin/<base>`.
    pub fn branch_diff(
        &self,
        base: String,
        line_limit: Option<u32>,
    ) -> Result<GitRawDiff, MobileError> {
        Ok(self
            .client
            .git_branch_diff(self.project, base, line_limit)?
            .into())
    }

    /// Every change as `commit_all` would commit it, without staging anything.
    pub fn changes_preview(
        &self,
        line_limit: Option<u32>,
    ) -> Result<GitChangesPreview, MobileError> {
        Ok(self
            .client
            .git_changes_preview(self.project, line_limit)?
            .into())
    }

    /// Stages the files, or every change when `paths` is empty.
    pub fn stage(&self, paths: Vec<String>) -> Result<(), MobileError> {
        Ok(self.client.git_stage(self.project, server_paths(paths))?)
    }

    /// Unstages the files, or everything when `paths` is empty.
    pub fn unstage(&self, paths: Vec<String>) -> Result<(), MobileError> {
        Ok(self.client.git_unstage(self.project, server_paths(paths))?)
    }

    /// Throws away the files' unstaged changes and deletes the untracked ones.
    /// Unstage or resolve a file before discarding it. Does nothing when
    /// `paths` is empty.
    pub fn discard(&self, paths: Vec<String>) -> Result<(), MobileError> {
        Ok(self.client.git_discard(self.project, server_paths(paths))?)
    }

    /// Returns the new commit's hash. `stage_all` stages every change first.
    pub fn commit(&self, message: String, stage_all: bool) -> Result<String, MobileError> {
        Ok(self.client.git_commit(self.project, message, stage_all)?)
    }

    /// Stages every change and commits only while it still matches the
    /// `changes_preview` that `expected_tree` came from.
    pub fn commit_all(
        &self,
        message: String,
        expected_head: Option<String>,
        expected_tree: String,
    ) -> Result<String, MobileError> {
        Ok(self
            .client
            .git_commit_all(self.project, message, expected_head, expected_tree)?)
    }

    /// Creates a repository in the project's folder; call `watch` again after it.
    pub fn init_repository(&self) -> Result<(), MobileError> {
        Ok(self.client.git_init(self.project)?)
    }

    /// Sends `GitChanged` when this repository changes. A connection watches
    /// one repository, so this replaces the previous watch.
    pub fn watch(&self) -> Result<(), MobileError> {
        Ok(self.client.git_watch(self.project)?)
    }

    pub fn switch_branch(&self, name: String) -> Result<(), MobileError> {
        Ok(self.client.git_switch_branch(self.project, name)?)
    }

    /// Creates the branch at the current commit and switches to it.
    pub fn create_branch(&self, name: String) -> Result<(), MobileError> {
        Ok(self.client.git_create_branch(self.project, name)?)
    }

    /// `force` also deletes a branch whose commits aren't merged.
    pub fn delete_branch(&self, name: String, force: bool) -> Result<(), MobileError> {
        Ok(self
            .client
            .git_delete_local_branch(self.project, name, force)?)
    }

    pub fn delete_remote_branch(&self, name: String) -> Result<(), MobileError> {
        Ok(self.client.git_delete_remote_branch(self.project, name)?)
    }

    /// Checks out a commit without a branch.
    pub fn checkout_commit(&self, hash: String) -> Result<(), MobileError> {
        Ok(self.client.git_checkout(self.project, hash)?)
    }

    pub fn cherry_pick(&self, hash: String) -> Result<(), MobileError> {
        Ok(self.client.git_cherry_pick(self.project, hash)?)
    }

    /// Undoes the commit's changes in the working tree, without committing.
    pub fn revert(&self, hash: String) -> Result<(), MobileError> {
        Ok(self.client.git_revert(self.project, hash)?)
    }

    pub fn create_tag(&self, name: String, hash: String) -> Result<(), MobileError> {
        Ok(self.client.git_create_tag(self.project, name, hash)?)
    }

    /// Pushes the current branch. A branch without an upstream, or any branch
    /// when `set_upstream`, goes to `origin` and tracks it.
    pub fn push(&self, set_upstream: bool) -> Result<(), MobileError> {
        Ok(self.client.git_push(self.project, set_upstream)?)
    }

    pub fn pull(&self) -> Result<(), MobileError> {
        Ok(self.client.git_pull(self.project)?)
    }

    /// Pushes `branch` where `changes_preview` said it would go.
    pub fn publish_branch(
        &self,
        branch: String,
        destination: GitPushDestination,
    ) -> Result<(), MobileError> {
        Ok(self
            .client
            .git_publish_branch(self.project, branch, destination.into())?)
    }

    /// Switches to a merged pull request's base branch and fast-forwards it.
    pub fn switch_to_base(&self, branch: String) -> Result<GitBaseSwitch, MobileError> {
        Ok(self.client.git_switch_to_base(self.project, branch)?.into())
    }
}

fn server_paths(paths: Vec<String>) -> Vec<ServerPath> {
    paths.into_iter().map(server_path).collect()
}
