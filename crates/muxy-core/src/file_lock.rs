use std::fs::File;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub struct FileLock(pub(crate) File);

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub fn try_lock(path: &Path) -> io::Result<FileLock> {
    let file = crate::files::private_file()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::WouldBlock,
            "An installation or server startup is already in progress",
        ),
        std::fs::TryLockError::Error(error) => error,
    })?;
    Ok(FileLock(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_the_owner_releases_the_lock_while_an_inherited_descriptor_is_open() -> io::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("installation.lock");
        let owner = try_lock(&path)?;
        let inherited = owner.0.try_clone()?;
        assert!(matches!(try_lock(&path), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
        drop(owner);
        let next = try_lock(&path)?;
        drop(inherited);
        assert!(matches!(try_lock(&path), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
        drop(next);
        let _owner = try_lock(&path)?;
        Ok(())
    }
}
