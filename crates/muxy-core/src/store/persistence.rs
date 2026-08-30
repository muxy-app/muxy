use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct AtomicWriteError {
    error: std::io::Error,
    publication_may_have_succeeded: bool,
}

impl AtomicWriteError {
    pub fn publication_may_have_succeeded(&self) -> bool {
        self.publication_may_have_succeeded
    }
}

impl std::fmt::Display for AtomicWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for AtomicWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<AtomicWriteError> for std::io::Error {
    fn from(error: AtomicWriteError) -> Self {
        let kind = error.error.kind();
        Self::new(kind, error)
    }
}

pub fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let temp = stage(path, contents, false)?;
    publish_replacing(&temp, path).map_err(Into::into)
}

pub fn write_private_durable(path: &Path, contents: &[u8]) -> Result<(), AtomicWriteError> {
    let temp = stage(path, contents, true).map_err(|error| AtomicWriteError {
        error,
        publication_may_have_succeeded: false,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if let Err(error) = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))
        {
            let _ = std::fs::remove_file(&temp);
            return Err(AtomicWriteError {
                error,
                publication_may_have_succeeded: false,
            });
        }
    }
    publish_replacing(&temp, path)
}

pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    write_private_durable(path, contents).map_err(Into::into)
}

fn stage(path: &Path, contents: &[u8], private: bool) -> std::io::Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    loop {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut name = path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("muxy"))
            .to_os_string();
        name.push(format!(".{}.{}.tmp", std::process::id(), sequence));
        let temp = parent.join(name);
        let opened = open_staging_file(&temp, private);
        let mut file = match opened {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let result = file.write_all(contents).and_then(|()| file.sync_all());
        if let Err(error) = result {
            drop(file);
            let _ = std::fs::remove_file(&temp);
            return Err(error);
        }
        return Ok(temp);
    }
}

#[cfg(unix)]
fn open_staging_file(path: &Path, private: bool) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(if private { 0o600 } else { 0o666 })
        .open(path)
}

#[cfg(not(unix))]
fn open_staging_file(path: &Path, _private: bool) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

fn publish_replacing(temp: &Path, path: &Path) -> Result<(), AtomicWriteError> {
    publish_replacing_with_sync(temp, path, || sync_parent(path))
}

