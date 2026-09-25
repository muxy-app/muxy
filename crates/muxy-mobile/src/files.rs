//! Files in a project's folder, one to one with the protocol's `FilesAction` and
//! `FilesReply`. Paths are relative to the project's folder; `""` is the folder.

use muxy_protocol as protocol;

use crate::records::{server_path, text};

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum FilesAction {
    // Not `List`: in Kotlin, a case of that name would hide the `List` type that
    // `Move` and `Delete` use.
    ListDirectory {
        path: String,
    },
    /// Reads a UTF-8 text file of up to 5 MiB.
    Read {
        path: String,
    },
    Stat {
        path: String,
    },
    Write {
        path: String,
        content: String,
    },
    Mkdir {
        path: String,
    },
    Rename {
        path: String,
        name: String,
    },
    Move {
        paths: Vec<String>,
        into: String,
    },
    /// Moves the files to the computer's Trash.
    Delete {
        paths: Vec<String>,
    },
    /// Sends `FilesChanged` when files in this project change.
    Watch,
    Unwatch,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum FilesReply {
    Entries { entries: Vec<FileEntry> },
    Content { file: FileContent },
    Info { info: FileInfo },
    Path { path: String },
    Paths { paths: Vec<String> },
    Done,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    /// Git ignores it.
    pub is_ignored: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub size: u64,
}

impl From<FilesAction> for protocol::FilesAction {
    fn from(action: FilesAction) -> Self {
        let paths = |paths: Vec<String>| paths.into_iter().map(server_path).collect();
        match action {
            FilesAction::ListDirectory { path } => Self::List(server_path(path)),
            FilesAction::Read { path } => Self::Read(server_path(path)),
            FilesAction::Stat { path } => Self::Stat(server_path(path)),
            FilesAction::Write { path, content } => Self::Write {
                path: server_path(path),
                content,
            },
            FilesAction::Mkdir { path } => Self::Mkdir(server_path(path)),
            FilesAction::Rename { path, name } => Self::Rename {
                path: server_path(path),
                name: server_path(name),
            },
            FilesAction::Move {
                paths: values,
                into,
            } => Self::Move {
                paths: paths(values),
                into: server_path(into),
            },
            FilesAction::Delete { paths: values } => Self::Delete(paths(values)),
            FilesAction::Watch => Self::Watch,
            FilesAction::Unwatch => Self::Unwatch,
        }
    }
}

impl From<protocol::FilesReply> for FilesReply {
    fn from(reply: protocol::FilesReply) -> Self {
        match reply {
            protocol::FilesReply::Entries(entries) => Self::Entries {
                entries: entries
                    .into_iter()
                    .map(|entry| {
                        let protocol::FileEntry {
                            name,
                            path,
                            is_directory,
                            is_ignored,
                        } = entry;
                        FileEntry {
                            name: text(&name),
                            path: text(&path),
                            is_directory,
                            is_ignored,
                        }
                    })
                    .collect(),
            },
            protocol::FilesReply::Content(file) => {
                let protocol::FileContent {
                    path,
                    content,
                    size,
                } = file;
                Self::Content {
                    file: FileContent {
                        path: text(&path),
                        content,
                        size,
                    },
                }
            }
            protocol::FilesReply::Info(info) => {
                let protocol::FileInfo {
                    name,
                    path,
                    is_directory,
                    size,
                } = info;
                Self::Info {
                    info: FileInfo {
                        name: text(&name),
                        path: text(&path),
                        is_directory,
                        size,
                    },
                }
            }
            protocol::FilesReply::Path(path) => Self::Path { path: text(&path) },
            protocol::FilesReply::Paths(paths) => Self::Paths {
                paths: paths.iter().map(text).collect(),
            },
            protocol::FilesReply::Done => Self::Done,
        }
    }
}
