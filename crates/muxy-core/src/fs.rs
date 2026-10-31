//! Durable whole-file replacement.

use std::fmt;
use std::fs::{File, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// An atomic write that failed, and whether `path` already holds the new
/// content.
#[derive(Debug)]
pub struct WriteError {
    pub error: io::Error,
    /// The new file replaced the old one, but syncing its folder failed, so
    /// the replacement may not survive a crash.
    pub renamed: bool,
}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> Self {
        Self {
            error,
            renamed: false,
        }
    }
}

impl From<WriteError> for io::Error {
    fn from(error: WriteError) -> Self {
        error.error
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Replaces `path` with `bytes`, created with `mode`, so readers and crashes
/// see either the old file or the new one.
pub fn write_atomic(path: &Path, bytes: &[u8], mode: u32) -> Result<(), WriteError> {
    write_atomic_with(path, mode, |file| file.write_all(bytes))
}

/// Like [`write_atomic`], with `write` filling the new file.
pub fn write_atomic_with(
    path: &Path,
    mode: u32,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), WriteError> {
    replace(path, mode, write, sync_parent)
}

fn replace(
    path: &Path,
    mode: u32,
    write: impl FnOnce(&mut File) -> io::Result<()>,
    sync: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), WriteError> {
    let directory = parent(path)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("an atomic write needs a file name"))?;
    let prefix = format!(".{}.", name.to_string_lossy());
    // A dropped temporary file deletes itself, so every early return cleans up.
    let mut temporary = tempfile::Builder::new()
        .prefix(&prefix)
        .suffix(".tmp")
        .permissions(Permissions::from_mode(mode))
        .tempfile_in(directory)?;
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    sync(path).map_err(|error| WriteError {
        error,
        renamed: true,
    })
}

/// Makes a rename or removal in `path`'s folder durable.
pub fn sync_parent(path: &Path) -> io::Result<()> {
    let directory = File::open(parent(path)?)?;
    if !directory.metadata()?.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not in a directory",
            path.display()
        )));
    }
    directory.sync_all()
}

fn parent(path: &Path) -> io::Result<&Path> {
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() => Ok(Path::new(".")),
        Some(parent) => Ok(parent),
        None => Err(io::Error::other(format!(
            "{} has no parent folder",
            path.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    fn entries(directory: &Path) -> io::Result<Vec<String>> {
        let mut names = std::fs::read_dir(directory)?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect::<io::Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }

    #[test]
    fn writes_the_content_with_the_given_mode_and_leaves_only_the_file() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.json");
        std::fs::write(&path, "old")?;
        write_atomic(&path, b"new", 0o600)?;
        assert_eq!(std::fs::read(&path)?, b"new");
        assert_eq!(std::fs::metadata(&path)?.mode() & 0o777, 0o600);
        write_atomic_with(&path, 0o644, |file| file.write_all(b"newer"))?;
        assert_eq!(std::fs::read(&path)?, b"newer");
        assert_eq!(std::fs::metadata(&path)?.mode() & 0o777, 0o644);
        assert_eq!(entries(directory.path())?, ["state.json"]);
        Ok(())
    }

    #[test]
    fn a_failed_write_keeps_the_old_file_and_removes_the_temporary_one() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.json");
        std::fs::write(&path, "old")?;
        let error = write_atomic_with(&path, 0o600, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("serializing failed"))
        })
        .expect_err("the writer failed");
        assert!(!error.renamed);
        assert_eq!(std::fs::read(&path)?, b"old");
        assert_eq!(entries(directory.path())?, ["state.json"]);
        Ok(())
    }

    #[test]
    fn a_failed_rename_removes_the_temporary_file() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state");
        std::fs::create_dir(&path)?;
        std::fs::write(path.join("inside"), "keeps the folder non-empty")?;
        let error = write_atomic(&path, b"new", 0o600).expect_err("a folder can't be replaced");
        assert!(!error.renamed);
        assert_eq!(entries(directory.path())?, ["state"]);
        Ok(())
    }

    #[test]
    fn a_failed_folder_sync_reports_that_the_file_was_already_replaced() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("state.json");
        std::fs::write(&path, "old")?;
        let error = replace(
            &path,
            0o600,
            |file| file.write_all(b"new"),
            |_| Err(io::Error::other("sync failed")),
        )
        .expect_err("the folder sync failed");
        assert!(error.renamed);
        assert_eq!(std::fs::read(&path)?, b"new");
        assert_eq!(entries(directory.path())?, ["state.json"]);
        Ok(())
    }

    #[test]
    fn leftover_temporary_files_do_not_block_the_write() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("muxy.zsh");
        for leftover in ["muxy.tmp-1", ".muxy.zsh.tmp", ".muxy.zsh.AAAAAA.tmp"] {
            std::fs::write(directory.path().join(leftover), "left by a crash")?;
        }
        write_atomic(&path, b"hooks", 0o600)?;
        assert_eq!(std::fs::read(&path)?, b"hooks");
        Ok(())
    }

    #[test]
    fn a_relative_path_writes_into_the_current_folder() {
        assert_eq!(parent(Path::new("state.json")).ok(), Some(Path::new(".")));
    }
}
