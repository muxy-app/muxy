#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::environment::BuildMode;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const SCHEMA_VERSION: u32 = 1;
const STATE_FILE: &str = "swift-profile-migration.json";
const LOCK_FILE: &str = "swift-profile-migration.lock";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const FILES: [&str; 14] = [
    "projects.json",
    "recently-removed-projects.json",
    "project-groups.json",
    "workspaces.json",
    "settings.json",
    "ui-scale.json",
    "keybindings.json",
    "command-shortcuts.json",
    "editor-settings.json",
    "quick-terminal-shortcut.json",
    "approved-devices.json",
    "remote-devices.json",
    "browser-profiles.json",
    "ghostty.conf",
];
const DIRECTORIES: [&str; 2] = ["worktrees", "logos"];
const DEFAULT_STRING_KEYS: [&str; 6] = [
    "muxy.activeProjectID",
    "muxy.ide.selectedBundleIdentifier",
    "muxy.projectSortMode",
    "muxy.projectPicker.defaultDirectory",
    "muxy.activeProjectGroupID",
    "muxy.settings.selectedRoute",
];
const DEFAULT_BOOL_KEYS: [&str; 3] = [
    "muxy.sidebarExpanded",
    "muxy.browser.enabled",
    "muxy.projects.keepOpenWhenNoTabs",
];
const DEFAULT_NUMBER_KEYS: [&str; 2] = ["muxy.sidebarExpandedCustomWidth", "muxy.tabs.maxWidth"];
const DEFAULT_DICTIONARY_KEYS: [&str; 1] = ["muxy.activeWorktreeIDs"];
const TEST_SOURCE: &str = "MUXY_TEST_SWIFT_APPLICATION_SUPPORT_DIRECTORY";
const TEST_DEFAULTS: &str = "MUXY_TEST_SWIFT_DEFAULTS_PATH";
const TEST_FAILURE: &str = "MUXY_TEST_MIGRATION_FAIL_PATH";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationOutcome {
    Pending,
    Completed,
    SourceMissing,
    Abandoned,
}

impl MigrationOutcome {
    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::SourceMissing | Self::Abandoned
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MigrationFailure {
    pub path: String,
    pub category: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MigrationState {
    pub schema_version: u32,
    pub attempt_count: u8,
    pub outcome: MigrationOutcome,
    pub imported_paths: Vec<String>,
    pub existing_paths: Vec<String>,
    pub missing_paths: Vec<String>,
    pub failure: Option<MigrationFailure>,
    pub defaults_import_completed: bool,
}

impl Default for MigrationState {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            attempt_count: 0,
            outcome: MigrationOutcome::Pending,
            imported_paths: Vec::new(),
            existing_paths: Vec::new(),
            missing_paths: Vec::new(),
            failure: None,
            defaults_import_completed: false,
        }
    }
}

#[derive(Debug)]
pub struct MigrationError {
    message: String,
}

impl Display for MigrationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MigrationError {}

#[derive(Default)]
struct Report {
    imported: BTreeSet<String>,
    existing: BTreeSet<String>,
    missing: BTreeSet<String>,
}

struct MigrationOptions<'a> {
    root: &'a Path,
    source: &'a Path,
    failure_path: Option<&'a Path>,
}

pub fn state_path(root: &Path) -> PathBuf {
    root.join(STATE_FILE)
}

pub fn read_state(root: &Path) -> std::io::Result<Option<MigrationState>> {
    let contents = match fs::read(state_path(root)) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let state: MigrationState = serde_json::from_slice(&contents)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if state.schema_version != SCHEMA_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "unsupported Swift profile migration schema {}",
                state.schema_version
            ),
        ));
    }
    Ok(Some(state))
}

fn eligible(mode: BuildMode, macos: bool) -> bool {
    mode == BuildMode::Production && macos
}

