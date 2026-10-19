//! Composer files live in private folders and are published by name only
//! once complete, never through a symbolic link.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use muxy_core::files::FileId;

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
    let temporary = temporary_name();
    directory
        .write_new_atomic(&temporary, contents)
        .map_err(|error| AtomicWriteError {
            error,
            published: false,
        })?;
    if let Err(error) = platform::replace(&directory, temporary.as_ref(), name) {
        let _ = platform::remove(&directory, temporary.as_ref());
        return Err(AtomicWriteError {
            error,
            published: false,
        });
    }
    platform::sync(&directory).map_err(|error| AtomicWriteError {
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
    Ok(PrivateDirectory {
        path: path.to_owned(),
        file: platform::create_directory(path)?,
    })
}

impl PrivateDirectory {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn verify(&self) -> io::Result<()> {
        let current = fs::symlink_metadata(&self.path)?;
        if !current.is_dir() || FileId::of(&self.file)? != FileId::of_link(&self.path)? {
            return Err(io::Error::other("Composer storage directory was replaced"));
        }
        Ok(())
    }

    pub(super) fn write_new_atomic(&self, name: &str, contents: &[u8]) -> io::Result<()> {
        self.verify()?;
        let temporary = temporary_name();
        let mut file = platform::create_new(self, temporary.as_ref())?;
        let result = (|| {
            file.write_all(contents)?;
            file.sync_all()?;
            self.verify()?;
            platform::link_new(self, temporary.as_ref(), name.as_ref())?;
            platform::sync(self)
        })();
        drop(file);
        let _ = platform::remove(self, temporary.as_ref());
        result
    }

    pub(super) fn read_regular(&self, name: &str) -> io::Result<Vec<u8>> {
        self.verify()?;
        let file = platform::open_regular(self, name.as_ref())?;
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
        if platform::is_regular(self, name.as_ref())? != Some(true) {
            return Ok(false);
        }
        platform::remove(self, name.as_ref())?;
        Ok(true)
    }

    pub(super) fn regular_file_names(&self) -> io::Result<Vec<String>> {
        self.verify()?;
        let mut names = Vec::new();
        for name in platform::names(self)? {
            let Some(name) = name.to_str() else {
                continue;
            };
            if super::image_storage::validate_image_filename(name).is_ok()
                && platform::is_regular(self, name.as_ref())? == Some(true)
            {
                names.push(name.to_owned());
            }
        }
        Ok(names)
    }
}

pub(super) fn lock_profile(root: &Path) -> io::Result<File> {
    let directory = ensure_private_directory(root)?;
    let file = platform::open_lock(&directory, OsStr::new(".composer.lock"))?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Composer lock must be a regular file"));
    }
    file.try_lock().map_err(|error| {
        io::Error::other(format!(
            "Composer drafts are in use by another Muxy instance: {error}"
        ))
    })?;
    Ok(file)
}

pub(super) fn read_document(path: &Path) -> io::Result<Vec<u8>> {
    let file = platform::open_document(path)?;
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

fn temporary_name() -> String {
    format!(".{}.tmp", uuid::Uuid::new_v4())
}

#[cfg(test)]
mod tests {
    use crate::composer::image_storage::ImageStorage;
    use crate::composer::{ComposerStore, DraftId, IMAGES_DIRECTORY_NAME};

    #[test]
    fn a_linked_drafts_document_is_never_read() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside.json");
        std::fs::write(&outside, b"{}").unwrap();
        let linked = root.path().join(crate::composer::DRAFTS_FILE_NAME);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &linked).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&outside, &linked).unwrap();
        assert_eq!(super::read_document(&outside).unwrap(), b"{}");
        assert!(super::read_document(&linked).is_err());
    }

    #[test]
    fn a_linked_image_folder_is_refused() {
        let profile = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let folder = profile.path().join(IMAGES_DIRECTORY_NAME);
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), &folder).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(outside.path(), &folder).unwrap();
        assert!(ImageStorage::open(profile.path()).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn a_junction_in_place_of_the_image_folder_is_refused() {
        let profile = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let folder = profile.path().join(IMAGES_DIRECTORY_NAME);
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&folder)
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "{made:?}");
        let permissions = || {
            std::process::Command::new("icacls")
                .arg(outside.path())
                .output()
                .unwrap()
                .stdout
        };
        let before = permissions();
        assert!(ImageStorage::open(profile.path()).is_err());
        assert_eq!(permissions(), before);
    }

    #[cfg(windows)]
    #[test]
    fn the_open_image_folder_and_its_parents_cannot_be_moved() {
        let parent = tempfile::tempdir().unwrap();
        let profile = parent.path().join("profile");
        let storage = ImageStorage::open(&profile).unwrap();
        let folder = storage.directory_path().to_owned();
        assert!(std::fs::rename(&folder, profile.join("moved")).is_err());
        assert!(std::fs::remove_dir_all(&folder).is_err());
        assert!(std::fs::rename(&profile, parent.path().join("moved")).is_err());
        assert!(
            storage
                .path_for("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE.png")
                .is_ok()
        );
        drop(storage);
        std::fs::rename(&folder, profile.join("moved")).unwrap();
    }

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
}
