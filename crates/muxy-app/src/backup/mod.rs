mod archive;
mod mobile;
mod transaction;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use muxy_app_core::backup::{ImportReport, Result};
use muxy_app_core::settings::{Settings, TerminalSettings};
use muxy_ui::tr;

pub(crate) use mobile::apply_mobile_settings;
pub(crate) use transaction::{apply_pending, cancel_pending, stage};

pub(super) const ROOTS: &[&str] = &[
    "settings.toml",
    "ghostty.conf",
    "server.toml",
    "mobile-settings.json",
    "desktop-state.json",
    "extension-enabled.json",
    "extension-folders.json",
    "extension-settings",
    "themes",
];
pub(super) const MAX_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub(super) const PENDING: &str = "pending-import.muxy";
type Files = BTreeMap<String, Vec<u8>>;

pub(crate) struct PreparedImport {
    pub(super) files: Files,
    pub(super) complete: bool,
    pub(crate) summary: String,
}

pub(crate) fn legacy_directory() -> Result<PathBuf> {
    let home =
        std::env::home_dir().ok_or_else(|| tr!("Home directory is unavailable").to_string())?;
    let path = home.join("Library/Application Support/Muxy");
    if !path.is_dir() {
        return Err(tr!("No Muxy 1.x configuration was found on this computer. Choose a 1.x backup file instead.").to_string().into());
    }
    Ok(path)
}

pub(crate) fn export(
    profile: &Path,
    destination: &Path,
    state: &muxy_app_core::AppState,
) -> Result<()> {
    let mut files = archive::collect(profile, ROOTS)?;
    files.insert(
        "desktop-state.json".into(),
        serde_json::to_vec_pretty(&state.configuration_backup())?,
    );
    if profile.join("ghostty.conf").exists() {
        files.insert(
            "ghostty.conf".into(),
            TerminalSettings::backup_source(&profile.join("ghostty.conf"))?.into_bytes(),
        );
    }
    files
        .entry("settings.toml".into())
        .or_insert(muxy_app_core::backup::settings_source(&Settings::default())?.into_bytes());
    files.insert(
        mobile::FILE.into(),
        serde_json::to_vec_pretty(&mobile::current(profile)?)?,
    );
    validate(&files)?;
    archive::write(destination, &files, true)
}

pub(crate) fn prepare(profile: &Path, source: &Path) -> Result<PreparedImport> {
    let (files, legacy, complete) = if source.is_dir() {
        (
            archive::collect(
                source,
                &[
                    "settings.json",
                    "projects.json",
                    "workspaces.json",
                    "project-groups.json",
                    "keybindings.json",
                    "command-shortcuts.json",
                    "ghostty.conf",
                    "worktrees",
                    "logos",
                ],
            )?,
            true,
            false,
        )
    } else if source
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        (
            BTreeMap::from([("settings.json".into(), archive::read_file(source)?)]),
            true,
            false,
        )
    } else {
        archive::read(source)?
    };
    let (files, report) = if legacy {
        migrate(profile, &files)?
    } else {
        (files, ImportReport::default())
    };
    validate_effective(profile, &files, complete)?;
    let projects = files
        .get("desktop-state.json")
        .map(|bytes| serde_json::from_slice::<muxy_app_core::AppState>(bytes))
        .transpose()?;
    if let Some(state) = &projects {
        let current = muxy_app_core::store::load(profile.join("desktop-state.json"))?;
        state.check_restore_directories(&current)?;
    }
    let count = projects
        .as_ref()
        .map_or(0, |state| state.projects().len().saturating_sub(1));
    let mut summary = tr!(
        "Restore settings and %lld saved projects with their layouts on the next launch. Existing projects are kept; matching projects receive the saved layouts. A recovery copy is saved in the profile’s Backups folder. Restart the server from Settings to apply restored server settings.",
        count
    )
    .to_string();
    if legacy {
        write!(
            summary,
            "\n\n{}",
            tr!("Imported %lld supported 1.x items.", report.imported)
        )?;
        if !report.skipped.is_empty() {
            write!(
                summary,
                " {}",
                tr!(
                    "Skipped %lld unsupported or invalid items:\n%@",
                    report.skipped.len(),
                    report
                        .skipped
                        .iter()
                        .take(15)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            )?;
        }
    }
    Ok(PreparedImport {
        files,
        complete,
        summary,
    })
}

