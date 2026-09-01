use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use thiserror::Error;

#[cfg(target_os = "macos")]
const MAX_SOCKET_PATH_BYTES: usize = 103;
#[cfg(not(target_os = "macos"))]
const MAX_SOCKET_PATH_BYTES: usize = 107;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("session socket path is not absolute: {0}")]
    RelativeSocket(PathBuf),
    #[error("session socket path exceeds the platform limit: {0}")]
    SocketPathTooLong(PathBuf),
    #[error("session directory has an invalid type, owner, or permissions: {0}")]
    InvalidDirectory(PathBuf),
    #[error("session singleton is already owned")]
    SingletonOwned,
    #[error("session socket has an invalid type or owner: {0}")]
    InvalidSocket(PathBuf),
    #[error("session socket already has an active owner: {0}")]
    ActiveSocket(PathBuf),
    #[error("session peer belongs to uid {actual}, expected {expected}")]
    WrongPeer { expected: u32, actual: u32 },
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct BoundSocket {
    pub listener: UnixListener,
    _lock: File,
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl BoundSocket {
    pub fn bind(path: &Path) -> Result<Self, SecurityError> {
        validate_socket_path(path)?;
        let directory = path
            .parent()
            .ok_or_else(|| SecurityError::RelativeSocket(path.to_path_buf()))?;
        ensure_private_directory(directory)?;
        let lock_path = directory.join("daemon.lock");
        let lock = open_lock(&lock_path)?;
        prepare_socket_path(path)?;
        let listener = UnixListener::bind(path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != current_uid()
            || metadata.mode() & 0o777 != 0o600
        {
            return Err(SecurityError::InvalidSocket(path.to_path_buf()));
        }
        Ok(Self {
            listener,
            _lock: lock,
            path: path.to_path_buf(),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

impl Drop for BoundSocket {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.dev() == self.device && metadata.ino() == self.inode {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn validate_peer(stream: &UnixStream) -> Result<(), SecurityError> {
    let expected = current_uid();
    let actual = peer_uid(stream)?;
    if actual == expected {
        Ok(())
    } else {
        Err(SecurityError::WrongPeer { expected, actual })
    }
}

pub fn validate_socket_path(path: &Path) -> Result<(), SecurityError> {
    use std::os::unix::ffi::OsStrExt;

    if !path.is_absolute() {
        return Err(SecurityError::RelativeSocket(path.to_path_buf()));
    }
    if path.as_os_str().as_bytes().len() > MAX_SOCKET_PATH_BYTES {
        return Err(SecurityError::SocketPathTooLong(path.to_path_buf()));
    }
    reject_symlink_ancestors(path)?;
    Ok(())
}

fn reject_symlink_ancestors(path: &Path) -> Result<(), SecurityError> {
    let mut cursor = PathBuf::new();
    for component in path.components() {
        cursor.push(component.as_os_str());
        match fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(SecurityError::InvalidDirectory(cursor));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn ensure_private_directory(path: &Path) -> Result<(), SecurityError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.uid() != current_uid() {
                return Err(SecurityError::InvalidDirectory(path.to_path_buf()));
            }
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| SecurityError::InvalidDirectory(path.to_path_buf()))?;
            let parent_metadata = fs::symlink_metadata(parent)?;
            if !parent_metadata.is_dir() || parent_metadata.file_type().is_symlink() {
                return Err(SecurityError::InvalidDirectory(parent.to_path_buf()));
            }
            fs::create_dir(path)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        }
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != current_uid()
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(SecurityError::InvalidDirectory(path.to_path_buf()));
    }
    Ok(())
}

fn open_lock(path: &Path) -> Result<File, SecurityError> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && (!metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.uid() != current_uid())
    {
        return Err(SecurityError::InvalidSocket(path.to_path_buf()));
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        Ok(file)
    } else if io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
        Err(SecurityError::SingletonOwned)
    } else {
        Err(io::Error::last_os_error().into())
    }
}

fn prepare_socket_path(path: &Path) -> Result<(), SecurityError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_socket() || metadata.uid() != current_uid() {
        return Err(SecurityError::InvalidSocket(path.to_path_buf()));
    }
    if UnixStream::connect(path).is_ok() {
        return Err(SecurityError::ActiveSocket(path.to_path_buf()));
    }
    fs::remove_file(path)?;
    Ok(())
}

fn current_uid() -> u32 {
    unsafe { libc::geteuid() }
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn peer_uid(stream: &UnixStream) -> Result<u32, SecurityError> {
    let mut uid = 0;
    let mut gid = 0;
    let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if result == 0 {
        Ok(uid)
    } else {
        Err(io::Error::last_os_error().into())
    }
}

#[cfg(target_os = "linux")]
fn peer_uid(stream: &UnixStream) -> Result<u32, SecurityError> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::from_mut(&mut credentials).cast(),
            &mut length,
        )
    };
    if result == 0 && length as usize == std::mem::size_of::<libc::ucred>() {
        Ok(credentials.uid)
    } else {
        Err(io::Error::last_os_error().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;
    use tempfile::TempDir;

    #[test]
    fn private_socket_rejects_aliases_and_preserves_replacements() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let real = root.join("real");
        fs::create_dir(&real).unwrap();
        let linked = root.join("linked");
        std::os::unix::fs::symlink(&real, &linked).unwrap();
        assert!(BoundSocket::bind(&linked.join("control.sock")).is_err());
        fs::create_dir(real.join("sessions")).unwrap();
        assert!(BoundSocket::bind(&linked.join("sessions/control.sock")).is_err());

        let socket = root.join("sessions/control.sock");
        let bound = BoundSocket::bind(&socket).unwrap();
        assert_eq!(
            fs::metadata(socket.parent().unwrap()).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::metadata(&socket).unwrap().mode() & 0o777, 0o600);
        fs::remove_file(&socket).unwrap();
        let replacement = UnixListener::bind(&socket).unwrap();
        drop(bound);
        assert!(socket.exists());
        drop(replacement);
    }

    #[test]
    fn peer_validation_accepts_the_current_uid() {
        let (first, second) = UnixStream::pair().unwrap();
        validate_peer(&first).unwrap();
        validate_peer(&second).unwrap();
    }
}
