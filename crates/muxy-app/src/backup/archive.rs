use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path};

use muxy_app_core::backup::Result;
use muxy_ui::tr;
use serde_json::Value;

use super::{Files, MAX_BYTES, MAX_FILE_BYTES, ROOTS, Restore};

const MAX_FILES: usize = 8192;

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['\\', '\0'])
        && Path::new(name)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn allowed(name: &str, roots: &[&str]) -> bool {
    safe_name(name)
        && roots.iter().any(|root| {
            name == *root
                || name
                    .strip_prefix(root)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
}

/// What a 1.x backup may contain.
const LEGACY_ROOTS: &[&str] = &[
    "settings.json",
    "projects.json",
    "workspaces.json",
    "project-groups.json",
    "keybindings.json",
    "command-shortcuts.json",
    "ghostty.conf",
    "worktrees",
    "logos",
];

/// A backup error in the app language.
fn failure(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    message.into()
}

pub(super) fn read_file(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return Err(failure(&tr!(
            "%@ must be a regular file no larger than 16 MiB",
            path.display().to_string()
        )));
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(failure(&tr!("Configuration file exceeds 16 MiB")));
    }
    Ok(bytes)
}

pub(super) fn collect(directory: &Path, roots: &[&str]) -> Result<Files> {
    let mut files = Files::new();
    let mut pending = Vec::new();
    for root in roots {
        match fs::symlink_metadata(directory.join(root)) {
            Ok(_) => pending.push(directory.join(root)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    let mut size = 0;
    let mut count = 0;
    while let Some(path) = pending.pop() {
        count += 1;
        if count > MAX_FILES {
            return Err(failure(&tr!("Configuration contains too many files")));
        }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                pending.push(entry?.path());
            }
            continue;
        }
        let name = path
            .strip_prefix(directory)?
            .to_str()
            .ok_or_else(|| failure(&tr!("Configuration filenames must be UTF-8")))?
            .to_owned();
        if !allowed(&name, roots) {
            return Err(failure(&tr!("Invalid configuration path")));
        }
        let bytes = read_file(&path)?;
        size += bytes.len() as u64;
        if size > MAX_BYTES {
            return Err(failure(&tr!("Configuration exceeds 64 MiB")));
        }
        files.insert(name, bytes);
    }
    Ok(files)
}

pub(super) fn write(destination: &Path, files: &Files, restore: Restore) -> Result<()> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut archive = zip::ZipWriter::new(temporary.as_file_mut());
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    archive.start_file("manifest.json", options)?;
    archive.write_all(&serde_json::to_vec_pretty(&serde_json::json!({
        "schemaVersion": 2,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "format": "muxy.configuration",
        "complete": restore == Restore::Complete,
        "merge": restore == Restore::Merge,
        "files": files.keys().collect::<Vec<_>>(),
    }))?)?;
    let mut size = 0;
    for (name, bytes) in files {
        size += bytes.len() as u64;
        if !allowed(name, ROOTS) || bytes.len() as u64 > MAX_FILE_BYTES || size > MAX_BYTES {
            return Err(failure(&tr!("Invalid or oversized backup content")));
        }
        archive.start_file(name, options)?;
        archive.write_all(bytes)?;
    }
    archive.finish()?;
    temporary.as_file().sync_all()?;
    temporary.persist(destination)?;
    Ok(())
}

pub(super) fn read(source: &Path) -> Result<(Files, bool, Restore)> {
    let file = File::open(source)?;
    if file.metadata()?.len() > MAX_BYTES {
        return Err(failure(&tr!("Backup exceeds 64 MiB")));
    }
    let mut archive = zip::ZipArchive::new(file)?;
    if archive.len() > MAX_FILES {
        return Err(failure(&tr!("Backup contains too many entries")));
    }
    let manifest: Value = {
        let file = archive.by_name("manifest.json")?;
        if file.size() > 1024 * 1024 {
            return Err(failure(&tr!("Backup manifest is too large")));
        }
        let mut bytes = Vec::new();
        file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        serde_json::from_slice(&bytes)?
    };
    let legacy = match manifest["schemaVersion"].as_u64() {
        Some(1) => true,
        Some(2) if manifest["format"].as_str() == Some("muxy.configuration") => false,
        _ => {
            return Err(failure(&tr!(
                "This backup format is not supported by this version of Muxy"
            )));
        }
    };
    let names: Vec<String> = serde_json::from_value(manifest["files"].clone())?;
    let roots = if legacy { LEGACY_ROOTS } else { ROOTS };
    let mut files = Files::new();
    let mut seen = HashSet::new();
    let mut size = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().trim_end_matches('/').to_owned();
        if !safe_name(&name)
            || !seen.insert(name.clone())
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170_000 == 0o120_000)
        {
            return Err(failure(&tr!(
                "Backup contains an unsafe or duplicate entry"
            )));
        }
        if entry.is_dir() || name == "manifest.json" || name.starts_with("__MACOSX/") {
            continue;
        }
        if !allowed(&name, roots)
            || !names.iter().any(|root| {
                name == *root
                    || name
                        .strip_prefix(root)
                        .is_some_and(|suffix| suffix.starts_with('/'))
            })
        {
            if legacy {
                continue;
            }
            return Err(failure(&tr!("Unexpected backup entry: %@", &name)));
        }
        if entry.size() > MAX_FILE_BYTES || size + entry.size() > MAX_BYTES {
            return Err(failure(&tr!("Backup contents exceed the size limit")));
        }
        let mut bytes = Vec::new();
        (&mut entry)
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        size += bytes.len() as u64;
        if bytes.len() as u64 > MAX_FILE_BYTES || size > MAX_BYTES {
            return Err(failure(&tr!("Backup contents exceed the size limit")));
        }
        files.insert(name, bytes);
    }
    for name in names {
        if !safe_name(&name) {
            return Err(failure(&tr!("Invalid manifest path")));
        }
        if allowed(&name, roots)
            && !seen.contains(&name)
            && !files
                .keys()
                .any(|file| file.starts_with(&format!("{name}/")))
        {
            return Err(failure(&tr!("Backup is missing %@", &name)));
        }
    }
    let restore = if legacy {
        Restore::Merge
    } else if manifest["complete"].as_bool().unwrap_or(false) {
        Restore::Complete
    } else if manifest["merge"].as_bool().unwrap_or(false) {
        Restore::Merge
    } else {
        Restore::Partial
    };
    Ok((files, legacy, restore))
}

pub(super) fn materialize(directory: &Path, files: &Files) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    for (name, bytes) in files {
        if !allowed(name, ROOTS) {
            return Err(failure(&tr!("Invalid restore path")));
        }
        let path = directory.join(name);
        if let Some(parent) = path.parent() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    Ok(())
}
