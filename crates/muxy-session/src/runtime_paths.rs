use muxy_core::environment::{BuildMode, RuntimePathPolicy};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Component, Path, PathBuf};

const MAX_SOCKET_PATH_BYTES: usize = 103;

#[derive(Debug)]
pub struct SecureRuntime {
    listener: UnixListener,
    _directory: File,
    _lock: File,
    _log: File,
    socket_path: PathBuf,
    socket_device: u64,
    socket_inode: u64,
}

struct SecureDirectory {
    file: File,
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl SecureDirectory {
    fn verify(&self) -> io::Result<()> {
        let current = open_directory(&self.path)?;
        let metadata = current.metadata()?;
        if metadata.dev() != self.device || metadata.ino() != self.inode {
            return Err(invalid("session directory identity changed"));
        }
        Ok(())
    }
}

impl SecureRuntime {
    pub fn bind(socket_path: impl AsRef<Path>) -> io::Result<Self> {
        let requested_socket_path = socket_path.as_ref();
        validate_socket_path(requested_socket_path)?;
        let directory_path = requested_socket_path
            .parent()
            .ok_or_else(|| invalid("session socket has no parent"))?;
        let directory = open_or_create_secure_directory(directory_path)?;
        let socket_path = directory.path.join("control.sock");
        let lock = open_private_file_at(directory.file.as_raw_fd(), "daemon.lock")?;
        lock_exclusive(lock.as_raw_fd())?;
        let log = open_private_file_at(directory.file.as_raw_fd(), "daemon.log")?;
        directory.verify()?;
        recover_stale_socket(&socket_path)?;
        directory.verify()?;
        let listener = UnixListener::bind(&socket_path)?;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
        let metadata = std::fs::symlink_metadata(&socket_path)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != effective_uid()
            || metadata.mode() & 0o777 != 0o600
        {
            remove_socket_if_identity(&socket_path, metadata.dev(), metadata.ino());
            return Err(invalid("session socket ownership or mode differs"));
        }
        if let Err(error) = directory.verify() {
            remove_socket_if_identity(&socket_path, metadata.dev(), metadata.ino());
            return Err(error);
        }
        Ok(Self {
            listener,
            _directory: directory.file,
            _lock: lock,
            _log: log,
            socket_path,
            socket_device: metadata.dev(),
            socket_inode: metadata.ino(),
        })
    }

    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }
}

impl Drop for SecureRuntime {
    fn drop(&mut self) {
        remove_socket_if_identity(&self.socket_path, self.socket_device, self.socket_inode);
    }
}

pub fn selected_socket_path(
    mode: BuildMode,
    app_support_root: impl AsRef<Path>,
    fallback_root: impl AsRef<Path>,
) -> PathBuf {
    let policy = RuntimePathPolicy::new(mode);
    let preferred = policy.preferred_session_socket_path(app_support_root);
    if preferred.as_os_str().as_bytes().len() <= MAX_SOCKET_PATH_BYTES {
        preferred
    } else {
        policy.fallback_session_socket_path(fallback_root, effective_uid())
    }
}

pub fn validate_socket_path(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(invalid("session socket path must be absolute"));
    }
    if path.file_name().and_then(|name| name.to_str()) != Some("control.sock") {
        return Err(invalid("session socket filename must be control.sock"));
    }
    if path.as_os_str().as_bytes().len() > MAX_SOCKET_PATH_BYTES {
        return Err(invalid("session socket path is too long"));
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err(invalid("session socket path is not normalized"));
        }
    }
    Ok(())
}

fn open_or_create_secure_directory(path: &Path) -> io::Result<SecureDirectory> {
    if path == Path::new("/") {
        return Err(invalid("session directory cannot be root"));
    }
    let parent_path = path
        .parent()
        .ok_or_else(|| invalid("session directory has no parent"))?
        .canonicalize()?;
    let leaf = path
        .file_name()
        .ok_or_else(|| invalid("session directory has no name"))?;
    let parent = open_directory(&parent_path)?;
    let name = cstring(leaf.as_bytes())?;
    let current = match open_directory_at(parent.as_raw_fd(), &name) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let result = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) };
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            open_directory_at(parent.as_raw_fd(), &name)?
        }
        Err(error) => return Err(error),
    };
    let metadata = current.metadata()?;
    if metadata.uid() != effective_uid() || metadata.mode() & 0o777 != 0o700 {
        return Err(invalid(
            "session directory must be owned by the effective UID with mode 0700",
        ));
    }
    let canonical_path = parent_path.join(leaf);
    let directory = SecureDirectory {
        file: current,
        path: canonical_path,
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    directory.verify()?;
    Ok(directory)
}

fn open_directory(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

fn open_directory_at(parent: RawFd, name: &CString) -> io::Result<File> {
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    owned_file(fd)
}

fn open_private_file_at(parent: RawFd, name: &str) -> io::Result<File> {
    let name = CString::new(name).map_err(|_| invalid("invalid private filename"))?;
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    let file = owned_file(fd)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(invalid("private runtime file ownership or mode differs"));
    }
    Ok(file)
}