fn publish_replacing_with_sync(
    temp: &Path,
    path: &Path,
    sync: impl FnOnce() -> std::io::Result<()>,
) -> Result<(), AtomicWriteError> {
    if let Err(error) = std::fs::rename(temp, path) {
        let _ = std::fs::remove_file(temp);
        return Err(AtomicWriteError {
            error,
            publication_may_have_succeeded: false,
        });
    }
    sync().map_err(|error| AtomicWriteError {
        error,
        publication_may_have_succeeded: true,
    })
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> std::io::Result<()> {
    std::fs::File::open(path.parent().unwrap_or_else(|| Path::new(".")))?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent(_: &Path) -> std::io::Result<()> {
    Ok(())
}

pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let contents = serde_json::to_vec(value)?;
    write_atomic(path, &contents)
}

#[cfg(unix)]
#[derive(Debug)]
pub struct PrivateDirectory {
    path: PathBuf,
    parent_path: PathBuf,
    directory: std::fs::File,
    parent: std::fs::File,
    name: std::ffi::CString,
    device: libc::dev_t,
    inode: libc::ino_t,
    parent_device: libc::dev_t,
    parent_inode: libc::ino_t,
}

#[cfg(not(unix))]
#[derive(Debug)]
pub struct PrivateDirectory {
    path: PathBuf,
}

impl PrivateDirectory {
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(unix)]
    pub fn write_new_atomic(&self, name: &str, contents: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::fd::{AsRawFd, FromRawFd};

        let final_name = private_basename(name)?;
        self.revalidate()?;
        loop {
            let staging_name = private_basename(&format!(
                ".{}.{}.tmp",
                crate::store::new_uuid(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ))?;
            let descriptor = unsafe {
                libc::openat(
                    self.directory.as_raw_fd(),
                    staging_name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if descriptor < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    continue;
                }
                return Err(error);
            }
            let mut file = unsafe { std::fs::File::from_raw_fd(descriptor) };
            let result = file.write_all(contents).and_then(|()| file.sync_all());
            drop(file);
            if let Err(error) = result {
                self.unlink_name(&staging_name);
                return Err(error);
            }
            if let Err(error) = self.revalidate() {
                self.unlink_name(&staging_name);
                return Err(error);
            }
            let published = publish_new_at(self.directory.as_raw_fd(), &staging_name, &final_name);
            if let Err(error) = published {
                self.unlink_name(&staging_name);
                return Err(error);
            }
            self.directory.sync_all()?;
            return Ok(());
        }
    }

    #[cfg(not(unix))]
    pub fn write_new_atomic(&self, _: &str, _: &[u8]) -> std::io::Result<()> {
        Err(unsupported_private_directory_operation())
    }

    #[cfg(unix)]
    pub fn read_regular(&self, name: &str) -> std::io::Result<Vec<u8>> {
        use std::io::Read;
        use std::os::fd::{AsRawFd, FromRawFd};

        let name = private_basename(name)?;
        self.revalidate()?;
        let descriptor = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if descriptor < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut file = unsafe { std::fs::File::from_raw_fd(descriptor) };
        ensure_regular_descriptor(file.as_raw_fd())?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        Ok(contents)
    }

    #[cfg(not(unix))]
    pub fn read_regular(&self, _: &str) -> std::io::Result<Vec<u8>> {
        Err(unsupported_private_directory_operation())
    }

    #[cfg(unix)]
    pub fn remove_regular(&self, name: &str) -> std::io::Result<bool> {
        use std::os::fd::AsRawFd;

        let name = private_basename(name)?;
        self.revalidate()?;
        let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
        let status = unsafe {
            libc::fstatat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                metadata.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if status != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(false);
            }
            return Err(error);
        }
        let metadata = unsafe { metadata.assume_init() };
        if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "private directory entry is not a regular file",
            ));
        }
        let status = unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) };
        if status != 0 {
            return Err(std::io::Error::last_os_error());
        }
        self.directory.sync_all()?;
        Ok(true)
    }

    #[cfg(not(unix))]
    pub fn remove_regular(&self, _: &str) -> std::io::Result<bool> {
        Err(unsupported_private_directory_operation())
    }

    #[cfg(unix)]
    pub fn regular_file_names(&self) -> std::io::Result<Vec<String>> {
        use std::os::fd::AsRawFd;

        self.revalidate()?;
        let duplicated = unsafe { libc::dup(self.directory.as_raw_fd()) };
        if duplicated < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe { libc::lseek(duplicated, 0, libc::SEEK_SET) } < 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                libc::close(duplicated);
            }
            return Err(error);
        }
        let stream = unsafe { libc::fdopendir(duplicated) };
        if stream.is_null() {
            unsafe {
                libc::close(duplicated);
            }
            return Err(std::io::Error::last_os_error());
        }
        let mut names = Vec::new();
        loop {
            let entry = unsafe { libc::readdir(stream) };
            if entry.is_null() {
                break;
            }
            let raw = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) };
            let bytes = raw.to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            let Ok(name) = std::str::from_utf8(bytes) else {
                continue;
            };
            let Ok(name) = private_basename(name) else {
                continue;
            };
            let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
            let status = unsafe {
                libc::fstatat(
                    self.directory.as_raw_fd(),
                    name.as_ptr(),
                    metadata.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if status == 0
                && unsafe { metadata.assume_init() }.st_mode & libc::S_IFMT == libc::S_IFREG
            {
                names.push(name.to_string_lossy().into_owned());
            }
        }
        let close_status = unsafe { libc::closedir(stream) };
        if close_status != 0 {
            return Err(std::io::Error::last_os_error());
        }
        names.sort();
        Ok(names)
    }

    #[cfg(not(unix))]
    pub fn regular_file_names(&self) -> std::io::Result<Vec<String>> {
        Err(unsupported_private_directory_operation())
    }

    #[cfg(unix)]
    fn revalidate(&self) -> std::io::Result<()> {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::MetadataExt;

        let parent_descriptor = descriptor_metadata(self.parent.as_raw_fd())?;
        let parent_path = std::fs::symlink_metadata(&self.parent_path)?;
        if parent_descriptor.st_dev != self.parent_device
            || parent_descriptor.st_ino != self.parent_inode
            || !is_owned_directory(&parent_descriptor, unsafe { libc::geteuid() })
            || parent_path.file_type().is_symlink()
            || !parent_path.is_dir()
            || parent_path.dev() as libc::dev_t != self.parent_device
            || parent_path.ino() as libc::ino_t != self.parent_inode
            || parent_path.uid() != unsafe { libc::geteuid() }
        {
            return Err(private_directory_identity_error());
        }
        let descriptor = descriptor_metadata(self.directory.as_raw_fd())?;
        if descriptor.st_dev != self.device
            || descriptor.st_ino != self.inode
            || !is_owned_directory(&descriptor, unsafe { libc::geteuid() })
        {
            return Err(private_directory_identity_error());
        }
        let mut linked = std::mem::MaybeUninit::<libc::stat>::uninit();
        let status = unsafe {
            libc::fstatat(
                self.parent.as_raw_fd(),
                self.name.as_ptr(),
                linked.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if status != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let linked = unsafe { linked.assume_init() };
        if linked.st_dev != self.device
            || linked.st_ino != self.inode
            || linked.st_mode & libc::S_IFMT != libc::S_IFDIR
            || linked.st_uid != unsafe { libc::geteuid() }
        {
            return Err(private_directory_identity_error());
        }
        Ok(())
    }

    #[cfg(unix)]
    fn unlink_name(&self, name: &std::ffi::CStr) {
        use std::os::fd::AsRawFd;

        unsafe {
            libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0);
        }
    }
}

