use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use muxy_core::files::private_file;

pub(super) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    muxy_core::fs::write_atomic(path, contents, 0o600).map_err(Into::into)
}

pub(super) fn lock(path: &Path) -> io::Result<File> {
    let mut lock = path.as_os_str().to_owned();
    lock.push(".lock");
    let file = private_file()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(PathBuf::from(lock))?;
    file.lock()?;
    Ok(file)
}