fn remove_socket_if_identity(path: &Path, device: u64, inode: u64) {
    if let Ok(metadata) = std::fs::symlink_metadata(path)
        && metadata.file_type().is_socket()
        && metadata.dev() == device
        && metadata.ino() == inode
    {
        let _ = std::fs::remove_file(path);
    }
}

fn recover_stale_socket(path: &Path) -> io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket()
        || metadata.uid() != effective_uid()
        || metadata.mode() & 0o777 != 0o600
    {
        return Err(invalid(
            "refusing to replace a socket with unsafe ownership or mode",
        ));
    }
    match UnixStream::connect(path) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "live session socket is already listening",
        )),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
            ) =>
        {
            std::fs::remove_file(path)
        }
        Err(error) => Err(error),
    }
}

fn lock_exclusive(fd: RawFd) -> io::Result<()> {
    if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn owned_file(fd: RawFd) -> io::Result<File> {
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        let owned = unsafe { OwnedFd::from_raw_fd(fd) };
        Ok(File::from(owned))
    }
}

fn cstring(bytes: &[u8]) -> io::Result<CString> {
    CString::new(bytes).map_err(|_| invalid("path contains NUL"))
}

fn effective_uid() -> u32 {
    unsafe { libc::geteuid() }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn private_root() -> tempfile::TempDir {
        let root = tempfile::Builder::new()
            .prefix("p8-isolated-test-")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        root
    }

    #[test]
    fn runtime_rejects_symlinks_modes_non_sockets_and_live_singletons() {
        let root = private_root();
        let directory = root.path().join("p8-isolated-runtime-test");
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let socket = directory.join("control.sock");
        let held = SecureRuntime::bind(&socket).unwrap();
        assert!(matches!(
            SecureRuntime::bind(&socket).unwrap_err().kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::AddrInUse
        ));
        drop(held);

        std::fs::write(&socket, b"held").unwrap();
        assert!(SecureRuntime::bind(&socket).is_err());
        std::fs::remove_file(&socket).unwrap();

        let target = root.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o700)).unwrap();
        let linked = root.path().join("linked");
        symlink(&target, &linked).unwrap();
        assert!(SecureRuntime::bind(linked.join("control.sock")).is_err());
    }

    #[test]
    fn runtime_rejects_wrong_modes_and_recovers_only_private_stale_sockets() {
        let root = private_root();
        let unsafe_directory = root.path().join("p8-isolated-unsafe-mode");
        std::fs::create_dir(&unsafe_directory).unwrap();
        std::fs::set_permissions(&unsafe_directory, std::fs::Permissions::from_mode(0o755))
            .unwrap();
        assert!(SecureRuntime::bind(unsafe_directory.join("control.sock")).is_err());

        let directory = root.path().join("p8-isolated-stale-test");
        std::fs::create_dir(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        let socket = directory.join("control.sock");
        let stale = UnixListener::bind(&socket).unwrap();
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
        drop(stale);
        let runtime = SecureRuntime::bind(&socket).unwrap();
        drop(runtime);

        let unsafe_stale = UnixListener::bind(&socket).unwrap();
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o666)).unwrap();
        drop(unsafe_stale);
        assert!(SecureRuntime::bind(&socket).is_err());
        assert!(socket.exists());
    }

    #[test]
    fn runtime_creates_private_leaf_lock_log_and_socket() {
        let root = private_root();
        let socket = root.path().join("p8-isolated-runtime-test/control.sock");
        let runtime = SecureRuntime::bind(&socket).unwrap();
        assert_eq!(
            std::fs::metadata(socket.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        for path in [
            socket.clone(),
            socket.with_file_name("daemon.lock"),
            socket.with_file_name("daemon.log"),
        ] {
            assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o777, 0o600);
        }
        drop(runtime);
        assert!(!socket.exists());
    }

    #[test]
    fn profile_socket_selection_uses_fallback_only_for_long_paths() {
        let root = private_root();
        let development = selected_socket_path(BuildMode::Development, root.path(), "/tmp");
        let production = selected_socket_path(BuildMode::Production, root.path(), "/tmp");
        assert_eq!(
            development,
            root.path()
                .join(RuntimePathPolicy::new(BuildMode::Development).session_directory_name())
                .join(RuntimePathPolicy::new(BuildMode::Development).session_socket_filename())
        );
        assert_eq!(
            production,
            root.path()
                .join(RuntimePathPolicy::new(BuildMode::Production).session_directory_name())
                .join(RuntimePathPolicy::new(BuildMode::Production).session_socket_filename())
        );
        assert_ne!(development, production);
        let development_runtime = SecureRuntime::bind(&development).unwrap();
        let production_runtime = SecureRuntime::bind(&production).unwrap();
        assert!(development.exists());
        assert!(production.exists());
        drop(development_runtime);
        drop(production_runtime);
        let long = root.path().join("x".repeat(120));
        let fallback = selected_socket_path(BuildMode::Production, long, "/tmp");
        let policy = RuntimePathPolicy::new(BuildMode::Production);
        assert_eq!(
            fallback,
            Path::new("/tmp")
                .join(policy.fallback_session_directory_name(effective_uid()))
                .join(policy.session_socket_filename())
        );
    }
}
