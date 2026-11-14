//! Files in a project's folder. Paths are relative to the folder, and `""` is
//! the folder itself.

use muxy_client::Client;
use muxy_protocol::{self as protocol, ProjectId};

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
}

#[uniffi::export]
impl ProjectFiles {
    /// Folders first, then files, by name. `.git` is left out.
    pub fn list(&self, path: String) -> Result<Vec<FileEntry>, MobileError> {
        let entries = self
            .client
            .list_directory(self.project, server_path(path))?;
        Ok(entries.into_iter().map(Into::into).collect())
    }

    pub fn stat(&self, path: String) -> Result<FileInfo, MobileError> {
        Ok(self
            .client
            .file_info(self.project, server_path(path))?
            .into())
    }

    /// A UTF-8 text file of up to 5 MiB. Other files fail with `Server`; read
    /// them with `read_bytes`.
    pub fn read_text(&self, path: String) -> Result<String, MobileError> {
        Ok(self
            .client
            .read_file(self.project, server_path(path))?
            .content)
    }

    /// Any file of up to 5 MiB, such as an image.
    pub fn read_bytes(&self, path: String) -> Result<Vec<u8>, MobileError> {
        Ok(self
            .client
            .read_file_bytes(self.project, server_path(path))?
            .bytes)
    }

    /// Creates or replaces the file, whose folder must exist, and returns its path.
    pub fn write_text(&self, path: String, text: String) -> Result<String, MobileError> {
        let written = self
            .client
            .write_file(self.project, server_path(path), text)?;
        Ok(crate::records::text(&written))
    }

    /// Like `write_text`, for any content of up to 5 MiB.
    pub fn write_bytes(&self, path: String, bytes: Vec<u8>) -> Result<String, MobileError> {
        let written = self
            .client
            .write_file_bytes(self.project, server_path(path), bytes)?;
        Ok(text(&written))
    }

    /// Returns the folder's path, which gets a number, such as `notes 2`,
    /// when the name is taken.
    pub fn create_directory(&self, path: String) -> Result<String, MobileError> {
        let created = self
            .client
            .create_directory(self.project, server_path(path))?;
        Ok(text(&created))
    }

    /// Renames within the same folder and returns the new path. Fails when
    /// `name` is taken.
    pub fn rename(&self, path: String, name: String) -> Result<String, MobileError> {
        let renamed =
            self.client
                .rename_file(self.project, server_path(path), server_path(name))?;
        Ok(text(&renamed))
    }

    /// Moves files and folders into the folder `into` and returns their new
    /// paths in order. A taken name gets a number.
    pub fn move_files(&self, paths: Vec<String>, into: String) -> Result<Vec<String>, MobileError> {
        let moved = self.client.move_files(
            self.project,
            paths.into_iter().map(server_path).collect(),
            server_path(into),
        )?;
        Ok(moved.iter().map(text).collect())
    }

    /// Moves files and folders to the computer's Trash.
    pub fn delete_files(&self, paths: Vec<String>) -> Result<(), MobileError> {
        Ok(self
            .client
            .delete_files(self.project, paths.into_iter().map(server_path).collect())?)
    }

    /// Sends `FilesChanged` for this project until `unwatch`. A connection
    /// watches up to 32 projects.
    pub fn watch(&self) -> Result<(), MobileError> {
        Ok(self.client.watch_files(self.project)?)
    }

    pub fn unwatch(&self) -> Result<(), MobileError> {
        Ok(self.client.unwatch_files(self.project)?)
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
