use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use muxy_core::file_lock::{self, FileLock};
use serde::Serialize;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) fn save_section(path: &Path, section: &str, values: &impl Serialize) -> Result<()> {
    let values = toml::Value::try_from(values)?;
    let values = values
        .as_table()
        .ok_or_else(|| io::Error::other(format!("{section} must be a table")))?;
    edit_document(path, |document| {
        let target = document
            .entry(section)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        target
            .as_table_mut()
            .ok_or_else(|| io::Error::other(format!("{section} must be a table")))?
            .extend(values.clone());
        Ok(())
    })
}

pub(crate) fn save_entries(
    path: &Path,
    section: &str,
    table: &str,
    entries: Vec<(&str, Option<toml::Value>)>,
) -> Result<toml::Value> {
    edit_document(path, |document| {
        let values = document
            .entry(section)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let values = values
            .as_table_mut()
            .ok_or_else(|| io::Error::other(format!("{section} must be a table")))?;
        let target = values
            .entry(table)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !target.is_table() {
            *target = toml::Value::Table(toml::Table::new());
        }
        if let Some(target) = target.as_table_mut() {
            for (key, value) in entries {
                match value {
                    Some(value) => {
                        target.insert(key.into(), value);
                    }
                    None => {
                        target.remove(key);
                    }
                }
            }
        }
        Ok(toml::Value::Table(values.clone()))
    })
}

pub(crate) fn replace_section(path: &Path, section: &str, values: &impl Serialize) -> Result<()> {
    replace_sections(path, [(section, toml::Value::try_from(values)?)])
}

pub(super) fn replace_sections<'a>(
    path: &Path,
    sections: impl IntoIterator<Item = (&'a str, toml::Value)>,
) -> Result<()> {
    edit_document(path, |document| {
        for (section, value) in sections {
            document.insert(section.into(), value);
        }
        Ok(())
    })
}

pub(super) fn edit_document<T>(
    path: &Path,
    change: impl FnOnce(&mut toml::Table) -> Result<T>,
) -> Result<T> {
    let _lock = lock(path)?;
    let mut document = read_document(path)?;
    let changed = change(&mut document)?;
    atomic_write(path, &toml::to_string_pretty(&document)?)?;
    Ok(changed)
}

pub(super) fn lock(path: &Path) -> Result<FileLock> {
    const TIMEOUT: Duration = Duration::from_secs(2);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("settings path has no parent"))?;
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("settings path has no file name"))?
        .to_string_lossy();
    let lock = file_lock::acquire(&path.with_file_name(format!("{name}.lock")), TIMEOUT).map_err(
        file_lock::busy(format!(
            "{name} is still being saved by another change; try again"
        )),
    )?;
    Ok(lock)
}

pub(crate) fn atomic_write(path: &Path, source: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("settings path has no parent"))?;
    fs::create_dir_all(parent)?;
    muxy_core::fs::write_atomic(path, source.as_bytes(), 0o644).map_err(io::Error::from)?;
    Ok(())
}

pub(super) fn read_document(path: &Path) -> Result<toml::Table> {
    match fs::read_to_string(path) {
        Ok(source) => source.parse().map_err(Into::into),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_saves_of_different_sections_all_land() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("settings.toml");
        let sections: Vec<String> = (0..8).map(|index| format!("section_{index}")).collect();
        std::thread::scope(|scope| {
            let saves: Vec<_> = sections
                .iter()
                .map(|section| {
                    let path = &path;
                    scope.spawn(move || -> Result<()> {
                        for round in 0..20 {
                            let mut values = toml::Table::new();
                            values.insert("round".into(), round.into());
                            save_section(path, section, &values)?;
                        }
                        Ok(())
                    })
                })
                .collect();
            saves
                .into_iter()
                .try_for_each(|save| save.join().map_err(|_| io::Error::other("save panicked"))?)
        })?;
        let document = read_document(&path)?;
        for section in &sections {
            assert_eq!(
                document[section]["round"].as_integer(),
                Some(19),
                "{section}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_held_lock_fails_the_save_after_the_timeout_and_keeps_the_file() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let settings = directory.path().join("settings.toml");
        let terminal = directory.path().join("terminal.toml");
        save_section(&settings, "window", &toml::Table::new())?;
        crate::settings::TerminalSettings::load_native(&terminal)?;
        let sources = [fs::read(&settings)?, fs::read(&terminal)?];
        let _settings_lock = lock(&settings)?;
        let _terminal_lock = lock(&terminal)?;

        let started = std::time::Instant::now();
        let error = save_section(&settings, "panes", &toml::Table::new())
            .err()
            .ok_or("the save should time out")?;
        assert!(started.elapsed() >= Duration::from_secs(2));
        assert_eq!(
            error.to_string(),
            "settings.toml is still being saved by another change; try again"
        );
        let error = crate::settings::TerminalSettings::edit_native(&terminal, |_| Ok(()))
            .err()
            .ok_or("the save should time out")?;
        assert_eq!(
            error.to_string(),
            "terminal.toml: terminal.toml is still being saved by another change; try again"
        );
        assert_eq!([fs::read(&settings)?, fs::read(&terminal)?], sources);
        Ok(())
    }
}
