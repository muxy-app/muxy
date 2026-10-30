//! Files only this user can read, replaced whole so a crash never leaves half
//! of one, and changed by one process at a time.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// Each writer gets its own temporary file, so writers in other processes
/// never write into or rename it.
pub(super) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    let temporary = PathBuf::from(temporary);
    let result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .and_then(|mut file| {
            file.write_all(contents)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temporary, path));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
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
