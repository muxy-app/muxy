use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};

use rustix::fs::{AtFlags, FileType, Mode, OFlags};

#[derive(Debug)]
pub(super) struct AtomicWriteError {
    error: io::Error,
    published: bool,
}

impl AtomicWriteError {
    pub(super) fn publication_may_have_succeeded(&self) -> bool {
        self.published
    }
}

impl From<AtomicWriteError> for io::Error {
    fn from(value: AtomicWriteError) -> Self {
        value.error
    }
}

pub(super) fn write_private_durable(path: &Path, contents: &[u8]) -> Result<(), AtomicWriteError> {
    let directory = ensure_private_directory(path.parent().unwrap_or_else(|| Path::new(".")))
        .map_err(|error| AtomicWriteError {
            error,
            published: false,
        })?;
    let name = path.file_name().ok_or_else(|| AtomicWriteError {
        error: io::Error::other("Missing draft filename"),
        published: false,
    })?;
    let temporary = format!(".{}.tmp", uuid::Uuid::new_v4());
    directory
        .write_new_atomic(&temporary, contents)
        .map_err(|error| AtomicWriteError {
            error,
            published: false,
        })?;
    if let Err(error) = rustix::fs::renameat(&directory.file, &temporary, &directory.file, name) {
        let _ = rustix::fs::unlinkat(&directory.file, &temporary, AtFlags::empty());
        return Err(AtomicWriteError {
            error: error.into(),
            published: false,
        });
    }
    directory.file.sync_all().map_err(|error| AtomicWriteError {
        error,
        published: true,
    })
}

#[derive(Debug)]
pub(super) struct PrivateDirectory {
    path: PathBuf,
    file: File,
}

pub(super) fn ensure_private_directory(path: &Path) -> io::Result<PrivateDirectory> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    let file = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into();
    Ok(PrivateDirectory {
        path: path.to_owned(),
        file,
    })
}

impl PrivateDirectory {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn verify(&self) -> io::Result<()> {
        let opened = self.file.metadata()?;
        let current = fs::symlink_metadata(&self.path)?;
        if !current.is_dir() || (opened.dev(), opened.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other("Composer storage directory was replaced"));
        }
        Ok(())
    }

    pub(super) fn write_new_atomic(&self, name: &str, contents: &[u8]) -> io::Result<()> {
        self.verify()?;
        let temporary = format!(".{}.tmp", uuid::Uuid::new_v4());
        let mut file = File::from(rustix::fs::openat(
            &self.file,
            &temporary,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?);
        let result = (|| {
            file.write_all(contents)?;
            file.sync_all()?;
            self.verify()?;
            rustix::fs::linkat(&self.file, &temporary, &self.file, name, AtFlags::empty())?;
            self.file.sync_all()
        })();
        let _ = rustix::fs::unlinkat(&self.file, &temporary, AtFlags::empty());
        result
    }

    pub(super) fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
        self.verify()?;
        let file = File::from(rustix::fs::openat(
            &self.file,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("Expected a regular composer image"));
        }
        let mut bytes = Vec::new();
        let limit = super::image_storage::MAX_ENCODED_IMAGE_BYTES;
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(io::Error::other("Composer image is too large"));
        }
        Ok(bytes)
    }

    pub(super) fn remove_regular(&self, name: &str) -> io::Result<bool> {
        self.verify()?;
        let stat = match rustix::fs::statat(&self.file, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(rustix::io::Errno::NOENT) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Ok(false);
        }
        rustix::fs::unlinkat(&self.file, name, AtFlags::empty())?;
        Ok(true)
    }

    pub(super) fn regular_file_names(&self) -> io::Result<Vec<String>> {
        self.verify()?;
        let mut names = Vec::new();
        for entry in rustix::fs::Dir::read_from(&self.file)? {
            let entry = entry?;
            let Ok(name) = entry.file_name().to_str() else {
                continue;
            };
            if super::image_storage::validate_image_filename(name).is_err() {
                continue;
            }
            let stat = rustix::fs::statat(&self.file, name, AtFlags::SYMLINK_NOFOLLOW)?;
            if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile {
                names.push(name.to_owned());
            }
        }
        Ok(names)
    }
}

pub(super) fn lock_profile(root: &Path) -> io::Result<File> {
    let directory = ensure_private_directory(root)?;
    let file = File::from(rustix::fs::openat(
        &directory.file,
        ".composer.lock",
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )?);
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Composer lock must be a regular file"));
    }
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive).map_err(
        |error| {
            io::Error::other(format!(
                "Composer drafts are in use by another Muxy instance: {error}"
            ))
        },
    )?;
    Ok(file)
}

pub(super) fn read_document(path: &Path) -> io::Result<Vec<u8>> {
    let file = File::from(rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Composer drafts must be a regular file"));
    }
    let limit = super::MAX_DRAFT_BYTES as u64;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() > usize::try_from(limit).unwrap_or(usize::MAX) {
        return Err(io::Error::other(
            "Composer drafts exceed the 32 MiB storage limit",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composer::{ComposerStore, DraftId};

    #[test]
    fn only_the_profile_owner_can_publish_drafts_and_the_lock_releases_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let mut owner = ComposerStore::load_from(root.path());
        let id = DraftId::for_project(crate::ServerId::new(), crate::ProjectId::new());
        owner
            .edit_content(id.clone(), "owner draft".into(), Vec::new())
            .unwrap();
        owner.flush().unwrap();
        let mut other = ComposerStore::load_from(root.path());
        assert!(other.load_status().overwrite_blocked);
        assert!(
            other
                .edit_content(id.clone(), "overwrite".into(), Vec::new())
                .is_err()
        );
        assert!(other.flush().is_err());
        drop(other);
        drop(owner);
        let mut reopened = ComposerStore::load_from(root.path());
        assert!(reopened.load_status().is_ready());
        assert_eq!(reopened.draft(&id).unwrap().text, "owner draft");
        reopened
            .edit_content(id.clone(), "new draft".into(), Vec::new())
            .unwrap();
        assert!(reopened.flush().unwrap());
    }

    #[test]
    fn draft_loading_rejects_symlinks_and_special_files() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target.json");
        fs::write(&target, b"{}").unwrap();
        let link = root.path().join("link.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(read_document(&link).is_err());
        assert!(read_document(root.path()).is_err());
    }
}
