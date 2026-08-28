use std::borrow::Cow;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RepositoryHead {
    Unborn,
    Commit(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositorySummary {
    pub branch: String,
    pub head: RepositoryHead,
    pub is_detached: bool,
    pub upstream: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub changed_count: usize,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub untracked_count: usize,
    pub conflicted_count: usize,
}

impl RepositorySummary {
    pub fn is_dirty(&self) -> bool {
        self.changed_count > 0
    }

    pub fn display_branch(&self) -> String {
        if !self.is_detached {
            return self.branch.clone();
        }
        match &self.head {
            RepositoryHead::Commit(oid) => {
                let end = oid.len().min(7);
                format!("Detached {}", &oid[..end])
            }
            RepositoryHead::Unborn => "Detached".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineStat {
    pub additions: Option<u64>,
    pub deletions: Option<u64>,
    pub binary: bool,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ChangedFileId {
    pub path: Vec<u8>,
    pub old_path: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangedFile {
    pub path: Vec<u8>,
    pub old_path: Option<Vec<u8>>,
    pub x_status: u8,
    pub y_status: u8,
    pub is_staged: bool,
    pub is_unstaged: bool,
    pub is_untracked: bool,
    pub is_conflicted: bool,
    pub is_binary: bool,
    pub combined_stat: Option<LineStat>,
    pub staged_stat: Option<LineStat>,
    pub unstaged_stat: Option<LineStat>,
}

impl ChangedFile {
    pub fn stable_id(&self) -> ChangedFileId {
        ChangedFileId {
            path: self.path.clone(),
            old_path: self.old_path.clone(),
        }
    }

    pub fn related_paths(&self) -> Vec<&[u8]> {
        match self.old_path.as_deref() {
            Some(old_path) if old_path != self.path => vec![old_path, self.path.as_slice()],
            _ => vec![self.path.as_slice()],
        }
    }

    pub fn display_path(&self) -> Cow<'_, str> {
        display_path_lossy(&self.path)
    }

    pub fn display_old_path(&self) -> Option<Cow<'_, str>> {
        self.old_path.as_deref().map(display_path_lossy)
    }
}

pub fn display_path_lossy(path: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(path)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineTotals {
    pub additions: u64,
    pub deletions: u64,
    pub binary_files: usize,
    pub unknown_files: usize,
}

impl LineTotals {
    pub(crate) fn add(&mut self, stat: Option<LineStat>) {
        match stat {
            Some(LineStat { binary: true, .. }) => {
                self.binary_files = self.binary_files.saturating_add(1);
            }
            Some(LineStat {
                additions: Some(additions),
                deletions: Some(deletions),
                binary: false,
            }) => {
                self.additions = self.additions.saturating_add(additions);
                self.deletions = self.deletions.saturating_add(deletions);
            }
            Some(_) | None => {
                self.unknown_files = self.unknown_files.saturating_add(1);
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChangedFiles {
    pub files: Vec<ChangedFile>,
    pub total_lines: LineTotals,
    pub staged_lines: LineTotals,
    pub unstaged_lines: LineTotals,
    pub conflict_lines: LineTotals,
}

impl ChangedFiles {
    pub fn conflicts(&self) -> Vec<&ChangedFile> {
        self.files
            .iter()
            .filter(|file| file.is_conflicted)
            .collect()
    }

    pub fn staged(&self) -> Vec<&ChangedFile> {
        self.files
            .iter()
            .filter(|file| file.is_staged && !file.is_conflicted)
            .collect()
    }

    pub fn unstaged(&self) -> Vec<&ChangedFile> {
        self.files
            .iter()
            .filter(|file| file.is_unstaged && !file.is_conflicted)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RepositoryParseError {
    #[error("malformed repository summary")]
    Summary,
    #[error("malformed repository status")]
    Status,
    #[error("malformed repository line statistics")]
    Numstat,
    #[error("duplicate repository line statistics")]
    DuplicateNumstat,
    #[error("malformed remote branch data")]
    RemoteBranches,
    #[error("malformed commit subjects")]
    CommitSubjects,
    #[error("malformed Git object identifier")]
    ObjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryIdentity {
    pub worktree_root: std::path::PathBuf,
    pub git_dir: std::path::PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BranchKind {
    Local,
    Remote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchEntry {
    pub name: Vec<u8>,
    pub oid: Vec<u8>,
    pub kind: BranchKind,
    pub current: bool,
    pub upstream: Option<Vec<u8>>,
    pub author: String,
    pub subject: String,
    pub timestamp: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedText {
    pub text: String,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UntrackedLineCount {
    Known(u64),
    Unknown,
}

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("repository {operation} process failed: {source}")]
    Process {
        operation: &'static str,
        #[source]
        source: crate::git::GitError,
    },
    #[error("repository {operation} exited with status {status:?}: {message}")]
    Status {
        operation: &'static str,
        status: Option<i32>,
        message: String,
    },
    #[error("repository {operation} output was truncated")]
    Truncated { operation: &'static str },
    #[error("repository {operation} returned malformed data: {source}")]
    Parse {
        operation: &'static str,
        #[source]
        source: RepositoryParseError,
    },
    #[error("repository {operation} file access failed: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("repository {operation} worker stopped")]
    Worker { operation: &'static str },
    #[error("repository path is invalid")]
    InvalidPath,
}
