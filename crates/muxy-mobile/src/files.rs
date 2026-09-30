//! Files in a project's folder. Paths are relative to the folder, and `""` is
//! the folder itself.

use muxy_client::{Client, ClientError};
use muxy_protocol::{
    self as protocol, FilesAction, FilesReply, FilesRequest, ProjectId, ReplyBody,
};

use crate::MobileError;
use crate::records::{server_path, text};

/// One project's folder. Every call talks to the server.
#[derive(Debug, uniffi::Object)]
pub struct ProjectFiles {
    client: Client,
    project: ProjectId,
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

impl ProjectFiles {
    pub(crate) fn new(client: Client, project: ProjectId) -> Self {
        Self { client, project }
    }

    fn request(&self, action: FilesAction) -> Result<FilesReply, MobileError> {
        let request = FilesRequest {
            project: self.project,
            action,
        };
        Ok(self.client.files(request)?)
    }

    /// Runs an action whose only answer is `Done`.
    fn run(&self, action: FilesAction) -> Result<(), MobileError> {
        match self.request(action)? {
            FilesReply::Done => Ok(()),
            other => Err(unexpected(other)),
        }
    }
}

#[uniffi::export]
impl ProjectFiles {
    /// Folders first, then files, by name. `.git` is left out.
    pub fn list(&self, path: String) -> Result<Vec<FileEntry>, MobileError> {
        match self.request(FilesAction::List(server_path(path)))? {
            FilesReply::Entries(entries) => Ok(entries.into_iter().map(Into::into).collect()),
            other => Err(unexpected(other)),
        }
    }

    pub fn stat(&self, path: String) -> Result<FileInfo, MobileError> {
        match self.request(FilesAction::Stat(server_path(path)))? {
            FilesReply::Info(info) => Ok(info.into()),
            other => Err(unexpected(other)),
        }
    }

    /// A UTF-8 text file of up to 5 MiB. Other files fail with `Server`; read
    /// them with `read_bytes`.
    pub fn read_text(&self, path: String) -> Result<String, MobileError> {
        match self.request(FilesAction::Read(server_path(path)))? {
            FilesReply::Content(file) => Ok(file.content),
            other => Err(unexpected(other)),
        }
    }

    /// Any file of up to 5 MiB, such as an image.
    pub fn read_bytes(&self, path: String) -> Result<Vec<u8>, MobileError> {
        match self.request(FilesAction::ReadBytes(server_path(path)))? {
            FilesReply::Bytes(file) => Ok(file.bytes),
            other => Err(unexpected(other)),
        }
    }

    /// Creates or replaces the file, whose folder must exist, and returns its path.
    pub fn write_text(&self, path: String, text: String) -> Result<String, MobileError> {
        written(self.request(FilesAction::Write {
            path: server_path(path),
            content: text,
        })?)
    }

    /// Like `write_text`, for any content of up to 5 MiB.
    pub fn write_bytes(&self, path: String, bytes: Vec<u8>) -> Result<String, MobileError> {
        written(self.request(FilesAction::WriteBytes {
            path: server_path(path),
            bytes,
        })?)
    }

    /// Returns the folder's path, which gets a number, such as `notes 2`,
    /// when the name is taken.
    pub fn create_directory(&self, path: String) -> Result<String, MobileError> {
        written(self.request(FilesAction::Mkdir(server_path(path)))?)
    }

    /// Renames within the same folder and returns the new path. Fails when
    /// `name` is taken.
    pub fn rename(&self, path: String, name: String) -> Result<String, MobileError> {
        written(self.request(FilesAction::Rename {
            path: server_path(path),
            name: server_path(name),
        })?)
    }

    /// Moves files and folders into the folder `into` and returns their new
    /// paths in order. A taken name gets a number.
    pub fn move_files(&self, paths: Vec<String>, into: String) -> Result<Vec<String>, MobileError> {
        let action = FilesAction::Move {
            paths: paths.into_iter().map(server_path).collect(),
            into: server_path(into),
        };
        match self.request(action)? {
            FilesReply::Paths(paths) => Ok(paths.iter().map(text).collect()),
            other => Err(unexpected(other)),
        }
    }

    /// Moves files and folders to the computer's Trash.
    pub fn delete_files(&self, paths: Vec<String>) -> Result<(), MobileError> {
        self.run(FilesAction::Delete(
            paths.into_iter().map(server_path).collect(),
        ))
    }

    /// Sends `FilesChanged` for this project until `unwatch`. A connection
    /// watches up to 32 projects.
    pub fn watch(&self) -> Result<(), MobileError> {
        self.run(FilesAction::Watch)
    }

    pub fn unwatch(&self) -> Result<(), MobileError> {
        self.run(FilesAction::Unwatch)
    }
}

/// A reply of another kind than the request asked for.
fn unexpected(reply: FilesReply) -> MobileError {
    ClientError::UnexpectedReply(Box::new(ReplyBody::Files(reply))).into()
}

fn written(reply: FilesReply) -> Result<String, MobileError> {
    match reply {
        FilesReply::Path(path) => Ok(text(&path)),
        other => Err(unexpected(other)),
    }
}

impl From<protocol::FileEntry> for FileEntry {
    fn from(entry: protocol::FileEntry) -> Self {
        let protocol::FileEntry {
            name,
            path,
            is_directory,
            is_ignored,
        } = entry;
        Self {
            name: text(&name),
            path: text(&path),
            is_directory,
            is_ignored,
        }
    }
}

impl From<protocol::FileInfo> for FileInfo {
    fn from(info: protocol::FileInfo) -> Self {
        let protocol::FileInfo {
            name,
            path,
            is_directory,
            size,
        } = info;
        Self {
            name: text(&name),
            path: text(&path),
            is_directory,
            size,
        }
    }
}
