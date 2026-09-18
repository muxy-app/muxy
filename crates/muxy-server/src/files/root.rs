use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};

use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, Stat};

use super::{Result, error, wire_path};

pub(super) struct Root {
    pub path: PathBuf,
    logical: PathBuf,
    directory: File,
}

pub(super) struct Target {
    pub parent: File,
    pub name: OsString,
    pub relative: PathBuf,
}

pub(super) fn directory_at(parent: &File, name: &OsStr) -> io::Result<File> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}

impl Root {
    pub(super) fn open(path: &Path) -> Result<Self> {
        let logical = path.to_owned();
        let path = path.canonicalize().map_err(error)?;
        let directory = fs::open(
            &path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(error)?;
        Ok(Self {
            path,
            logical,
            directory,
        })
    }

    pub(super) fn resolve(&self, path: &Path) -> Result<Target> {
        if path.is_absolute() || path.as_os_str().as_bytes().contains(&0) {
            return Err(error("Expected a relative workspace path"));
        }
        let mut pending = components(path);
        let mut names = Vec::<OsString>::new();
        let mut parents = vec![self.directory.try_clone().map_err(error)?];
        let mut links = 0;
        while let Some(name) = pending.pop_front() {
            if name == "." {
                continue;
            }
            if name == ".." {
                if names.pop().is_none() {
                    return Err(error("Path escapes the workspace root"));
                }
                parents.pop();
                continue;
            }
            let parent = parents
                .last()
                .ok_or_else(|| error("Missing workspace root"))?;
            let stat = fs::statat(parent, &name, AtFlags::SYMLINK_NOFOLLOW);
            if stat
                .as_ref()
                .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::Symlink)
            {
                links += 1;
                if links > 64 {
                    return Err(error("Too many symbolic links"));
                }
                let value = fs::readlinkat(parent, &name, Vec::new()).map_err(error)?;
                let value = PathBuf::from(OsString::from_vec(value.into_bytes()));
                let target = if value.is_absolute() {
                    names.clear();
                    parents.truncate(1);
                    value
                        .strip_prefix(&self.path)
                        .or_else(|_| value.strip_prefix(&self.logical))
                        .map_err(|_| error("Path escapes the workspace root"))?
                } else {
                    &value
                };
                let mut expanded = components(target);
                expanded.append(&mut pending);
                pending = expanded;
                continue;
            }
            if pending.is_empty() {
                if let Err(cause) = stat
                    && cause != rustix::io::Errno::NOENT
                {
                    return Err(error(cause));
                }
                let mut relative: PathBuf = names.iter().collect();
                relative.push(&name);
                return Ok(Target {
                    parent: parent.try_clone().map_err(error)?,
                    name,
                    relative,
                });
            }
            stat.map_err(error)?;
            let child = directory_at(parent, &name).map_err(error)?;
            names.push(name);
            parents.push(child);
        }
        let relative: PathBuf = names.iter().collect();
        Ok(Target {
            parent: parents
                .last()
                .ok_or_else(|| error("Missing workspace root"))?
                .try_clone()
                .map_err(error)?,
            name: ".".into(),
            relative,
        })
    }

    pub(super) fn mutation(&self, path: &Path) -> Result<Target> {
        let target = self.resolve(path)?;
        if target.relative.as_os_str().is_empty() {
            return Err(error("Cannot modify the workspace root"));
        }
        if target.name == "." {
            return self.resolve(&target.relative);
        }
        Ok(target)
    }

    pub(super) fn info(&self, target: &Target) -> Result<muxy_protocol::FileInfo> {
        let stat = target.stat()?;
        let name = target
            .relative
            .file_name()
            .or_else(|| self.path.file_name())
            .unwrap_or(OsStr::new(""));
        Ok(muxy_protocol::FileInfo {
            name: wire_path(Path::new(name)),
            path: wire_path(&target.relative),
            is_directory: FileType::from_raw_mode(stat.st_mode) == FileType::Directory,
            size: u64::try_from(stat.st_size).unwrap_or(0),
        })
    }
}

impl Target {
    pub(super) fn stat(&self) -> Result<Stat> {
        let stat =
            fs::statat(&self.parent, &self.name, AtFlags::SYMLINK_NOFOLLOW).map_err(error)?;
        if FileType::from_raw_mode(stat.st_mode) == FileType::Symlink {
            return Err(error("Path changed during the file operation; retry"));
        }
        Ok(stat)
    }

    pub(super) fn directory(&self) -> Result<File> {
        directory_at(&self.parent, &self.name).map_err(error)
    }
}

fn components(path: &Path) -> VecDeque<OsString> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_owned()),
            Component::ParentDir => Some("..".into()),
            Component::CurDir => Some(".".into()),
            _ => None,
        })
        .collect()
}
