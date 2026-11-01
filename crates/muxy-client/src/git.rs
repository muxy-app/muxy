use std::time::Duration;

use muxy_protocol::{
    GitAction, GitBaseSwitch, GitBranch, GitChangesPreview, GitCommit, GitDiff, GitDiffRequest,
    GitFile, GitMergeMethod, GitPullRequest, GitPullRequestAction, GitPullRequestFilter,
    GitPushDestination, GitRawDiff, GitReply, GitRepoInfo, GitRequest, GitStatus, GitSummary,
    GitWorktree, OperationId, ProjectDescriptor, ProjectId, ReplyBody, RequestBody, ServerPath,
    WorktreeAction, WorktreeHook, WorktreeIntent, WorktreeOptions, WorktreeRemoval,
};

use crate::typed::{decoders, typed_requests};
use crate::{Client, ClientError, Request};

/// A worktree project that a create, register or pull request checkout made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddedWorktree {
    pub project: ProjectDescriptor,
    /// Why its setup commands failed. The worktree is still there.
    pub setup_failure: Option<String>,
}

pub(crate) fn git_timeout(action: &GitAction) -> Duration {
    let hooks = match action {
        GitAction::Worktree(intent) => intent
            .options
            .as_ref()
            .and_then(|options| options.hooks.as_ref())
            .is_some_and(|hooks| !hooks.is_empty()),
        _ => false,
    };
    // Hooks have their own five-minute server budget, in addition to Git work.
    // Leave reply headroom so the UI receives the final lifecycle result.
    Duration::from_secs(if hooks { 610 } else { 300 })
}

impl Client {
    /// Any Git action, answered with whatever the server sends, for callers
    /// that handle actions as data.
    pub fn git(&self, request: GitRequest) -> Result<GitReply, ClientError> {
        self.send_git(request.project, request.action, any)
    }

    /// Like `git`, without blocking.
    pub fn git_async(&self, request: GitRequest) -> Request<GitReply> {
        self.send_git_async(request.project, request.action, any)
    }

    fn send_git<T>(
        &self,
        project: ProjectId,
        action: GitAction,
        decode: fn(ReplyBody) -> Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        let timeout = git_timeout(&action);
        decode(
            self.request_with_timeout(RequestBody::Git(GitRequest { project, action }), timeout)?,
        )
    }

    fn send_git_async<T>(
        &self,
        project: ProjectId,
        action: GitAction,
        decode: fn(ReplyBody) -> Result<T, ClientError>,
    ) -> Request<T> {
        let timeout = git_timeout(&action);
        self.request_async(
            RequestBody::Git(GitRequest { project, action }),
            timeout,
            decode,
        )
    }
}

decoders! { Git;
    any -> GitReply { reply => reply }
    done -> () { GitReply::Done => () }
    summary -> Option<GitSummary> { GitReply::Summary(summary) => summary }
    branches -> Vec<GitBranch> { GitReply::Branches(branches) => branches }
    changes -> Vec<GitFile> { GitReply::Changes(files) => files }
    worktrees -> Vec<GitWorktree> { GitReply::Worktrees(worktrees) => worktrees }
    removal -> WorktreeRemoval { GitReply::Removal(removal) => removal }
    added_worktree -> AddedWorktree {
        GitReply::Project(project) => AddedWorktree { project, setup_failure: None },
        GitReply::WorktreeSetupFailed { project, message } => AddedWorktree {
            project,
            setup_failure: Some(message),
        },
    }
    status -> GitStatus { GitReply::Status(status) => *status }
    repo_info -> GitRepoInfo { GitReply::RepoInfo(info) => info }
    remote_branches -> Vec<String> { GitReply::RemoteBranches(branches) => branches }
    log -> Vec<GitCommit> { GitReply::Log(commits) => commits }
    diff -> GitDiff { GitReply::Diff(diff) => diff }
    raw_diff -> GitRawDiff { GitReply::RawDiff(diff) => diff }
    commit -> String { GitReply::Commit(hash) => hash }
    pull_request -> Option<GitPullRequest> { GitReply::PullRequest(found) => found.map(|pr| *pr) }
    created_pull_request -> GitPullRequest { GitReply::PullRequest(Some(created)) => *created }
    pull_request_number -> Option<u64> { GitReply::PullRequestNumber(number) => number }
    pull_requests -> Vec<GitPullRequest> { GitReply::PullRequests(found) => found }
    changes_preview -> GitChangesPreview { GitReply::ChangesPreview(preview) => *preview }
    base_switch -> GitBaseSwitch { GitReply::BaseSwitch(switch) => switch }
    worktree_hooks -> Vec<WorktreeHook> { GitReply::WorktreeHooks(hooks) => hooks }
}

