mod archive;
mod extensions;
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

/// How a staged import changes the profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Restore {
    /// Replaces every configuration file, resetting what the backup leaves out.
    Complete,
    /// Replaces only the files the backup holds.
    Partial,
    /// Replaces only the files the import holds, and adds its projects and
    /// workspaces to the current ones.
    Merge,
}

pub(crate) struct PreparedImport {
    pub(super) files: Files,
    pub(super) restore: Restore,
    pub(crate) summary: String,
    attention: Vec<String>,
    extensions: Vec<PathBuf>,
}

/// Where 1.x installed extensions, and whether it had one turned on.
type LegacyExtensions<'a> = Option<(&'a Path, &'a dyn Fn(&str) -> bool)>;

/// Prepares the installed 1.x configuration, with its extensions.
pub(crate) fn prepare_installed(profile: &Path) -> Result<PreparedImport> {
    let root = extensions::legacy_root();
    prepare_from(
        profile,
        &legacy_directory()?,
        root.as_deref()
            .map(|root| (root, &extensions::enabled_in_legacy as _)),
    )
}

fn legacy_directory() -> Result<PathBuf> {
    installed_legacy()?.ok_or_else(|| tr!("No Muxy 1.x configuration was found on this computer. Choose a 1.x backup file instead.").to_string().into())
}

fn installed_legacy() -> Result<Option<PathBuf>> {
    let home =
        std::env::home_dir().ok_or_else(|| tr!("Home directory is unavailable").to_string())?;
    let path = home.join("Library/Application Support/Muxy");
    Ok(path.is_dir().then_some(path))
}

/// Stages the installed 1.x configuration for a profile the desktop app has
/// never opened, so upgrading keeps projects, workspaces, and layouts.
/// A profile chosen with `MUXY_DIR` is left alone. Returns what the user
/// should check, if anything could not come along.
pub(crate) fn migrate_installed(profile: &Path) -> Result<Option<String>> {
    if std::env::var_os("MUXY_DIR").is_some() {
        return Ok(None);
    }
    let Some(source) = installed_legacy()? else {
        return Ok(None);
    };
    let root = extensions::legacy_root();
    migrate_into_new_profile(
        profile,
        &source,
        root.as_deref()
            .map(|root| (root, &extensions::enabled_in_legacy as _)),
    )
}

fn migrate_into_new_profile(
    profile: &Path,
    source: &Path,
    extensions: LegacyExtensions<'_>,
) -> Result<Option<String>> {
    for name in ["desktop-state.json", "state.json", PENDING] {
        if profile.join(name).try_exists()? {
            return Ok(None);
        }
    }
    let import = prepare_from(profile, source, extensions)?;
    stage(profile, &import)?;
    Ok((!import.attention.is_empty()).then(|| {
        format!(
            "{}\n{}",
            tr!("Muxy 1.x was imported. Check these items:"),
            checklist(&import.attention)
        )
    }))
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
    archive::write(destination, &files, Restore::Complete)
}

pub(crate) fn prepare(profile: &Path, source: &Path) -> Result<PreparedImport> {
    prepare_from(profile, source, None)
}

fn prepare_from(
    profile: &Path,
    source: &Path,
    extensions: LegacyExtensions<'_>,
) -> Result<PreparedImport> {
    let (files, legacy, restore) = if source.is_dir() {
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
            Restore::Merge,
        )
    } else if source
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        (
            BTreeMap::from([("settings.json".into(), archive::read_file(source)?)]),
            true,
            Restore::Merge,
        )
    } else {
        archive::read(source)?
    };
    let (mut files, mut report) = if legacy {
        migrate(profile, &files)?
    } else {
        (files, ImportReport::default())
    };
    let packages = match extensions {
        Some(extensions) => import_extensions(profile, extensions, &mut files, &mut report)?,
        None => Vec::new(),
    };
    validate_effective(profile, &files, restore)?;
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
    let mut summary = if restore == Restore::Merge {
        tr!(
            "Import settings and %lld projects with their layouts on the next launch. Projects you already have keep their details and tabs, and workspaces with the same name are combined. A recovery copy is saved in the profile’s Backups folder.",
            count
        )
    } else {
        tr!(
            "Restore settings and %lld saved projects with their layouts on the next launch. Existing projects are kept; matching projects receive the saved layouts. A recovery copy is saved in the profile’s Backups folder. Restart the server from Settings to apply restored server settings.",
            count
        )
    }
    .to_string();
    if !packages.is_empty() {
        write!(
            summary,
            " {}",
            tr!("Adds %lld extensions from Muxy 1.x.", packages.len())
        )?;
    }
    if restore == Restore::Merge && files.contains_key("server.toml") {
        write!(
            summary,
            " {}",
            tr!("Restart the server from Settings to use the imported default shell.")
        )?;
    }
    if legacy {
        describe_legacy(&mut summary, &report)?;
    }
    Ok(PreparedImport {
        files,
        restore,
        summary,
        attention: report.attention,
        extensions: packages,
    })
}

