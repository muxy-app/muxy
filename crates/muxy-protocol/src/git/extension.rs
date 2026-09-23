use serde::{Deserialize, Serialize};

use super::{GitBranch, GitFile, GitSummary};
use crate::{ErrorCode, ServerPath};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitRepoInfo {
    pub root: ServerPath,
    pub git_dir: ServerPath,
    pub is_worktree: bool,
    pub current_branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitStatus {
    pub summary: GitSummary,
    pub default_branch: Option<String>,
    pub branches: Vec<GitBranch>,
    pub files: Vec<GitFileStatus>,
    pub pull_request: Option<GitPullRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitFileStatus {
    pub file: GitFile,
    pub staged: GitLineStat,
    pub unstaged: GitLineStat,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitLineStat {
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub binary: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitDiffRequest {
    pub path: Option<ServerPath>,
    pub staged: bool,
    pub raw: bool,
    pub line_limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitRawDiff {
    pub diff: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitDiff {
    pub rows: Vec<GitDiffRow>,
    pub additions: u64,
    pub deletions: u64,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitDiffRow {
    pub kind: GitDiffKind,
    pub old_line_number: Option<u64>,
    pub new_line_number: Option<u64>,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GitDiffKind {
    Hunk,
    Context,
    Addition,
    Deletion,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub subject: String,
    pub author_name: String,
    pub author_date: String,
    pub parent_hashes: Vec<String>,
    pub refs: Vec<GitRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitRef {
    pub name: String,
    pub kind: GitRefKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GitRefKind {
    LocalBranch,
    RemoteBranch,
    Tag,
    Head,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    Merge {
        number: u64,
        method: GitMergeMethod,
        delete_branch: bool,
        /// Refuse the merge when the pull request head moved past this commit.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GitPullRequestFilter {
    Open,
    Closed,
    Merged,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GitMergeMethod {
    Merge,
    Squash,
    Rebase,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct GitChecks {
    pub passing: u32,
    pub failing: u32,
    pub pending: u32,
}

pub(super) fn validate_hash(hash: &str) -> Result<(), ErrorCode> {
    if hash.is_empty() || hash.len() > 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Err(ErrorCode::BadRequest)
    } else {
        Ok(())
    }
}

pub(super) fn validate_message(value: &str, limit: usize) -> Result<(), ErrorCode> {
    if value.trim().is_empty() || value.len() > limit || value.contains('\0') {
        Err(ErrorCode::BadRequest)
    } else {
        Ok(())
    }
}

pub(super) fn validate_number(number: u64) -> Result<(), ErrorCode> {
    if number == 0 {
        Err(ErrorCode::BadRequest)
    } else {
        Ok(())
    }
}

pub(super) fn validate_line_limit(limit: Option<u32>) -> Result<(), ErrorCode> {
    if limit.is_some_and(|limit| limit == 0 || limit > 100_000) {
        Err(ErrorCode::BadRequest)
    } else {
        Ok(())
    }
}

impl GitPullRequestAction {
    pub(super) fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Info | Self::Number => Ok(()),
            Self::Diff { number, line_limit } => {
                validate_number(*number)?;
                validate_line_limit(*line_limit)
            }
            Self::List { limit, .. } => {
                if *limit == 0 || *limit > 200 {
                    Err(ErrorCode::BadRequest)
                } else {
                    Ok(())
                }
            }
            Self::Create {
                title,
                body,
                base_branch,
                ..
            } => {
                validate_message(title, 1024)?;
                if body.len() > 64 * 1024 || body.contains('\0') {
                    return Err(ErrorCode::BadRequest);
                }
                if let Some(base) = base_branch {
                    validate_message(base, 1024)?;
                    if base.starts_with('-') {
                        return Err(ErrorCode::BadRequest);
                    }
                }
                Ok(())
            }
            Self::Merge {
                number,
                expected_head,
                ..
            } => {
                validate_number(*number)?;
                expected_head.as_deref().map_or(Ok(()), validate_hash)
            }
            Self::Close { number } | Self::Checkout { number } => validate_number(*number),
            Self::UpdateBranch {
                number,
                expected_head,
            } => {
                validate_number(*number)?;
                validate_hash(expected_head)
            }
        }
    }
}