pub fn run_startup() -> Result<(), MigrationError> {
    let mode = crate::build_mode!();
    let root = crate::prefs::app_support_dir();
    create_private_directory(&root).map_err(|error| MigrationError {
        message: format!(
            "failed to prepare Rust profile root {}: {error}",
            root.display()
        ),
    })?;
    if !eligible(mode, cfg!(target_os = "macos")) {
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let test_process = crate::prefs::is_test_process();
        let test_source = test_process
            .then(|| std::env::var_os(TEST_SOURCE))
            .flatten()
            .map(PathBuf::from);
        if test_process && test_source.is_none() {
            return Ok(());
        }
        let source = test_source.unwrap_or_else(|| {
            crate::environment::StoragePathPolicy::swift_source(crate::prefs::home_dir())
        });
        let failure_path = test_process
            .then(|| std::env::var_os(TEST_FAILURE))
            .flatten()
            .map(PathBuf::from);
        let options = MigrationOptions {
            root: &root,
            source: &source,
            failure_path: failure_path.as_deref(),
        };
        run_with(&options, || {
            if test_process {
                return read_test_defaults();
            }
            read_production_defaults()
        })
        .map(|_| ())
    }
}

fn run_with<F>(
    options: &MigrationOptions<'_>,
    read_defaults: F,
) -> Result<MigrationState, MigrationError>
where
    F: FnOnce() -> Result<Map<String, Value>, String>,
{
    create_private_directory(options.root).map_err(|error| MigrationError {
        message: format!(
            "failed to create Rust profile root {}: {error}",
            options.root.display()
        ),
    })?;
    let _lock = acquire_lock(options.root).map_err(|error| MigrationError {
        message: format!("failed to lock Swift profile migration: {error}"),
    })?;
    let loaded = read_state(options.root).map_err(|error| MigrationError {
        message: format!("failed to read Swift profile migration state: {error}"),
    })?;
    if let Some(state) = loaded.as_ref() {
        if state.outcome.terminal() {
            return Ok(state.clone());
        }
        if state.attempt_count >= 2 {
            let mut state = state.clone();
            state.outcome = MigrationOutcome::Abandoned;
            write_state(options.root, &state).map_err(|error| MigrationError {
                message: format!("failed to record interrupted migration abandonment: {error}"),
            })?;
            return Ok(state);
        }
    }
    let mut state = loaded.unwrap_or_default();
    state.attempt_count = state.attempt_count.saturating_add(1);
    state.failure = None;
    write_state(options.root, &state).map_err(|error| MigrationError {
        message: format!("failed to record Swift migration attempt: {error}"),
    })?;

    let source_metadata = match fs::symlink_metadata(options.source) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return finish_failure(
                options.root,
                state,
                Path::new("."),
                "unsafe_source",
                "Swift profile source is not a directory",
            );
        }
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            state.outcome = MigrationOutcome::SourceMissing;
            state.missing_paths = FILES
                .iter()
                .chain(DIRECTORIES.iter())
                .map(|path| (*path).to_owned())
                .collect();
            write_state(options.root, &state).map_err(|write_error| MigrationError {
                message: format!("failed to record missing Swift profile: {write_error}"),
            })?;
            return Ok(state);
        }
        Err(error) => {
            return finish_failure(
                options.root,
                state,
                Path::new("."),
                io_category(&error),
                &error.to_string(),
            );
        }
    };
    if !source_metadata.is_dir() {
        return finish_failure(
            options.root,
            state,
            Path::new("."),
            "unsafe_source",
            "Swift profile source is not a directory",
        );
    }

    let mut report = Report::default();
    let copy_result = (|| -> Result<(), CopyFailure> {
        for path in FILES {
            copy_entry(options, Path::new(path), false, &mut report)?;
        }
        for path in DIRECTORIES {
            copy_entry(options, Path::new(path), true, &mut report)?;
        }
        Ok(())
    })();
    if let Err(failure) = copy_result {
        merge_report(&mut state, report);
        return finish_failure(
            options.root,
            state,
            &failure.path,
            &failure.category,
            &failure.message,
        );
    }

    let imported_defaults = match read_defaults() {
        Ok(values) => filter_defaults(&values),
        Err(message) => {
            merge_report(&mut state, report);
            return finish_failure(
                options.root,
                state,
                Path::new("preferences.json"),
                "defaults_read",
                &message,
            );
        }
    };
    if let Err(error) = crate::prefs::defaults::merge_imported(options.root, &imported_defaults) {
        merge_report(&mut state, report);
        return finish_failure(
            options.root,
            state,
            Path::new("preferences.json"),
            io_category(&error),
            &error.to_string(),
        );
    }

    merge_report(&mut state, report);
    state.defaults_import_completed = true;
    state.outcome = MigrationOutcome::Completed;
    state.failure = None;
    write_state(options.root, &state).map_err(|error| MigrationError {
        message: format!("failed to complete Swift profile migration: {error}"),
    })?;
    Ok(state)
}

