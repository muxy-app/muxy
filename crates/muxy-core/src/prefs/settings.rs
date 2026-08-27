use super::defaults;
use crate::environment::{MobileSettingsKeys, MobileSettingsPolicy};
use serde_json::{Map, Number, Value};
use std::cell::Cell;
use std::path::{Path, PathBuf};

const COMMIT_PROMPT: &str = "Write a concise commit message that explains the intent of all staged changes. Follow the repository's existing commit-message style.";
const PULL_REQUEST_PROMPT: &str = "Write an accurate pull request title and a concise summary of the changes. Choose a short descriptive branch name and the appropriate target branch.";

pub const MOBILE_POLICY: MobileSettingsPolicy = MobileSettingsPolicy::new(crate::build_mode!());
pub const MOBILE_KEYS: MobileSettingsKeys = MOBILE_POLICY.keys();

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Source {
    Defaults(Kind),
    UiScale,
    ThemeDark,
    ThemeLight,
    EditorSetting(&'static str, Kind),
    ShortcutsApp,
    QuickTerminalShortcut,
    CustomCommands,
    AiProviders,
    ApprovedDevices,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub key: &'static str,
    pub source: Source,
}

const fn flag(key: &'static str, value: bool) -> Entry {
    Entry {
        key,
        source: Source::Defaults(Kind::Bool(value)),
    }
}

const fn text(key: &'static str, value: &'static str) -> Entry {
    Entry {
        key,
        source: Source::Defaults(Kind::Str(value)),
    }
}

const fn integer(key: &'static str, value: i64) -> Entry {
    Entry {
        key,
        source: Source::Defaults(Kind::Int(value)),
    }
}

const fn double(key: &'static str, value: f64) -> Entry {
    Entry {
        key,
        source: Source::Defaults(Kind::Double(value)),
    }
}

const fn special(key: &'static str, source: Source) -> Entry {
    Entry { key, source }
}

pub const fn mirror(mode: crate::environment::BuildMode) -> [Entry; 69] {
    let mobile_policy = MobileSettingsPolicy::new(mode);
    let mobile_keys = mobile_policy.keys();
    [
        flag("diagnostics.profiler.enabled", false),
        flag("muxy.general.autoExpandWorktreesOnProjectSwitch", false),
        flag("muxy.showHomeProject", true),
        flag("muxy.tips.visible", true),
        flag("muxy.showProjectSearch", false),
        flag("muxy.worktrees.groupWorktrees", false),
        flag("muxy.worktrees.showUnreadIndicator", true),
        flag("muxy.worktrees.orderByMRU", true),
        flag("muxy.projects.keepOpenWhenNoTabs", false),
        flag("muxy.general.autoCopyTerminalSelection", false),
        flag("muxy.tabs.confirmCloseRunningProcess", true),
        flag("muxy.app.confirmQuit", true),
        flag("SUAutomaticallyUpdate", true),
        flag("muxy.showTopBarActions", true),
        flag("muxy.showStatusBar", true),
        flag("muxy.showResourceUsageInStatusBar", true),
        flag("muxy.richInput.clearAfterSending", false),
        flag("muxy.richInput.clearOnClose", false),
        flag("muxy.terminalOffline.enabled", false),
        flag("muxy.terminalPersistentSession.enabled", false),
        flag("muxy.quickTerminal.enabled", true),
        flag("muxy.recording.autoSend", false),
        flag("muxy.notifications.toastEnabled", true),
        flag("muxy.notifications.desktopEnabled", false),
        flag(
            mobile_keys.enabled,
            mobile_policy.settings_enabled_default(),
        ),
        text("muxy.update.channel", "stable"),
        text("muxy.localization", ""),
        text("muxy.activeSidebar", ""),
        text("muxy.projectPicker.mode", "custom"),
        text("muxy.projectPicker.defaultDirectory", ""),
        text("muxy.projectSortMode", "manual"),
        text("muxy.defaultFileOpener", ""),
        text("muxy.general.defaultWorktreePathTemplate", ""),
        text("muxy.general.defaultWorktreeParentPath", ""),
        text("muxy.sentry.consent", ""),
        text("muxy.browser.searchEngine", "google"),
        text("muxy.browser.homePageURL", "about:blank"),
        text("muxy.appBackgroundStyle", "vibrant"),
        text("muxy.sidebarCollapsedStyle", "icons"),
        text("muxy.sidebarExpandedStyle", "wide"),
        text("muxy.richInput.presentationMode", "panel"),
        text("muxy.ai.repositoryActions.commit.provider", ""),
        text("muxy.ai.repositoryActions.commit.prompt", COMMIT_PROMPT),
        text("muxy.ai.repositoryActions.createPullRequest.provider", ""),
        text(
            "muxy.ai.repositoryActions.createPullRequest.prompt",
            PULL_REQUEST_PROMPT,
        ),
        text("muxy.recording.language", ""),
        text("muxy.notifications.sound", "Funk"),
        text("muxy.notifications.toastPosition", "Top Center"),
        integer("muxy.app.transparency", 0),
        integer("muxy.app.blur", 70),
        integer("muxy.quickTerminal.width", 720),
        integer("muxy.quickTerminal.height", 430),
        integer("muxy.quickTerminal.transparency", 18),
        integer("muxy.quickTerminal.blur", 70),
        integer(mobile_keys.port, mobile_policy.default_port() as i64),
        integer(
            mobile_keys.scrollback_cap,
            mobile_policy.default_scrollback_cap(),
        ),
        double("muxy.tabs.maxWidth", 200.0),
        double("muxy.terminalOffline.idleThresholdSeconds", 300.0),
        special("muxy.ui.scale", Source::UiScale),
        special("muxy.theme.dark", Source::ThemeDark),
        special("muxy.theme.light", Source::ThemeLight),
        special(
            "editor.richInputImageStrategy",
            Source::EditorSetting("richInputImageStrategy", Kind::Str("clipboard")),
        ),
        special(
            "editor.richInputFontFamily",
            Source::EditorSetting("richInputFontFamily", Kind::Str("SF Mono")),
        ),
        special(
            "editor.richInputLineHeightMultiplier",
            Source::EditorSetting("richInputLineHeightMultiplier", Kind::Double(1.2)),
        ),
        special("shortcuts.app", Source::ShortcutsApp),
        special("shortcuts.quickTerminal", Source::QuickTerminalShortcut),
        special("shortcuts.customCommands", Source::CustomCommands),
        special("ai.providers", Source::AiProviders),
        special("mobile.approvedDevices", Source::ApprovedDevices),
    ]
}

pub const MIRROR: [Entry; 69] = mirror(crate::build_mode!());

pub const NOTIFICATION_SOUNDS: [&str; 15] = [
    "None",
    "Basso",
    "Blow",
    "Bottle",
    "Frog",
    "Funk",
    "Glass",
    "Hero",
    "Morse",
    "Ping",
    "Pop",
    "Purr",
    "Sosumi",
    "Submarine",
    "Tink",
];

pub const TOAST_POSITIONS: [&str; 4] = ["Top Center", "Top Right", "Bottom Center", "Bottom Right"];

pub const AI_PROVIDERS: [(&str, &str); 10] = [
    ("claude", "Claude Code"),
    ("opencode", "OpenCode"),
    ("codex", "Codex"),
    ("cursor", "Cursor CLI"),
    ("copilot", "GitHub Copilot"),
    ("droid", "Droid"),
    ("pi", "Pi"),
    ("grok", "Grok"),
    ("kiro", "Kiro CLI"),
    ("xal", "Xal"),
];

pub fn provider_key(id: &str) -> String {
    format!("muxy.notifications.provider.{id}.enabled")
}

const ALLOWED_STRINGS: [(&str, &[&str]); 11] = [
    ("muxy.update.channel", &["stable", "beta"]),
    ("muxy.projectPicker.mode", &["custom", "finder"]),
    ("muxy.sentry.consent", &["", "allowed", "denied"]),
    ("muxy.ui.scale", &["regular", "large", "extraLarge", "huge"]),
    ("muxy.appBackgroundStyle", &["vibrant", "solid"]),
    ("muxy.sidebarCollapsedStyle", &["hidden", "icons"]),
    ("muxy.sidebarExpandedStyle", &["icons", "wide"]),
    ("muxy.richInput.presentationMode", &["panel", "floating"]),
    (
        "editor.richInputImageStrategy",
        &["clipboard", "inlinePath"],
    ),
    ("muxy.notifications.sound", &NOTIFICATION_SOUNDS),
    ("muxy.notifications.toastPosition", &TOAST_POSITIONS),
];

const fn int_ranges(mode: crate::environment::BuildMode) -> [(&'static str, i64, i64); 8] {
    let policy = MobileSettingsPolicy::new(mode);
    let keys = policy.keys();
    [
        (
            keys.port,
            policy.minimum_port() as i64,
            policy.maximum_port() as i64,
        ),
        (
            keys.scrollback_cap,
            policy.minimum_scrollback_cap(),
            policy.maximum_scrollback_cap(),
        ),
        ("muxy.quickTerminal.width", 480, 1200),
        ("muxy.quickTerminal.height", 280, 800),
        ("muxy.quickTerminal.transparency", 0, 55),
        ("muxy.quickTerminal.blur", 0, 100),
        ("muxy.app.transparency", 0, 55),
        ("muxy.app.blur", 0, 100),
    ]
}

#[derive(Debug, PartialEq, Eq)]
pub enum SettingsError {
    TopLevelObjectRequired,
    InvalidValue(String),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TopLevelObjectRequired => write!(formatter, "Settings JSON must be an object."),
            Self::InvalidValue(key) => write!(formatter, "Invalid JSON value for \"{key}\"."),
        }
    }
}

