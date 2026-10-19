use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, RwLock, Weak};

use muxy_protocol::{ErrorCode, FilesAction, FilesReply, FilesRequest, ServerPath};

use crate::{Registry, ServerError};

#[cfg(any(windows, test))]
pub(crate) mod names;
mod native;
mod operations;
mod root;
mod trash;
pub(crate) mod watch;

type Result<T> = std::result::Result<T, ServerError>;

fn error(cause: impl std::fmt::Display) -> ServerError {
    ServerError::new(ErrorCode::BadPath, cause.to_string())
}

fn path(path: &ServerPath) -> Result<PathBuf> {
    path.to_path_buf()
        .map_err(|code| ServerError::new(code, "Path is not valid on this computer"))
}

/// A name or a workspace-relative path as it travels, `/`-separated.
fn wire_path(path: &Path) -> Result<ServerPath> {
    let mut path = ServerPath::from_path(path)
        .map_err(|code| ServerError::new(code, "Path is not valid Unicode"))?;
    if cfg!(windows) {
        for byte in &mut path.0 {
            if *byte == b'\\' {
                *byte = b'/';
            }
        }
    }
    Ok(path)
}

/// Git's folder, which listings and change reports leave out. Windows file
/// systems ignore case.
fn is_git(name: &std::ffi::OsStr) -> bool {
    muxy_core::paths::path_eq(Path::new(name), Path::new(".git"))
}

#[derive(Debug, Default)]
pub(crate) struct Files {
    locks: Mutex<HashMap<PathBuf, Weak<RwLock<()>>>>,
}

impl Files {
    fn lock_for(&self, path: &Path) -> Arc<RwLock<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
        locks.retain(|_, lock| lock.strong_count() > 0);
        let lock = locks.get(path).and_then(Weak::upgrade).unwrap_or_default();
        locks.insert(path.to_owned(), Arc::downgrade(&lock));
        lock
    }
}

/// The folders directly inside `directory`, an absolute path anywhere on
/// this computer, for choosing a new project's folder. Links to folders
/// count. A folder with more than the protocol allows lists only its first.
pub(crate) fn folders(directory: &ServerPath) -> Result<Vec<ServerPath>> {
    muxy_protocol::validate_folder_path(directory)
        .map_err(|code| ServerError::new(code, "Use an absolute folder path"))?;
    let directory = path(directory)?;
    let entries = std::fs::read_dir(directory).map_err(|cause| match cause.kind() {
        std::io::ErrorKind::NotFound => error("folder does not exist"),
        std::io::ErrorKind::NotADirectory => error("not a folder"),
        _ => error(cause),
    })?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(error)?;
        let folder = entry
            .file_type()
            .is_ok_and(|kind| kind.is_dir() || (kind.is_symlink() && entry.path().is_dir()));
        if folder && let Ok(name) = wire_path(Path::new(&entry.file_name())) {
            names.push(name);
        }
    }
    names.sort_by_cached_key(|name| String::from_utf8_lossy(&name.0).to_lowercase());
    names.truncate(muxy_protocol::MAX_FILE_ENTRIES);
    let mut bytes = 0;
    names.retain(|name| {
        bytes += name.0.len();
        bytes <= muxy_protocol::MAX_FILE_PATH_BYTES
    });
    Ok(names)
}

pub(crate) fn is_read(action: &FilesAction) -> bool {
    matches!(
        action,
        FilesAction::List(_)
            | FilesAction::Read(_)
            | FilesAction::ReadBytes(_)
            | FilesAction::Stat(_)
    )
}

impl Registry {
    pub fn files(&self, request: &FilesRequest) -> Result<FilesReply> {
        request
            .validate()
            .map_err(|code| ServerError::new(code, "Invalid files request"))?;
        let project = self.catalog.project(request.project)?;
        let root = root::Root::open(&path(&project.directory)?)?;
        let lock = self.files.lock_for(&root.path);
        let read = is_read(&request.action);
        let _read = read.then(|| lock.read().unwrap_or_else(PoisonError::into_inner));
        let _write = (!read).then(|| lock.write().unwrap_or_else(PoisonError::into_inner));
        let reply = match &request.action {
            FilesAction::List(value) => FilesReply::Entries(root.list(&path(value)?)?),
            FilesAction::Read(value) => FilesReply::Content(root.read(&path(value)?)?),
            FilesAction::ReadBytes(value) => FilesReply::Bytes(root.read_bytes(&path(value)?)?),
            FilesAction::Stat(value) => FilesReply::Info(root.info(&root.resolve(&path(value)?)?)?),
            FilesAction::Write {
                path: value,
                content,
            } => FilesReply::Path(root.write(&path(value)?, content.as_bytes())?),
            FilesAction::WriteBytes { path: value, bytes } => {
                FilesReply::Path(root.write(&path(value)?, bytes)?)
            }
            FilesAction::Mkdir(value) => FilesReply::Path(root.mkdir(&path(value)?)?),
            FilesAction::Rename { path: value, name } => {
                FilesReply::Path(root.rename(&path(value)?, path(name)?.as_os_str())?)
            }
            FilesAction::Move { paths, into } => {
                FilesReply::Paths(root.move_files(paths, &path(into)?)?)
            }
            FilesAction::Delete(paths) => {
                root.delete(paths)?;
                FilesReply::Done
            }
            FilesAction::Watch | FilesAction::Unwatch => {
                return Err(ServerError::new(
                    ErrorCode::BadRequest,
                    "File subscriptions require a client connection",
                ));
            }
        };
        reply
            .validate()
            .map_err(|code| ServerError::new(code, "File result exceeds the protocol limits"))?;
        Ok(reply)
    }
}

#[cfg(test)]
mod tests;
