use std::collections::{BTreeMap, VecDeque};
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

const FIRST_RETRY: Duration = Duration::from_millis(1);
const LAST_RETRY: Duration = Duration::from_millis(25);

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
/// lock lasts until the file closes. Waiters in this process are served in
/// the order they arrived.
pub fn acquire_file(file: &File, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    let _turn = Turn::wait(file, deadline)?;
    let mut retry = FIRST_RETRY;
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                std::thread::sleep(left.min(retry));
                retry = (retry * 2).min(LAST_RETRY);
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
}

/// A file, whatever path or handle reached it.
type FileId = (u64, u64);

/// This process's waiters, one line per file. Only the waiter at the front
/// of a line tries the lock, so a thread that just released it can't take it
/// straight back ahead of the others.
struct Lines {
    next_ticket: u64,
    waiting: BTreeMap<FileId, VecDeque<u64>>,
}

static LINES: Mutex<Lines> = Mutex::new(Lines {
    next_ticket: 0,
    waiting: BTreeMap::new(),
});
static LINE_MOVED: Condvar = Condvar::new();

fn lines() -> MutexGuard<'static, Lines> {
    LINES.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Lines {
    fn is_first(&self, file: FileId, ticket: u64) -> bool {
        self.waiting.get(&file).and_then(VecDeque::front) == Some(&ticket)
    }

    fn leave(&mut self, file: FileId, ticket: u64) {
        if let Some(line) = self.waiting.get_mut(&file) {
            line.retain(|waiting| *waiting != ticket);
            if line.is_empty() {
                self.waiting.remove(&file);
            }
        }
        LINE_MOVED.notify_all();
    }
}

/// The front place in a file's line, held until the file lock is taken or
/// the wait gives up.
struct Turn {
    file: FileId,
    ticket: u64,
}

impl Turn {
    fn wait(file: &File, deadline: Instant) -> io::Result<Self> {
        let metadata = file.metadata()?;
        let file = (metadata.dev(), metadata.ino());
        let mut lines = lines();
        let ticket = lines.next_ticket;
        lines.next_ticket += 1;
        lines.waiting.entry(file).or_default().push_back(ticket);
        while !lines.is_first(file, ticket) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                lines.leave(file, ticket);
                return Err(io::ErrorKind::WouldBlock.into());
            }
            lines = LINE_MOVED
                .wait_timeout(lines, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        Ok(Self { file, ticket })
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        lines().leave(self.file, self.ticket);
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
    fn waiters_in_one_process_are_served_in_arrival_order() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("settings.lock");
        let holder = try_lock(&path)?;
        let metadata = std::fs::metadata(&path)?;
        let file = (metadata.dev(), metadata.ino());
        let waiting = || lines().waiting.get(&file).map_or(0, VecDeque::len);
        let served = Mutex::new(Vec::new());

        std::thread::scope(|scope| -> io::Result<()> {
            let waiters: Vec<_> = (0..6)
                .map(|index| {
                    let (path, served) = (&path, &served);
                    let waiter = scope.spawn(move || -> io::Result<()> {
                        let _lock = acquire(path, Duration::from_secs(10))?;
                        served
                            .lock()
                            .map_err(|_| io::Error::other("poisoned"))?
                            .push(index);
                        Ok(())
                    });
                    let queued = Instant::now() + Duration::from_secs(10);
                    while waiting() <= index && Instant::now() < queued {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    waiter
                })
                .collect();
            drop(holder);
            waiters.into_iter().try_for_each(|waiter| {
                waiter
                    .join()
                    .map_err(|_| io::Error::other("waiter panicked"))?
            })
        })?;

        let served = served
            .into_inner()
            .map_err(|_| io::Error::other("poisoned"))?;
        assert_eq!(served, (0..6).collect::<Vec<_>>());
        assert_eq!(waiting(), 0);
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
