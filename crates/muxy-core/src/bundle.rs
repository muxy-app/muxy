use crate::file_lock::{self, FileLock};
use crate::files::{FileId, private_dir, private_file};
use std::fs::{self, File, OpenOptions};
use std::io;
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

fn create_replacement_lock(directory: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o644);
    options.open(replacement_lock(directory))
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
    if cfg!(windows) {
        return Ok(None);
    }
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
    FileId::of_path(bundle).map(|id| id.to_string())
}

fn lease_directory() -> io::Result<PathBuf> {
    Ok(crate::paths::home_dir()?.join("Library/Application Support/Muxy Runtime/leases"))
}

fn lease_file(directory: &Path, identity: &str) -> io::Result<File> {
    private_dir(directory)?;
    private_file()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
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

    #[cfg(unix)]
    fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
    }

    #[cfg(unix)]
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let mode = fs::metadata(replacement_lock(directory.path()))?.mode() & 0o777;
            assert_eq!(mode & !0o644, 0, "only the owner may write the lock");
        }
        assert_eq!(
            lock_replacement(directory.path())
                .err()
                .map(|error| error.kind()),
            Some(io::ErrorKind::WouldBlock)
        );
        drop((first, second));

        let update = lock_replacement(directory.path())?;
        let started = std::time::Instant::now();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            drop(update);
        });
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
        lease.unlock()?;
        old.try_lock()?;
        Ok(())
    }
}