/// Adds the 1.x packages this profile lacks and turns on the ones 1.x had on.
fn import_extensions(
    profile: &Path,
    (root, enabled): (&Path, &dyn Fn(&str) -> bool),
    files: &mut Files,
    report: &mut ImportReport,
) -> Result<Vec<PathBuf>> {
    let (packages, names) = extensions::find(profile, root, enabled, report)?;
    if !names.is_empty() {
        extensions::enable(profile, files, names)?;
    }
    if !packages.is_empty() {
        report.attention.push(format!(
            "Settings of extensions from 1.x start from their defaults: {}",
            packages
                .iter()
                .filter_map(|package| package.file_name()?.to_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(packages)
}

/// What came over, what to check, and the options Muxy 2 doesn't have.
fn describe_legacy(summary: &mut String, report: &ImportReport) -> Result<()> {
    write!(
        summary,
        "\n\n{}",
        tr!("Imported %lld supported 1.x items.", report.imported)
    )?;
    if !report.attention.is_empty() {
        write!(
            summary,
            "\n\n{}\n{}",
            tr!("Check after import:"),
            checklist(&report.attention)
        )?;
    }
    if !report.skipped.is_empty() {
        let mut names = report.skipped.iter().take(8).cloned().collect::<Vec<_>>();
        if report.skipped.len() > names.len() {
            names.push("…".into());
        }
        write!(
            summary,
            "\n\n{}",
            tr!(
                "Not available in Muxy 2 (%lld): %@",
                report.skipped.len(),
                names.join(", ")
            )
        )?;
    }
    Ok(())
}

fn checklist(items: &[String]) -> String {
    const SHOWN: usize = 20;
    let mut lines: Vec<_> = items
        .iter()
        .take(SHOWN)
        .map(|item| format!("• {item}"))
        .collect();
    if items.len() > SHOWN {
        lines.push(tr!("…and %lld more", items.len() - SHOWN).to_string());
    }
    lines.join("\n")
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
    report.attention.extend(settings_report.attention);
    if let Some(source) = legacy.get("ghostty.conf") {
        let ghostty = std::str::from_utf8(source)?;
        let server = profile.join("server.toml");
        let current = server
            .exists()
            .then(|| std::fs::read_to_string(&server))
            .transpose()?;
        if let Some(server) =
            muxy_app_core::backup::import_shell(ghostty, current.as_deref(), &mut report)?
        {
            files.insert("server.toml".into(), server.into_bytes());
        }
        let (source, skipped) = TerminalSettings::import_supported_source(ghostty);
        report.skipped.extend(
            skipped
                .into_iter()
                .filter(|key| key != "ghostty.conf: command"),
        );
        report.imported += source.lines().count();
        if !source.trim().is_empty() {
            files.insert(
                "ghostty.conf".into(),
                merge_terminal(profile, &source)?.into_bytes(),
            );
        }
    }
    if ["projects.json", "workspaces.json", "project-groups.json"]
        .iter()
        .any(|name| legacy.contains_key(*name))
    {
        let (state, projects) = muxy_app_core::backup::import_projects(
            legacy.get("projects.json").map_or(b"[]", Vec::as_slice),
            legacy.get("workspaces.json").map(Vec::as_slice),
            legacy.get("project-groups.json").map(Vec::as_slice),
            legacy,
            &mut settings,
        )?;
        report.imported += projects.imported;
        report.skipped.extend(projects.skipped);
        report.attention.extend(projects.attention);
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

fn validate_effective(profile: &Path, files: &Files, restore: Restore) -> Result<()> {
    if restore == Restore::Complete
        || files.contains_key("ghostty.conf")
        || !profile.join("ghostty.conf").exists()
    {
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
