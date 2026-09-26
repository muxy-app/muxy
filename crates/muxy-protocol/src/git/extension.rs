use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::wire::cbor::open_enum;

use super::{GitBranch, GitFile, GitSummary};
use crate::{ErrorCode, ServerPath};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitRepoInfo {
    #[n(0)]
    pub root: ServerPath,
    #[n(1)]
    pub git_dir: ServerPath,
    #[n(2)]
    pub is_worktree: bool,
    #[n(3)]
    pub current_branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitStatus {
    #[n(0)]
    pub summary: GitSummary,
    #[n(1)]
    pub default_branch: Option<String>,
    #[n(2)]
    pub branches: Vec<GitBranch>,
    #[n(3)]
    pub files: Vec<GitFileStatus>,
    #[n(4)]
    pub pull_request: Option<GitPullRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitFileStatus {
    #[n(0)]
    pub file: GitFile,
    #[n(1)]
    pub staged: GitLineStat,
    #[n(2)]
    pub unstaged: GitLineStat,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitLineStat {
    #[n(0)]
    pub additions: Option<u64>,
    #[n(1)]
    pub deletions: Option<u64>,
    #[n(2)]
    pub binary: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitDiffRequest {
    #[n(0)]
    pub path: Option<ServerPath>,
    #[n(1)]
    pub staged: bool,
    #[n(2)]
    pub raw: bool,
    #[n(3)]
    pub line_limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitRawDiff {
    #[n(0)]
    pub diff: String,
    #[n(1)]
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitDiff {
    #[n(0)]
    pub rows: Vec<GitDiffRow>,
    #[n(1)]
    pub additions: u64,
    #[n(2)]
    pub deletions: u64,
    #[n(3)]
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitDiffRow {
    #[n(0)]
    pub kind: GitDiffKind,
    #[n(1)]
    pub old_line_number: Option<u64>,
    #[n(2)]
    pub new_line_number: Option<u64>,
    #[n(3)]
    pub old_text: Option<String>,
    #[n(4)]
    pub new_text: Option<String>,
    #[n(5)]
    pub text: String,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum GitDiffKind {
        Hunk = 0,
        Context = 1,
        Addition = 2,
        Deletion = 3,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitCommit {
    #[n(0)]
    pub hash: String,
    #[n(1)]
    pub short_hash: String,
    #[n(2)]
    pub subject: String,
    #[n(3)]
    pub author_name: String,
    #[n(4)]
    pub author_date: String,
    #[n(5)]
    pub parent_hashes: Vec<String>,
    #[n(6)]
    pub refs: Vec<GitRef>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitRef {
    #[n(0)]
    pub name: String,
    #[n(1)]
    pub kind: GitRefKind,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum GitRefKind {
        LocalBranch = 0,
        RemoteBranch = 1,
        Tag = 2,
        Head = 3,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum GitPullRequestAction {
    #[n(0)]
    Info,
    #[n(1)]
    Number,
    #[n(2)]
    Diff {
        #[n(0)]
        number: u64,
        #[n(1)]
        line_limit: Option<u32>,
    },
    #[n(3)]
    List {
        #[n(0)]
        filter: GitPullRequestFilter,
        #[n(1)]
        limit: u32,
        #[n(2)]
        checks: bool,
    },
    #[n(4)]
    Create {
        #[n(0)]
        title: String,
        #[n(1)]
        body: String,
        #[n(2)]
        base_branch: Option<String>,
        #[n(3)]
        draft: bool,
    },
    #[n(5)]
    Merge {
        #[n(0)]
        number: u64,
        #[n(1)]
        method: GitMergeMethod,
        #[n(2)]
        delete_branch: bool,
        /// Refuse the merge when the pull request head moved past this commit.
        #[n(3)]
        expected_head: Option<String>,
    },
    #[n(6)]
    Close {
        #[n(0)]
        number: u64,
    },
    #[n(7)]
    Checkout {
        #[n(0)]
        number: u64,
    },
    #[n(8)]
    UpdateBranch {
        #[n(0)]
        number: u64,
        #[n(1)]
        expected_head: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum GitPullRequestFilter {
    #[n(0)]
    Open,
    #[n(1)]
    Closed,
    #[n(2)]
    Merged,
    #[n(3)]
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum GitMergeMethod {
    #[n(0)]
    Merge,
    #[n(1)]
    Squash,
    #[n(2)]
    Rebase,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitPullRequest {
    #[n(0)]
    pub number: u64,
    #[n(1)]
    pub url: String,
    #[n(2)]
    pub title: String,
    #[n(3)]
    pub author: String,
    #[n(4)]
    pub head_branch: String,
    #[n(5)]
    pub head_oid: String,
    #[n(6)]
    pub base_branch: String,
    #[n(7)]
    pub state: String,
    #[n(8)]
    pub draft: bool,
    #[n(9)]
    pub updated_at: Option<String>,
    #[n(10)]
    pub mergeable: Option<bool>,
    #[n(11)]
    pub merge_state: String,
    #[n(12)]
    pub cross_repository: bool,
    #[n(13)]
    pub checks: GitChecks,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct GitChecks {
    #[n(0)]
    pub passing: u32,
    #[n(1)]
    pub failing: u32,
    #[n(2)]
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