thread_local! {
    static SUPPRESS_SYNC: Cell<bool> = const { Cell::new(false) };
}

fn path() -> PathBuf {
    super::app_support_dir().join("settings.json")
}

fn ui_scale_path() -> PathBuf {
    super::app_support_dir().join("ui-scale.json")
}

fn read_defaults(key: &str, kind: Kind, existing: Option<&Value>) -> Value {
    match kind {
        Kind::Bool(fallback) => Value::Bool(
            defaults::read_bool(key)
                .or_else(|| existing.and_then(Value::as_bool))
                .unwrap_or(fallback),
        ),
        Kind::Int(fallback) => Value::Number(Number::from(
            defaults::read_i64(key)
                .or_else(|| existing.and_then(Value::as_i64))
                .unwrap_or(fallback),
        )),
        Kind::Double(fallback) => Number::from_f64(
            defaults::read_f64(key)
                .or_else(|| existing.and_then(Value::as_f64))
                .unwrap_or(fallback),
        )
        .map_or(Value::Null, Value::Number),
        Kind::Str(fallback) => Value::String(
            defaults::read_string(key)
                .or_else(|| existing.and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_else(|| fallback.to_owned()),
        ),
    }
}

fn read_entry(entry: &Entry, existing: Option<&Value>) -> Option<Value> {
    match entry.source {
        Source::Defaults(kind) => Some(read_defaults(entry.key, kind, existing)),
        Source::UiScale => Some(Value::String(read_ui_scale())),
        Source::ThemeDark => Some(Value::String(
            crate::store::ghostty_conf::theme_selection()
                .0
                .unwrap_or_else(|| "Muxy".to_owned()),
        )),
        Source::ThemeLight => Some(Value::String(
            crate::store::ghostty_conf::theme_selection()
                .1
                .unwrap_or_else(|| "Muxy".to_owned()),
        )),
        Source::EditorSetting(name, kind) => read_editor_setting(name, kind),
        Source::ShortcutsApp => Some(read_shortcuts_app(existing)),
        Source::CustomCommands => Some(read_custom_commands()),
        Source::QuickTerminalShortcut => Some(read_quick_terminal_shortcut()),
        Source::AiProviders => Some(read_ai_providers(existing)),
        Source::ApprovedDevices => super::read_json(&approved_devices_path()),
    }
}

pub fn quick_terminal_shortcut_path() -> PathBuf {
    super::app_support_dir().join("quick-terminal-shortcut.json")
}

fn approved_devices_path() -> PathBuf {
    super::app_support_dir().join("approved-devices.json")
}

pub fn quick_terminal_kind() -> String {
    read_quick_terminal_shortcut()
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unassigned")
        .to_owned()
}

fn read_quick_terminal_shortcut() -> Value {
    super::read_json(&quick_terminal_shortcut_path())
        .unwrap_or_else(|| serde_json::json!({ "type": "unassigned" }))
}

pub fn set_quick_terminal_shortcut(kind: &str) {
    let mut root = Map::new();
    root.insert("type".to_owned(), Value::String(kind.to_owned()));
    let contents = to_foundation_json(&Value::Object(root), true, false);
    let path = quick_terminal_shortcut_path();
    if let Err(error) = crate::store::write_private(&path, contents.as_bytes()) {
        log::warn!("failed to write {}: {error}", path.display());
        return;
    }
    sync();
}

fn read_ai_providers(existing: Option<&Value>) -> Value {
    let object: Map<String, Value> = AI_PROVIDERS
        .iter()
        .map(|(id, _)| {
            (
                (*id).to_owned(),
                Value::Bool(bool_value(&provider_key(id), true)),
            )
        })
        .collect();
    Value::Object(carry_through(object, existing))
}

fn read_shortcuts_app(existing: Option<&Value>) -> Value {
    Value::Object(carry_through(
        crate::shortcuts::ShortcutMap::load().mirror_object(),
        existing,
    ))
}

fn carry_through(mut object: Map<String, Value>, existing: Option<&Value>) -> Map<String, Value> {
    if let Some(Value::Object(previous)) = existing {
        for (key, value) in previous {
            object.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    object
}

fn read_custom_commands() -> Value {
    crate::store::CommandShortcuts::load().mirror_value()
}

fn editor_settings_path() -> PathBuf {
    super::app_support_dir().join("editor-settings.json")
}

fn read_editor_setting(name: &str, kind: Kind) -> Option<Value> {
    let stored = super::read_json(&editor_settings_path())
        .and_then(|root| root.as_object().and_then(|map| map.get(name)).cloned());
    Some(stored.unwrap_or_else(|| match kind {
        Kind::Bool(value) => Value::Bool(value),
        Kind::Int(value) => Value::Number(Number::from(value)),
        Kind::Double(value) => Number::from_f64(value).map_or(Value::Null, Value::Number),
        Kind::Str(value) => Value::String(value.to_owned()),
    }))
}

pub fn editor_setting(name: &str, default: Value) -> Value {
    super::read_json(&editor_settings_path())
        .as_ref()
        .and_then(|root| root.get(name))
        .cloned()
        .unwrap_or(default)
}

pub fn set_editor_setting(name: &str, value: Value) {
    let path = editor_settings_path();
    let mut root = match super::read_json(&path) {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    root.insert(name.to_owned(), value);
    let contents = to_foundation_json(&Value::Object(root), true, false);
    if let Err(error) = crate::store::write_private(&path, contents.as_bytes()) {
        log::warn!("failed to write {}: {error}", path.display());
        return;
    }
    sync();
}

fn read_ui_scale() -> String {
    super::read_json(&ui_scale_path())
        .as_ref()
        .and_then(|root| root.get("preset"))
        .and_then(Value::as_str)
        .unwrap_or("regular")
        .to_owned()
}

fn stored_setting(key: &str) -> Option<Value> {
    super::read_json(&path())
        .as_ref()
        .and_then(|root| root.get(key))
        .cloned()
}

pub fn string_value(key: &str, default: &str) -> String {
    special_string(key)
        .or_else(|| defaults::read_string(key))
        .or_else(|| stored_setting(key).and_then(|value| value.as_str().map(str::to_owned)))
        .unwrap_or_else(|| default.to_owned())
}

fn special_string(key: &str) -> Option<String> {
    let entry = MIRROR.iter().find(|entry| entry.key == key)?;
    if matches!(entry.source, Source::Defaults(_)) {
        return None;
    }
    match read_entry(entry, None)? {
        Value::String(value) => Some(value),
        _ => None,
    }
}

pub fn bool_value(key: &str, default: bool) -> bool {
    defaults::read_bool(key)
        .or_else(|| stored_setting(key).and_then(|value| value.as_bool()))
        .unwrap_or(default)
}

pub fn i64_value(key: &str, default: i64) -> i64 {
    defaults::read_i64(key)
        .or_else(|| stored_setting(key).and_then(|value| value.as_i64()))
        .unwrap_or(default)
}

pub fn f64_value(key: &str, default: f64) -> f64 {
    defaults::read_f64(key)
        .or_else(|| stored_setting(key).and_then(|value| value.as_f64()))
        .unwrap_or(default)
}

pub fn set_ui_scale(preset: crate::prefs::ScalePreset) {
    let mut root = Map::new();
    root.insert("preset".to_owned(), Value::String(preset.raw().to_owned()));
    let contents = to_foundation_json(&Value::Object(root), true, false);
    let path = ui_scale_path();
    if let Err(error) = crate::store::write_private(&path, contents.as_bytes()) {
        log::warn!("failed to write {}: {error}", path.display());
        return;
    }
    sync();
}

pub fn set(key: &str, value: Value) {
    match &value {
        Value::Bool(value) => defaults::store_bool(key, *value),
        Value::String(value) => defaults::store_string(key, Some(value)),
        Value::Number(number) => match number.as_i64() {
            Some(value) => defaults::store_i64(key, value),
            None => {
                if let Some(value) = number.as_f64() {
                    defaults::store_f64(key, value);
                }
            }
        },
        Value::Null => defaults::remove(key),
        _ => return,
    }
    sync();
}

pub fn sync() {
    if SUPPRESS_SYNC.get() {
        return;
    }
    sync_at(&path(), crate::build_mode!());
}

fn sync_at(path: &Path, mode: crate::environment::BuildMode) -> bool {
    sync_at_with(path, mode, read_entry)
}

fn sync_at_with(
    path: &Path,
    mode: crate::environment::BuildMode,
    mut resolve: impl FnMut(&Entry, Option<&Value>) -> Option<Value>,
) -> bool {
    let existing = std::fs::read_to_string(path).ok();
    let mut root = match &existing {
        Some(contents) => match serde_json::from_str::<Value>(contents) {
            Ok(Value::Object(map)) => map,
            _ => {
                log::warn!(
                    "settings.json does not parse as an object; leaving it untouched: {}",
                    path.display()
                );
                return false;
            }
        },
        None => Map::new(),
    };

    let mut changed = false;
    for entry in &mirror(mode) {
        let Some(value) = resolve(entry, root.get(entry.key)) else {
            continue;
        };
        if !root.get(entry.key).is_some_and(|held| equal(held, &value)) {
            changed = true;
        }
        root.insert(entry.key.to_owned(), value);
    }

    if !changed {
        return false;
    }

    let contents = to_foundation_json(&Value::Object(root), false, true);
    if let Err(error) = crate::store::write_private(path, contents.as_bytes()) {
        log::warn!("failed to write {}: {error}", path.display());
        return false;
    }
    true
}

fn equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => match (left.as_f64(), right.as_f64()) {
            (Some(left), Some(right)) => left == right,
            _ => left == right,
        },
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| equal(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, value)| right.get(key).is_some_and(|held| equal(value, held)))
        }
        _ => left == right,
    }
}

