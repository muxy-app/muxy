use std::fs;
use std::io::Write;
use std::path::Path;

use muxy_app_core::backup::Result;
use muxy_ui::tr;

use super::{PENDING, PreparedImport, ROOTS, Restore, archive, extensions, validate_effective};

/// Replaces any pending import. When staging fails, nothing stays pending.
pub(crate) fn stage(profile: &Path, import: &PreparedImport) -> Result<()> {
    validate_effective(profile, &import.files, import.restore)?;
    let staged = extensions::stage(profile, import.extensions.as_ref())
        .and_then(|()| archive::write(&profile.join(PENDING), &import.files, import.restore));
    if staged.is_err() {
        cancel_pending(profile)?;
    }
    staged
}

pub(crate) fn cancel_pending(profile: &Path) -> Result<()> {
    extensions::cancel(profile)?;
    match fs::remove_file(profile.join(PENDING)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn apply_pending(profile: &Path) -> Result<Option<String>> {
    recover(profile)?;
    let pending = profile.join(PENDING);
    if !pending.try_exists()? {
        return Ok(None);
    }
    if let Err(error) = restore(profile, &pending) {
        recover(profile)?;
        return Ok(Some(
            tr!(
                "Could not restore backup: %@. Your previous configuration was kept. Cancel or replace the pending import in Settings → Backup & Restore, or reopen Muxy to retry.",
                error.to_string()
            )
            .to_string(),
        ));
    }
    Ok(None)
}

fn restore(profile: &Path, pending: &Path) -> Result<()> {
    let (mut files, legacy, restore) = archive::read(pending)?;
    if legacy {
        return Err(tr!("Pending import must use the current backup format")
            .to_string()
            .into());
    }
    super::normalize_terminal(&mut files)?;
    validate_effective(profile, &files, restore)?;
    if let Some(bytes) = files.get("desktop-state.json") {
        let imported: muxy_app_core::AppState = serde_json::from_slice(bytes)?;
        let current = muxy_app_core::store::load(profile.join("desktop-state.json"))?;
        let restored = if restore == Restore::Merge {
            imported.merge_configuration(&current)?
        } else {
            imported.restore_configuration(&current)?
        };
        let settings = muxy_app_core::backup::remap_settings(
            std::str::from_utf8(&files["settings.toml"])?,
            &imported,
            &restored,
        )?;
        files.insert("settings.toml".into(), settings.into_bytes());
        files.insert(
            "desktop-state.json".into(),
            serde_json::to_vec_pretty(&restored)?,
        );
    }
    let staging = tempfile::tempdir_in(profile)?;
    archive::materialize(staging.path(), &files)?;
    let roots: Vec<_> = ROOTS
        .iter()
        .copied()
        .filter(|root| restore == Restore::Complete || staging.path().join(root).exists())
        .collect();
    let original = archive::collect(profile, &roots)?;
    let backups = profile.join("Backups");
    fs::create_dir_all(&backups)?;
    let recovery = tempfile::Builder::new()
        .prefix("pre-import-")
        .tempdir_in(&backups)?
        .keep();
    archive::materialize(&recovery, &original)?;
    let mut portable = original.clone();
    if roots.contains(&"terminal.toml") && !portable.contains_key("terminal.toml") {
        let legacy = profile.join("ghostty.conf");
        let terminal = if legacy.try_exists()? {
            muxy_app_core::settings::TerminalSettings::from_legacy_source(
                &muxy_app_core::settings::TerminalSettings::backup_source(&legacy)?,
            )?
        } else {
            muxy_app_core::settings::TerminalSettings::default()
        };
        portable.insert(
            "terminal.toml".into(),
            terminal.native_source()?.into_bytes(),
        );
    }
    if portable.contains_key("terminal.toml") {
        portable.remove("ghostty.conf");
    } else if portable.contains_key("ghostty.conf") {
        portable.insert(
            "ghostty.conf".into(),
            muxy_app_core::settings::TerminalSettings::backup_source(
                &profile.join("ghostty.conf"),
            )?
            .into_bytes(),
        );
    }
    if roots.contains(&super::mobile::FILE) {
        portable.insert(
            super::mobile::FILE.into(),
            serde_json::to_vec_pretty(&super::mobile::current(profile)?)?,
        );
    }
    let recovery_restore = if restore == Restore::Complete {
        Restore::Complete
    } else {
        Restore::Partial
    };
    archive::write(&recovery.join("recovery.muxy"), &portable, recovery_restore)?;
    fs::write(recovery.join("roots.json"), serde_json::to_vec(&roots)?)?;
    fs::write(
        recovery.join("extensions.json"),
        serde_json::to_vec(&extensions::additions(profile)?)?,
    )?;
    let marker = profile.join("restore-in-progress.json");
    let mut journal = tempfile::NamedTempFile::new_in(profile)?;
    journal.write_all(&serde_json::to_vec(&recovery)?)?;
    journal.as_file().sync_all()?;
    journal.persist(&marker)?;
    for root in &roots {
        remove(&profile.join(root))?;
        if staging.path().join(root).exists() {
            fs::rename(staging.path().join(root), profile.join(root))?;
        }
    }
    extensions::install(profile)?;
    fs::remove_file(pending)?;
    fs::remove_file(&marker)?;
    Ok(())
}

fn recover(profile: &Path) -> Result<()> {
    let marker = profile.join("restore-in-progress.json");
    if !marker.try_exists()? {
        return Ok(());
    }
    let recovery: std::path::PathBuf = serde_json::from_slice(&archive::read_file(&marker)?)?;
    let backups = fs::canonicalize(profile.join("Backups"))?;
    if fs::canonicalize(&recovery)?.parent() != Some(backups.as_path()) {
        return Err(tr!("Invalid recovery directory").to_string().into());
    }
    let roots: Vec<String> =
        serde_json::from_slice(&archive::read_file(&recovery.join("roots.json"))?)?;
    if roots.iter().any(|root| !ROOTS.contains(&root.as_str())) {
        return Err(tr!("Invalid recovery entry").to_string().into());
    }
    let added = recovery.join("extensions.json");
    let added: Vec<String> = if added.try_exists()? {
        serde_json::from_slice(&archive::read_file(&added)?)?
    } else {
        Vec::new()
    };
    if added.iter().any(|name| {
        let mut parts = Path::new(name).components();
        !matches!(
            (parts.next(), parts.next()),
            (Some(std::path::Component::Normal(_)), None)
        )
    }) {
        return Err(tr!("Invalid recovery entry").to_string().into());
    }
    extensions::take_back(profile, &added)?;
    let names: Vec<_> = roots.iter().map(String::as_str).collect();
    let original = archive::collect(&recovery, &names)?;
    for root in roots {
        remove(&profile.join(root))?;
    }
    archive::materialize(profile, &original)?;
    fs::remove_file(marker)?;
    Ok(())
}

fn remove(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