#[cfg(not(unix))]
fn unsupported_private_directory_operation() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "safe private directory operations are unavailable on this target",
    )
}

#[cfg(unix)]
fn private_basename(name: &str) -> std::io::Result<std::ffi::CString> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "private directory entry must be one basename",
        ));
    }
    std::ffi::CString::new(name).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "private directory entry contains NUL",
        )
    })
}

#[cfg(unix)]
fn descriptor_metadata(descriptor: std::os::fd::RawFd) -> std::io::Result<libc::stat> {
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(descriptor, metadata.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { metadata.assume_init() })
}

#[cfg(unix)]
fn is_owned_directory(metadata: &libc::stat, owner: libc::uid_t) -> bool {
    metadata.st_mode & libc::S_IFMT == libc::S_IFDIR && metadata.st_uid == owner
}

#[cfg(unix)]
fn ensure_regular_descriptor(descriptor: std::os::fd::RawFd) -> std::io::Result<()> {
    let metadata = descriptor_metadata(descriptor)?;
    if metadata.st_mode & libc::S_IFMT != libc::S_IFREG || metadata.st_nlink != 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "private directory entry is not an exclusive regular file",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn private_directory_identity_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "private directory identity changed",
    )
}

#[cfg(target_os = "macos")]
fn publish_new_at(
    descriptor: std::os::fd::RawFd,
    staging: &std::ffi::CStr,
    destination: &std::ffi::CStr,
) -> std::io::Result<()> {
    let status = unsafe {
        libc::renameatx_np(
            descriptor,
            staging.as_ptr(),
            descriptor,
            destination.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
fn publish_new_at(
    descriptor: std::os::fd::RawFd,
    staging: &std::ffi::CStr,
    destination: &std::ffi::CStr,
) -> std::io::Result<()> {
    let status = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            descriptor,
            staging.as_ptr(),
            descriptor,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if status == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
fn publish_new_at(
    _: std::os::fd::RawFd,
    _: &std::ffi::CStr,
    _: &std::ffi::CStr,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "exclusive descriptor-relative rename is unavailable on this target",
    ))
}

#[cfg(unix)]
pub fn ensure_private_directory(path: &Path) -> std::io::Result<PrivateDirectory> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::OpenOptionsExt;

    let parent_path = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "private directory requires a parent",
        )
    })?;
    let name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "private directory requires a UTF-8 basename",
            )
        })?;
    let name = private_basename(name)?;
    std::fs::create_dir_all(parent_path)?;
    let parent = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(parent_path)?;
    let parent_metadata = descriptor_metadata(parent.as_raw_fd())?;
    if !is_owned_directory(&parent_metadata, unsafe { libc::geteuid() }) {
        return Err(private_directory_identity_error());
    }
    let created = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
    if created != 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error);
        }
    }
    let descriptor = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let directory = unsafe { std::fs::File::from_raw_fd(descriptor) };
    let metadata = descriptor_metadata(directory.as_raw_fd())?;
    if !is_owned_directory(&metadata, unsafe { libc::geteuid() }) {
        return Err(private_directory_identity_error());
    }
    if unsafe { libc::fchmod(directory.as_raw_fd(), 0o700) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let capability = PrivateDirectory {
        path: path.to_owned(),
        parent_path: parent_path.to_owned(),
        directory,
        parent,
        name,
        device: metadata.st_dev,
        inode: metadata.st_ino,
        parent_device: parent_metadata.st_dev,
        parent_inode: parent_metadata.st_ino,
    };
    capability.revalidate()?;
    Ok(capability)
}

