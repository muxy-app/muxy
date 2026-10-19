//! Every name opens relative to the held directory, never following a
//! symbolic link.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{AtFlags, FileType, Mode, OFlags};

use super::PrivateDirectory;

/// Modes apply only to the folders this creates, so an existing link is
/// left as it is and then refused.
pub(super) fn create_directory(path: &Path) -> io::Result<File> {
    muxy_core::files::private_dir(path)?;
    Ok(rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into())
}

pub(super) fn create_new(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    Ok(rustix::fs::openat(
        &directory.file,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )?
    .into())
}

pub(super) fn open_regular(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    Ok(rustix::fs::openat(
        &directory.file,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into())
}

pub(super) fn open_lock(directory: &PrivateDirectory, name: &OsStr) -> io::Result<File> {
    Ok(rustix::fs::openat(
        &directory.file,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )?
    .into())
}

pub(super) fn open_document(path: &Path) -> io::Result<File> {
    Ok(rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into())
}

/// Gives `from` the name `to`, failing if `to` exists.
pub(super) fn link_new(directory: &PrivateDirectory, from: &OsStr, to: &OsStr) -> io::Result<()> {
    Ok(rustix::fs::linkat(
        &directory.file,
        from,
        &directory.file,
        to,
        AtFlags::empty(),
    )?)
}

pub(super) fn replace(directory: &PrivateDirectory, from: &OsStr, to: &OsStr) -> io::Result<()> {
    Ok(rustix::fs::renameat(
        &directory.file,
        from,
        &directory.file,
        to,
    )?)
}

pub(super) fn remove(directory: &PrivateDirectory, name: &OsStr) -> io::Result<()> {
    Ok(rustix::fs::unlinkat(
        &directory.file,
        name,
        AtFlags::empty(),
    )?)
}

/// Whether `name` is a regular file, or `None` when it doesn't exist.
pub(super) fn is_regular(directory: &PrivateDirectory, name: &OsStr) -> io::Result<Option<bool>> {
    match rustix::fs::statat(&directory.file, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Ok(Some(
            FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile,
        )),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn names(directory: &PrivateDirectory) -> io::Result<Vec<OsString>> {
    rustix::fs::Dir::read_from(&directory.file)?
        .map(|entry| Ok(OsStr::from_bytes(entry?.file_name().to_bytes()).to_owned()))
        .collect()
}

pub(super) fn sync(directory: &PrivateDirectory) -> io::Result<()> {
    directory.file.sync_all()
}