fn migrate(profile: &Path, legacy: &Files) -> Result<(Files, ImportReport)> {
    let current = Settings::load(&profile.join("settings.toml"))?;
    let source = muxy_app_core::backup::merge_legacy_files(
        legacy.get("settings.json").map(Vec::as_slice),
        legacy.get("keybindings.json").map(Vec::as_slice),
        legacy.get("command-shortcuts.json").map(Vec::as_slice),
    )?;
    let mut source: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&source)?;
    let mut files = Files::new();
    let mut report = ImportReport::default();
    mobile::migrate(profile, &mut source, &mut files, &mut report)?;
    let (mut settings, settings_report) =
        muxy_app_core::backup::import_settings(&serde_json::to_vec(&source)?, &current)?;
    report.imported += settings_report.imported;
    report.skipped.extend(settings_report.skipped);
    if let Some(source) = legacy.get("ghostty.conf") {
        let (source, skipped) =
            TerminalSettings::import_supported_source(std::str::from_utf8(source)?);
        report.skipped.extend(skipped);
        report.imported += source.lines().count();
        if !source.trim().is_empty() {
            files.insert(
                "ghostty.conf".into(),
                merge_terminal(profile, &source)?.into_bytes(),
            );
        }
    }
    if let Some(projects) = legacy.get("projects.json") {
        let (state, projects) = muxy_app_core::backup::import_projects(
            projects,
            legacy.get("workspaces.json").map(Vec::as_slice),
            legacy.get("project-groups.json").map(Vec::as_slice),
            legacy,
            &mut settings,
        )?;
        report.imported += projects.imported;
        report.skipped.extend(projects.skipped);
        files.insert(
            "desktop-state.json".into(),
            serde_json::to_vec_pretty(&state)?,
        );
    }
    if report.imported == 0 {
        return Err(tr!(
            "The selected 1.x configuration contains no supported settings or projects."
        )
        .to_string()
        .into());
    }
    files.insert(
        "settings.toml".into(),
        muxy_app_core::backup::settings_source(&settings)?.into_bytes(),
    );
    Ok((files, report))
}

fn validate(files: &Files) -> Result<()> {
    if !files.contains_key("settings.toml") {
        return Err(tr!("The backup does not contain settings.toml")
            .to_string()
            .into());
    }
    let directory = tempfile::tempdir()?;
    archive::materialize(directory.path(), files)?;
    if let Some(source) = files.get("ghostty.conf")
        && std::str::from_utf8(source)?.lines().any(|line| {
            line.split('=')
                .next()
                .is_some_and(|key| key.trim() == "config-file")
        })
    {
        return Err(tr!(
            "Backups must contain resolved Ghostty settings, without config-file references"
        )
        .to_string()
        .into());
    }
    muxy_app_core::backup::validate_configuration(directory.path())?;
    if let Some(state) = files.get("desktop-state.json") {
        let _: muxy_app_core::AppState = serde_json::from_slice(state)?;
    }
    if let Some(source) = files.get(mobile::FILE) {
        mobile::parse(source)?;
    }
    for (name, source) in files {
        if name.starts_with("extension-") {
            let value: serde_json::Value = serde_json::from_slice(source)?;
            match name.as_str() {
                "extension-enabled.json" => {
                    let _: Vec<String> = serde_json::from_value(value)?;
                }
                "extension-folders.json" => {
                    let _: BTreeMap<String, PathBuf> = serde_json::from_value(value)?;
                }
                _ if !value.is_object() => {
                    return Err(tr!("%@ must contain an object", name).to_string().into());
                }
                _ => (),
            }
        }
    }
    Ok(())
}

fn validate_effective(profile: &Path, files: &Files, complete: bool) -> Result<()> {
    if complete || files.contains_key("ghostty.conf") || !profile.join("ghostty.conf").exists() {
        return validate(files);
    }
    let mut effective = files.clone();
    effective.insert(
        "ghostty.conf".into(),
        TerminalSettings::backup_source(&profile.join("ghostty.conf"))?.into_bytes(),
    );
    validate(&effective)
}

fn merge_terminal(profile: &Path, imported: &str) -> Result<String> {
    let path = profile.join("ghostty.conf");
    let current = if path.exists() {
        TerminalSettings::backup_source(&path)?
    } else {
        String::new()
    };
    let replaced: std::collections::HashSet<_> = imported
        .lines()
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim()))
        .filter(|key| !matches!(*key, "keybind" | "palette"))
        .collect();
    let mut merged = String::new();
    for line in current.lines() {
        if !line
            .split_once('=')
            .is_some_and(|(key, _)| replaced.contains(key.trim()))
        {
            writeln!(merged, "{line}")?;
        }
    }
    merged.push_str(imported);
    Ok(merged)
}