pub fn user_path() -> PathBuf {
    path()
}

pub fn system_defaults_text() -> String {
    system_defaults_text_for(crate::build_mode!())
}

fn system_defaults_text_for(mode: crate::environment::BuildMode) -> String {
    let root: Map<String, Value> = mirror(mode)
        .iter()
        .map(|entry| (entry.key.to_owned(), default_entry_value(entry)))
        .collect();
    to_foundation_json(&Value::Object(root), false, true)
}

fn default_entry_value(entry: &Entry) -> Value {
    match entry.source {
        Source::Defaults(kind) => default_value(kind),
        Source::UiScale => Value::String("regular".to_owned()),
        Source::ThemeDark => Value::String("Muxy".to_owned()),
        Source::ThemeLight => Value::String("Muxy".to_owned()),
        Source::EditorSetting(_, kind) => default_value(kind),
        Source::ShortcutsApp => Value::Object(default_shortcuts_app()),
        Source::QuickTerminalShortcut => serde_json::json!({ "type": "unassigned" }),
        Source::CustomCommands => crate::store::CommandShortcuts::default().mirror_value(),
        Source::AiProviders => Value::Object(
            AI_PROVIDERS
                .iter()
                .map(|(id, _)| ((*id).to_owned(), Value::Bool(true)))
                .collect(),
        ),
        Source::ApprovedDevices => Value::Array(Vec::new()),
    }
}

