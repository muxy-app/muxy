use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

use super::ServerError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SocketIdentity {
    device: u64,
    inode: u64,
}

impl SocketIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

struct BoundPathGuard<'a> {
    path: &'a Path,
    identity: Option<SocketIdentity>,
    armed: bool,
}

impl BoundPathGuard<'_> {
    fn identify(&mut self, identity: SocketIdentity) {
        self.identity = Some(identity);
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for BoundPathGuard<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Some(identity) = self.identity {
            remove_matching_socket(self.path, identity);
        }
    }
}

struct DirectoryLock {
    file: File,
}

impl DirectoryLock {
    fn acquire(parent: &Path) -> Result<Self, ServerError> {
        let file = File::open(parent).map_err(|source| ServerError::DirectoryLock {
            path: parent.to_path_buf(),
            source,
        })?;
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
        if result != 0 {
            return Err(ServerError::DirectoryLock {
                path: parent.to_path_buf(),
                source: io::Error::last_os_error(),
            });
        }
        Ok(Self { file })
    }
}

impl Drop for DirectoryLock {
    fn drop(&mut self) {
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

pub(super) struct BoundSocket {
    pub(super) listener: UnixListener,
    path: PathBuf,
    parent: PathBuf,
    identity: SocketIdentity,
}

impl BoundSocket {
    pub(super) fn bind(path: &Path) -> Result<Self, ServerError> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        ensure_parent(parent)?;
        let lock = DirectoryLock::acquire(parent)?;

        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if !metadata.file_type().is_socket() {
                    return Err(ServerError::PathNotSocket(path.to_path_buf()));
                }
                let stale_identity = SocketIdentity::from_metadata(&metadata);
                match UnixStream::connect(path) {
                    Ok(_) => return Err(ServerError::ActiveListener(path.to_path_buf())),
                    Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {}
                    Err(source) => {
                        return Err(ServerError::SocketProbe {
                            path: path.to_path_buf(),
                            source,
                        });
                    }
                }
                let current =
                    fs::symlink_metadata(path).map_err(|source| ServerError::SocketMetadata {
                        path: path.to_path_buf(),
                        source,
                    })?;
                if !current.file_type().is_socket()
                    || SocketIdentity::from_metadata(&current) != stale_identity
                {
                    return Err(ServerError::SocketChanged(path.to_path_buf()));
                }
                fs::remove_file(path).map_err(|source| ServerError::RemoveStaleSocket {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ServerError::SocketMetadata {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }

        let listener = UnixListener::bind(path).map_err(|source| ServerError::SocketBind {
            path: path.to_path_buf(),
            source,
        })?;
        let mut guard = BoundPathGuard {
            path,
            identity: None,
            armed: true,
        };
        listener
            .set_nonblocking(true)
            .map_err(|source| ServerError::SocketNonblocking {
                path: path.to_path_buf(),
                source,
            })?;
        let metadata =
            fs::symlink_metadata(path).map_err(|source| ServerError::SocketMetadata {
                path: path.to_path_buf(),
                source,
            })?;
        if !metadata.file_type().is_socket() {
            return Err(ServerError::SocketChanged(path.to_path_buf()));
        }
        let identity = SocketIdentity::from_metadata(&metadata);
        require_bound_listener(path, &listener, identity)?;
        guard.identify(identity);
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|source| {
            ServerError::SocketPermissions {
                path: path.to_path_buf(),
                source,
            }
        })?;
        require_bound_listener(path, &listener, identity)?;
        guard.disarm();
        drop(lock);

        Ok(Self {
            listener,
            path: path.to_path_buf(),
            parent: parent.to_path_buf(),
            identity,
        })
    }

    fn cleanup(&self) {
        let Ok(_lock) = DirectoryLock::acquire(&self.parent) else {
            return;
        };
        remove_matching_socket(&self.path, self.identity);
    }
}

impl Drop for BoundSocket {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn require_matching_socket(path: &Path, identity: SocketIdentity) -> Result<(), ServerError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ServerError::SocketMetadata {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_socket() && SocketIdentity::from_metadata(&metadata) == identity {
        Ok(())
    } else {
        Err(ServerError::SocketChanged(path.to_path_buf()))
    }
}

fn require_bound_listener(
    path: &Path,
    listener: &UnixListener,
    identity: SocketIdentity,
) -> Result<(), ServerError> {
    require_matching_socket(path, identity)?;
    let token = Uuid::new_v4().to_string().into_bytes();
    let mut probe = UnixStream::connect(path).map_err(|source| ServerError::SocketProbe {
        path: path.to_path_buf(),
        source,
    })?;
    probe
        .write_all(&token)
        .and_then(|()| probe.shutdown(Shutdown::Write))
        .map_err(|source| ServerError::SocketProbe {
            path: path.to_path_buf(),
            source,
        })?;
    let mut matched = false;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
                let mut payload = Vec::new();
                let _ = stream
                    .take((token.len() + 1) as u64)
                    .read_to_end(&mut payload);
                if payload == token {
                    matched = true;
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    require_matching_socket(path, identity)?;
    if matched {
        Ok(())
    } else {
        Err(ServerError::SocketChanged(path.to_path_buf()))
    }
}

fn remove_matching_socket(path: &Path, identity: SocketIdentity) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_socket() && SocketIdentity::from_metadata(&metadata) == identity {
        let _ = fs::remove_file(path);
    }
}

fn ensure_parent(parent: &Path) -> Result<(), ServerError> {
    match fs::create_dir(parent) {
        Ok(()) => {
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|source| {
                ServerError::ParentDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if parent.is_dir() {
                Ok(())
            } else {
                Err(ServerError::ParentDirectory {
                    path: parent.to_path_buf(),
                    source: error,
                })
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(parent).map_err(|source| ServerError::ParentDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|source| {
                ServerError::ParentDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })
        }
        Err(source) => Err(ServerError::ParentDirectory {
            path: parent.to_path_buf(),
            source,
        }),
    }
}

pub(super) fn prepare_stream(stream: &UnixStream) -> io::Result<()> {
    stream.set_nonblocking(true)?;
    set_no_sigpipe(stream.as_raw_fd())
}

#[cfg(target_os = "macos")]
fn set_no_sigpipe(fd: RawFd) -> io::Result<()> {
    let value: libc::c_int = 1;
    let result = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE,
            (&raw const value).cast(),
            std::mem::size_of_val(&value) as libc::socklen_t,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
fn set_no_sigpipe(_fd: RawFd) -> io::Result<()> {
    Ok(())
}

pub(super) fn send_no_sigpipe(stream: &UnixStream, bytes: &[u8]) -> io::Result<usize> {
    let flags = send_flags();
    let written = unsafe {
        libc::send(
            stream.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
            flags,
        )
    };
    if written >= 0 {
        Ok(written as usize)
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "linux")]
const fn send_flags() -> libc::c_int {
    libc::MSG_NOSIGNAL
}

#[cfg(not(target_os = "linux"))]
const fn send_flags() -> libc::c_int {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclaimed_guard_preserves_a_replacement_socket() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let identity = SocketIdentity::from_metadata(&fs::symlink_metadata(&path).unwrap());
        fs::remove_file(&path).unwrap();
        let replacement = UnixListener::bind(&path).unwrap();
        let replacement_identity =
            SocketIdentity::from_metadata(&fs::symlink_metadata(&path).unwrap());

        assert!(require_bound_listener(&path, &listener, identity).is_err());
        {
            let _guard = BoundPathGuard {
                path: &path,
                identity: None,
                armed: true,
            };
        }
        assert_eq!(
            SocketIdentity::from_metadata(&fs::symlink_metadata(&path).unwrap()),
            replacement_identity
        );
        assert!(UnixStream::connect(&path).is_ok());
        drop(replacement);
    }
}
