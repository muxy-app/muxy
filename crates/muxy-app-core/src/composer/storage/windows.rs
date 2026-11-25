use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

use muxy_core::files::private_file;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

use super::PrivateDirectory;

pub(super) fn create_directory(path: &Path) -> io::Result<File> {
    fs::create_dir_all(path)?;
    let directory = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    if !directory.metadata()?.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "Composer storage must be a folder, not a link",
        ));
    }
    muxy_core::files::private_dir(path)?;
    Ok(directory)
}

pub(super) fn create_new(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    private_file()
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(directory.path.join(name))
}

pub(super) fn open_regular(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    open_document(&directory.path.join(name))
}

pub(super) fn open_lock(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    private_file()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(directory.path.join(name))
}

pub(super) fn open_document(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

pub(super) fn link_new(directory: &PrivateDirectory, from: &OsStr, to: &OsStr) -> io::Result<()> {
    fs::hard_link(directory.path.join(from), directory.path.join(to))
}

pub(super) fn replace(directory: &PrivateDirectory, from: &OsStr, to: &OsStr) -> io::Result<()> {
    muxy_core::files::replace_file(&directory.path.join(from), &directory.path.join(to))
}

pub(super) fn remove(directory: &PrivateDirectory, name: &OsStr) -> io::Result<()> {
    fs::remove_file(directory.path.join(name))
}

pub(super) fn is_regular(directory: &PrivateDirectory, name: &OsStr) -> io::Result<Option<bool>> {
    match fs::symlink_metadata(directory.path.join(name)) {
        Ok(metadata) => Ok(Some(metadata.is_file())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn names(directory: &PrivateDirectory) -> io::Result<Vec<OsString>> {
    fs::read_dir(&directory.path)?
        .map(|entry| Ok(entry?.file_name()))
        .collect()
}

#[allow(clippy::unnecessary_wraps, reason = "Matches the Unix signature")]
pub(super) fn sync(_directory: &PrivateDirectory) -> io::Result<()> {
    Ok(())
}
