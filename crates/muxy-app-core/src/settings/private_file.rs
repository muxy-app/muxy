//! Files only this user can read, replaced whole so a crash never leaves half
//! of one, and changed by one process at a time.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub(super) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    muxy_core::fs::write_atomic(path, contents, 0o600).map_err(Into::into)
}

/// Holds `path`'s lock, which every process takes before reading the file to
/// change it, until dropped.
pub(super) fn lock(path: &Path) -> io::Result<File> {
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(PathBuf::from(lock))?;
    file.lock()?;
    Ok(file)
}
