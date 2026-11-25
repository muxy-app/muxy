#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::file_lock::FileLock;

pub fn private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
        restrict_directory(path)
    }
    #[cfg(windows)]
    {
        fs::create_dir_all(path)?;
        windows::protect_directory(path)
    }
}

#[cfg(unix)]
fn restrict_directory(path: &Path) -> io::Result<()> {
    use rustix::fs::{Mode, OFlags};

    let link = fs::symlink_metadata(path)?.file_type().is_symlink();
    let mut flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    if !link {
        flags |= OFlags::NOFOLLOW;
    }
    let directory = rustix::fs::open(path, flags, Mode::empty())?;
    let stat = rustix::fs::fstat(&directory)?;
    if stat.st_uid != rustix::process::geteuid().as_raw() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} belongs to another user", path.display()),
        ));
    }
    let mode = stat.st_mode & 0o7777;
    let shared = mode & 0o077;
    if shared == 0 {
        return Ok(());
    }
    if link {
        if mode & 0o022 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("other users can change {}", path.display()),
            ));
        }
        return Ok(());
    }
    Ok(rustix::fs::fchmod(
        &directory,
        Mode::from_raw_mode(mode & !0o077),
    )?)
}

pub fn private_file() -> OpenOptions {
    #[cfg_attr(windows, allow(unused_mut))]
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
}

pub fn sync_directory(directory: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(directory)?.sync_all()
    }
    #[cfg(windows)]
    {
        let _ = directory;
        Ok(())
    }
}

pub fn lock_directory(directory: &Path, wait: Duration) -> io::Result<FileLock> {
    #[cfg(unix)]
    let file = File::open(directory)?;
    #[cfg(windows)]
    let file = private_file()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(".lock"))?;
    let deadline = Instant::now().checked_add(wait);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(FileLock(file)),
            Err(fs::TryLockError::WouldBlock)
                if deadline.is_none_or(|deadline| Instant::now() < deadline) =>
            {
                thread::sleep(deadline.map_or(Duration::from_millis(10), |deadline| {
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(10))
                }));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileId {
    device: u64,
    index: u64,
}

impl FileId {
    pub fn of(file: &File) -> io::Result<Self> {
        #[cfg(unix)]
        {
            Ok(Self::from_metadata(&file.metadata()?))
        }
        #[cfg(windows)]
        {
            windows::file_id(file).map(|(device, index)| Self { device, index })
        }
    }

    pub fn of_path(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        {
            Ok(Self::from_metadata(&fs::metadata(path)?))
        }
        #[cfg(windows)]
        {
            Self::open(path, 0)
        }
    }

    pub fn of_link(path: &Path) -> io::Result<Self> {
        #[cfg(unix)]
        {
            Ok(Self::from_metadata(&fs::symlink_metadata(path)?))
        }
        #[cfg(windows)]
        {
            Self::open(
                path,
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            )
        }
    }

    pub fn device(self) -> u64 {
        self.device
    }

    pub fn index(self) -> u64 {
        self.index
    }

    #[cfg(windows)]
    fn open(path: &Path, flags: u32) -> io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        let file = OpenOptions::new()
            .access_mode(0)
            .custom_flags(
                flags | windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS,
            )
            .open(path)?;
        Self::of(&file)
    }

    #[cfg(unix)]
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            device: metadata.dev(),
            index: metadata.ino(),
        }
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}-{:x}", self.device, self.index)
    }
}

pub fn same_file(first: &Path, second: &Path) -> io::Result<bool> {
    Ok(FileId::of_path(first)? == FileId::of_path(second)?)
}

pub fn replace_file(source: &Path, target: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::rename(source, target)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{
            ERROR_ACCESS_DENIED, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION,
        };
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match fs::rename(source, target) {
                Err(error)
                    if error.raw_os_error().is_some_and(|code| {
                        [
                            ERROR_ACCESS_DENIED,
                            ERROR_SHARING_VIOLATION,
                            ERROR_LOCK_VIOLATION,
                        ]
                        .contains(&code.cast_unsigned())
                    }) && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(20));
                }
                result => return result,
            }
        }
    }
}

