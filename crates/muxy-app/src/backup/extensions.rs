//! 1.x kept installed extensions in `~/.config/muxy/extensions` and whether
//! each is on in its preferences.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use muxy_app_core::backup::{ImportReport, Result};
use muxy_app_core::extensions::Extension;
use tempfile::TempDir;

use super::{Files, archive};

const PENDING: &str = "pending-extensions";

/// 1.x packages this profile lacks, copied beside it until the import is
/// staged.
pub(super) struct Copies {
    folder: TempDir,
    pub(super) names: Vec<String>,
    pub(super) enabled: Vec<String>,
}

/// A package that can't be loaded or copied is reported and left out.
pub(super) fn copy_legacy(
    profile: &Path,
    root: &Path,
    enabled: impl Fn(&str) -> bool,
    report: &mut ImportReport,
) -> Result<Option<Copies>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut copies = Copies {
        folder: tempfile::Builder::new()
            .prefix(".import-extensions-")
            .tempdir_in(profile)?,
        names: Vec::new(),
        enabled: Vec::new(),
    };
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with('.') || !path.is_dir() || profile.join("extensions").join(name).exists()
        {
            continue;
        }
        let destination = copies.folder.path().join(name);
        match copy_package(&path, name, &destination) {
            Ok(0) => (),
            Ok(dropped) => report.attention.push(format!(
                "Extension {name} ({dropped} links outside its folder were left out)"
            )),
            Err(error) => {
                remove_dir(&destination)?;
                report.attention.push(format!("Extension {name} ({error})"));
                continue;
            }
        }
        if enabled(name) {
            copies.enabled.push(name.to_owned());
        }
        copies.names.push(name.to_owned());
    }
    Ok((!copies.names.is_empty()).then_some(copies))
}

/// Returns how many links were left out because they leave the package.
fn copy_package(package: &Path, name: &str, destination: &Path) -> Result<usize> {
    if Extension::load(package)?.name != name {
        return Err("name does not match its folder".into());
    }
    let root = fs::canonicalize(package)?;
    let mut dropped = 0;
    copy(&root, destination, &root, &mut dropped)?;
    Extension::load(destination)?;
    Ok(dropped)
}

pub(super) fn legacy_root() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(".config/muxy/extensions"))
}

pub(super) fn enabled_in_legacy(name: &str) -> bool {
    std::process::Command::new("/usr/bin/defaults")
        .args(["read", "com.muxy.app", &format!("muxy.ext.enabled.{name}")])
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout.trim_ascii() == b"1")
}

pub(super) fn enable(profile: &Path, files: &mut Files, names: &[String]) -> Result<()> {
    let path = profile.join("extension-enabled.json");
    let mut enabled: BTreeSet<String> = if path.exists() {
        serde_json::from_slice(&archive::read_file(&path)?)?
    } else {
        BTreeSet::new()
    };
    enabled.extend(names.iter().cloned());
    files.insert(
        "extension-enabled.json".into(),
        serde_json::to_vec_pretty(&enabled)?,
    );
    Ok(())
}

/// Moves the copies to where the next launch installs them from, replacing
/// earlier ones.
pub(super) fn stage(profile: &Path, copies: Option<&Copies>) -> Result<()> {
    cancel(profile)?;
    if let Some(copies) = copies {
        fs::rename(copies.folder.path(), profile.join(PENDING))?;
    }
    Ok(())
}

/// An extension this profile already has stays. When one move fails, the
/// ones before it are moved back.
pub(super) fn install(profile: &Path) -> Result<()> {
    let pending = profile.join(PENDING);
    let entries = match fs::read_dir(&pending) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let installed = profile.join("extensions");
    fs::create_dir_all(&installed)?;
    let mut moved = Vec::new();
    for entry in entries {
        let result = entry.and_then(|entry| {
            let destination = installed.join(entry.file_name());
            if fs::symlink_metadata(&destination).is_ok() {
                return Ok(());
            }
            fs::rename(entry.path(), &destination)?;
            moved.push((entry.path(), destination));
            Ok(())
        });
        if let Err(error) = result {
            for (from, to) in moved.into_iter().rev() {
                fs::rename(to, from)?;
            }
            return Err(error.into());
        }
    }
    cancel(profile)
}

/// Staged packages that installing would add, for recovery to take back.
pub(super) fn additions(profile: &Path) -> Result<Vec<String>> {
    let entries = match fs::read_dir(profile.join(PENDING)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let installed = profile.join("extensions");
    let mut names = Vec::new();
    for entry in entries {
        let name = entry?.file_name();
        if fs::symlink_metadata(installed.join(&name)).is_err() {
            names.push(
                name.into_string()
                    .map_err(|_| "Extension folder names must be UTF-8")?,
            );
        }
    }
    Ok(names)
}

/// Moves extensions an interrupted restore added back to the staged ones.
pub(super) fn take_back(profile: &Path, names: &[String]) -> Result<()> {
    let pending = profile.join(PENDING);
    for name in names {
        let installed = profile.join("extensions").join(name);
        if fs::symlink_metadata(&installed).is_ok()
            && fs::symlink_metadata(pending.join(name)).is_err()
        {
            fs::create_dir_all(&pending)?;
            fs::rename(installed, pending.join(name))?;
        }
    }
    Ok(())
}

pub(super) fn cancel(profile: &Path) -> Result<()> {
    Ok(remove_dir(&profile.join(PENDING))?)
}

fn remove_dir(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Keeps only relative links that stay inside `root`, and only regular files.
fn copy(from: &Path, to: &Path, root: &Path, dropped: &mut usize) -> io::Result<()> {
    let metadata = fs::symlink_metadata(from)?;
    if metadata.file_type().is_symlink() {
        let target = fs::read_link(from)?;
        if target.is_relative()
            && fs::canonicalize(from).is_ok_and(|resolved| resolved.starts_with(root))
        {
            return std::os::unix::fs::symlink(target, to);
        }
        *dropped += 1;
        return Ok(());
    }
    if metadata.is_file() {
        return fs::copy(from, to).map(|_| ());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        copy(&entry.path(), &to.join(entry.file_name()), root, dropped)?;
    }
    Ok(())
}
