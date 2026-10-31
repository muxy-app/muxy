use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

const RETRY: Duration = Duration::from_millis(25);

#[derive(Debug)]
pub struct FileLock(File);

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Takes the lock at `path`, or fails with `WouldBlock` while another holder
/// has it.
pub fn try_lock(path: &Path) -> io::Result<FileLock> {
    let file = open(path)?;
    file.try_lock().map_err(io::Error::from)?;
    Ok(FileLock(file))
}

/// Takes the lock at `path`, waiting up to `timeout` for another holder to
/// release it.
pub fn acquire(path: &Path, timeout: Duration) -> io::Result<FileLock> {
    let file = open(path)?;
    acquire_file(&file, timeout)?;
    Ok(FileLock(file))
}

/// Locks an open file, such as a folder handle, waiting up to `timeout`. The
/// lock lasts until the file closes.
pub fn acquire_file(file: &File, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                std::thread::sleep(left.min(RETRY));
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
}

/// Words a busy lock's error for the caller, keeping its `WouldBlock` kind.
pub fn busy(message: impl Into<String>) -> impl FnOnce(io::Error) -> io::Error {
    move |error| {
        if error.kind() == io::ErrorKind::WouldBlock {
            io::Error::new(io::ErrorKind::WouldBlock, message.into())
        } else {
            error
        }
    }
}

fn open(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn would_block<T>(result: &io::Result<T>) -> bool {
        matches!(result, Err(error) if error.kind() == io::ErrorKind::WouldBlock)
    }

    #[test]
    fn dropping_the_owner_releases_the_lock_while_an_inherited_descriptor_is_open() -> io::Result<()>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("installation.lock");
        let owner = try_lock(&path)?;
        let inherited = owner.0.try_clone()?;
        assert!(would_block(&try_lock(&path)));
        drop(owner);
        let next = try_lock(&path)?;
        drop(inherited);
        assert!(would_block(&try_lock(&path)));
        drop(next);
        let _owner = try_lock(&path)?;
        Ok(())
    }

    #[test]
    fn acquire_times_out_while_the_lock_is_held_and_succeeds_once_released() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("startup.lock");
        let holder = try_lock(&path)?;

        let started = Instant::now();
        assert!(would_block(&acquire(&path, Duration::from_millis(100))));
        assert!(started.elapsed() >= Duration::from_millis(100));

        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(holder);
        });
        let _lock = acquire(&path, Duration::from_secs(5))?;
        release
            .join()
            .map_err(|_| io::Error::other("release panicked"))?;
        Ok(())
    }

    #[test]
    fn busy_rewords_only_a_busy_lock() {
        let error = busy("An update is already in progress")(io::ErrorKind::WouldBlock.into());
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(error.to_string(), "An update is already in progress");
        let error = busy("unused")(io::ErrorKind::NotFound.into());
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