fn merge_report(state: &mut MigrationState, report: Report) {
    let mut imported: BTreeSet<String> = state.imported_paths.iter().cloned().collect();
    imported.extend(report.imported);
    state.imported_paths = imported.into_iter().collect();
    let mut existing: BTreeSet<String> = state.existing_paths.iter().cloned().collect();
    existing.extend(report.existing);
    state.existing_paths = existing.into_iter().collect();
    let mut missing: BTreeSet<String> = state.missing_paths.iter().cloned().collect();
    missing.extend(report.missing);
    state.missing_paths = missing.into_iter().collect();
}

fn finish_failure(
    root: &Path,
    mut state: MigrationState,
    path: &Path,
    category: &str,
    message: &str,
) -> Result<MigrationState, MigrationError> {
    state.failure = Some(MigrationFailure {
        path: display_path(path),
        category: category.to_owned(),
    });
    let abandoned = state.attempt_count >= 2;
    state.outcome = if abandoned {
        MigrationOutcome::Abandoned
    } else {
        MigrationOutcome::Pending
    };
    write_state(root, &state).map_err(|error| MigrationError {
        message: format!("failed to record Swift migration failure: {error}"),
    })?;
    if abandoned {
        Ok(state)
    } else {
        Err(MigrationError {
            message: format!(
                "Swift profile migration failed at {}: {message}; retry by launching Muxy again",
                display_path(path)
            ),
        })
    }
}

#[derive(Debug)]
struct CopyFailure {
    path: PathBuf,
    category: String,
    message: String,
}

fn copy_entry(
    options: &MigrationOptions<'_>,
    relative: &Path,
    expect_directory: bool,
    report: &mut Report,
) -> Result<(), CopyFailure> {
    let destination = options.root.join(relative);
    if let Ok(destination_metadata) = fs::symlink_metadata(&destination) {
        if expect_directory && destination_metadata.is_dir() {
            let source = options.source.join(relative);
            let source_metadata = match fs::symlink_metadata(&source) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    report.missing.insert(display_path(relative));
                    return Ok(());
                }
                Err(error) => return Err(copy_failure(relative, &error)),
            };
            if source_metadata.is_dir() && !source_metadata.file_type().is_symlink() {
                return copy_directory(options, relative, report);
            }
        }
        report.existing.insert(display_path(relative));
        return Ok(());
    }
    let source = options.source.join(relative);
    let metadata = match fs::symlink_metadata(&source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            report.missing.insert(display_path(relative));
            return Ok(());
        }
        Err(error) => return Err(copy_failure(relative, &error)),
    };
    if metadata.file_type().is_symlink()
        || (expect_directory && !metadata.is_dir())
        || (!expect_directory && !metadata.is_file())
    {
        report.missing.insert(display_path(relative));
        return Ok(());
    }
    if metadata.is_dir() {
        create_private_directory(&destination).map_err(|error| copy_failure(relative, &error))?;
        copy_directory(options, relative, report)
    } else {
        if copy_regular_file(options, relative)? {
            report.imported.insert(display_path(relative));
        } else {
            report.existing.insert(display_path(relative));
        }
        Ok(())
    }
}

fn copy_directory(
    options: &MigrationOptions<'_>,
    relative: &Path,
    report: &mut Report,
) -> Result<(), CopyFailure> {
    let source = options.source.join(relative);
    let entries = fs::read_dir(&source).map_err(|error| copy_failure(relative, &error))?;
    for entry in entries {
        let entry = entry.map_err(|error| copy_failure(relative, &error))?;
        let child = relative.join(entry.file_name());
        let destination = options.root.join(&child);
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|error| copy_failure(&child, &error))?;
        if let Ok(destination_metadata) = fs::symlink_metadata(&destination) {
            if destination_metadata.is_dir()
                && metadata.is_dir()
                && !metadata.file_type().is_symlink()
            {
                copy_directory(options, &child, report)?;
            } else {
                report.existing.insert(display_path(&child));
            }
            continue;
        }
        if metadata.file_type().is_symlink() {
            report.missing.insert(display_path(&child));
        } else if metadata.is_dir() {
            create_private_directory(&destination).map_err(|error| copy_failure(&child, &error))?;
            copy_directory(options, &child, report)?;
        } else if metadata.is_file() {
            if copy_regular_file(options, &child)? {
                report.imported.insert(display_path(&child));
            } else {
                report.existing.insert(display_path(&child));
            }
        } else {
            report.missing.insert(display_path(&child));
        }
    }
    Ok(())
}

