//! Files apps send for a session, such as an image dropped on its terminal
//! from another computer. They live in a private folder in the system's
//! temporary folder, which the system clears, under `<session>/`.

use std::fs::{self, DirBuilder, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use muxy_protocol::{ErrorCode, ServerPath, UploadChunk};

use crate::ServerError;

/// The longest name kept, leaving room for a `-N` that tells copies apart.
const NAME_BYTES: usize = 240;

#[derive(Debug)]
pub(crate) struct Uploads {
    directory: PathBuf,
}

impl Uploads {
    /// This user's folder in the system's temporary folder.
    pub(crate) fn system() -> Self {
        let user = rustix::process::geteuid().as_raw();
        Self::in_directory(std::env::temp_dir().join(format!("muxy-uploads-{user}")))
    }

    pub(crate) fn in_directory(directory: PathBuf) -> Self {
        Self { directory }
    }

    /// Appends a chunk to its file. After the last one, the file takes its
    /// name, never replacing another, and its absolute path is returned.
    pub(crate) fn receive(&self, chunk: &UploadChunk) -> Result<Option<ServerPath>, ServerError> {
        chunk
            .validate()
            .map_err(|code| ServerError::new(code, "The upload is too large"))?;
        let folder = self.directory.join(chunk.session.get().to_string());
        private_folder(&self.directory)
            .and_then(|()| private_folder(&folder))
            .map_err(failed)?;
        let partial = folder.join(format!("{}.part", chunk.upload));
        let mut file = open(&partial, chunk.offset)?;
        let written = append(&mut file, chunk);
        if let Err(error) = written {
            let _ = fs::remove_file(&partial);
            return Err(error);
        }
        if !chunk.last {
            return Ok(None);
        }
        drop(file);
        let kept = keep(&partial, &folder, &file_name(&chunk.name));
        if kept.is_err() {
            let _ = fs::remove_file(&partial);
        }
        Ok(Some(ServerPath(kept?.as_os_str().as_bytes().to_vec())))
    }
}

/// Makes `path` a folder only this user can open, or checks that it is one.
/// Other users can create names first in a shared temporary folder, so a
/// link, a file, or someone else's folder is refused.
fn private_folder(path: &Path) -> io::Result<()> {
    match DirBuilder::new().recursive(true).mode(0o700).create(path) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.uid() != rustix::process::geteuid().as_raw() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{} isn't this user's own folder", path.display()),
        ));
    }
    if metadata.mode() & 0o077 != 0 {
        fs::set_permissions(path, Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Starts the file at offset 0, otherwise continues it where it ended.
fn open(partial: &Path, offset: u64) -> Result<File, ServerError> {
    let file = if offset == 0 {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(partial)
    } else {
        OpenOptions::new().append(true).open(partial)
    };
    file.map_err(|error| match error.kind() {
        io::ErrorKind::AlreadyExists => out_of_order("The upload already started"),
        io::ErrorKind::NotFound => out_of_order("The upload's first chunk is missing"),
        _ => failed(error),
    })
}

fn append(file: &mut File, chunk: &UploadChunk) -> Result<(), ServerError> {
    if file.metadata().map_err(failed)?.len() != chunk.offset {
        return Err(out_of_order("The upload's chunks arrived out of order"));
    }
    file.write_all(&chunk.bytes).map_err(failed)
}

/// Gives the finished file `name`, or `name-2`, `name-3`, … when taken.
fn keep(partial: &Path, folder: &Path, name: &str) -> Result<PathBuf, ServerError> {
    for copy in 1..=10_000 {
        let path = folder.join(numbered(name, copy));
        match take_name(partial, &path) {
            Ok(()) => return path.canonicalize().map_err(failed),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(failed(error)),
        }
    }
    Err(ServerError::new(
        ErrorCode::BadPath,
        "Too many uploads share this name",
    ))
}

/// Moves the partial file to `path`, unless something already has that name.
fn take_name(partial: &Path, path: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    use rustix::io::Errno;
    match renameat_with(CWD, partial, CWD, path, RenameFlags::NOREPLACE) {
        // Some file systems can't refuse to replace a file; a hard link can.
        Err(Errno::INVAL | Errno::NOSYS | Errno::NOTSUP) => {
            fs::hard_link(partial, path)?;
            fs::remove_file(partial)
        }
        result => result.map_err(io::Error::from),
    }
}

/// The last component of `name`, without control characters, short enough
/// to number. A name that would mean a folder is `upload`.
pub(crate) fn file_name(name: &str) -> String {
    let last = name.rsplit('/').next().unwrap_or_default();
    let cleaned: String = last
        .chars()
        .map(|character| {
            if character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return "upload".into();
    }
    if cleaned.len() <= NAME_BYTES {
        return cleaned.into();
    }
    let (stem, extension) = split(cleaned);
    let extension = if extension.len() <= 16 { extension } else { "" };
    let mut end = NAME_BYTES - extension.len();
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{extension}", &stem[..end])
}

fn numbered(name: &str, copy: usize) -> String {
    if copy == 1 {
        return name.into();
    }
    let (stem, extension) = split(name);
    format!("{stem}-{copy}{extension}")
}

/// `shot.png` is `("shot", ".png")`; `.env` and `notes` have no extension.
fn split(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    }
}

fn failed(error: impl std::fmt::Display) -> ServerError {
    ServerError::new(
        ErrorCode::PersistenceFailed,
        format!("Could not keep the upload: {error}"),
    )
}

fn out_of_order(message: &str) -> ServerError {
    ServerError::new(ErrorCode::BadRequest, message)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "Test fixtures and assertions fail immediately"
)]
mod tests {
    use super::*;
    use muxy_protocol::{MAX_UPLOAD_BYTES, OperationId, SessionId};
    use std::collections::BTreeSet;

    fn chunk(upload: OperationId, offset: u64, bytes: &[u8], last: bool) -> UploadChunk {
        UploadChunk {
            session: SessionId::new(7).unwrap(),
            upload,
            name: "shot.png".into(),
            offset,
            bytes: bytes.to_vec(),
            last,
        }
    }

    fn uploads() -> (PathBuf, Uploads) {
        let root = std::env::temp_dir().join(format!("muxy-uploads-{}", OperationId::new()));
        (root.clone(), Uploads::in_directory(root.join("uploads")))
    }

    fn text(path: &ServerPath) -> PathBuf {
        PathBuf::from(std::ffi::OsStr::from_bytes(&path.0))
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn chunks_append_in_order_and_the_last_names_the_file_without_replacing_another() {
        let (root, uploads) = uploads();
        let upload = OperationId::new();
        assert_eq!(uploads.receive(&chunk(upload, 0, b"ab", false)), Ok(None));
        let gap = uploads.receive(&chunk(upload, 5, b"x", false)).unwrap_err();
        assert_eq!(gap.code(), ErrorCode::BadRequest);
        let again = OperationId::new();
        uploads.receive(&chunk(again, 0, b"ab", false)).unwrap();
        let path = text(
            &uploads
                .receive(&chunk(again, 2, b"cd", true))
                .unwrap()
                .unwrap(),
        );
        assert!(path.is_absolute());
        assert!(path.ends_with("uploads/7/shot.png"), "{}", path.display());
        assert_eq!(fs::read(&path).unwrap(), b"abcd");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        assert_eq!(mode(&root.join("uploads")), 0o700);
        let second = OperationId::new();
        let copy = text(
            &uploads
                .receive(&chunk(second, 0, b"z", true))
                .unwrap()
                .unwrap(),
        );
        assert!(copy.ends_with("uploads/7/shot-2.png"));
        assert_eq!(fs::read(&path).unwrap(), b"abcd", "the first stays");
        let late = uploads.receive(&chunk(OperationId::new(), 3, b"x", true));
        assert_eq!(late.unwrap_err().code(), ErrorCode::BadRequest);
        let names: BTreeSet<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(
            names,
            BTreeSet::from(["shot.png".into(), "shot-2.png".into()]),
            "the out-of-order upload's partial file is gone"
        );
        let huge = chunk(OperationId::new(), MAX_UPLOAD_BYTES, b"x", true);
        assert_eq!(
            uploads.receive(&huge).unwrap_err().code(),
            ErrorCode::BadRequest
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn uploads_go_to_a_private_folder_of_this_user_in_the_temporary_folder() {
        let system = Uploads::system();
        assert!(system.directory.starts_with(std::env::temp_dir()));
        let (root, uploads) = uploads();
        fs::create_dir_all(&root).unwrap();
        // Someone could create the name first in a shared temporary folder.
        std::os::unix::fs::symlink(std::env::temp_dir(), root.join("uploads")).unwrap();
        assert!(
            uploads
                .receive(&chunk(OperationId::new(), 0, b"x", true))
                .is_err()
        );
        fs::remove_file(root.join("uploads")).unwrap();
        fs::write(root.join("uploads"), "").unwrap();
        assert!(
            uploads
                .receive(&chunk(OperationId::new(), 0, b"x", true))
                .is_err()
        );
        fs::remove_file(root.join("uploads")).unwrap();
        fs::create_dir(root.join("uploads")).unwrap();
        fs::set_permissions(root.join("uploads"), Permissions::from_mode(0o777)).unwrap();
        uploads
            .receive(&chunk(OperationId::new(), 0, b"x", true))
            .unwrap();
        assert_eq!(
            mode(&root.join("uploads")),
            0o700,
            "an open folder is closed"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn names_keep_one_safe_component() {
        for (given, kept) in [
            ("shot.png", "shot.png"),
            ("../../etc/passwd", "passwd"),
            ("/abs/dir/", "upload"),
            ("..", "upload"),
            ("  ", "upload"),
            ("a\nb\u{7}.txt", "a_b_.txt"),
            (".env", ".env"),
        ] {
            assert_eq!(file_name(given), kept, "{given:?}");
        }
        let long = format!("{}.png", "é".repeat(300));
        let kept = file_name(&long);
        assert!(kept.len() <= NAME_BYTES, "{kept}");
        assert_eq!(Path::new(&kept).extension(), Some("png".as_ref()));
        assert_eq!(numbered(".env", 2), ".env-2");
        assert_eq!(numbered("a.tar.gz", 3), "a.tar-3.gz");
    }
}