#[cfg(not(unix))]
pub fn ensure_private_directory(path: &Path) -> std::io::Result<PrivateDirectory> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "private directory path is not a real directory",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(path)?;
        }
        Err(error) => return Err(error),
    }
    Ok(PrivateDirectory {
        path: path.to_owned(),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn mode(path: &std::path::Path) -> u32 {
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    #[test]
    fn private_staging_is_private_from_creation() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("private.json");
        let staged = super::stage(&path, b"{}", true).expect("stage");
        assert_eq!(mode(&staged), 0o600);
    }

    #[test]
    fn write_private_restricts_permissions() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("private.json");
        super::write_private(&path, b"{}").expect("write");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(std::fs::read(&path).expect("read"), b"{}");
    }

    #[test]
    fn write_private_downgrades_an_existing_readable_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("private.json");
        super::write_atomic(&path, b"old").expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!(mode(&path), 0o644);

        super::write_private(&path, b"new").expect("write");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(std::fs::read(&path).expect("read"), b"new");
    }

    #[test]
    fn write_private_does_not_follow_a_predictable_temporary_symlink() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("private.json");
        let external = dir.path().join("external.json");
        std::fs::write(&external, b"external").expect("external");
        symlink(&external, dir.path().join("private.json.tmp")).expect("symlink");

        super::write_private(&path, b"private").expect("write");
        assert_eq!(std::fs::read(&external).expect("external"), b"external");
        assert_eq!(std::fs::read(&path).expect("private"), b"private");
    }

    #[test]
    fn private_directory_metadata_rejects_non_directories_and_wrong_owners() {
        let root = tempfile::tempdir().unwrap();
        let directory = std::fs::File::open(root.path()).unwrap();
        let metadata = super::descriptor_metadata(directory.as_raw_fd()).unwrap();
        assert!(super::is_owned_directory(&metadata, unsafe {
            libc::geteuid()
        }));
        assert!(!super::is_owned_directory(
            &metadata,
            unsafe { libc::geteuid() }.wrapping_add(1)
        ));
        let file = root.path().join("file");
        std::fs::write(&file, b"file").unwrap();
        let file = std::fs::File::open(file).unwrap();
        let metadata = super::descriptor_metadata(file.as_raw_fd()).unwrap();
        assert!(!super::is_owned_directory(&metadata, unsafe {
            libc::geteuid()
        }));
    }

    #[test]
    fn private_directory_capability_creates_private_directory_and_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("images");
        let directory = super::ensure_private_directory(&path).unwrap();
        assert_eq!(mode(&path), 0o700);
        directory.write_new_atomic("image.png", b"image").unwrap();
        assert_eq!(mode(&path.join("image.png")), 0o600);
        assert_eq!(directory.read_regular("image.png").unwrap(), b"image");
        assert_eq!(directory.regular_file_names().unwrap(), ["image.png"]);
    }

    #[test]
    fn private_directory_capability_rejects_symlinks_non_directories_and_traversal() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let linked = root.path().join("linked");
        symlink(&outside, &linked).unwrap();
        assert!(super::ensure_private_directory(&linked).is_err());
        let file = root.path().join("file");
        std::fs::write(&file, b"file").unwrap();
        assert!(super::ensure_private_directory(&file).is_err());
        let directory = super::ensure_private_directory(&root.path().join("images")).unwrap();
        for name in ["", ".", "..", "../outside", "folder/image"] {
            assert!(
                directory.write_new_atomic(name, b"image").is_err(),
                "{name}"
            );
            assert!(directory.read_regular(name).is_err(), "{name}");
            assert!(directory.remove_regular(name).is_err(), "{name}");
        }
    }

    #[test]
    fn private_directory_capability_rejects_replaced_path_identity() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("images");
        let directory = super::ensure_private_directory(&path).unwrap();
        let held = root.path().join("held");
        std::fs::rename(&path, &held).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(directory.write_new_atomic("image.png", b"image").is_err());
        assert!(!path.join("image.png").exists());
        assert!(!held.join("image.png").exists());
    }

    #[test]
    fn private_directory_capability_rejects_replaced_parent_identity() {
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("profile");
        std::fs::create_dir(&profile).unwrap();
        let path = profile.join("images");
        let directory = super::ensure_private_directory(&path).unwrap();
        let held = root.path().join("held-profile");
        std::fs::rename(&profile, &held).unwrap();
        std::fs::create_dir(&profile).unwrap();
        std::fs::create_dir(profile.join("images")).unwrap();
        assert!(directory.write_new_atomic("image.png", b"image").is_err());
        assert!(!profile.join("images/image.png").exists());
        assert!(!held.join("images/image.png").exists());
    }

    #[test]
    fn private_directory_capability_never_reads_or_enumerates_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("images");
        let directory = super::ensure_private_directory(&path).unwrap();
        let outside = root.path().join("outside");
        std::fs::write(&outside, b"outside").unwrap();
        symlink(&outside, path.join("linked.png")).unwrap();
        assert!(directory.read_regular("linked.png").is_err());
        assert!(directory.regular_file_names().unwrap().is_empty());
        assert!(directory.remove_regular("linked.png").is_err());
        assert_eq!(std::fs::read(outside).unwrap(), b"outside");
    }

    #[test]
    fn private_directory_capability_never_replaces_an_existing_destination() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("images");
        let directory = super::ensure_private_directory(&path).unwrap();
        directory.write_new_atomic("image.png", b"first").unwrap();
        let error = directory
            .write_new_atomic("image.png", b"second")
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(directory.read_regular("image.png").unwrap(), b"first");
    }

    #[test]
    fn replacing_publication_reports_an_ambiguous_parent_sync_failure() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("drafts.json");
        let staged = super::stage(&path, b"new", true).unwrap();
        let error = super::publish_replacing_with_sync(&staged, &path, || {
            Err(std::io::Error::other("injected parent sync failure"))
        })
        .unwrap_err();
        assert!(error.publication_may_have_succeeded());
        assert_eq!(std::fs::read(path).unwrap(), b"new");
    }
}

#[cfg(all(test, not(unix)))]
mod non_unix_tests {
    #[test]
    fn private_directory_creation_is_neutral_and_capability_operations_fail_closed() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("images");
        let directory = super::ensure_private_directory(&path).unwrap();
        assert!(path.is_dir());
        assert!(directory.write_new_atomic("image.png", b"image").is_err());
        assert!(directory.read_regular("image.png").is_err());
        assert!(directory.remove_regular("image.png").is_err());
        assert!(directory.regular_file_names().is_err());
    }
}
