use std::path::Path;

use muxy_ui::tr;

use super::Result;

pub(super) fn lock(directory: &Path) -> Result<std::fs::File> {
    Ok(muxy_client::local::bundle::lock_replacement(directory)?)
}

pub(super) fn replace_and_restart(
    current: &Path,
    next: &Path,
    backup: &Path,
    restart: impl FnOnce() -> Result<()>,
) -> Result<()> {
    replace(current, next, backup)?;
    if let Err(error) = restart() {
        replace(current, backup, next)?;
        return Err(tr!(
            "Could not restart %@; the previous app was restored: %@",
            muxy_core::release::Channel::current().app_name(),
            error.to_string()
        )
        .to_string()
        .into());
    }
    Ok(())
}

pub(super) fn replace(current: &Path, next: &Path, backup: &Path) -> Result<()> {
    replace_with(current, next, backup, |from, to| std::fs::rename(from, to))
}

fn replace_with(
    current: &Path,
    next: &Path,
    backup: &Path,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    rename(current, backup)?;
    if let Err(error) = rename(next, current) {
        if let Err(rollback) = rename(backup, current) {
            return Err(tr!(
                "Could not install update (%@) or restore the previous app (%@). The previous app is at %@",
                error.to_string(),
                rollback.to_string(),
                backup.display().to_string()
            )
            .to_string()
            .into());
        }
        return Err(tr!(
            "Could not install update; the previous app was restored: %@",
            error.to_string()
        )
        .to_string()
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_restart_restores_the_previous_app() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let current = directory.path().join("current.app");
        let next = directory.path().join("next.app");
        let backup = directory.path().join("previous.app");
        std::fs::create_dir(&current)?;
        std::fs::create_dir(&next)?;
        std::fs::write(current.join("version"), b"old")?;
        std::fs::write(next.join("version"), b"new")?;
        assert!(
            replace_and_restart(&current, &next, &backup, || Err("spawn failed".into())).is_err()
        );
        assert_eq!(std::fs::read(current.join("version"))?, b"old");
        assert_eq!(std::fs::read(next.join("version"))?, b"new");
        Ok(())
    }

    #[test]
    fn failed_replacement_restores_the_whole_previous_bundle() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let current = dir.path().join("Muxy Beta.app");
        let backup = dir.path().join("previous.app");
        std::fs::create_dir(&current)?;
        std::fs::write(current.join("server"), b"old server")?;
        assert!(replace(&current, &dir.path().join("missing.app"), &backup).is_err());
        assert_eq!(std::fs::read(current.join("server"))?, b"old server");
        assert!(!backup.exists());
        Ok(())
    }

    #[test]
    fn replacement_keeps_the_old_bundle_available_until_restart() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let current = dir.path().join("current.app");
        let next = dir.path().join("next.app");
        let backup = dir.path().join("previous.app");
        for (path, version) in [(&current, "old"), (&next, "new")] {
            std::fs::create_dir(path)?;
            std::fs::write(path.join("app"), version)?;
            std::fs::write(path.join("server"), version)?;
        }
        replace(&current, &next, &backup)?;
        assert_eq!(std::fs::read(current.join("app"))?, b"new");
        assert_eq!(std::fs::read(current.join("server"))?, b"new");
        assert_eq!(std::fs::read(backup.join("server"))?, b"old");
        Ok(())
    }
}