fn copy_regular_file(options: &MigrationOptions<'_>, relative: &Path) -> Result<bool, CopyFailure> {
    let source = options.source.join(relative);
    let destination = options.root.join(relative);
    let parent = destination.parent().unwrap_or(options.root);
    create_private_directory(parent).map_err(|error| copy_failure(relative, &error))?;
    let (temporary, mut output) = loop {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{}.muxy-import-{}.{}.tmp",
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("file"),
            std::process::id(),
            sequence
        ));
        match open_new_private_file(&temporary, false) {
            Ok(output) => break (temporary, output),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(copy_failure(relative, &error)),
        }
    };
    let result = (|| -> std::io::Result<()> {
        let mut input = open_source_file(&source)?;
        if options.failure_path == Some(relative) {
            return Err(std::io::Error::other("injected migration copy failure"));
        }
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
        }
        output.sync_all()?;
        set_private_file_permissions(&temporary)
    })();
    drop(output);
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(copy_failure(relative, &error));
    }
    match fs::hard_link(&temporary, &destination) {
        Ok(()) => {
            fs::remove_file(&temporary).map_err(|error| copy_failure(relative, &error))?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&temporary);
            Ok(false)
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(copy_failure(relative, &error))
        }
    }
}

#[cfg(unix)]
fn open_source_file(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_source_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

#[cfg(unix)]
fn open_new_private_file(path: &Path, readable: bool) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(readable)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_new_private_file(path: &Path, readable: bool) -> std::io::Result<File> {
    OpenOptions::new()
        .read(readable)
        .write(true)
        .create_new(true)
        .open(path)
}

fn acquire_lock(root: &Path) -> std::io::Result<File> {
    let path = root.join(LOCK_FILE);
    let created = open_new_private_file(&path, true);
    let file = match created {
        Ok(file) => {
            set_private_file_permissions(&path)?;
            file
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "migration lock is not a regular file",
                ));
            }
            OpenOptions::new().read(true).write(true).open(&path)?
        }
        Err(error) => return Err(error),
    };
    file.try_lock()?;
    Ok(file)
}

fn write_state(root: &Path, state: &MigrationState) -> std::io::Result<()> {
    let contents = serde_json::to_vec_pretty(state)?;
    crate::store::write_private(&state_path(root), &contents)
}

fn create_private_directory(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => return Ok(()),
        Ok(_) => return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    create_missing_private_directories(path)
}

