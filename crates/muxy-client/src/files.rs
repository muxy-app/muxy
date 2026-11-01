use std::time::Duration;

use muxy_protocol::{
    FileBytes, FileContent, FileEntry, FileInfo, FilesAction, FilesReply, FilesRequest, ProjectId,
    ReplyBody, RequestBody, ServerPath,
};

use crate::typed::{decoders, typed_requests};
use crate::{Client, ClientError, Request};

const FILES_TIMEOUT: Duration = Duration::from_secs(300);

impl Client {
    /// Any file action, answered with whatever the server sends, for callers
    /// that handle actions as data.
    pub fn files(&self, request: FilesRequest) -> Result<FilesReply, ClientError> {
        self.send_files(request.project, request.action, any)
    }

    /// Like `files`, without blocking.
    pub fn files_async(&self, request: FilesRequest) -> Request<FilesReply> {
        self.send_files_async(request.project, request.action, any)
    }

    fn send_files<T>(
        &self,
        project: ProjectId,
        action: FilesAction,
        decode: fn(ReplyBody) -> Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        decode(self.request_with_timeout(
            RequestBody::Files(FilesRequest { project, action }),
            FILES_TIMEOUT,
        )?)
    }

    fn send_files_async<T>(
        &self,
        project: ProjectId,
        action: FilesAction,
        decode: fn(ReplyBody) -> Result<T, ClientError>,
    ) -> Request<T> {
        self.request_async(
            RequestBody::Files(FilesRequest { project, action }),
            FILES_TIMEOUT,
            decode,
        )
    }
}

decoders! { Files;
    any -> FilesReply { reply => reply }
    done -> () { FilesReply::Done => () }
    entries -> Vec<FileEntry> { FilesReply::Entries(entries) => entries }
    content -> FileContent { FilesReply::Content(file) => file }
    file_bytes -> FileBytes { FilesReply::Bytes(file) => file }
    info -> FileInfo { FilesReply::Info(info) => info }
    new_path -> ServerPath { FilesReply::Path(path) => path }
    new_paths -> Vec<ServerPath> { FilesReply::Paths(paths) => paths }
}

typed_requests! { send_files, send_files_async;
    /// Folders first, then files, by name, inside `path` of the project's folder.
    list_directory, list_directory_async(path: ServerPath) -> Vec<FileEntry>
        = FilesAction::List(path) => entries;
    /// A UTF-8 text file.
    read_file, read_file_async(path: ServerPath) -> FileContent = FilesAction::Read(path) => content;
    /// Any file, as it is on disk.
    read_file_bytes, read_file_bytes_async(path: ServerPath) -> FileBytes
        = FilesAction::ReadBytes(path) => file_bytes;
    file_info, file_info_async(path: ServerPath) -> FileInfo = FilesAction::Stat(path) => info;
    /// Creates or replaces the file and returns its path.
    write_file, write_file_async(path: ServerPath, content: String) -> ServerPath
        = FilesAction::Write { path, content } => new_path;
    /// Like `write_file`, for any content.
    write_file_bytes, write_file_bytes_async(path: ServerPath, bytes: Vec<u8>) -> ServerPath
        = FilesAction::WriteBytes { path, bytes } => new_path;
    /// Returns the new folder's path, numbered when the name is taken.
    create_directory, create_directory_async(path: ServerPath) -> ServerPath
        = FilesAction::Mkdir(path) => new_path;
    /// Renames within the same folder and returns the new path.
    rename_file, rename_file_async(path: ServerPath, name: ServerPath) -> ServerPath
        = FilesAction::Rename { path, name } => new_path;
    /// Moves files and folders into `into` and returns their new paths in order.
    move_files, move_files_async(paths: Vec<ServerPath>, into: ServerPath) -> Vec<ServerPath>
        = FilesAction::Move { paths, into } => new_paths;
    /// Moves files and folders to the computer's Trash.
    delete_files, delete_files_async(paths: Vec<ServerPath>) -> ()
        = FilesAction::Delete(paths) => done;
    /// Sends `FilesChanged` for this project until `unwatch_files`.
    watch_files, watch_files_async() -> () = FilesAction::Watch => done;
    unwatch_files, unwatch_files_async() -> () = FilesAction::Unwatch => done;
}

#[cfg(test)]
mod tests;
