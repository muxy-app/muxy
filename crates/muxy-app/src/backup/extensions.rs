//! 1.x kept installed extensions in `~/.config/muxy/extensions` and whether
//! each is on in its preferences.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use muxy_app_core::backup::{ImportReport, Result};
use muxy_app_core::extensions::Extension;

use super::{Files, archive};

const PENDING: &str = "pending-extensions";

/// Packages 1.x had that this profile lacks, and the names 1.x had turned on.
pub(super) fn find(
    profile: &Path,
    root: &Path,
    enabled: impl Fn(&str) -> bool,
    report: &mut ImportReport,
) -> Result<(Vec<PathBuf>, Vec<String>)> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(error) => return Err(error.into()),
    };
    let mut packages = Vec::new();
    let mut names = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with('.') || !path.is_dir() || profile.join("extensions").join(name).exists()
        {
            continue;
        }
        match Extension::load(&path) {
            Ok(extension) if extension.name == name => {
                if enabled(name) {
                    names.push(name.to_owned());
                }
                packages.push(path);
            }
            Ok(_) => report
                .attention
                .push(format!("Extension {name} (name does not match its folder)")),
            Err(error) => report.attention.push(format!("Extension {name} ({error})")),
        }
    }
    Ok((packages, names))
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

/// Adds `names` to the extensions this profile has turned on.
pub(super) fn enable(profile: &Path, files: &mut Files, names: Vec<String>) -> Result<()> {
    let path = profile.join("extension-enabled.json");
    let mut enabled: BTreeSet<String> = if path.exists() {
        serde_json::from_slice(&archive::read_file(&path)?)?
    } else {
        BTreeSet::new()
    };
    enabled.extend(names);
    files.insert(
        "extension-enabled.json".into(),
        serde_json::to_vec_pretty(&enabled)?,
    );
    Ok(())
}

/// Copies `packages` beside the staged import, replacing earlier copies.
pub(super) fn stage(profile: &Path, packages: &[PathBuf]) -> Result<()> {
    cancel(profile)?;
    let pending = profile.join(PENDING);
    for package in packages {
        let name = package.file_name().ok_or("Invalid extension folder")?;
        if let Err(error) =
            fs::create_dir_all(&pending).and_then(|()| copy(package, &pending.join(name)))
        {
            cancel(profile)?;
            return Err(error.into());
        }
    }
    Ok(())
}

/// Moves staged packages in; an extension this profile already has stays.
pub(super) fn install(profile: &Path) -> Result<()> {
    let pending = profile.join(PENDING);
    let entries = match fs::read_dir(&pending) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let installed = profile.join("extensions");
    fs::create_dir_all(&installed)?;
    for entry in entries {
        let entry = entry?;
        let destination = installed.join(entry.file_name());
        if fs::symlink_metadata(&destination).is_err() {
            fs::rename(entry.path(), destination)?;
        }
    }
    cancel(profile)
}

pub(super) fn cancel(profile: &Path) -> Result<()> {
    match fs::remove_dir_all(profile.join(PENDING)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

fn copy(from: &Path, to: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(from)?;
    if metadata.file_type().is_symlink() {
        return std::os::unix::fs::symlink(fs::read_link(from)?, to);
    }
    if !metadata.is_dir() {
        return fs::copy(from, to).map(|_| ());
    }
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        copy(&entry.path(), &to.join(entry.file_name()))?;
    }
    Ok(())
}