fn default_value(kind: Kind) -> Value {
    match kind {
        Kind::Bool(value) => Value::Bool(value),
        Kind::Int(value) => Value::Number(Number::from(value)),
        Kind::Double(value) => Number::from_f64(value).map_or(Value::Null, Value::Number),
        Kind::Str(value) => Value::String(value.to_owned()),
    }
}

fn default_shortcuts_app() -> Map<String, Value> {
    let mut object = Map::new();
    for (action, combo) in crate::shortcuts::default_bindings() {
        let Ok(Value::String(name)) = serde_json::to_value(action) else {
            continue;
        };
        let Ok(combo) = serde_json::to_value(&combo) else {
            continue;
        };
        object.insert(name, combo);
    }
    for (name, key, modifiers) in crate::shortcuts::UNMODELLED_DEFAULTS {
        object.insert(
            name.to_owned(),
            serde_json::json!({ "key": key, "modifiers": modifiers }),
        );
    }
    object
}

pub fn load_user_text() -> String {
    let path = path();
    if !path.exists() {
        reset_user_file();
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".to_owned());
    prettify(&text).unwrap_or(text)
}

pub fn prettify(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    value
        .is_object()
        .then(|| to_foundation_json(&value, false, true))
}

pub fn reset_user_file() {
    let path = path();
    if let Err(error) =
        reset_user_file_at(&path, crate::build_mode!(), |entry| read_entry(entry, None))
    {
        log::warn!("failed to write {}: {error}", path.display());
    }
}

fn reset_user_file_at(
    path: &Path,
    mode: crate::environment::BuildMode,
    mut resolve: impl FnMut(&Entry) -> Option<Value>,
) -> std::io::Result<()> {
    let existing = read_object(path);
    let mut root = Map::new();
    for entry in &mirror(mode) {
        let Some(value) = resolve(entry) else {
            continue;
        };
        root.insert(entry.key.to_owned(), value);
    }
    preserve_inactive_mobile_values(mode, existing.as_ref(), &mut root);
    write_settings_document(path, root)
}

pub fn save_user_text(text: &str) -> Result<(), SettingsError> {
    SUPPRESS_SYNC.set(true);
    let result = save_user_text_at(&path(), crate::build_mode!(), text, apply_value);
    SUPPRESS_SYNC.set(false);
    if result.is_ok() {
        sync();
    }
    result
}

fn save_user_text_at(
    path: &Path,
    mode: crate::environment::BuildMode,
    text: &str,
    mut apply: impl FnMut(&str, &Value),
) -> Result<(), SettingsError> {
    let root: Value =
        serde_json::from_str(text).map_err(|_| SettingsError::TopLevelObjectRequired)?;
    let Value::Object(mut document) = root else {
        return Err(SettingsError::TopLevelObjectRequired);
    };
    let existing = read_object(path);
    preserve_inactive_mobile_values(mode, existing.as_ref(), &mut document);
    let settings = validate(mode, &document)?;
    write_settings_document(path, document)
        .map_err(|_| SettingsError::InvalidValue("settings.json".to_owned()))?;
    for (key, value) in &settings {
        apply(key, value);
    }
    Ok(())
}

fn read_object(path: &Path) -> Option<Map<String, Value>> {
    let Value::Object(root) = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?
    else {
        return None;
    };
    Some(root)
}

fn preserve_inactive_mobile_values(
    mode: crate::environment::BuildMode,
    existing: Option<&Map<String, Value>>,
    document: &mut Map<String, Value>,
) {
    let inactive = MobileSettingsPolicy::new(mode.other()).keys();
    for key in [inactive.enabled, inactive.port, inactive.scrollback_cap] {
        match existing.and_then(|root| root.get(key)) {
            Some(value) => {
                document.insert(key.to_owned(), value.clone());
            }
            None => {
                document.remove(key);
            }
        }
    }
}

fn write_settings_document(path: &Path, root: Map<String, Value>) -> std::io::Result<()> {
    let contents = to_foundation_json(&Value::Object(root), false, true);
    crate::store::write_private(path, contents.as_bytes())
}

fn validate(
    mode: crate::environment::BuildMode,
    document: &Map<String, Value>,
) -> Result<Vec<(String, Value)>, SettingsError> {
    let mut settings = Vec::new();
    for entry in &mirror(mode) {
        let Some(value) = document.get(entry.key) else {
            continue;
        };
        validate_value_for(mode, entry, value)?;
        settings.push((entry.key.to_owned(), value.clone()));
    }
    Ok(settings)
}

fn validate_value_for(
    mode: crate::environment::BuildMode,
    entry: &Entry,
    value: &Value,
) -> Result<(), SettingsError> {
    let invalid = || SettingsError::InvalidValue(entry.key.to_owned());
    if value.is_null() {
        return Ok(());
    }
    match entry.source {
        Source::Defaults(kind) | Source::EditorSetting(_, kind) => match kind {
            Kind::Bool(_) => value.as_bool().map(|_| ()).ok_or_else(invalid),
            Kind::Int(_) => {
                let number = value
                    .as_i64()
                    .filter(|_| !value.is_boolean())
                    .ok_or_else(invalid)?;
                validate_range(mode, entry.key, number as f64).ok_or_else(invalid)
            }
            Kind::Double(_) => {
                let number = value
                    .as_f64()
                    .filter(|_| !value.is_boolean())
                    .ok_or_else(invalid)?;
                validate_range(mode, entry.key, number).ok_or_else(invalid)
            }
            Kind::Str(_) => {
                let text = value.as_str().ok_or_else(invalid)?;
                validate_string(entry.key, text).ok_or_else(invalid)
            }
        },
        Source::UiScale | Source::ThemeDark | Source::ThemeLight => {
            let text = value.as_str().ok_or_else(invalid)?;
            validate_string(entry.key, text).ok_or_else(invalid)
        }
        Source::ShortcutsApp => value
            .as_object()
            .filter(|object| !object.is_empty())
            .map(|_| ())
            .ok_or_else(invalid),
        Source::CustomCommands => {
            let object = value.as_object().ok_or_else(invalid)?;
            let assigned = |combo: Option<&Value>| {
                combo
                    .and_then(|combo| combo.get("key"))
                    .and_then(Value::as_str)
                    .is_some_and(|key| !key.is_empty())
            };
            if !assigned(object.get("prefixCombo")) {
                return Err(invalid());
            }
            let rows = object
                .get("shortcuts")
                .and_then(Value::as_array)
                .ok_or_else(invalid)?;
            if rows.iter().any(|row| !assigned(row.get("combo"))) {
                return Err(invalid());
            }
            Ok(())
        }
        Source::AiProviders => {
            let object = value.as_object().ok_or_else(invalid)?;
            if object.values().any(|value| !value.is_boolean()) {
                return Err(invalid());
            }
            Ok(())
        }
        Source::QuickTerminalShortcut => value.as_object().map(|_| ()).ok_or_else(invalid),
        Source::ApprovedDevices => value.as_array().map(|_| ()).ok_or_else(invalid),
    }
}

fn validate_string(key: &str, value: &str) -> Option<()> {
    if key == "muxy.general.defaultWorktreePathTemplate"
        && !value.trim().is_empty()
        && !value.contains("{branch}")
    {
        return None;
    }
    let Some((_, allowed)) = ALLOWED_STRINGS.iter().find(|(name, _)| *name == key) else {
        return Some(());
    };
    allowed.contains(&value).then_some(())
}

fn validate_range(mode: crate::environment::BuildMode, key: &str, value: f64) -> Option<()> {
    if key == "muxy.tabs.maxWidth" {
        return (value >= 0.0 && value.is_finite()).then_some(());
    }
    if key == "editor.richInputLineHeightMultiplier" {
        return (1.1..=2.0).contains(&value).then_some(());
    }
    let ranges = int_ranges(mode);
    let Some((_, low, high)) = ranges.iter().find(|(name, _, _)| *name == key) else {
        return Some(());
    };
    (value >= *low as f64 && value <= *high as f64).then_some(())
}

fn apply_value(key: &str, value: &Value) {
    match key {
        "muxy.ui.scale" => {
            let Some(raw) = value.as_str() else { return };
            set_ui_scale(crate::prefs::ScalePreset::parse(raw));
        }
        "muxy.theme.dark" | "muxy.theme.light" => {
            let Some(name) = value.as_str() else { return };
            let (dark, light) = crate::store::ghostty_conf::theme_selection();
            if key == "muxy.theme.dark" {
                crate::store::ghostty_conf::set_theme(
                    name,
                    &light.unwrap_or_else(|| name.to_owned()),
                );
            } else {
                crate::store::ghostty_conf::set_theme(
                    &dark.unwrap_or_else(|| name.to_owned()),
                    name,
                );
            }
        }
        _ => set(key, value.clone()),
    }
}

pub fn to_foundation_json(value: &Value, escape_slashes: bool, trailing_newline: bool) -> String {
    let mut out = String::new();
    write_value(&mut out, value, 0, escape_slashes);
    if trailing_newline {
        out.push('\n');
    }
    out
}

fn write_value(out: &mut String, value: &Value, depth: usize, escape_slashes: bool) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => out.push_str(&number_text(number)),
        Value::String(text) => write_string(out, text, escape_slashes),
        Value::Array(items) => {
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                indent(out, depth + 1);
                write_value(out, item, depth + 1, escape_slashes);
            }
            out.push('\n');
            indent(out, depth);
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|left, right| compare_keys(left, right));
            out.push_str("{\n");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                indent(out, depth + 1);
                write_string(out, key, escape_slashes);
                out.push_str(" : ");
                write_value(out, &map[*key], depth + 1, escape_slashes);
            }
            out.push('\n');
            indent(out, depth);
            out.push('}');
        }
    }
}

