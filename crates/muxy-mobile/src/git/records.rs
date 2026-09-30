//! What the Git calls return, converted from the protocol's types.

use muxy_protocol as protocol;

use crate::records::{server_path, text};

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
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

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    pub checked_out: bool,
    pub default: bool,
}

/// A changed file. `staged` is what the next commit takes; `unstaged` is the
/// rest, and holds untracked and conflicted files.
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitFile {
    pub path: String,
    /// The path before a rename or copy.
    pub original_path: Option<String>,
    pub staged: Option<GitChangeKind>,
    pub unstaged: Option<GitChangeKind>,
    /// Lines added and removed, staged and unstaged together.
    pub added: Option<u64>,
    pub removed: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum GitChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    /// Became a symbolic link, or stopped being one.
    TypeChanged,
    Untracked,
    /// A merge left conflicts to resolve.
    Conflicted,
    /// A change this SDK doesn't know.
    Other,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitWorktree {
    pub directory: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub primary: bool,
    pub locked: bool,
    pub bare: bool,
    pub detached: bool,
    pub prunable: bool,
    /// The worktree's project, if it has one.
    pub registered: Option<String>,
}

/// Binds a removal to the worktree as it was inspected; pass it back unchanged.
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct WorktreeRemoval {
    pub directory: String,
    pub device: u64,
    pub inode: u64,
    pub dirty: bool,
    pub status: Vec<u8>,
    pub head: Option<String>,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitPushDestination {
    pub remote: String,
    pub branch: String,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitChangesPreview {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub tree: String,
    pub diff: GitRawDiff,
    pub files: Vec<GitPreviewFile>,
    pub destination: Option<GitPushDestination>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitPreviewFile {
    pub path: String,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub untracked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum GitBaseSwitch {
    Updated,
    /// Another worktree has the base branch checked out, so nothing changed.
    CheckedOutElsewhere {
        directory: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitRepoInfo {
    pub root: String,
    pub git_dir: String,
    pub is_worktree: bool,
    pub current_branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitStatus {
    pub summary: GitSummary,
    pub default_branch: Option<String>,
    pub branches: Vec<GitBranch>,
    pub files: Vec<GitFileStatus>,
    pub pull_request: Option<GitPullRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitFileStatus {
    pub file: GitFile,
    pub staged_lines: GitLineStat,
    pub unstaged_lines: GitLineStat,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitLineStat {
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub binary: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitRawDiff {
    pub diff: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitDiff {
    pub rows: Vec<GitDiffRow>,
    pub additions: u64,
    pub deletions: u64,
    pub truncated: bool,
    /// Git compared the file as binary, so there are no rows.
    pub binary: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitDiffRow {
    pub kind: GitDiffKind,
    pub old_line_number: Option<u64>,
    pub new_line_number: Option<u64>,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum GitDiffKind {
    Hunk,
    Context,
    Addition,
    Deletion,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub subject: String,
    pub author_name: String,
    pub author_date: String,
    pub parent_hashes: Vec<String>,
    pub refs: Vec<GitRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitRef {
    pub name: String,
    pub kind: GitRefKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum GitRefKind {
    LocalBranch,
    RemoteBranch,
    Tag,
    Head,
    /// A kind of reference added in a newer server.
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitPullRequest {
    pub number: u64,
    pub url: String,
    pub title: String,
    pub author: String,
    pub head_branch: String,
    pub head_oid: String,
    pub base_branch: String,
    pub state: String,
    pub draft: bool,
    pub updated_at: Option<String>,
    pub mergeable: Option<bool>,
    pub merge_state: String,
    pub cross_repository: bool,
    pub checks: GitChecks,
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

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct GitChecks {
    pub passing: u32,
    pub failing: u32,
    pub pending: u32,
}

pub(super) fn collect<T, U: From<T>>(items: Vec<T>) -> Vec<U> {
    items.into_iter().map(U::from).collect()
}

impl From<protocol::GitSummary> for GitSummary {
    fn from(summary: protocol::GitSummary) -> Self {
        let protocol::GitSummary {
            branch,
            head,
            upstream,
            ahead,
            behind,
            changed,
            staged,
            unstaged,
            untracked,
            conflicted,
        } = summary;
        Self {
            branch,
            head,
            upstream,
            ahead,
            behind,
            changed,
            staged,
            unstaged,
            untracked,
            conflicted,
        }
    }
}

impl From<protocol::GitBranch> for GitBranch {
    fn from(branch: protocol::GitBranch) -> Self {
        let protocol::GitBranch {
            name,
            current,
            checked_out,
            default,
        } = branch;
        Self {
            name,
            current,
            checked_out,
            default,
        }
    }
}

impl From<protocol::GitFile> for GitFile {
    fn from(file: protocol::GitFile) -> Self {
        let (staged, unstaged) = if file.conflicted() {
            (None, Some(GitChangeKind::Conflicted))
        } else if file.untracked() {
            (None, Some(GitChangeKind::Untracked))
        } else {
            (change(file.index), change(file.worktree))
        };
        let protocol::GitFile {
            path,
            original_path,
            added,
            removed,
            ..
        } = file;
        Self {
            path: text(&path),
            original_path: original_path.as_ref().map(text),
            staged,
            unstaged,
            added,
            removed,
        }
    }
}

/// Reads one of Git's short status letters, where a space means unchanged.
fn change(letter: u8) -> Option<GitChangeKind> {
    Some(match letter {
        b' ' => return None,
        b'A' => GitChangeKind::Added,
        b'M' => GitChangeKind::Modified,
        b'D' => GitChangeKind::Deleted,
        b'R' => GitChangeKind::Renamed,
        b'C' => GitChangeKind::Copied,
        b'T' => GitChangeKind::TypeChanged,
        _ => GitChangeKind::Other,
    })
}

impl From<protocol::GitWorktree> for GitWorktree {
    fn from(worktree: protocol::GitWorktree) -> Self {
        let protocol::GitWorktree {
            directory,
            head,
            branch,
            primary,
            locked,
            bare,
            detached,
            prunable,
            registered,
        } = worktree;
        Self {
            directory: text(&directory),
            head,
            branch,
            primary,
            locked,
            bare,
            detached,
            prunable,
            registered: registered.map(|project| project.to_string()),
        }
    }
}

impl From<protocol::WorktreeRemoval> for WorktreeRemoval {
    fn from(removal: protocol::WorktreeRemoval) -> Self {
        let protocol::WorktreeRemoval {
            directory,
            device,
            inode,
            dirty,
            status,
            head,
            branch,
        } = removal;
        Self {
            directory: text(&directory),
            device,
            inode,
            dirty,
            status,
            head,
            branch,
        }
    }
}

impl From<WorktreeRemoval> for protocol::WorktreeRemoval {
    fn from(removal: WorktreeRemoval) -> Self {
        let WorktreeRemoval {
            directory,
            device,
            inode,
            dirty,
            status,
            head,
            branch,
        } = removal;
        Self {
            directory: server_path(directory),
            device,
            inode,
            dirty,
            status,
            head,
            branch,
        }
    }
}

impl From<protocol::GitPushDestination> for GitPushDestination {
    fn from(destination: protocol::GitPushDestination) -> Self {
        let protocol::GitPushDestination { remote, branch } = destination;
        Self { remote, branch }
    }
}

impl From<GitPushDestination> for protocol::GitPushDestination {
    fn from(destination: GitPushDestination) -> Self {
        let GitPushDestination { remote, branch } = destination;
        Self { remote, branch }
    }
}

impl From<protocol::GitChangesPreview> for GitChangesPreview {
    fn from(preview: protocol::GitChangesPreview) -> Self {
        let protocol::GitChangesPreview {
            branch,
            head,
            tree,
            diff,
            files,
            destination,
        } = preview;
        Self {
            branch,
            head,
            tree,
            diff: diff.into(),
            files: collect(files),
            destination: destination.map(Into::into),
        }
    }
}

impl From<protocol::GitPreviewFile> for GitPreviewFile {
    fn from(file: protocol::GitPreviewFile) -> Self {
        let protocol::GitPreviewFile {
            path,
            added,
            removed,
            untracked,
        } = file;
        Self {
            path: text(&path),
            added,
            removed,
            untracked,
        }
    }
}

impl From<protocol::GitRepoInfo> for GitRepoInfo {
    fn from(info: protocol::GitRepoInfo) -> Self {
        let protocol::GitRepoInfo {
            root,
            git_dir,
            is_worktree,
            current_branch,
        } = info;
        Self {
            root: text(&root),
            git_dir: text(&git_dir),
            is_worktree,
            current_branch,
        }
    }
}

impl From<protocol::GitStatus> for GitStatus {
    fn from(status: protocol::GitStatus) -> Self {
        let protocol::GitStatus {
            summary,
            default_branch,
            branches,
            files,
            pull_request,
        } = status;
        Self {
            summary: summary.into(),
            default_branch,
            branches: collect(branches),
            files: collect(files),
            pull_request: pull_request.map(Into::into),
        }
    }
}

impl From<protocol::GitFileStatus> for GitFileStatus {
    fn from(status: protocol::GitFileStatus) -> Self {
        let protocol::GitFileStatus {
            file,
            staged,
            unstaged,
        } = status;
        Self {
            file: file.into(),
            staged_lines: staged.into(),
            unstaged_lines: unstaged.into(),
        }
    }
}

impl From<protocol::GitLineStat> for GitLineStat {
    fn from(stat: protocol::GitLineStat) -> Self {
        let protocol::GitLineStat {
            additions,
            deletions,
            binary,
        } = stat;
        Self {
            additions,
            deletions,
            binary,
        }
    }
}

impl From<protocol::GitRawDiff> for GitRawDiff {
    fn from(diff: protocol::GitRawDiff) -> Self {
        let protocol::GitRawDiff { diff, truncated } = diff;
        Self { diff, truncated }
    }
}

impl From<protocol::GitDiff> for GitDiff {
    fn from(diff: protocol::GitDiff) -> Self {
        let protocol::GitDiff {
            rows,
            additions,
            deletions,
            truncated,
            binary,
        } = diff;
        Self {
            rows: collect(rows),
            additions,
            deletions,
            truncated,
            binary,
        }
    }
}

impl From<protocol::GitDiffRow> for GitDiffRow {
    fn from(row: protocol::GitDiffRow) -> Self {
        let protocol::GitDiffRow {
            kind,
            old_line_number,
            new_line_number,
            old_text,
            new_text,
            text,
        } = row;
        Self {
            kind: match kind {
                protocol::GitDiffKind::Hunk => GitDiffKind::Hunk,
                protocol::GitDiffKind::Context | protocol::GitDiffKind::Unrecognized(_) => {
                    GitDiffKind::Context
                }
                protocol::GitDiffKind::Addition => GitDiffKind::Addition,
                protocol::GitDiffKind::Deletion => GitDiffKind::Deletion,
            },
            old_line_number,
            new_line_number,
            old_text,
            new_text,
            text,
        }
    }
}

impl From<protocol::GitCommit> for GitCommit {
    fn from(commit: protocol::GitCommit) -> Self {
        let protocol::GitCommit {
            hash,
            short_hash,
            subject,
            author_name,
            author_date,
            parent_hashes,
            refs,
        } = commit;
        Self {
            hash,
            short_hash,
            subject,
            author_name,
            author_date,
            parent_hashes,
            refs: collect(refs),
        }
    }
}

impl From<protocol::GitRef> for GitRef {
    fn from(reference: protocol::GitRef) -> Self {
        let protocol::GitRef { name, kind } = reference;
        Self {
            name,
            kind: match kind {
                protocol::GitRefKind::LocalBranch => GitRefKind::LocalBranch,
                protocol::GitRefKind::RemoteBranch => GitRefKind::RemoteBranch,
                protocol::GitRefKind::Tag => GitRefKind::Tag,
                protocol::GitRefKind::Head => GitRefKind::Head,
                protocol::GitRefKind::Unrecognized(_) => GitRefKind::Other,
            },
        }
    }
}

impl From<protocol::GitPullRequest> for GitPullRequest {
    fn from(pull_request: protocol::GitPullRequest) -> Self {
        let protocol::GitPullRequest {
            number,
            url,
            title,
            author,
            head_branch,
            head_oid,
            base_branch,
            state,
            draft,
            updated_at,
            mergeable,
            merge_state,
            cross_repository,
            checks,
        } = pull_request;
        let protocol::GitChecks {
            passing,
            failing,
            pending,
        } = checks;
        Self {
            number,
            url,
            title,
            author,
            head_branch,
            head_oid,
            base_branch,
            state,
            draft,
            updated_at,
            mergeable,
            merge_state,
            cross_repository,
            checks: GitChecks {
                passing,
                failing,
                pending,
            },
        }
    }
}

impl From<GitPullRequestFilter> for protocol::GitPullRequestFilter {
    fn from(filter: GitPullRequestFilter) -> Self {
        match filter {
            GitPullRequestFilter::Open => Self::Open,
            GitPullRequestFilter::Closed => Self::Closed,
            GitPullRequestFilter::Merged => Self::Merged,
            GitPullRequestFilter::All => Self::All,
        }
    }
}

impl From<GitMergeMethod> for protocol::GitMergeMethod {
    fn from(method: GitMergeMethod) -> Self {
        match method {
            GitMergeMethod::Merge => Self::Merge,
            GitMergeMethod::Squash => Self::Squash,
            GitMergeMethod::Rebase => Self::Rebase,
        }
    }
}

impl From<protocol::GitBaseSwitch> for GitBaseSwitch {
    fn from(base_switch: protocol::GitBaseSwitch) -> Self {
        match base_switch {
            protocol::GitBaseSwitch::Updated => Self::Updated,
            protocol::GitBaseSwitch::CheckedOutElsewhere(directory) => Self::CheckedOutElsewhere {
                directory: text(&directory),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(status: [u8; 2]) -> GitFile {
        protocol::GitFile {
            path: protocol::ServerPath(b"file".to_vec()),
            original_path: None,
            index: status[0],
            worktree: status[1],
            added: None,
            removed: None,
        }
        .into()
    }

    #[test]
    fn status_letters_become_staged_and_unstaged_changes() {
        use GitChangeKind::{
            Added, Conflicted, Deleted, Modified, Other, Renamed, TypeChanged, Untracked,
        };
        for (status, staged, unstaged) in [
            (b"??", None, Some(Untracked)),
            (b"A ", Some(Added), None),
            (b"RM", Some(Renamed), Some(Modified)),
            (b" D", None, Some(Deleted)),
            (b"T ", Some(TypeChanged), None),
            (b"UU", None, Some(Conflicted)),
            (b"AA", None, Some(Conflicted)),
            (b"DD", None, Some(Conflicted)),
            (b"X ", Some(Other), None),
        ] {
            let file = file(*status);
            assert_eq!(
                (file.staged, file.unstaged),
                (staged, unstaged),
                "{status:?}"
            );
        }
    }
}
