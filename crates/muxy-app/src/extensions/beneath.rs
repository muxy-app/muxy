use std::fs::File;
use std::io;
use std::path::{Component, Path};

#[derive(Debug)]
pub(crate) struct Folder {
    #[cfg_attr(
        windows,
        allow(dead_code, reason = "held open so the folder can't be renamed")
    )]
    file: File,
    #[cfg(windows)]
    path: std::path::PathBuf,
}

fn linked() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "a link where a file was listed")
}

impl Folder {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        Ok(Self {
            file: platform::open_folder(path)?,
            #[cfg(windows)]
            path: path.to_owned(),
        })
    }

    pub(crate) fn open_file(&self, relative: &Path) -> io::Result<File> {
        let mut names = Vec::new();
        for part in relative.components() {
            match part {
                Component::Normal(name) => names.push(name),
                _ => return Err(linked()),
            }
        }
        let (name, folders) = names.split_last().ok_or_else(linked)?;
        platform::open_file(self, folders, name)
    }
}

#[cfg(unix)]
mod platform {
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io;
    use std::path::Path;

    use rustix::fs::{Mode, OFlags};

    use super::{Folder, linked};

    const FOLDER: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);
    const FILE: OFlags = OFlags::RDONLY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);

    pub(super) fn open_folder(path: &Path) -> io::Result<File> {
        Ok(rustix::fs::open(path, FOLDER, Mode::empty())?.into())
    }

    pub(super) fn open_file(folder: &Folder, folders: &[&OsStr], name: &OsStr) -> io::Result<File> {
        let mut held: Option<File> = None;
        for part in folders {
            let parent = held.as_ref().unwrap_or(&folder.file);
            held = Some(rustix::fs::openat(parent, *part, FOLDER, Mode::empty())?.into());
        }
        let parent = held.as_ref().unwrap_or(&folder.file);
        let file: File = rustix::fs::openat(parent, name, FILE, Mode::empty())?.into();
        if !file.metadata()?.is_file() {
            return Err(linked());
        }
        Ok(file)
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::Path;

    use super::{Folder, linked};

    const FILE_SHARE_READ: u32 = 0x1;
    const FILE_SHARE_WRITE: u32 = 0x2;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

    pub(super) fn open_folder(path: &Path) -> io::Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        let kind = file.metadata()?.file_type();
        if !kind.is_dir() || kind.is_symlink() {
            return Err(linked());
        }
        Ok(file)
    }

    pub(super) fn open_file(folder: &Folder, folders: &[&OsStr], name: &OsStr) -> io::Result<File> {
        let mut path = folder.path.clone();
        let mut held = Vec::with_capacity(folders.len());
        for part in folders {
            path.push(part);
            held.push(open_folder(&path)?);
        }
        path.push(name);
        let file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&path)?;
        let kind = file.metadata()?.file_type();
        if !kind.is_file() || kind.is_symlink() {
            return Err(linked());
        }
        drop(held);
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_open_below_the_folder() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(folder.path().join("a/b")).unwrap();
        std::fs::write(folder.path().join("a/b/c.txt"), "hi").unwrap();
        let held = Folder::open(folder.path()).unwrap();
        let mut text = String::new();
        io::Read::read_to_string(
            &mut held.open_file(Path::new("a/b/c.txt")).unwrap(),
            &mut text,
        )
        .unwrap();
        assert_eq!(text, "hi");
        assert!(
            held.open_file(Path::new("a/b")).is_err(),
            "a folder isn't a file"
        );
        assert!(held.open_file(Path::new("../x")).is_err());
        assert!(held.open_file(Path::new("")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_anywhere_on_the_way_is_refused() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(outside.path().join("b")).unwrap();
        std::fs::write(outside.path().join("b/c.txt"), "secret").unwrap();
        let folder = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), folder.path().join("a")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("b/c.txt"), folder.path().join("c.txt"))
            .unwrap();
        let held = Folder::open(folder.path()).unwrap();
        assert!(held.open_file(Path::new("a/b/c.txt")).is_err());
        assert!(held.open_file(Path::new("c.txt")).is_err());
        assert!(Folder::open(&folder.path().join("a")).is_err());
    }
}
