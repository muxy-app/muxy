//! Git in a project's repository: every action the desktop can run, one to one
//! with the protocol's `GitAction` and `GitReply`.

mod action;
mod reply;

pub use action::{
    GitAction, GitDiffRequest, GitMergeMethod, GitPullRequestAction, GitPullRequestFilter,
    WorktreeAction,
};
pub use reply::{
    GitBaseSwitch, GitBranch, GitChangesPreview, GitChecks, GitCommit, GitDiff, GitDiffKind,
    GitDiffRow, GitFile, GitFileStatus, GitLineStat, GitPreviewFile, GitPullRequest,
    GitPushDestination, GitRawDiff, GitRef, GitRefKind, GitReply, GitRepoInfo, GitStatus,
    GitSummary, GitWorktree, WorktreeRemoval,
};
