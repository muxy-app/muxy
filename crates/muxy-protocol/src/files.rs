use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ErrorCode, ProjectId, ServerPath};

pub const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_FILE_ENTRIES: usize = 16_384;
pub const MAX_FILE_PATH_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_FILE_CHANGES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct FilesRequest {
    #[n(0)]
    pub project: ProjectId,
    #[n(1)]
    pub action: FilesAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum FilesAction {
    #[n(0)]
    List(#[n(0)] ServerPath),
    #[n(1)]
    Read(#[n(0)] ServerPath),
    #[n(2)]
    Stat(#[n(0)] ServerPath),
    #[n(3)]
    Write {
        #[n(0)]
        path: ServerPath,
        #[n(1)]
        content: String,
    },
    #[n(4)]
    Mkdir(#[n(0)] ServerPath),
    #[n(5)]
    Rename {
        #[n(0)]
        path: ServerPath,
        #[n(1)]
        name: ServerPath,
    },
    #[n(6)]
    Move {
        #[n(0)]
        paths: Vec<ServerPath>,
        #[n(1)]
        into: ServerPath,
    },
    #[n(7)]
    Delete(#[n(0)] Vec<ServerPath>),
    #[n(8)]
    Watch,
    #[n(9)]
    Unwatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct FileEntry {
    #[n(0)]
    pub name: ServerPath,
    #[n(1)]
    pub path: ServerPath,
    #[n(2)]
    pub is_directory: bool,
    #[n(3)]
    pub is_ignored: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct FileInfo {
    #[n(0)]
    pub name: ServerPath,
    #[n(1)]
    pub path: ServerPath,
    #[n(2)]
    pub is_directory: bool,
    #[n(3)]
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct FileContent {
    #[n(0)]
    pub path: ServerPath,
    #[n(1)]
    pub content: String,
    #[n(2)]
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum FilesReply {
    #[n(0)]
    Entries(#[n(0)] Vec<FileEntry>),
    #[n(1)]
    Content(#[n(0)] FileContent),
    #[n(2)]
    Info(#[n(0)] FileInfo),
    #[n(3)]
    Path(#[n(0)] ServerPath),
    #[n(4)]
    Paths(#[n(0)] Vec<ServerPath>),
    #[n(5)]
    Done,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct FileChanges {
    #[n(0)]
    pub paths: Vec<ServerPath>,
    #[n(1)]
    pub rescan: bool,
}

impl FileChanges {
    pub fn merge(&mut self, other: &Self) {
        if self.rescan || other.rescan {
            self.rescan = true;
            self.paths.clear();
            return;
        }
        for path in &other.paths {
            if !self.paths.contains(path) {
                self.paths.push(path.clone());
            }
            if self.paths.len() > MAX_FILE_CHANGES {
                self.rescan = true;
                self.paths.clear();
                return;
            }
        }
    }

    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.paths.len() > MAX_FILE_CHANGES || (self.rescan && !self.paths.is_empty()) {
            return Err(ErrorCode::BadRequest);
        }
        self.paths.iter().try_for_each(relative_path)
    }
}

fn relative_path(path: &ServerPath) -> Result<(), ErrorCode> {
    if path.0.len() > 4096 || path.0.contains(&0) || path.0.starts_with(b"/") {
        return Err(ErrorCode::BadPath);
    }
    Ok(())
}

fn paths(values: &[ServerPath]) -> Result<(), ErrorCode> {
    if values.len() > 4096 || values.iter().map(|p| p.0.len()).sum::<usize>() > MAX_FILE_PATH_BYTES
    {
        return Err(ErrorCode::BadRequest);
    }
    values.iter().try_for_each(relative_path)
}

impl FilesRequest {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match &self.action {
            FilesAction::List(path)
            | FilesAction::Read(path)
            | FilesAction::Stat(path)
            | FilesAction::Mkdir(path) => relative_path(path),
            FilesAction::Write { path, content } => {
                relative_path(path)?;
                if content.len() > MAX_FILE_BYTES {
                    return Err(ErrorCode::BadRequest);
                }
                Ok(())
            }
            FilesAction::Rename { path, name } => {
                relative_path(path)?;
                relative_path(name)?;
                if name.0.is_empty() || name.0.contains(&b'/') || name.0 == b"." || name.0 == b".."
                {
                    return Err(ErrorCode::BadPath);
                }
                Ok(())
            }
            FilesAction::Move {
                paths: sources,
                into,
            } => {
                paths(sources)?;
                relative_path(into)
            }
            FilesAction::Delete(sources) => paths(sources),
            FilesAction::Watch | FilesAction::Unwatch => Ok(()),
        }
    }
}

impl FilesReply {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Entries(entries) => {
                if entries.len() > MAX_FILE_ENTRIES
                    || entries
                        .iter()
                        .map(|e| e.path.0.len() + e.name.0.len())
                        .sum::<usize>()
                        > MAX_FILE_PATH_BYTES
                {
                    return Err(ErrorCode::BadRequest);
                }
                for entry in entries {
                    relative_path(&entry.path)?;
                    relative_path(&entry.name)?;
                }
                Ok(())
            }
            Self::Content(file) => {
                relative_path(&file.path)?;
                if file.content.len() > MAX_FILE_BYTES || file.size != file.content.len() as u64 {
                    return Err(ErrorCode::BadRequest);
                }
                Ok(())
            }
            Self::Info(file) => relative_path(&file.path),
            Self::Path(path) => relative_path(path),
            Self::Paths(values) => paths(values),
            Self::Done => Ok(()),
        }
    }
}