typed_requests! { send_git, send_git_async;
    /// `None` when the project's folder has no repository.
    git_summary, git_summary_async() -> Option<GitSummary> = GitAction::Summary => summary;
    git_branches, git_branches_async() -> Vec<GitBranch> = GitAction::Branches => branches;
    git_changes, git_changes_async() -> Vec<GitFile> = GitAction::Changes => changes;
    git_switch_branch, git_switch_branch_async(name: String) -> ()
        = GitAction::SwitchBranch(name) => done;
    git_create_branch, git_create_branch_async(name: String) -> ()
        = GitAction::CreateBranch(name) => done;
    git_delete_branch, git_delete_branch_async(name: String) -> ()
        = GitAction::DeleteBranch(name) => done;
    git_stage, git_stage_async(paths: Vec<ServerPath>) -> () = GitAction::Stage(paths) => done;
    git_unstage, git_unstage_async(paths: Vec<ServerPath>) -> ()
        = GitAction::Unstage(paths) => done;
    git_discard, git_discard_async(paths: Vec<ServerPath>) -> ()
        = GitAction::Discard(paths) => done;
    git_worktrees, git_worktrees_async() -> Vec<GitWorktree> = GitAction::Worktrees => worktrees;
    /// Describes this worktree project, to confirm its removal.
    git_inspect_removal, git_inspect_removal_async() -> WorktreeRemoval
        = GitAction::InspectRemoval => removal;
    /// Creates or registers a worktree, or checks out a pull request in one.
    git_add_worktree, git_add_worktree_async(intent: WorktreeIntent) -> AddedWorktree
        = GitAction::Worktree(intent) => added_worktree;
    /// Removes this worktree project if it still matches `expected`, running
    /// the approved teardown `hooks`.
    git_remove_worktree, git_remove_worktree_async(
        expected: WorktreeRemoval,
        hooks: Option<Vec<WorktreeHook>>
    ) -> () = GitAction::Worktree(WorktreeIntent {
        operation: OperationId::new(),
        action: WorktreeAction::Remove { expected },
        options: hooks.map(|hooks| WorktreeOptions { name: None, hooks: Some(hooks) }),
    }) => done;
    /// Replaces this connection's repository watch with this project's.
    git_watch, git_watch_async() -> () = GitAction::Watch => done;
    git_status, git_status_async(local: bool, fresh: bool) -> GitStatus
        = GitAction::Status { local, fresh } => status;
    git_repo_info, git_repo_info_async() -> GitRepoInfo = GitAction::RepoInfo => repo_info;
    git_remote_branches, git_remote_branches_async() -> Vec<String>
        = GitAction::RemoteBranches => remote_branches;
    git_log, git_log_async(max_count: u32, skip: u32) -> Vec<GitCommit>
        = GitAction::Log { max_count, skip } => log;
    /// One file's changes as rows.
    git_diff, git_diff_async(path: ServerPath, staged: bool, line_limit: Option<u32>) -> GitDiff
        = GitAction::Diff(GitDiffRequest { path: Some(path), staged, raw: false, line_limit })
        => diff;
    /// Git's diff text for one file, or for every tracked change.
    git_raw_diff, git_raw_diff_async(
        path: Option<ServerPath>,
        staged: bool,
        line_limit: Option<u32>
    ) -> GitRawDiff = GitAction::Diff(GitDiffRequest { path, staged, raw: true, line_limit })
        => raw_diff;
    git_init, git_init_async() -> () = GitAction::Init => done;
    /// Returns the new commit's hash.
    git_commit, git_commit_async(message: String, stage_all: bool) -> String
        = GitAction::Commit { message, stage_all } => commit;
    git_push, git_push_async(set_upstream: bool) -> () = GitAction::Push { set_upstream } => done;
    git_pull, git_pull_async() -> () = GitAction::Pull => done;
    git_checkout, git_checkout_async(hash: String) -> () = GitAction::Checkout(hash) => done;
    git_cherry_pick, git_cherry_pick_async(hash: String) -> ()
        = GitAction::CherryPick(hash) => done;
    git_revert, git_revert_async(hash: String) -> () = GitAction::Revert(hash) => done;
    git_delete_local_branch, git_delete_local_branch_async(name: String, force: bool) -> ()
        = GitAction::DeleteLocalBranch { name, force } => done;
    git_delete_remote_branch, git_delete_remote_branch_async(name: String) -> ()
        = GitAction::DeleteRemoteBranch(name) => done;
    git_create_tag, git_create_tag_async(name: String, hash: String) -> ()
        = GitAction::CreateTag { name, hash } => done;
    /// The current branch's open pull request, if any.
    git_pull_request, git_pull_request_async() -> Option<GitPullRequest>
        = GitAction::PullRequest(GitPullRequestAction::Info) => pull_request;
    git_pull_request_number, git_pull_request_number_async() -> Option<u64>
        = GitAction::PullRequest(GitPullRequestAction::Number) => pull_request_number;
    git_pull_request_diff, git_pull_request_diff_async(number: u64, line_limit: Option<u32>)
        -> GitRawDiff = GitAction::PullRequest(GitPullRequestAction::Diff { number, line_limit })
        => raw_diff;
    git_pull_requests, git_pull_requests_async(
        filter: GitPullRequestFilter,
        limit: u32,
        checks: bool
    ) -> Vec<GitPullRequest>
        = GitAction::PullRequest(GitPullRequestAction::List { filter, limit, checks })
        => pull_requests;
    git_create_pull_request, git_create_pull_request_async(
        title: String,
        body: String,
        base_branch: Option<String>,
        draft: bool
    ) -> GitPullRequest = GitAction::PullRequest(GitPullRequestAction::Create {
        title,
        body,
        base_branch,
        draft,
    }) => created_pull_request;
    /// Refuses when the pull request's head moved past `expected_head`.
    git_merge_pull_request, git_merge_pull_request_async(
        number: u64,
        method: GitMergeMethod,
        delete_branch: bool,
        expected_head: Option<String>
    ) -> () = GitAction::PullRequest(GitPullRequestAction::Merge {
        number,
        method,
        delete_branch,
        expected_head,
    }) => done;
    git_close_pull_request, git_close_pull_request_async(number: u64) -> ()
        = GitAction::PullRequest(GitPullRequestAction::Close { number }) => done;
    git_checkout_pull_request, git_checkout_pull_request_async(number: u64) -> ()
        = GitAction::PullRequest(GitPullRequestAction::Checkout { number }) => done;
    git_update_pull_request_branch, git_update_pull_request_branch_async(
        number: u64,
        expected_head: String
    ) -> () = GitAction::PullRequest(GitPullRequestAction::UpdateBranch {
        number,
        expected_head,
    }) => done;
    /// The commits on this branch compared with `origin/<base>`.
    git_branch_diff, git_branch_diff_async(base: String, line_limit: Option<u32>) -> GitRawDiff
        = GitAction::BranchDiff { base, line_limit } => raw_diff;
    /// Every change as it would be committed, without touching the index.
    git_changes_preview, git_changes_preview_async(line_limit: Option<u32>) -> GitChangesPreview
        = GitAction::ChangesPreview { line_limit } => changes_preview;
    /// Stages and commits every change while it still matches its preview.
    git_commit_all, git_commit_all_async(
        message: String,
        expected_head: Option<String>,
        expected_tree: String
    ) -> String = GitAction::CommitAll { message, expected_head, expected_tree } => commit;
    git_publish_branch, git_publish_branch_async(
        branch: String,
        destination: GitPushDestination
    ) -> () = GitAction::PublishBranch { branch, destination } => done;
    /// Switches to a merged pull request's base branch and fast-forwards it.
    git_switch_to_base, git_switch_to_base_async(branch: String) -> GitBaseSwitch
        = GitAction::SwitchToBase(branch) => base_switch;
    git_worktree_hooks, git_worktree_hooks_async(teardown: bool) -> Vec<WorktreeHook>
        = GitAction::WorktreeHooks { teardown } => worktree_hooks;
    /// Only the staged changes, without touching the index.
    git_staged_preview, git_staged_preview_async(line_limit: Option<u32>) -> GitChangesPreview
        = GitAction::StagedPreview { line_limit } => changes_preview;
    /// Commits the index while it still matches its staged preview.
    git_commit_staged, git_commit_staged_async(
        message: String,
        expected_head: Option<String>,
        expected_tree: String
    ) -> String = GitAction::CommitStaged { message, expected_head, expected_tree } => commit;
}

#[cfg(test)]
mod tests;
