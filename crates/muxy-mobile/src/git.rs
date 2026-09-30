//! Git in a project's repository: one call for each action the desktop can run.

mod pull_requests;
mod records;
mod repository;
mod worktrees;

pub use records::{
    GitBaseSwitch, GitBranch, GitChangeKind, GitChangesPreview, GitChecks, GitCommit, GitDiff,
    GitDiffKind, GitDiffRow, GitFile, GitFileStatus, GitLineStat, GitMergeMethod, GitPreviewFile,
    GitPullRequest, GitPullRequestFilter, GitPushDestination, GitRawDiff, GitRef, GitRefKind,
    GitRepoInfo, GitStatus, GitSummary, GitWorktree, WorktreeRemoval,
};
pub use repository::GitRepository;
