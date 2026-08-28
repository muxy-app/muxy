use super::GitError;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

pub fn validate_branch(branch: &str) -> Result<(), GitError> {
    if branch.is_empty()
        || branch.starts_with('-')
        || !branch
            .chars()
            .all(|character| character.is_alphanumeric() || "._/-".contains(character))
    {
        return Err(GitError::InvalidBranch);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RepositoryPathError {
    #[error("repository path is empty")]
    Empty,
    #[error("repository path contains a NUL byte")]
    Nul,
    #[error("repository path is absolute")]
    Absolute,
    #[error("repository path escapes its repository")]
    ParentTraversal,
    #[cfg(not(unix))]
    #[error("repository path is unsupported on this platform")]
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ValidatedRepositoryPath {
    path: PathBuf,
}

impl ValidatedRepositoryPath {
    pub(crate) fn as_os_str(&self) -> &OsStr {
        self.path.as_os_str()
    }

    #[cfg(unix)]
    fn components(&self) -> Vec<&OsStr> {
        self.path
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => Some(value),
                Component::CurDir => None,
                _ => None,
            })
            .collect()
    }
}

pub(crate) fn validate_repository_path(
    bytes: &[u8],
) -> Result<ValidatedRepositoryPath, RepositoryPathError> {
    if bytes.is_empty() {
        return Err(RepositoryPathError::Empty);
    }
    if bytes.contains(&0) {
        return Err(RepositoryPathError::Nul);
    }
    let path = PathBuf::from(os_string_from_bytes(bytes.to_vec())?);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
    {
        return Err(RepositoryPathError::Absolute);
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(RepositoryPathError::ParentTraversal);
    }
    if !path
        .components()
        .any(|component| matches!(component, Component::Normal(_)))
    {
        return Err(RepositoryPathError::Empty);
    }
    Ok(ValidatedRepositoryPath { path })
}

#[cfg(unix)]
fn os_string_from_bytes(bytes: Vec<u8>) -> Result<OsString, RepositoryPathError> {
    use std::os::unix::ffi::OsStringExt;

    Ok(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn os_string_from_bytes(bytes: Vec<u8>) -> Result<OsString, RepositoryPathError> {
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|_| RepositoryPathError::Unsupported)
}

#[derive(Debug, thiserror::Error)]
pub enum SafeDeleteError {
    #[error(transparent)]
    InvalidPath(#[from] RepositoryPathError),
    #[error("repository root cannot be opened safely")]
    Root(#[source] std::io::Error),
    #[error("repository path cannot be traversed safely")]
    Traverse(#[source] std::io::Error),
    #[error("repository path cannot be inspected safely")]
    Inspect(#[source] std::io::Error),
    #[error("repository directories cannot be removed")]
    Directory,
    #[error("repository entry cannot be removed safely")]
    Remove(#[source] std::io::Error),
    #[error("safe repository deletion is unsupported on this platform")]
    Unsupported,
}

pub struct SafeUntrackedDelete;

impl SafeUntrackedDelete {
    pub fn delete(repository: &Path, path: &[u8]) -> Result<(), SafeDeleteError> {
        #[cfg(unix)]
        {
            delete_unix_with_hook(repository, path, || {})
        }
        #[cfg(not(unix))]
        {
            unsupported_delete(repository, path)
        }
    }
}

#[cfg(any(test, not(unix)))]
fn unsupported_delete(_repository: &Path, _path: &[u8]) -> Result<(), SafeDeleteError> {
    Err(SafeDeleteError::Unsupported)
}

#[cfg(unix)]
struct OwnedFileDescriptor(libc::c_int);

#[cfg(unix)]
impl Drop for OwnedFileDescriptor {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.0);
        }
    }
}

#[cfg(unix)]
fn delete_unix_with_hook(
    repository: &Path,
    path: &[u8],
    before_unlink: impl FnOnce(),
) -> Result<(), SafeDeleteError> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let path = validate_repository_path(path)?;
    let components = path.components();
    let (file_name, parents) = components.split_last().ok_or(RepositoryPathError::Empty)?;
    let repository =
        CString::new(repository.as_os_str().as_bytes()).map_err(|_| RepositoryPathError::Nul)?;
    let root = unsafe {
        libc::open(
            repository.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if root < 0 {
        return Err(SafeDeleteError::Root(std::io::Error::last_os_error()));
    }
    let mut directory = OwnedFileDescriptor(root);
    for parent in parents {
        let parent = CString::new(parent.as_bytes()).map_err(|_| RepositoryPathError::Nul)?;
        let next = unsafe {
            libc::openat(
                directory.0,
                parent.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if next < 0 {
            return Err(SafeDeleteError::Traverse(std::io::Error::last_os_error()));
        }
        directory = OwnedFileDescriptor(next);
    }
    let file_name = CString::new(file_name.as_bytes()).map_err(|_| RepositoryPathError::Nul)?;
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    let inspected = unsafe {
        libc::fstatat(
            directory.0,
            file_name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if inspected != 0 {
        return Err(SafeDeleteError::Inspect(std::io::Error::last_os_error()));
    }
    let metadata = unsafe { metadata.assume_init() };
    if metadata.st_mode & libc::S_IFMT == libc::S_IFDIR {
        return Err(SafeDeleteError::Directory);
    }
    before_unlink();
    if unsafe { libc::unlinkat(directory.0, file_name.as_ptr(), 0) } != 0 {
        return Err(SafeDeleteError::Remove(std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn repository_path_validation_accepts_relative_raw_paths_and_rejects_escapes() {
        assert!(validate_repository_path(b"folder/file.txt").is_ok());
        for path in [
            b"".as_slice(),
            b"/absolute",
            b"../outside",
            b"folder/../../outside",
            b"folder/\0file",
        ] {
            assert!(validate_repository_path(path).is_err(), "{path:?}");
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn repository_path_validation_preserves_non_utf8_bytes() {
        use std::os::unix::ffi::OsStrExt;

        let path = validate_repository_path(b"raw-\xff").unwrap();
        assert_eq!(path.as_os_str().as_bytes(), b"raw-\xff");
    }

    #[cfg(unix)]
    #[test]
    fn repository_path_delete_is_contained_and_handles_final_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let repository = temp.path().join("repo");
        let sibling = temp.path().join("repo-sibling");
        std::fs::create_dir_all(repository.join("nested")).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(repository.join("nested/file.txt"), "inside").unwrap();
        std::fs::write(sibling.join("sentinel.txt"), "outside").unwrap();
        symlink(sibling.join("sentinel.txt"), repository.join("link.txt")).unwrap();

        SafeUntrackedDelete::delete(&repository, b"nested/file.txt").unwrap();
        assert!(!repository.join("nested/file.txt").exists());
        SafeUntrackedDelete::delete(&repository, b"link.txt").unwrap();
        assert!(!repository.join("link.txt").exists());
        assert_eq!(
            std::fs::read(sibling.join("sentinel.txt")).unwrap(),
            b"outside"
        );

        for path in [b"../repo-sibling/sentinel.txt".as_slice(), b"/tmp/file"] {
            assert!(SafeUntrackedDelete::delete(&repository, path).is_err());
        }
        assert_eq!(
            std::fs::read(sibling.join("sentinel.txt")).unwrap(),
            b"outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn repository_path_delete_rejects_symlinked_parents_missing_entries_and_directories() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let repository = temp.path().join("repo");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&repository).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("sentinel.txt"), "outside").unwrap();
        symlink(&outside, repository.join("linked-parent")).unwrap();
        std::fs::create_dir(repository.join("directory")).unwrap();

        for path in [
            b"linked-parent/sentinel.txt".as_slice(),
            b"missing.txt",
            b"directory",
        ] {
            assert!(SafeUntrackedDelete::delete(&repository, path).is_err());
        }
        assert_eq!(
            std::fs::read(outside.join("sentinel.txt")).unwrap(),
            b"outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn repository_path_delete_ancestor_swap_cannot_reach_outside_sentinel() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let repository = temp.path().join("repo");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(repository.join("parent")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(repository.join("parent/file.txt"), "inside").unwrap();
        std::fs::write(outside.join("file.txt"), "outside").unwrap();

        delete_unix_with_hook(&repository, b"parent/file.txt", || {
            std::fs::rename(repository.join("parent"), repository.join("detached")).unwrap();
            symlink(&outside, repository.join("parent")).unwrap();
        })
        .unwrap();

        assert_eq!(std::fs::read(outside.join("file.txt")).unwrap(), b"outside");
        assert!(!repository.join("detached/file.txt").exists());
    }

    #[test]
    fn repository_path_unsupported_delete_never_removes_the_entry() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("file.txt");
        std::fs::write(&path, "inside").unwrap();

        assert!(matches!(
            unsupported_delete(Path::new(temp.path()), b"file.txt"),
            Err(SafeDeleteError::Unsupported)
        ));
        assert!(path.exists());
    }
}
