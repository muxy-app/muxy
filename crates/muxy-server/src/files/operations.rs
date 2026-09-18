use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use muxy_protocol::{
    FileContent, FileEntry, MAX_FILE_BYTES, MAX_FILE_ENTRIES, MAX_FILE_PATH_BYTES, OperationId,
    ServerPath,
};
use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, RenameFlags};

use super::root::{Root, Target};
use super::{Result, error, path, wire_path};

impl Root {
    pub(super) fn list(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let target = self.resolve(path)?;
        let directory = target.directory()?;
        let mut entries = Vec::new();
        let mut size = 0;
        for entry in fs::Dir::read_from(&directory).map_err(error)? {
            let entry = entry.map_err(error)?;
            let name = OsStr::from_bytes(entry.file_name().to_bytes());
            if matches!(name.as_bytes(), b"." | b".." | b".git") {
                continue;
            }
            let relative = target.relative.join(name);
            size += relative.as_os_str().len() + name.len();
            if entries.len() == MAX_FILE_ENTRIES || size > MAX_FILE_PATH_BYTES {
                return Err(error("Directory listing exceeds the limit"));
            }
            let stat = match fs::statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => stat,
                Err(rustix::io::Errno::NOENT) => continue,
                Err(cause) => return Err(error(cause)),
            };
            let kind = FileType::from_raw_mode(stat.st_mode);
            let is_directory = if kind == FileType::Symlink {
                self.resolve(&relative)
                    .and_then(|target| target.stat())
                    .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::Directory)
            } else {
                kind == FileType::Directory
            };
            entries.push(FileEntry {
                name: wire_path(Path::new(name)),
                path: wire_path(&relative),
                is_directory,
                is_ignored: false,
            });
        }
        let ignored = crate::git::ignored_names(&self.path.join(&target.relative), &entries);
        for entry in &mut entries {
            entry.is_ignored = ignored.contains(&entry.name.0);
        }
        entries.sort_by_cached_key(|entry| {
            (
                !entry.is_directory,
                String::from_utf8_lossy(&entry.name.0).to_lowercase(),
                entry.name.0.clone(),
            )
        });
        Ok(entries)
    }

    pub(super) fn read(&self, path: &Path) -> Result<FileContent> {
        let target = self.resolve(path)?;
        let file = fs::openat(
            &target.parent,
            &target.name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(error)?;
        let metadata = file.metadata().map_err(error)?;
        if !metadata.is_file() {
            return Err(error("Only regular files can be read"));
        }
        if metadata.len() > MAX_FILE_BYTES as u64 {
            return Err(error("File exceeds the 5 MiB limit"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(error("File exceeds the 5 MiB limit"));
        }
        let size = bytes.len() as u64;
        let content = String::from_utf8(bytes).map_err(|_| error("File is not valid UTF-8"))?;
        Ok(FileContent {
            path: wire_path(&target.relative),
            content,
            size,
        })
    }

    pub(super) fn write(&self, path: &Path, content: &str) -> Result<ServerPath> {
        if content.len() > MAX_FILE_BYTES {
            return Err(error("Content exceeds the 5 MiB limit"));
        }
        let target = self.mutation(path)?;
        let mode = match fs::statat(&target.parent, &target.name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile => {
                Some(Mode::from_raw_mode(stat.st_mode))
            }
            Ok(_) => return Err(error("Only regular files can be overwritten")),
            Err(rustix::io::Errno::NOENT) => None,
            Err(cause) => return Err(error(cause)),
        };
        let temporary = format!(".muxy-write-{}", OperationId::new());
        let mut file = fs::openat(
            &target.parent,
            &temporary,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o666),
        )
        .map(File::from)
        .map_err(error)?;
        let result = (|| {
            file.write_all(content.as_bytes()).map_err(error)?;
            if let Some(mode) = mode {
                fs::fchmod(&file, mode).map_err(error)?;
            }
            file.sync_all().map_err(error)?;
            fs::renameat(&target.parent, &temporary, &target.parent, &target.name).map_err(error)
        })();
        if result.is_err() {
            let _ = fs::unlinkat(&target.parent, &temporary, AtFlags::empty());
        }
        result?;
        Ok(wire_path(&target.relative))
    }

    pub(super) fn mkdir(&self, path: &Path) -> Result<ServerPath> {
        let target = self.mutation(path)?;
        let name = sanitized(&target.name)?;
        for number in 1..=10_000 {
            let name = unique_name(&name, number);
            match fs::mkdirat(&target.parent, &name, Mode::from_raw_mode(0o777)) {
                Ok(()) => return Ok(wire_path(&target.relative.with_file_name(name))),
                Err(rustix::io::Errno::EXIST) => (),
                Err(cause) => return Err(error(cause)),
            }
        }
        Err(error("Cannot find an available directory name"))
    }

    pub(super) fn rename(&self, path: &Path, name: &OsStr) -> Result<ServerPath> {
        let source = self.mutation(path)?;
        source.stat()?;
        let name = sanitized(name)?;
        if source.name == name {
            return Ok(wire_path(&source.relative));
        }
        fs::renameat_with(
            &source.parent,
            &source.name,
            &source.parent,
            &name,
            RenameFlags::NOREPLACE,
        )
        .map_err(error)?;
        Ok(wire_path(&source.relative.with_file_name(name)))
    }

    pub(super) fn selected(&self, paths: &[ServerPath]) -> Result<Vec<Target>> {
        let sources = paths
            .iter()
            .map(|value| {
                let target = self.mutation(path(value))?;
                target.stat()?;
                Ok(target)
            })
            .collect::<Result<Vec<_>>>()?;
        for (index, source) in sources.iter().enumerate() {
            if sources[..index].iter().any(|other| {
                source.relative.starts_with(&other.relative)
                    || other.relative.starts_with(&source.relative)
            }) {
                return Err(error("Selection contains duplicate or overlapping paths"));
            }
        }
        Ok(sources)
    }

    pub(super) fn move_files(&self, paths: &[ServerPath], into: &Path) -> Result<Vec<ServerPath>> {
        let destination = self.resolve(into)?;
        let directory = destination.directory()?;
        let sources = self.selected(paths)?;
        for source in &sources {
            if destination.relative.starts_with(&source.relative) {
                return Err(error("Cannot move a directory into itself"));
            }
        }
        let mut moved = Vec::new();
        for source in sources {
            if source.relative.parent() == Some(destination.relative.as_path()) {
                moved.push(wire_path(&source.relative));
                continue;
            }
            let mut result = None;
            for number in 1..=10_000 {
                let name = unique_name(&source.name, number);
                match fs::renameat_with(
                    &source.parent,
                    &source.name,
                    &directory,
                    &name,
                    RenameFlags::NOREPLACE,
                ) {
                    Ok(()) => {
                        result = Some(wire_path(&destination.relative.join(name)));
                        break;
                    }
                    Err(rustix::io::Errno::EXIST) => (),
                    Err(cause) => return Err(error(cause)),
                }
            }
            moved.push(result.ok_or_else(|| error("Cannot find an available destination name"))?);
        }
        Ok(moved)
    }

    pub(super) fn delete(&self, paths: &[ServerPath]) -> Result<()> {
        let sources = self.selected(paths)?;
        for source in sources {
            super::trash::move_to_trash(self, &source)?;
        }
        Ok(())
    }
}

fn sanitized(name: &OsStr) -> Result<OsString> {
    let name = name
        .to_str()
        .map_or_else(|| name.to_owned(), |value| OsString::from(value.trim()));
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.as_bytes().contains(&b'/')
        || name.as_bytes().contains(&0)
    {
        return Err(error("Invalid file name"));
    }
    Ok(name)
}

fn unique_name(name: &OsStr, number: u32) -> OsString {
    if number == 1 {
        return name.to_owned();
    }
    let path = Path::new(name);
    let mut name = path.file_stem().unwrap_or(name).to_owned();
    name.push(format!(" {number}"));
    if let Some(extension) = path.extension() {
        name.push(".");
        name.push(extension);
    }
    name
}
