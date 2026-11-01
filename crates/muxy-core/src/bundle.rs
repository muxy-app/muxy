use crate::file_lock::{self, FileLock};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

const REPLACEMENT_BUSY: &str = "A Muxy installation or update is already in progress";

#[derive(Debug)]
pub struct BundleLease {
    _file: File,
}

pub fn lock_replacement(directory: &Path) -> io::Result<FileLock> {
    file_lock::try_lock_file(create_replacement_lock(directory)?)
        .map_err(file_lock::busy(REPLACEMENT_BUSY))
}

/// Waits while an installation or update replaces Muxy in `directory`. Starts
/// share the lock, so they never wait for each other. A user who can't open or
/// create the lock can't replace Muxy there either, so starts without it.
fn wait_for_replacement(directory: &Path) -> io::Result<Option<FileLock>> {
    let file = match File::open(replacement_lock(directory)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => create_replacement_lock(directory),
        opened => opened,
    };
    let file = match file {
        Ok(file) => file,
        Err(error) if lacks_access(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    file_lock::acquire_shared(file, Duration::from_secs(3))
        .map(Some)
        .map_err(file_lock::busy(REPLACEMENT_BUSY))
}

/// Opens the replacement lock for this user's installations and updates,
/// readable by every user who starts Muxy from `directory`.
fn create_replacement_lock(directory: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o644)
        .open(replacement_lock(directory))
}

fn lacks_access(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
    )
}

fn replacement_lock(directory: &Path) -> PathBuf {
    directory.join(".muxy-beta-update.lock")
}

/// The app bundle whose `Contents/MacOS` holds `executable`.
pub fn containing(executable: &Path) -> Option<&Path> {
    executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
}

pub fn acquire_runtime(
    executable: &Path,
    expected_metadata: &[u8],
) -> io::Result<Option<BundleLease>> {
    #[cfg(target_os = "linux")]
    if let Some(root) = linux_root(executable) {
        let directory = executable
            .parent()
            .ok_or_else(|| io::Error::other("Missing executable directory"))?;
        let _replacement = wait_for_replacement(root)?;
        if crate::executable::build_metadata(executable)? != expected_metadata {
            return Err(io::Error::other(
                "The installed Muxy changed during startup. Run the command again.",
            ));
        }
        let file = match File::open(directory.join(".lease")) {
            Ok(file) => file,
            Err(error) if lacks_access(&error) => return Ok(None),
            Err(error) => return Err(error),
        };
        file.lock_shared()?;
        return Ok(Some(BundleLease { _file: file }));
    }
    let Some(bundle) = containing(executable) else {
        return Ok(None);
    };
    let identity = identity(bundle)?;
    let _replacement = wait_for_replacement(
        bundle
            .parent()
            .ok_or_else(|| io::Error::other("Missing application directory"))?,
    )?;
    if self::identity(bundle)? != identity
        || crate::executable::build_metadata(executable)? != expected_metadata
    {
        return Err(io::Error::other(
            "The installed Muxy changed during startup. Run the command again.",
        ));
    }
    let file = lease_file(&lease_directory()?, &identity)?;
    file.lock_shared()?;
    Ok(Some(BundleLease { _file: file }))
}

/// The root of a versioned, user-installed Linux desktop, including its matching helpers.
#[cfg(target_os = "linux")]
pub fn linux_root(executable: &Path) -> Option<&Path> {
    let version = executable.parent()?;
    let versions = version.parent()?;
    let root = versions.parent()?;
    (versions.file_name()? == "versions"
        && crate::release::Version::parse(version.file_name()?.to_str()?).is_some()
        && root.join("installation.json").is_file())
    .then_some(root)
}

pub fn lock_unused(bundle: &Path) -> io::Result<Option<File>> {
    let file = lease_file(&lease_directory()?, &identity(bundle)?)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(error)) => Err(error),
    }
}

fn identity(bundle: &Path) -> io::Result<String> {
    let metadata = fs::metadata(bundle)?;
    Ok(format!("{:x}-{:x}", metadata.dev(), metadata.ino()))
}

fn lease_directory() -> io::Result<PathBuf> {
    let home =
        std::env::home_dir().ok_or_else(|| io::Error::other("Home directory unavailable"))?;
    Ok(home.join("Library/Application Support/Muxy Runtime/leases"))
}

fn lease_file(directory: &Path, identity: &str) -> io::Result<File> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(directory.join(format!("{identity}.lock")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_executables_inside_an_app_bundle_have_one() {
        assert_eq!(
            containing(Path::new("/Applications/Muxy.app/Contents/MacOS/muxy-app")),
            Some(Path::new("/Applications/Muxy.app"))
        );
        assert_eq!(containing(Path::new("/repo/target/debug/muxy-app")), None);
    }

    fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
    }

    #[test]
    fn starting_needs_no_access_to_the_replacement_lock() -> io::Result<()> {
        let read_only = tempfile::tempdir()?;
        set_mode(read_only.path(), 0o555)?;
        let started = wait_for_replacement(read_only.path());
        let replaced = lock_replacement(read_only.path());
        set_mode(read_only.path(), 0o755)?;
        started?;
        if rustix::process::geteuid().is_root() {
            return Ok(());
        }
        assert_eq!(
            replaced.err().map(|error| error.kind()),
            Some(io::ErrorKind::PermissionDenied)
        );

        let unreadable = tempfile::tempdir()?;
        drop(lock_replacement(unreadable.path())?);
        set_mode(&replacement_lock(unreadable.path()), 0o200)?;
        assert!(wait_for_replacement(unreadable.path())?.is_none());
        Ok(())
    }

    #[test]
    fn starts_share_the_replacement_lock_and_wait_while_an_update_holds_it() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let first = wait_for_replacement(directory.path())?;
        let second = wait_for_replacement(directory.path())?;
        assert!(first.is_some() && second.is_some());
        assert_eq!(
            fs::metadata(replacement_lock(directory.path()))?.mode() & 0o777,
            0o644
        );
        assert!(lock_replacement(directory.path()).is_err());
        drop((first, second));

        let update = lock_replacement(directory.path())?;
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(update);
        });
        let started = std::time::Instant::now();
        assert!(wait_for_replacement(directory.path())?.is_some());
        assert!(started.elapsed() >= Duration::from_millis(200));
        release
            .join()
            .map_err(|_| io::Error::other("release panicked"))?;
        Ok(())
    }

    #[test]
    fn leases_follow_bundle_identity_through_replacement_without_holding_installation_lock()
    -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let current = root.path().join("Muxy Beta.app");
        let retired = root.path().join("previous.app");
        fs::create_dir(&current)?;
        let lease = lease_file(root.path(), &identity(&current)?)?;
        lease.lock_shared()?;
        let _installation = lock_replacement(root.path())?;
        fs::rename(&current, &retired)?;
        fs::create_dir(&current)?;
        let old = lease_file(root.path(), &identity(&retired)?)?;
        assert!(matches!(old.try_lock(), Err(fs::TryLockError::WouldBlock)));
        let new = lease_file(root.path(), &identity(&current)?)?;
        new.try_lock()?;
        // Closing alone keeps the lock while a child spawned by another test still shares the file.
        lease.unlock()?;
        old.try_lock()?;
        Ok(())
    }
}