#[cfg(unix)]
fn create_missing_private_directories(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_missing_private_directories(path: &Path) -> std::io::Result<()> {
    fs::create_dir(path)
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn copy_failure(path: &Path, error: &std::io::Error) -> CopyFailure {
    CopyFailure {
        path: path.to_path_buf(),
        category: io_category(error).to_owned(),
        message: error.to_string(),
    }
}

fn io_category(error: &std::io::Error) -> &'static str {
    match error.kind() {
        std::io::ErrorKind::NotFound => "not_found",
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::AlreadyExists => "temporary_exists",
        std::io::ErrorKind::InvalidData => "invalid_data",
        _ => "io",
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read_test_defaults() -> Result<Map<String, Value>, String> {
    let Some(path) = std::env::var_os(TEST_DEFAULTS).map(PathBuf::from) else {
        return Ok(Map::new());
    };
    let contents = fs::read(path).map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_slice(&contents).map_err(|error| error.to_string())?;
    let values = value
        .as_object()
        .ok_or_else(|| "injected defaults must be a JSON object".to_owned())?;
    Ok(filter_defaults(values))
}

fn filter_defaults(values: &Map<String, Value>) -> Map<String, Value> {
    let mut filtered = Map::new();
    for key in DEFAULT_STRING_KEYS {
        if let Some(Value::String(value)) = values.get(key) {
            filtered.insert(key.to_owned(), Value::String(value.clone()));
        }
    }
    for key in DEFAULT_BOOL_KEYS {
        if let Some(Value::Bool(value)) = values.get(key) {
            filtered.insert(key.to_owned(), Value::Bool(*value));
        }
    }
    for key in DEFAULT_NUMBER_KEYS {
        if let Some(Value::Number(value)) = values.get(key) {
            filtered.insert(key.to_owned(), Value::Number(value.clone()));
        }
    }
    for key in DEFAULT_DICTIONARY_KEYS {
        if let Some(Value::Object(values)) = values.get(key) {
            let values: Map<String, Value> = values
                .iter()
                .filter_map(|(entry, value)| {
                    Some((entry.clone(), Value::String(value.as_str()?.to_owned())))
                })
                .collect();
            filtered.insert(key.to_owned(), Value::Object(values));
        }
    }
    filtered
}

#[cfg(target_os = "macos")]
fn nsnumber_is_boolean(value: &objc2_foundation::NSNumber) -> bool {
    let encoding = unsafe { std::ffi::CStr::from_ptr(value.objCType().as_ptr()) }.to_bytes();
    matches!(encoding, b"c" | b"B")
}

#[cfg(target_os = "macos")]
fn read_production_defaults() -> Result<Map<String, Value>, String> {
    use objc2_foundation::{NSDictionary, NSString, NSUserDefaults};

    let defaults = NSUserDefaults::standardUserDefaults();
    let domain_name = NSString::from_str("com.muxy.app");
    let Some(domain) = defaults.persistentDomainForName(&domain_name) else {
        return Ok(Map::new());
    };
    let mut values = Map::new();
    for key in DEFAULT_STRING_KEYS {
        let key_string = NSString::from_str(key);
        let Some(value) = domain.objectForKey(&key_string) else {
            continue;
        };
        if let Ok(value) = value.downcast::<NSString>() {
            values.insert(key.to_owned(), Value::String(value.to_string()));
        }
    }
    for key in DEFAULT_BOOL_KEYS {
        let key_string = NSString::from_str(key);
        let Some(value) = domain.objectForKey(&key_string) else {
            continue;
        };
        if let Ok(value) = value.downcast::<objc2_foundation::NSNumber>()
            && nsnumber_is_boolean(&value)
        {
            values.insert(key.to_owned(), Value::Bool(value.boolValue()));
        }
    }
    for key in DEFAULT_NUMBER_KEYS {
        let key_string = NSString::from_str(key);
        let Some(value) = domain.objectForKey(&key_string) else {
            continue;
        };
        if let Ok(value) = value.downcast::<objc2_foundation::NSNumber>()
            && !nsnumber_is_boolean(&value)
            && let Some(value) = serde_json::Number::from_f64(value.doubleValue())
        {
            values.insert(key.to_owned(), Value::Number(value));
        }
    }
    for key in DEFAULT_DICTIONARY_KEYS {
        let key_string = NSString::from_str(key);
        let Some(value) = domain.objectForKey(&key_string) else {
            continue;
        };
        let Ok(dictionary) = value.downcast::<NSDictionary>() else {
            continue;
        };
        let (keys, objects) = dictionary.to_vecs();
        let entries = keys
            .into_iter()
            .zip(objects)
            .filter_map(|(entry, value)| {
                Some((
                    entry.downcast::<NSString>().ok()?.to_string(),
                    Value::String(value.downcast::<NSString>().ok()?.to_string()),
                ))
            })
            .collect();
        values.insert(key.to_owned(), Value::Object(entries));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::{
        MigrationOptions, MigrationOutcome, eligible, filter_defaults, read_state, run_with,
        state_path, write_state,
    };
    use crate::environment::BuildMode;
    use serde_json::{Map, Value, json};
    use std::cell::Cell;
    use std::fs;
    use std::path::Path;

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write fixture");
    }

    fn run(
        root: &Path,
        source: &Path,
        defaults: Map<String, Value>,
        failure_path: Option<&Path>,
    ) -> Result<super::MigrationState, super::MigrationError> {
        run_with(
            &MigrationOptions {
                root,
                source,
                failure_path,
            },
            || Ok(defaults),
        )
    }

    #[test]
    fn migration_eligibility_requires_macos_release() {
        assert!(eligible(BuildMode::Production, true));
        assert!(!eligible(BuildMode::Development, true));
        assert!(!eligible(BuildMode::Production, false));
        assert!(!eligible(BuildMode::Development, false));
    }

    #[test]
    fn migration_imports_allowlisted_files_and_merges_directories() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        write(&source.join("projects.json"), "swift-projects");
        write(&source.join("worktrees/project/one.json"), "worktree");
        write(&source.join("logos/project/logo.png"), "logo");
        write(&source.join("sessions/session.json"), "runtime");
        write(&root.join("projects.json"), "rust-projects");
        write(&root.join("worktrees/project/existing.json"), "existing");
        let defaults = json!({
            "muxy.activeProjectID": "project",
            "muxy.sidebarExpanded": true,
            "unknown": "excluded"
        })
        .as_object()
        .expect("object")
        .clone();

        let state = run(&root, &source, defaults, None).expect("migration");
        assert_eq!(state.outcome, MigrationOutcome::Completed);
        assert_eq!(
            fs::read_to_string(root.join("projects.json")).unwrap(),
            "rust-projects"
        );
        assert_eq!(
            fs::read_to_string(root.join("worktrees/project/one.json")).unwrap(),
            "worktree"
        );
        assert_eq!(
            fs::read_to_string(root.join("logos/project/logo.png")).unwrap(),
            "logo"
        );
        assert!(!root.join("sessions").exists());
        let preferences: Value =
            serde_json::from_slice(&fs::read(root.join("preferences.json")).unwrap()).unwrap();
        assert_eq!(preferences["muxy.activeProjectID"], "project");
        assert_eq!(preferences["muxy.sidebarExpanded"], true);
        assert!(preferences.get("unknown").is_none());
        assert!(state.existing_paths.contains(&"projects.json".to_owned()));
        assert!(
            state
                .imported_paths
                .contains(&"worktrees/project/one.json".to_owned())
        );
        assert!(state.missing_paths.contains(&"settings.json".to_owned()));
    }

    #[test]
    #[cfg(unix)]
    fn migration_staging_is_private_from_creation() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("staging");
        let _file = super::open_new_private_file(&path, false).expect("staging");
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    #[cfg(unix)]
    fn migration_keeps_existing_directory_permissions_and_creates_private_directories() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        write(&source.join("worktrees/project/one.json"), "worktree");
        fs::create_dir_all(root.join("worktrees")).unwrap();
        fs::set_permissions(root.join("worktrees"), fs::Permissions::from_mode(0o755)).unwrap();
        run(&root, &source, Map::new(), None).expect("migration");
        assert_eq!(
            fs::metadata(root.join("worktrees"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            fs::metadata(root.join("worktrees/project"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    fn migration_leaves_source_bytes_unchanged() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        let file = source.join("projects.json");
        write(&file, "source bytes\n");
        let before = fs::read(&file).unwrap();
        run(&root, &source, Map::new(), None).expect("migration");
        assert_eq!(fs::read(&file).unwrap(), before);
    }

    #[test]
    fn migration_refuses_symlinks_without_following_them() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let directory = tempfile::tempdir().expect("temp dir");
            let source = directory.path().join("swift");
            let root = directory.path().join("rust");
            write(&directory.path().join("secret"), "secret");
            fs::create_dir_all(&source).unwrap();
            symlink(
                directory.path().join("secret"),
                source.join("projects.json"),
            )
            .unwrap();
            let state = run(&root, &source, Map::new(), None).expect("migration");
            assert_eq!(state.outcome, MigrationOutcome::Completed);
            assert!(!root.join("projects.json").exists());
            assert!(state.missing_paths.contains(&"projects.json".to_owned()));
        }
    }

    #[test]
    fn migration_copy_publication_never_replaces_an_existing_destination() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        write(&source.join("projects.json"), "swift");
        write(&root.join("projects.json"), "rust");
        let options = MigrationOptions {
            root: &root,
            source: &source,
            failure_path: None,
        };
        assert!(!super::copy_regular_file(&options, Path::new("projects.json")).expect("copy"));
        assert_eq!(
            fs::read_to_string(root.join("projects.json")).unwrap(),
            "rust"
        );
    }

    #[test]
    fn migration_cleans_temporary_files_after_copy_failure() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        write(&source.join("projects.json"), "projects");
        fs::create_dir(&root).expect("root");
        let unowned = root.join(format!(
            ".projects.json.muxy-import-{}.tmp",
            std::process::id()
        ));
        fs::write(&unowned, b"unowned").expect("unowned temporary");
        let result = run(&root, &source, Map::new(), Some(Path::new("projects.json")));
        assert!(result.is_err());
        assert!(!root.join("projects.json").exists());
        assert_eq!(fs::read(&unowned).expect("unowned preserved"), b"unowned");
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.to_string_lossy().contains("muxy-import"))
            .collect();
        assert_eq!(leftovers, vec![unowned]);
    }

    #[test]
    fn migration_marks_an_absent_source_terminal_without_retrying() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("missing");
        let root = directory.path().join("rust");
        let state = run(&root, &source, Map::new(), None).expect("migration");
        assert_eq!(state.outcome, MigrationOutcome::SourceMissing);
        assert_eq!(state.attempt_count, 1);
        fs::create_dir_all(&source).unwrap();
        write(&source.join("projects.json"), "late");
        let state = run(&root, &source, Map::new(), None).expect("terminal");
        assert_eq!(state.outcome, MigrationOutcome::SourceMissing);
        assert!(!root.join("projects.json").exists());
    }

    #[test]
    fn migration_refuses_a_concurrent_run_before_source_inspection() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("missing-swift");
        let root = directory.path().join("rust");
        fs::create_dir(&root).expect("root");
        let _lock = super::acquire_lock(&root).expect("first lock");
        let error = run(&root, &source, Map::new(), None).expect_err("concurrent run");
        assert!(
            error
                .to_string()
                .contains("failed to lock Swift profile migration")
        );
        assert!(!state_path(&root).exists());
    }

    #[test]
    fn migration_fails_closed_on_a_malformed_state_file() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("missing-swift");
        let root = directory.path().join("rust");
        fs::create_dir(&root).expect("root");
        fs::write(state_path(&root), b"not json").expect("malformed state");
        let error = run(&root, &source, Map::new(), None).expect_err("malformed state");
        assert!(
            error
                .to_string()
                .contains("failed to read Swift profile migration state")
        );
        assert_eq!(fs::read(state_path(&root)).expect("unchanged"), b"not json");
    }

    #[test]
    fn migration_retries_once_then_abandons_and_stops_inspecting_source() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        write(&source.join("projects.json"), "projects");
        write(
            &source.join("recently-removed-projects.json"),
            "recently removed",
        );
        let failure = Some(Path::new("recently-removed-projects.json"));
        assert!(run(&root, &source, Map::new(), failure).is_err());
        assert_eq!(
            fs::read_to_string(root.join("projects.json")).unwrap(),
            "projects"
        );
        let pending = read_state(&root)
            .expect("read pending state")
            .expect("pending state");
        assert_eq!(pending.outcome, MigrationOutcome::Pending);
        assert_eq!(pending.attempt_count, 1);
        let abandoned = run(&root, &source, Map::new(), failure).expect("abandoned");
        assert_eq!(abandoned.outcome, MigrationOutcome::Abandoned);
        assert_eq!(abandoned.attempt_count, 2);
        assert_eq!(
            fs::read_to_string(root.join("projects.json")).unwrap(),
            "projects"
        );
        fs::remove_dir_all(&source).unwrap();
        let terminal = run(&root, &source, Map::new(), None).expect("terminal");
        assert_eq!(terminal.outcome, MigrationOutcome::Abandoned);
        assert_eq!(terminal.attempt_count, 2);
    }

    #[test]
    fn migration_abandons_an_interrupted_second_attempt_without_source_inspection() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        write(&source.join("projects.json"), "projects");
        assert!(run(&root, &source, Map::new(), Some(Path::new("projects.json")),).is_err());
        let mut state = read_state(&root)
            .expect("read pending state")
            .expect("pending state");
        state.attempt_count = 2;
        write_state(&root, &state).expect("interrupted state");
        fs::remove_dir_all(&source).unwrap();
        let state = run(&root, &source, Map::new(), None).expect("abandoned");
        assert_eq!(state.outcome, MigrationOutcome::Abandoned);
        assert_eq!(state.attempt_count, 2);
    }

    #[test]
    fn migration_terminal_state_never_calls_defaults_reader() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        fs::create_dir_all(&source).unwrap();
        run(&root, &source, Map::new(), None).expect("migration");
        fs::remove_dir_all(&source).unwrap();
        let called = Cell::new(false);
        run_with(
            &MigrationOptions {
                root: &root,
                source: &source,
                failure_path: None,
            },
            || {
                called.set(true);
                Ok(Map::new())
            },
        )
        .expect("terminal");
        assert!(!called.get());
    }

    #[test]
    fn migration_allowlist_fixture_covers_every_file_and_directory() {
        const EXPECTED_FILES: [&str; 14] = [
            "projects.json",
            "recently-removed-projects.json",
            "project-groups.json",
            "workspaces.json",
            "settings.json",
            "ui-scale.json",
            "keybindings.json",
            "command-shortcuts.json",
            "editor-settings.json",
            "quick-terminal-shortcut.json",
            "approved-devices.json",
            "remote-devices.json",
            "browser-profiles.json",
            "ghostty.conf",
        ];
        const EXPECTED_DIRECTORIES: [&str; 2] = ["worktrees", "logos"];
        assert_eq!(super::FILES, EXPECTED_FILES);
        assert_eq!(super::DIRECTORIES, EXPECTED_DIRECTORIES);

        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        for relative in EXPECTED_FILES {
            write(&source.join(relative), &format!("source:{relative}"));
        }
        for relative in EXPECTED_DIRECTORIES {
            write(
                &source.join(relative).join("nested/value"),
                &format!("source:{relative}"),
            );
        }

        let existing_root = directory.path().join("existing-rust");
        for relative in EXPECTED_FILES {
            write(&existing_root.join(relative), &format!("rust:{relative}"));
        }
        for relative in EXPECTED_DIRECTORIES {
            write(
                &existing_root.join(relative).join("nested/value"),
                &format!("rust:{relative}"),
            );
        }
        let existing = run(&existing_root, &source, Map::new(), None).expect("existing import");
        for relative in EXPECTED_FILES {
            assert_eq!(
                fs::read_to_string(existing_root.join(relative)).expect("existing file"),
                format!("rust:{relative}")
            );
            assert!(existing.existing_paths.contains(&relative.to_owned()));
        }
        for relative in EXPECTED_DIRECTORIES {
            assert_eq!(
                fs::read_to_string(existing_root.join(relative).join("nested/value"))
                    .expect("existing directory file"),
                format!("rust:{relative}")
            );
        }

        let imported_root = directory.path().join("imported-rust");
        let imported = run(&imported_root, &source, Map::new(), None).expect("fresh import");
        for relative in EXPECTED_FILES {
            assert_eq!(
                fs::read_to_string(imported_root.join(relative)).expect("imported file"),
                format!("source:{relative}")
            );
            assert!(imported.imported_paths.contains(&relative.to_owned()));
        }
        for relative in EXPECTED_DIRECTORIES {
            let nested = format!("{relative}/nested/value");
            assert_eq!(
                fs::read_to_string(imported_root.join(&nested)).expect("imported directory file"),
                format!("source:{relative}")
            );
            assert!(imported.imported_paths.contains(&nested));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn production_defaults_distinguish_booleans_from_numbers() {
        use objc2_foundation::NSNumber;

        assert!(super::nsnumber_is_boolean(&NSNumber::new_bool(true)));
        assert!(!super::nsnumber_is_boolean(&NSNumber::new_i64(1)));
        assert!(!super::nsnumber_is_boolean(&NSNumber::new_f64(1.0)));
    }

    #[test]
    fn migration_defaults_filter_is_exact_and_typed() {
        let source = json!({
            "muxy.activeProjectID": "project",
            "muxy.sidebarExpanded": true,
            "muxy.tabs.maxWidth": 240.0,
            "muxy.activeWorktreeIDs": { "project": "worktree", "invalid": false },
            "muxy.settings.selectedRoute": "builtin.appearance",
            "muxy.unapproved": "excluded",
            "muxy.browser.enabled": "wrong type"
        });
        let filtered = filter_defaults(source.as_object().expect("object"));
        assert_eq!(filtered.len(), 5);
        assert!(filtered.get("muxy.unapproved").is_none());
        assert!(filtered.get("muxy.browser.enabled").is_none());
        assert_eq!(
            filtered["muxy.activeWorktreeIDs"],
            json!({ "project": "worktree" })
        );
    }

    #[test]
    fn migration_defaults_existing_destination_values_win() {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = directory.path().join("swift");
        let root = directory.path().join("rust");
        fs::create_dir_all(&source).unwrap();
        write(
            &root.join("preferences.json"),
            r#"{"muxy.activeProjectID":"rust"}"#,
        );
        let defaults = json!({ "muxy.activeProjectID": "swift", "muxy.sidebarExpanded": true })
            .as_object()
            .unwrap()
            .clone();
        run(&root, &source, defaults, None).expect("migration");
        let preferences: Value =
            serde_json::from_slice(&fs::read(root.join("preferences.json")).unwrap()).unwrap();
        assert_eq!(preferences["muxy.activeProjectID"], "rust");
        assert_eq!(preferences["muxy.sidebarExpanded"], true);
    }
}
