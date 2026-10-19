use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, RenameFlags};

use super::{Entry, Kind};

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

pub(in crate::files) fn open_root(path: &Path) -> io::Result<File> {
    Ok(fs::open(path, DIRECTORY, Mode::empty())?.into())
}

pub(in crate::files) fn open_directory(parent: &File, name: &OsStr) -> io::Result<File> {
    Ok(fs::openat(parent, name, DIRECTORY, Mode::empty())?.into())
}

pub(in crate::files) fn stat(parent: &File, name: &OsStr) -> io::Result<Entry> {
    let stat = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
    Ok(Entry {
        kind: match FileType::from_raw_mode(stat.st_mode) {
            FileType::RegularFile => Kind::File,
            FileType::Directory => Kind::Directory,
            FileType::Symlink => Kind::Link,
            _ => Kind::Other,
        },
        size: u64::try_from(stat.st_size).unwrap_or(0),
        #[allow(
            clippy::useless_conversion,
            reason = "st_dev is signed on some systems"
        )]
        device: u64::try_from(stat.st_dev).unwrap_or(0),
        #[allow(
            clippy::useless_conversion,
            reason = "st_ino is narrower on some systems"
        )]
        index: u64::from(stat.st_ino),
        #[allow(
            clippy::useless_conversion,
            reason = "st_mode is narrower on some systems"
        )]
        mode: u32::from(stat.st_mode),
    })
}

pub(in crate::files) fn read_link(parent: &File, name: &OsStr) -> io::Result<PathBuf> {
    let value = fs::readlinkat(parent, name, Vec::new())?;
    Ok(PathBuf::from(OsString::from_vec(value.into_bytes())))
}

/// Opens a file for reading. Opening never blocks, even on a FIFO.
pub(in crate::files) fn open_file(parent: &File, name: &OsStr) -> io::Result<File> {
    Ok(fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into())
}

pub(in crate::files) fn create_file(parent: &File, name: &OsStr) -> io::Result<File> {
    Ok(fs::openat(
        parent,
        name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o666),
    )?
    .into())
}

/// Gives `file` the permissions of the file it replaces.
pub(in crate::files) fn copy_mode(file: &File, replaced: &Entry) -> io::Result<()> {
    #[allow(
        clippy::unnecessary_fallible_conversions,
        reason = "The raw mode is narrower on macOS"
    )]
    let mode = fs::RawMode::try_from(replaced.mode).map_err(io::Error::other)?;
    Ok(fs::fchmod(file, Mode::from_raw_mode(mode))?)
}

/// Replaces `to` with `from`, both in `parent`.
pub(in crate::files) fn replace(parent: &File, from: &OsStr, to: &OsStr) -> io::Result<()> {
    Ok(fs::renameat(parent, from, parent, to)?)
}

/// Renames without replacing anything; a taken name is `AlreadyExists`.
pub(in crate::files) fn rename(
    parent: &File,
    from: &OsStr,
    destination: &File,
    to: &OsStr,
) -> io::Result<()> {
    Ok(fs::renameat_with(
        parent,
        from,
        destination,
        to,
        RenameFlags::NOREPLACE,
    )?)
}

pub(in crate::files) fn remove_file(parent: &File, name: &OsStr) -> io::Result<()> {
    Ok(fs::unlinkat(parent, name, AtFlags::empty())?)
}

pub(in crate::files) fn create_directory(parent: &File, name: &OsStr) -> io::Result<()> {
    Ok(fs::mkdirat(parent, name, Mode::from_raw_mode(0o777))?)
}

/// The names in `directory`, without `.` and `..`.
pub(in crate::files) fn names(directory: &File) -> io::Result<Vec<OsString>> {
    let mut names = Vec::new();
    for entry in fs::Dir::read_from(directory)? {
        let name = OsStr::from_bytes(entry?.file_name().to_bytes()).to_owned();
        if name != "." && name != ".." {
            names.push(name);
        }
    }
    Ok(names)
}
