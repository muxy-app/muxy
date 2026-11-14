use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::Path;

use muxy_protocol::{
    FileBytes, FileContent, FileEntry, MAX_FILE_BYTES, MAX_FILE_ENTRIES, MAX_FILE_PATH_BYTES,
    OperationId, ServerPath,
};

use super::native::{self, Kind};
use super::root::{Root, Target};
use super::{Result, error, is_git, path, wire_path};

impl Root {
    pub(super) fn list(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let target = self.resolve(path)?;
        let directory = target.directory()?;
        let mut entries = Vec::new();
        let mut size = 0;
        for name in native::names(&directory).map_err(error)? {
            if is_git(&name) {
                continue;
            }
            let relative = target.relative.join(&name);
            // A name Windows can't represent on the wire is left out.
            let (Ok(wire_name), Ok(wire_relative)) =
                (wire_path(Path::new(&name)), wire_path(&relative))
            else {
                continue;
            };
            size += wire_relative.0.len() + wire_name.0.len();
            if entries.len() == MAX_FILE_ENTRIES || size > MAX_FILE_PATH_BYTES {
                return Err(error("Directory listing exceeds the limit"));
            }
            let stat = match native::stat(&directory, &name) {
                Ok(stat) => stat,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => continue,
                Err(cause) => return Err(error(cause)),
            };
            let is_directory = if stat.kind == Kind::Link {
                self.resolve(&relative)
                    .and_then(|target| target.stat())
                    .is_ok_and(|stat| stat.kind == Kind::Directory)
            } else {
                stat.kind == Kind::Directory
            };
            entries.push(FileEntry {
                name: wire_name,
                path: wire_relative,
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
        let FileBytes { path, bytes } = self.read_bytes(path)?;
        let size = bytes.len() as u64;
        let content = String::from_utf8(bytes).map_err(|_| error("File is not valid UTF-8"))?;
        Ok(FileContent {
            path,
            content,
            size,
        })
    }

    pub(super) fn read_bytes(&self, path: &Path) -> Result<FileBytes> {
        let target = self.resolve(path)?;
        let file = native::open_file(&target.parent, &target.name).map_err(error)?;
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
        Ok(FileBytes {
            path: wire_path(&target.relative)?,
            bytes,
        })
    }

    pub(super) fn write(&self, path: &Path, content: &[u8]) -> Result<ServerPath> {
        if content.len() > MAX_FILE_BYTES {
            return Err(error("Content exceeds the 5 MiB limit"));
        }
        let target = self.mutation(path)?;
        let replaced = match native::stat(&target.parent, &target.name) {
            Ok(stat) if stat.kind == Kind::File => Some(stat),
            Ok(_) => return Err(error("Only regular files can be overwritten")),
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => None,
            Err(cause) => return Err(error(cause)),
        };
        let temporary = OsString::from(format!(".muxy-write-{}", OperationId::new()));
        let mut file = native::create_file(&target.parent, &temporary).map_err(error)?;
        let result = (|| {
            file.write_all(content).map_err(error)?;
            if let Some(replaced) = &replaced {
                native::copy_mode(&file, replaced).map_err(error)?;
            }
            file.sync_all().map_err(error)?;
            drop(file);
            native::replace(&target.parent, &temporary, &target.name).map_err(error)
        })();
        if result.is_err() {
            let _ = native::remove_file(&target.parent, &temporary);
        }
        result?;
        wire_path(&target.relative)
    }

    pub(super) fn mkdir(&self, path: &Path) -> Result<ServerPath> {
        let target = self.mutation(path)?;
        let name = sanitized(&target.name)?;
        for number in 1..=10_000 {
            let name = unique_name(&name, number);
            match native::create_directory(&target.parent, &name) {
                Ok(()) => return wire_path(&target.relative.with_file_name(name)),
                Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => (),
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
            return wire_path(&source.relative);
        }
        native::rename(&source.parent, &source.name, &source.parent, &name).map_err(error)?;
        wire_path(&source.relative.with_file_name(name))
    }

    pub(super) fn selected(&self, paths: &[ServerPath]) -> Result<Vec<Target>> {
        let sources = paths
            .iter()
            .map(|value| {
                let target = self.mutation(&path(value)?)?;
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
                moved.push(wire_path(&source.relative)?);
                continue;
            }
            let mut result = None;
            for number in 1..=10_000 {
                let name = unique_name(&source.name, number);
                match native::rename(&source.parent, &source.name, &directory, &name) {
                    Ok(()) => {
                        result = Some(wire_path(&destination.relative.join(name))?);
                        break;
                    }
                    Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => (),
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
    let bytes = name.as_encoded_bytes();
    if name.is_empty() || name == "." || name == ".." || bytes.contains(&b'/') || bytes.contains(&0)
    {
        return Err(error("Invalid file name"));
    }
    #[cfg(windows)]
    if let Some(refusal) = name.to_str().map_or(
        Some("File names must be valid Unicode"),
        super::names::windows_refusal,
    ) {
        return Err(error(refusal));
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
