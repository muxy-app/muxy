use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Path, PathBuf};

use muxy_protocol::OperationId;
use rustix::fs::{self, AtFlags, Mode, OFlags, RenameFlags};

use super::super::root::directory_at;
use super::{Result, Root, Target, error, percent_encode};

struct Trash {
    path: PathBuf,
    files: File,
    info: File,
    relative_to: Option<PathBuf>,
}

#[cfg(target_os = "linux")]
pub(super) fn move_to_trash(root: &Root, source: &Target) -> Result<()> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".local/share"))
        })
        .ok_or_else(|| error("Cannot locate the user's Trash directory"))?;
    std::fs::create_dir_all(&data).map_err(error)?;
    let data = data.canonicalize().map_err(error)?;
    let absolute = root.path.join(&source.relative);
    let device = source.parent.metadata().map_err(error)?.dev();
    let trash = if std::fs::metadata(&data).map_err(error)?.dev() == device {
        Trash::open(&data, OsStr::new("Trash"), None)?
    } else {
        let parent = absolute
            .parent()
            .ok_or_else(|| error("Cannot trash the filesystem root"))?;
        let top = mount_root(parent, device)?;
        volume_trash(&top)?
    };
    trash.put(source, &absolute)
}

fn mount_root(path: &Path, device: u64) -> Result<PathBuf> {
    let mut top = path.to_owned();
    while let Some(parent) = top.parent() {
        if std::fs::metadata(parent).map_err(error)?.dev() != device {
            break;
        }
        top = parent.to_owned();
    }
    Ok(top)
}

fn volume_trash(top: &Path) -> Result<Trash> {
    let shared = top.join(".Trash");
    let directory = File::open(top).map_err(error)?;
    if let Ok(shared_fd) = directory_at(&directory, OsStr::new(".Trash"))
        && shared_fd.metadata().map_err(error)?.mode() & 0o1000 != 0
        && let Ok(trash) = Trash::open_from(
            &shared_fd,
            &shared,
            OsStr::new(&rustix::process::getuid().as_raw().to_string()),
            Some(top.to_owned()),
        )
    {
        return Ok(trash);
    }
    let name = format!(".Trash-{}", rustix::process::getuid().as_raw());
    Trash::open(top, OsStr::new(&name), Some(top.to_owned()))
}

impl Trash {
    fn open(parent: &Path, name: &OsStr, relative_to: Option<PathBuf>) -> Result<Self> {
        let directory = fs::open(
            parent,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(error)?;
        Self::open_from(&directory, parent, name, relative_to)
    }

    fn open_from(
        parent: &File,
        path: &Path,
        name: &OsStr,
        relative_to: Option<PathBuf>,
    ) -> Result<Self> {
        let directory = private_directory(parent, name).map_err(error)?;
        Ok(Self {
            path: path.join(name),
            files: private_directory(&directory, OsStr::new("files")).map_err(error)?,
            info: private_directory(&directory, OsStr::new("info")).map_err(error)?,
            relative_to,
        })
    }

    fn put(&self, source: &Target, absolute: &Path) -> Result<()> {
        if self.path.starts_with(absolute) || absolute.starts_with(&self.path) {
            return Err(error("Cannot move Trash or its contents into itself"));
        }
        let original = self
            .relative_to
            .as_ref()
            .map_or(Ok(absolute), |top| absolute.strip_prefix(top))
            .map_err(error)?;
        let date = deletion_date()?;
        let contents = format!(
            "[Trash Info]\nPath={}\nDeletionDate={date}\n",
            percent_encode(original.as_os_str().as_bytes())
        );
        for _ in 0..32 {
            let name = format!("muxy-{}", OperationId::new());
            let info_name = format!("{name}.trashinfo");
            let mut info = match fs::openat(
                &self.info,
                &info_name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            ) {
                Ok(file) => File::from(file),
                Err(rustix::io::Errno::EXIST) => continue,
                Err(cause) => return Err(error(cause)),
            };
            let result = (|| {
                info.write_all(contents.as_bytes()).map_err(error)?;
                info.sync_all().map_err(error)?;
                self.info.sync_all().map_err(error)?;
                fs::renameat_with(
                    &source.parent,
                    &source.name,
                    &self.files,
                    &name,
                    RenameFlags::NOREPLACE,
                )
                .map_err(error)
            })();
            if result.is_err() {
                let _ = fs::unlinkat(&self.info, &info_name, AtFlags::empty());
            }
            result?;
            return Ok(());
        }
        Err(error("Cannot reserve a Trash entry"))
    }
}

fn private_directory(parent: &File, name: &OsStr) -> io::Result<File> {
    match fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => (),
        Err(cause) => return Err(cause.into()),
    }
    let directory = directory_at(parent, name)?;
    let metadata = directory.metadata()?;
    if metadata.uid() != rustix::process::getuid().as_raw() || metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Trash directory must be owned by this user and private",
        ));
    }
    Ok(directory)
}

fn deletion_date() -> Result<String> {
    let output = std::process::Command::new("/bin/date")
        .arg("+%Y-%m-%dT%H:%M:%S")
        .output()
        .map_err(error)?;
    let date = String::from_utf8(output.stdout).map_err(error)?;
    let date = date.trim();
    if !output.status.success()
        || date.len() != 19
        || !date
            .bytes()
            .all(|byte| byte.is_ascii_digit() || b"-T:".contains(&byte))
    {
        return Err(error("Cannot determine Trash deletion time"));
    }
    Ok(date.into())
}

#[cfg(test)]
mod tests;