pub fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    let path = fs::canonicalize(path)?;
    #[cfg(windows)]
    if let Some(plain) = path.to_str().and_then(strip_verbatim) {
        return Ok(PathBuf::from(plain));
    }
    Ok(path)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn strip_verbatim(path: &str) -> Option<String> {
    let (plain, rest) = if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        (format!(r"\\{rest}"), rest)
    } else {
        let rest = path.strip_prefix(r"\\?\")?;
        let drive = rest.get(..3)?.as_bytes();
        if !drive[0].is_ascii_alphabetic() || &drive[1..] != br":\" {
            return None;
        }
        if rest.len() == 3 {
            return Some(rest.to_owned());
        }
        (rest.to_owned(), &rest[3..])
    };
    rest.split('\\').all(plain_component).then_some(plain)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn plain_component(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let device = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|port| {
        stem.strip_prefix(port).is_some_and(|number| {
            matches!(
                number,
                "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    !component.is_empty() && !device && !component.ends_with(['.', ' ']) && !component.contains('/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn verbatim_prefixes_are_removed_only_when_the_path_keeps_its_meaning() {
        for (verbatim, plain) in [
            (r"\\?\C:\", Some(r"C:\")),
            (r"\\?\C:\Users\me\Muxy", Some(r"C:\Users\me\Muxy")),
            (r"\\?\d:\a b\c.txt", Some(r"d:\a b\c.txt")),
            (r"\\?\UNC\host\share\dir", Some(r"\\host\share\dir")),
            (
                r"\\?\C:\COMfort\NULL\Console",
                Some(r"C:\COMfort\NULL\Console"),
            ),
            (r"C:\already\plain", None),
            (r"\\?\Volume{1234}\dir", None),
            (r"\\?\C:", None),
            (r"\\?\C:\dir.", None),
            (r"\\?\C:\dir \file", None),
            (r"\\?\C:\a\NUL", None),
            (r"\\?\C:\a\nul.txt", None),
            (r"\\?\C:\a\com1", None),
            (r"\\?\C:\a\LPT0.log", None),
            (r"\\?\C:\a\LPT²", None),
            (r"\\?\C:\a\CONIN$", None),
            (r"\\?\C:\a/b", None),
            (r"\\?\C:\a\\b", None),
            (r"\\?\UNC\host\share\..\x", None),
            (r"\\?\UNC\", None),
        ] {
            assert_eq!(strip_verbatim(verbatim).as_deref(), plain, "{verbatim}");
        }
    }

    #[test]
    fn canonical_paths_have_no_verbatim_prefix() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let canonical = canonicalize(directory.path())?;
        assert!(canonical.is_absolute());
        assert!(!canonical.to_string_lossy().starts_with(r"\\?\"));
        assert!(same_file(&canonical, directory.path())?);
        Ok(())
    }

    #[test]
    fn private_files_inside_private_directories_open_only_for_the_user() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let directory = root.path().join("first").join("second");
        private_dir(&directory)?;
        private_dir(&directory)?;
        let path = directory.join("secret");
        private_file()
            .write(true)
            .create_new(true)
            .open(&path)?
            .write_all(b"secret")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&directory)?.permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        }
        #[cfg(windows)]
        {
            let user = windows::sddl_trustee(&windows::current_user_sid()?)?;
            let directory = windows::dacl(&directory)?;
            assert!(directory.starts_with("D:P"), "{directory}");
            assert!(
                directory.ends_with(&format!("(A;OICI;FA;;;{user})(A;OICI;FA;;;SY)")),
                "{directory}"
            );
            let file = windows::dacl(&path)?;
            assert!(
                file.ends_with(&format!("(A;ID;FA;;;{user})(A;ID;FA;;;SY)")),
                "{file}"
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn an_existing_folder_loses_other_users_access() -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir()?;
        let directory = root.path().join("profile");
        fs::create_dir(&directory)?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o777))?;
        private_dir(&directory)?;
        assert_eq!(
            fs::metadata(&directory)?.permissions().mode() & 0o777,
            0o700
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_folder_is_kept_unless_other_users_can_change_it() -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir()?;
        let target = root.path().join("target");
        let link = root.path().join("link");
        fs::create_dir(&target)?;
        std::os::unix::fs::symlink(&target, &link)?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
        private_dir(&link)?;
        assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o777, 0o755);
        fs::set_permissions(&target, fs::Permissions::from_mode(0o777))?;
        let error = private_dir(&link).err().map(|error| error.kind());
        assert_eq!(error, Some(io::ErrorKind::PermissionDenied));
        assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o777, 0o777);
        Ok(())
    }

    #[test]
    fn directory_sync_succeeds_after_a_rename() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        fs::write(directory.path().join("a"), b"")?;
        fs::rename(directory.path().join("a"), directory.path().join("b"))?;
        sync_directory(directory.path())
    }

    #[test]
    fn a_locked_directory_stays_locked_until_its_owner_drops() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let owner = lock_directory(directory.path(), Duration::ZERO)?;
        let started = Instant::now();
        let error = lock_directory(directory.path(), Duration::from_millis(50))
            .err()
            .map(|error| error.kind());
        assert_eq!(error, Some(io::ErrorKind::WouldBlock));
        assert!(started.elapsed() >= Duration::from_millis(50));
        drop(owner);
        lock_directory(directory.path(), Duration::MAX).map(drop)
    }

    #[test]
    fn a_link_has_its_own_identity_apart_from_its_target() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("target");
        let link = directory.path().join("link");
        fs::write(&target, b"")?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&target, &link)?;
        assert_eq!(FileId::of_path(&link)?, FileId::of_path(&target)?);
        assert_ne!(FileId::of_link(&link)?, FileId::of_path(&target)?);
        assert!(same_file(&link, &target)?);
        Ok(())
    }

    #[test]
    fn identities_follow_files_through_renames_and_differ_between_files() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        fs::write(&first, b"first")?;
        fs::write(&second, b"second")?;
        let opened = File::open(&first)?;
        assert_eq!(FileId::of(&opened)?, FileId::of_path(&first)?);
        assert!(!same_file(&first, &second)?);
        assert!(same_file(directory.path(), &directory.path().join("."))?);
        assert_eq!(FileId::of_link(&first)?, FileId::of_path(&first)?);
        fs::rename(&first, directory.path().join("moved"))?;
        assert_eq!(
            FileId::of(&opened)?,
            FileId::of_path(&directory.path().join("moved"))?
        );
        Ok(())
    }

    #[test]
    fn replacing_a_file_swaps_its_contents_and_keeps_open_readers_working() -> io::Result<()> {
        use std::io::Read;
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("settings");
        let source = directory.path().join("settings.tmp");
        fs::write(&target, b"old")?;
        fs::write(&source, b"new")?;
        let mut reader = File::open(&target)?;
        replace_file(&source, &target)?;
        assert_eq!(fs::read(&target)?, b"new");
        assert!(!source.exists());
        let mut old = String::new();
        reader.read_to_string(&mut old)?;
        assert_eq!(old, "old");
        Ok(())
    }
}