fn compare_keys(left: &str, right: &str) -> std::cmp::Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn number_text(number: &Number) -> String {
    if let Some(value) = number.as_u64() {
        return value.to_string();
    }
    if let Some(value) = number.as_i64() {
        return value.to_string();
    }
    let Some(value) = number.as_f64() else {
        return number.to_string();
    };
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return format!("{}", value as i64);
    }
    format!("{value}")
}

fn write_string(out: &mut String, value: &str, escape_slashes: bool) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '/' if escape_slashes => out.push_str("\\/"),
            character if (character as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => out.push(character),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    #[test]
    fn keys_with_their_own_source_do_not_read_from_portable_preferences() {
        assert!(super::special_string("muxy.ui.scale").is_some());
        assert!(super::special_string("muxy.theme.dark").is_some());
        assert!(super::special_string("muxy.theme.light").is_some());
        assert!(super::special_string("editor.richInputFontFamily").is_some());
        assert!(super::special_string("muxy.tabs.maxWidth").is_none());
        assert!(super::special_string("ai.providers").is_none());
        assert!(super::special_string("not.a.settings.key").is_none());
    }

    #[test]
    fn imported_settings_values_survive_when_portable_preferences_are_missing() {
        assert_eq!(
            super::read_defaults(
                "muxy.tests.importedBool",
                super::Kind::Bool(false),
                Some(&Value::Bool(true)),
            ),
            Value::Bool(true)
        );
        assert_eq!(
            super::read_defaults(
                "muxy.tests.importedString",
                super::Kind::Str("fallback"),
                Some(&Value::String("imported".to_owned())),
            ),
            Value::String("imported".to_owned())
        );
    }

    #[test]
    fn the_mirror_has_no_duplicate_keys() {
        let mut keys: Vec<&str> = super::MIRROR.iter().map(|entry| entry.key).collect();
        keys.sort_unstable();
        let count = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), count);
    }

    #[test]
    fn explicit_mirrors_differ_only_at_the_three_mobile_entries() {
        let development = super::mirror(crate::environment::BuildMode::Development);
        let production = super::mirror(crate::environment::BuildMode::Production);
        let differences: Vec<usize> = development
            .iter()
            .zip(production.iter())
            .enumerate()
            .filter_map(|(index, (left, right))| (left != right).then_some(index))
            .collect();
        assert_eq!(development.len(), 69);
        assert_eq!(production.len(), 69);
        for entries in [&development, &production] {
            let mut keys: Vec<&str> = entries.iter().map(|entry| entry.key).collect();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), 69);
        }
        assert_eq!(differences.len(), 3);
        for index in differences {
            assert!(development[index].key.ends_with(".dev"));
            assert_eq!(
                development[index].key.strip_suffix(".dev"),
                Some(production[index].key)
            );
        }
        assert_eq!(super::MIRROR, super::mirror(crate::build_mode!()));
    }

    #[test]
    fn active_mobile_entries_match_the_current_artifact() {
        let keys: Vec<&str> = super::MIRROR.iter().map(|entry| entry.key).collect();
        for key in [
            super::MOBILE_KEYS.enabled,
            super::MOBILE_KEYS.port,
            super::MOBILE_KEYS.scrollback_cap,
        ] {
            assert!(keys.contains(&key));
            assert_eq!(key.ends_with(".dev"), crate::build_mode!().is_development());
        }

        let defaults: Value = serde_json::from_str(&super::system_defaults_text()).unwrap();
        assert_eq!(
            defaults[super::MOBILE_KEYS.enabled],
            json!(super::MOBILE_POLICY.settings_enabled_default())
        );
        assert_eq!(
            defaults[super::MOBILE_KEYS.port],
            json!(super::MOBILE_POLICY.default_port())
        );
        assert_eq!(
            defaults[super::MOBILE_KEYS.scrollback_cap],
            json!(super::MOBILE_POLICY.default_scrollback_cap())
        );
    }

    #[test]
    fn system_defaults_lists_only_the_selected_mobile_namespace() {
        for mode in [
            crate::environment::BuildMode::Development,
            crate::environment::BuildMode::Production,
        ] {
            let active = crate::environment::MobileSettingsPolicy::new(mode).keys();
            let inactive = crate::environment::MobileSettingsPolicy::new(mode.other()).keys();
            let root: Value = serde_json::from_str(&super::system_defaults_text_for(mode)).unwrap();
            let root = root.as_object().unwrap();
            assert_eq!(root.len(), 69);
            for key in [active.enabled, active.port, active.scrollback_cap] {
                assert!(root.contains_key(key));
            }
            for key in [inactive.enabled, inactive.port, inactive.scrollback_cap] {
                assert!(!root.contains_key(key));
            }
        }
    }

    #[test]
    fn both_modes_sync_without_changing_the_inactive_mobile_keys() {
        for mode in [
            crate::environment::BuildMode::Development,
            crate::environment::BuildMode::Production,
        ] {
            let inactive = crate::environment::MobileSettingsPolicy::new(mode.other()).keys();
            let fixture = json!({
                (inactive.enabled): false,
                (inactive.port): 6123,
                (inactive.scrollback_cap): 21,
            });
            let (_dir, path) = sync_fixture(&serde_json::to_string(&fixture).unwrap());
            assert!(super::sync_at_with(&path, mode, |entry, _| Some(
                super::default_entry_value(entry)
            )));
            let root: Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(root[inactive.enabled], json!(false));
            assert_eq!(root[inactive.port], json!(6123));
            assert_eq!(root[inactive.scrollback_cap], json!(21));
        }
    }

    #[test]
    fn apply_restores_inactive_mobile_values_and_applies_only_active_keys() {
        for mode in [
            crate::environment::BuildMode::Development,
            crate::environment::BuildMode::Production,
        ] {
            let active_policy = crate::environment::MobileSettingsPolicy::new(mode);
            let active = active_policy.keys();
            let inactive = crate::environment::MobileSettingsPolicy::new(mode.other()).keys();
            let held = mobile_values(
                inactive,
                json!({ "held": false }),
                json!([6123]),
                Value::Null,
            );
            for replacement in [
                None,
                Some(mobile_values(inactive, json!(true), json!(7000), json!(32))),
                Some(mobile_values(
                    inactive,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                )),
                Some(mobile_values(
                    inactive,
                    json!("invalid"),
                    json!({ "invalid": true }),
                    json!(false),
                )),
            ] {
                let (_dir, path) =
                    sync_fixture(&serde_json::to_string(&Value::Object(held.clone())).unwrap());
                let mut submitted = mobile_values(
                    active,
                    json!(false),
                    json!(active_policy.minimum_port()),
                    json!(active_policy.maximum_scrollback_cap()),
                );
                if let Some(replacement) = replacement {
                    submitted.extend(replacement);
                }
                let mut applied = Vec::new();
                super::save_user_text_at(
                    &path,
                    mode,
                    &serde_json::to_string(&Value::Object(submitted)).unwrap(),
                    |key, value| applied.push((key.to_owned(), value.clone())),
                )
                .unwrap();
                let saved = super::read_object(&path).unwrap();
                for key in [inactive.enabled, inactive.port, inactive.scrollback_cap] {
                    assert_eq!(saved.get(key), held.get(key));
                    assert!(!applied.iter().any(|(applied, _)| applied == key));
                }
                assert_eq!(applied.len(), 3);
            }
        }
    }

    #[test]
    fn apply_cannot_add_missing_inactive_mobile_values() {
        for mode in [
            crate::environment::BuildMode::Development,
            crate::environment::BuildMode::Production,
        ] {
            let active_policy = crate::environment::MobileSettingsPolicy::new(mode);
            let active = active_policy.keys();
            let inactive = crate::environment::MobileSettingsPolicy::new(mode.other()).keys();
            let (_dir, path) = sync_fixture("{}");
            let mut submitted = mobile_values(
                active,
                json!(true),
                json!(active_policy.default_port()),
                json!(active_policy.default_scrollback_cap()),
            );
            submitted.extend(mobile_values(inactive, json!(true), json!(7000), json!(32)));
            let mut applied = Vec::new();
            super::save_user_text_at(
                &path,
                mode,
                &serde_json::to_string(&Value::Object(submitted)).unwrap(),
                |key, value| applied.push((key.to_owned(), value.clone())),
            )
            .unwrap();
            let saved = super::read_object(&path).unwrap();
            for key in [inactive.enabled, inactive.port, inactive.scrollback_cap] {
                assert!(!saved.contains_key(key));
                assert!(!applied.iter().any(|(applied, _)| applied == key));
            }
            assert_eq!(applied.len(), 3);
        }
    }

    #[test]
    fn apply_validates_active_mobile_boundaries_for_both_modes() {
        for mode in [
            crate::environment::BuildMode::Development,
            crate::environment::BuildMode::Production,
        ] {
            let policy = crate::environment::MobileSettingsPolicy::new(mode);
            let keys = policy.keys();
            let valid = mobile_values(
                keys,
                json!(true),
                json!(policy.maximum_port()),
                json!(policy.minimum_scrollback_cap()),
            );
            assert!(super::validate(mode, &valid).is_ok());

            let mut invalid_port = valid.clone();
            invalid_port.insert(keys.port.to_owned(), json!(80));
            assert!(super::validate(mode, &invalid_port).is_err());

            let mut invalid_cap = valid;
            invalid_cap.insert(keys.scrollback_cap.to_owned(), json!(129));
            assert!(super::validate(mode, &invalid_cap).is_err());
        }
    }

    #[test]
    fn reset_preserves_inactive_mobile_values_for_both_modes() {
        for mode in [
            crate::environment::BuildMode::Development,
            crate::environment::BuildMode::Production,
        ] {
            let inactive = crate::environment::MobileSettingsPolicy::new(mode.other()).keys();
            let held = mobile_values(inactive, json!(false), json!(7001), json!({ "mb": 16 }));
            let (_dir, path) =
                sync_fixture(&serde_json::to_string(&Value::Object(held.clone())).unwrap());
            super::reset_user_file_at(&path, mode, |entry| Some(super::default_entry_value(entry)))
                .unwrap();
            let saved = super::read_object(&path).unwrap();
            for key in [inactive.enabled, inactive.port, inactive.scrollback_cap] {
                assert_eq!(saved.get(key), held.get(key));
            }
        }
    }

    #[test]
    fn serializes_settings_json_the_way_foundation_does() {
        let value = json!({
            "SUAutomaticallyUpdate": true,
            "shortcuts.quickTerminal": { "type": "doubleShift" },
            "ai.providers": { "claude": true, "cursor": false },
            "muxy.app.blur": 0,
            "muxy.appBackgroundStyle": "solid",
            "muxy.tabs.maxWidth": 200.0,
            "muxy.extensionIconRailOrder": ["git:scm", "files:files"],
            "muxy.browser.homePageURL": "https://example.com/a",
            "muxy.emptyObject": {},
            "muxy.emptyArray": [],
            "muxy.approvedAt": 803589878.938123_f64,
        });
        let expected = concat!(
            "{\n",
            "  \"ai.providers\" : {\n",
            "    \"claude\" : true,\n",
            "    \"cursor\" : false\n",
            "  },\n",
            "  \"muxy.app.blur\" : 0,\n",
            "  \"muxy.appBackgroundStyle\" : \"solid\",\n",
            "  \"muxy.approvedAt\" : 803589878.938123,\n",
            "  \"muxy.browser.homePageURL\" : \"https://example.com/a\",\n",
            "  \"muxy.emptyArray\" : [\n",
            "\n",
            "  ],\n",
            "  \"muxy.emptyObject\" : {\n",
            "\n",
            "  },\n",
            "  \"muxy.extensionIconRailOrder\" : [\n",
            "    \"git:scm\",\n",
            "    \"files:files\"\n",
            "  ],\n",
            "  \"muxy.tabs.maxWidth\" : 200,\n",
            "  \"shortcuts.quickTerminal\" : {\n",
            "    \"type\" : \"doubleShift\"\n",
            "  },\n",
            "  \"SUAutomaticallyUpdate\" : true\n",
            "}\n",
        );
        assert_eq!(super::to_foundation_json(&value, false, true), expected);
    }

    #[test]
    fn serializes_ui_scale_json_without_a_trailing_newline() {
        let value = json!({ "preset": "large" });
        let text = super::to_foundation_json(&value, true, false);
        assert_eq!(text, "{\n  \"preset\" : \"large\"\n}");
        assert_eq!(text.len(), 24);
    }

    #[test]
    fn escapes_slashes_only_when_asked() {
        let value = json!({ "command": "scripts/setup.sh" });
        assert_eq!(
            super::to_foundation_json(&value, true, false),
            "{\n  \"command\" : \"scripts\\/setup.sh\"\n}"
        );
        assert_eq!(
            super::to_foundation_json(&value, false, false),
            "{\n  \"command\" : \"scripts/setup.sh\"\n}"
        );
    }

    #[test]
    fn escapes_control_characters_the_way_foundation_does() {
        let value = json!({ "s": "a\u{8}\u{c}\u{b}\u{1}\t\n\"\\" });
        assert_eq!(
            super::to_foundation_json(&value, false, false),
            "{\n  \"s\" : \"a\\b\\f\\u000b\\u0001\\t\\n\\\"\\\\\"\n}"
        );
    }

    fn sync_fixture(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.json");
        std::fs::write(&path, contents).expect("write");
        (dir, path)
    }

    fn mobile_values(
        keys: crate::environment::MobileSettingsKeys,
        enabled: Value,
        port: Value,
        cap: Value,
    ) -> serde_json::Map<String, Value> {
        let mut root = serde_json::Map::new();
        root.insert(keys.enabled.to_owned(), enabled);
        root.insert(keys.port.to_owned(), port);
        root.insert(keys.scrollback_cap.to_owned(), cap);
        root
    }

    #[test]
    fn sync_preserves_unknown_keys_and_overwrites_mirrored_ones() {
        let (_dir, path) =
            sync_fixture("{\"zzz.unknown\":{\"kept\":1},\"muxy.showStatusBar\":\"not a bool\"}");
        assert!(super::sync_at(&path, crate::build_mode!()));
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root.get("zzz.unknown"), Some(&json!({ "kept": 1 })));
        assert!(root.get("muxy.showStatusBar").unwrap().is_boolean());
        assert!(root.get("muxy.theme.dark").unwrap().is_string());
    }

    #[test]
    fn project_sort_mode_is_mirrored_into_portable_settings_json() {
        let (_dir, path) = sync_fixture("{}");
        assert!(super::sync_at_with(
            &path,
            crate::build_mode!(),
            |entry, _| (entry.key == "muxy.projectSortMode")
                .then(|| Value::String("nameDescending".to_owned())),
        ));
        let root: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(root["muxy.projectSortMode"], "nameDescending");
    }

    #[test]
    fn sync_emits_every_special_key_whose_source_it_can_read() {
        let (_dir, path) = sync_fixture("{}");
        super::sync_at(&path, crate::build_mode!());
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(root["shortcuts.quickTerminal"].get("type").is_some());
        assert!(!root["shortcuts.app"].as_object().unwrap().is_empty());
        assert!(
            root["shortcuts.customCommands"]
                .get("prefixCombo")
                .is_some()
        );
        assert_eq!(
            root["ai.providers"].as_object().map(serde_json::Map::len),
            Some(super::AI_PROVIDERS.len())
        );
    }

    #[test]
    fn the_shortcuts_app_arm_never_drops_a_key_it_does_not_model() {
        let mut object = serde_json::Map::new();
        object.insert("newTab".to_owned(), json!({ "key": "t", "modifiers": 1 }));
        let existing = json!({
            "newTab": { "key": "x", "modifiers": 9 },
            "inspectElement": { "key": "i", "modifiers": 1572864 },
        });
        let merged = super::carry_through(object, Some(&existing));
        assert_eq!(merged.len(), 2);
        assert_eq!(merged["newTab"], json!({ "key": "t", "modifiers": 1 }));
        assert_eq!(
            merged["inspectElement"],
            json!({ "key": "i", "modifiers": 1572864 })
        );
    }

    #[test]
    fn sync_skips_the_write_when_every_mirrored_value_already_agrees() {
        let (_dir, path) = sync_fixture("{}");
        assert!(super::sync_at(&path, crate::build_mode!()));
        let canonical: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        let compact = serde_json::to_string(&canonical).unwrap();
        std::fs::write(&path, &compact).expect("write");
        assert!(!super::sync_at(&path, crate::build_mode!()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), compact);
    }

    #[test]
    fn sync_leaves_a_corrupt_file_untouched() {
        let (_dir, path) = sync_fixture("this is not json");
        assert!(!super::sync_at(&path, crate::build_mode!()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "this is not json");
    }

    #[test]
    fn both_error_messages_match_swift() {
        assert_eq!(
            super::SettingsError::TopLevelObjectRequired.to_string(),
            "Settings JSON must be an object."
        );
        assert_eq!(
            super::SettingsError::InvalidValue("muxy.tabs.maxWidth".to_owned()).to_string(),
            "Invalid JSON value for \"muxy.tabs.maxWidth\"."
        );
    }

    #[test]
    fn the_system_defaults_pane_lists_every_mirrored_key_and_all_67_bindings() {
        let text = super::system_defaults_text();
        let root: Value = serde_json::from_str(&text).expect("json");
        let object = root.as_object().expect("object");
        assert_eq!(object.len(), super::MIRROR.len());

        let bindings = object["shortcuts.app"].as_object().expect("bindings");
        assert_eq!(bindings.len(), 67);
        for (name, key, modifiers) in crate::shortcuts::UNMODELLED_DEFAULTS {
            assert_eq!(
                bindings[name],
                json!({ "key": key, "modifiers": modifiers }),
                "{name}"
            );
        }
        assert_eq!(
            object["shortcuts.quickTerminal"],
            json!({ "type": "unassigned" })
        );
        assert_eq!(object["mobile.approvedDevices"], json!([]));
        assert_eq!(
            object["ai.providers"].as_object().map(serde_json::Map::len),
            Some(10)
        );
    }

    #[test]
    fn validation_accepts_swifts_values_and_rejects_everything_else() {
        let entry = super::MIRROR
            .iter()
            .find(|entry| entry.key == "muxy.tabs.maxWidth")
            .expect("entry");
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(180.0)).is_ok());
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(-1.0)).is_err());
        assert_eq!(
            super::validate_value_for(crate::build_mode!(), entry, &json!("wide")),
            Err(super::SettingsError::InvalidValue(
                "muxy.tabs.maxWidth".to_owned()
            ))
        );
        assert!(super::validate_value_for(crate::build_mode!(), entry, &Value::Null).is_ok());

        let entry = super::MIRROR
            .iter()
            .find(|entry| entry.key == "muxy.ui.scale")
            .expect("entry");
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!("huge")).is_ok());
        assert!(
            super::validate_value_for(crate::build_mode!(), entry, &json!("enormous")).is_err()
        );

        let entry = super::MIRROR
            .iter()
            .find(|entry| entry.key == super::MOBILE_KEYS.port)
            .expect("entry");
        assert!(
            super::validate_value_for(
                crate::build_mode!(),
                entry,
                &json!(super::MOBILE_POLICY.default_port())
            )
            .is_ok()
        );
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(80)).is_err());
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(true)).is_err());

        let entry = super::MIRROR
            .iter()
            .find(|entry| entry.key == super::MOBILE_KEYS.scrollback_cap)
            .expect("entry");
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(1)).is_ok());
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(128)).is_ok());
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(0)).is_err());
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(129)).is_err());

        let entry = super::MIRROR
            .iter()
            .find(|entry| entry.key == "muxy.showStatusBar")
            .expect("entry");
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(false)).is_ok());
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!(0)).is_err());

        let entry = super::MIRROR
            .iter()
            .find(|entry| entry.key == "shortcuts.app")
            .expect("entry");
        assert!(
            super::validate_value_for(crate::build_mode!(), entry, &json!({ "newTab": {} }))
                .is_ok()
        );
        assert!(super::validate_value_for(crate::build_mode!(), entry, &json!({})).is_err());

        let entry = super::MIRROR
            .iter()
            .find(|entry| entry.key == "shortcuts.customCommands")
            .expect("entry");
        assert!(
            super::validate_value_for(
                crate::build_mode!(),
                entry,
                &json!({ "prefixCombo": { "key": "g" }, "shortcuts": [] })
            )
            .is_ok()
        );
        assert!(
            super::validate_value_for(
                crate::build_mode!(),
                entry,
                &json!({ "prefixCombo": { "key": "" }, "shortcuts": [] })
            )
            .is_err()
        );

        let entry = super::MIRROR
            .iter()
            .find(|entry| entry.key == "ai.providers")
            .expect("entry");
        assert!(
            super::validate_value_for(crate::build_mode!(), entry, &json!({ "claude": true }))
                .is_ok()
        );
        assert!(
            super::validate_value_for(crate::build_mode!(), entry, &json!({ "claude": 1 }))
                .is_err()
        );
    }

    #[test]
    fn an_unknown_key_is_skipped_rather_than_rejected() {
        let mut document = serde_json::Map::new();
        document.insert("zzz.unknown".to_owned(), json!(1));
        document.insert("muxy.showStatusBar".to_owned(), json!(false));
        let settings = super::validate(crate::build_mode!(), &document).expect("valid");
        assert_eq!(settings.len(), 1);
        assert_eq!(settings[0].0, "muxy.showStatusBar");
    }

    #[test]
    fn prettify_rejects_a_non_object_and_reformats_an_object() {
        assert_eq!(super::prettify("[1,2]"), None);
        assert_eq!(super::prettify("not json"), None);
        assert_eq!(
            super::prettify("{\"a\":1}").as_deref(),
            Some("{\n  \"a\" : 1\n}\n")
        );
    }

    #[test]
    fn sync_writes_a_private_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("settings.json");
        assert!(super::sync_at(&path, crate::build_mode!()));
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
