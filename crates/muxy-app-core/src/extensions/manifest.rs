use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Deserializer};
use serde_json::Value;

const MAX_ICON_BYTES: u64 = 256 * 1024;
const MAX_CATALOG_BYTES: u64 = 4 * 1024 * 1024;

/// Every permission an extension can declare. Unknown names fail the load, as on main.
pub const PERMISSIONS: [&str; 25] = [
    "panes:read",
    "panes:write",
    "tabs:read",
    "tabs:write",
    "browser:read",
    "browser:write",
    "projects:read",
    "projects:write",
    "projects:delete",
    "worktrees:read",
    "worktrees:write",
    "agents:read",
    "git:read",
    "git:write",
    "files:read",
    "files:write",
    "storage:read",
    "storage:write",
    "notifications:write",
    "panels:write",
    "commands:run-script",
    "commands:exec",
    "shortcuts:register",
    "remote:serve",
    "gh:read",
];

/// An SF Symbol name or a template SVG bundled with the extension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Icon {
    Symbol(String),
    Svg(String),
}

impl Icon {
    /// The lenient runtime form accepted by `topbar.set`, `statusbar.set`, and `tabs.setIcon`.
    pub fn parse(value: &Value) -> Option<Self> {
        match value {
            Value::String(symbol) if !symbol.is_empty() => Some(Self::Symbol(symbol.clone())),
            Value::Object(fields) => {
                let text = |key| {
                    fields
                        .get(key)
                        .and_then(Value::as_str)
                        .filter(|value| !value.is_empty())
                };
                text("symbol")
                    .map(|symbol| Self::Symbol(symbol.into()))
                    .or_else(|| text("svg").map(|svg| Self::Svg(svg.into())))
            }
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for Icon {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        match Value::deserialize(deserializer)? {
            Value::String(symbol) if !symbol.is_empty() => Ok(Self::Symbol(symbol)),
            Value::Object(fields) if fields.len() == 1 => match fields.into_iter().next() {
                Some((key, Value::String(value))) if !value.is_empty() && key == "symbol" => {
                    Ok(Self::Symbol(value))
                }
                Some((key, Value::String(value))) if !value.is_empty() && key == "svg" => {
                    Ok(Self::Svg(value))
                }
                _ => Err(D::Error::custom(
                    "icon requires one non-empty 'symbol' or 'svg' field",
                )),
            },
            _ => Err(D::Error::custom(
                "icon requires a symbol name or exactly one 'symbol' or 'svg' field",
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabType {
    pub id: String,
    pub title: String,
    pub entry: String,
    #[serde(default)]
    pub default_data: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeView {
    pub id: String,
    pub title: String,
    pub icon: Option<Icon>,
    pub entry: String,
    #[serde(default)]
    pub default_data: Value,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelPosition {
    #[default]
    Right,
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelMode {
    #[default]
    Floating,
    Pinned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelControl {
    Icon,
    Title,
    Close,
    #[serde(alias = "mode")]
    Pin,
    Position,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Panel {
    pub id: String,
    pub title: Option<String>,
    pub icon: Option<Icon>,
    pub entry: String,
    #[serde(default)]
    pub position: PanelPosition,
    #[serde(default)]
    pub mode: PanelMode,
    #[serde(default)]
    pub hidden_controls: Vec<PanelControl>,
    #[serde(default)]
    pub header_buttons: Vec<BarItem>,
    #[serde(default)]
    pub hide_topbar: bool,
    #[serde(default)]
    pub default_data: Value,
}

impl Panel {
    pub fn hides(&self, control: PanelControl) -> bool {
        self.hide_topbar || self.hidden_controls.contains(&control)
    }

    /// Whether the user's float/dock choice applies instead of the declared mode.
    pub fn allows_mode_selection(&self) -> bool {
        !self.hides(PanelControl::Pin)
    }
}

fn default_popover_width() -> f64 {
    320.0
}

fn default_popover_height() -> f64 {
    360.0
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Popover {
    pub id: String,
    pub title: Option<String>,
    pub entry: String,
    #[serde(default = "default_popover_width")]
    pub width: f64,
    #[serde(default = "default_popover_height")]
    pub height: f64,
    #[serde(default)]
    pub default_data: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    pub id: String,
    pub title: Option<String>,
    pub icon: Option<Icon>,
    pub entry: String,
    #[serde(default)]
    pub default_data: Value,
}

/// What running a palette command does.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Action {
    /// Fires `command.<id>` to the extension.
    #[default]
    Event,
    OpenTab {
        tab_type: String,
        data: Value,
    },
    /// `openPanel` is 2.x's non-toggling spelling of `togglePanel`.
    TogglePanel {
        panel: String,
        toggle: bool,
    },
    OpenPopover {
        popover: String,
    },
    OpenModal {
        entry: String,
        width: Option<f64>,
        height: Option<f64>,
        dismiss_on_outside_click: bool,
        data: Value,
    },
    RunScript {
        script: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredAction {
    kind: String,
    tab_type: Option<String>,
    panel: Option<String>,
    popover: Option<String>,
    script: Option<String>,
    entry: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
    dismiss_on_outside_click: Option<bool>,
    #[serde(default)]
    data: Value,
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let stored = StoredAction::deserialize(deserializer)?;
        let required = |value: Option<String>, field: &'static str| {
            value.ok_or_else(|| D::Error::missing_field(field))
        };
        Ok(match stored.kind.as_str() {
            "event" => Self::Event,
            "openTab" => Self::OpenTab {
                tab_type: required(stored.tab_type, "tabType")?,
                data: stored.data,
            },
            "togglePanel" | "openPanel" => Self::TogglePanel {
                toggle: stored.kind == "togglePanel",
                panel: required(stored.panel, "panel")?,
            },
            "openPopover" => Self::OpenPopover {
                popover: required(stored.popover, "popover")?,
            },
            "openModal" => Self::OpenModal {
                entry: required(stored.entry, "entry")?,
                width: stored.width,
                height: stored.height,
                dismiss_on_outside_click: stored.dismiss_on_outside_click.unwrap_or(true),
                data: stored.data,
            },
            "runScript" => Self::RunScript {
                script: required(stored.script, "script")?,
            },
            other => {
                return Err(D::Error::custom(format!(
                    "unknown command action '{other}'"
                )));
            }
        })
    }
}

impl Action {
    /// The manifest permission the action needs before it runs.
    pub fn permission(&self) -> Option<&'static str> {
        match self {
            Self::Event => None,
            Self::OpenTab { .. } => Some("tabs:write"),
            Self::TogglePanel { .. } | Self::OpenPopover { .. } | Self::OpenModal { .. } => {
                Some("panels:write")
            }
            Self::RunScript { .. } => Some("commands:run-script"),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    #[serde(default)]
    pub action: Action,
    pub default_shortcut: Option<String>,
}

fn visible() -> bool {
    true
}

/// A topbar item or panel header button that runs one of the extension's commands.
#[derive(Clone, Debug, Deserialize)]
pub struct BarItem {
    pub id: String,
    pub icon: Icon,
    pub tooltip: Option<String>,
    pub command: String,
    #[serde(default = "visible")]
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StatusBarItem {
    pub id: String,
    pub icon: Icon,
    pub text: Option<String>,
    pub tooltip: Option<String>,
    pub side: Side,
    pub command: String,
    #[serde(default = "visible")]
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingKind {
    String,
    Bool,
    Number,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: SettingKind,
    #[serde(default)]
    pub default_value: Value,
}

fn all_files() -> Vec<String> {
    vec!["*".into()]
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOpener {
    pub id: String,
    pub title: Option<String>,
    pub tab_type: String,
    #[serde(default = "all_files")]
    pub patterns: Vec<String>,
    #[serde(default = "visible")]
    pub singleton: bool,
}

impl FileOpener {
    /// Case-insensitive `*`/`?` glob match against a project-relative path.
    pub fn matches(&self, relative: &str) -> bool {
        let path: Vec<char> = relative.to_lowercase().chars().collect();
        self.patterns.is_empty()
            || self
                .patterns
                .iter()
                .any(|pattern| glob(&pattern.to_lowercase().chars().collect::<Vec<_>>(), &path))
    }
}

fn glob(pattern: &[char], text: &[char]) -> bool {
    let (mut p, mut t) = (0, 0);
    let mut resume: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == '*' {
            resume = Some((p, t));
            p += 1;
        } else if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if let Some((star, matched)) = resume {
            p = star + 1;
            t = matched + 1;
            resume = Some((star, matched + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|character| *character == '*')
}

#[derive(Clone, Debug, Deserialize)]
pub struct Localization {
    pub id: String,
    pub language: String,
    pub title: String,
    pub bundle: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RemoteMethod {
    pub id: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub description: String,
    pub background: Option<String>,
    pub permissions: BTreeSet<String>,
    pub events: BTreeSet<String>,
    pub commands: Vec<Command>,
    pub tab_types: Vec<TabType>,
    pub home_views: Vec<HomeView>,
    pub panels: Vec<Panel>,
    pub popovers: Vec<Popover>,
    pub sidebar: Option<Sidebar>,
    pub file_openers: Vec<FileOpener>,
    pub localizations: Vec<Localization>,
    pub topbar_items: Vec<BarItem>,
    pub status_bar_items: Vec<StatusBarItem>,
    pub settings: Vec<Setting>,
    pub remote_methods: Vec<RemoteMethod>,
}

impl Manifest {
    pub fn command(&self, id: &str) -> Option<&Command> {
        self.commands.iter().find(|command| command.id == id)
    }

    pub fn tab_type(&self, id: &str) -> Option<&TabType> {
        self.tab_types.iter().find(|tab| tab.id == id)
    }

    pub fn panel(&self, id: &str) -> Option<&Panel> {
        self.panels.iter().find(|panel| panel.id == id)
    }

    pub fn popover(&self, id: &str) -> Option<&Popover> {
        self.popovers.iter().find(|popover| popover.id == id)
    }

    pub fn setting(&self, key: &str) -> Option<&Setting> {
        self.settings.iter().find(|setting| setting.key == key)
    }
}

#[derive(Clone, Debug)]
pub struct Extension {
    pub name: String,
    pub version: String,
    /// The resource root: the package's `dist/` build output when present.
    pub directory: PathBuf,
    pub manifest: Manifest,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

fn language_tag(value: &str) -> bool {
    let mut parts = value.split('-');
    parts.next().is_some_and(|primary| {
        (2..=8).contains(&primary.len()) && primary.bytes().all(|b| b.is_ascii_alphabetic())
    }) && parts.all(|part| {
        (1..=8).contains(&part.len()) && part.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

/// Extension-local event names: `extension.` plus up to 190 safe characters.
pub fn local_event(name: &str) -> bool {
    name.len() > "extension.".len()
        && name.len() <= 200
        && name.starts_with("extension.")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn distinct<'a>(kind: &str, ids: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(format!("duplicate {kind} '{id}'"));
        }
    }
    Ok(())
}

fn unique<'a>(kind: &str, ids: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    let ids: Vec<_> = ids.into_iter().collect();
    if ids.iter().any(|id| id.is_empty()) {
        return Err(format!("{kind} id must not be empty"));
    }
    distinct(kind, ids)
}

/// A regular file inside `directory`, read up to main's 4 MiB catalog limit;
/// empty when it does not exist.
fn bounded_text(directory: &Path, name: &str) -> Result<String, String> {
    use std::io::Read;
    let Ok(path) = directory.join(name).canonicalize() else {
        return Ok(String::new());
    };
    let too_large = || format!("'{name}' must be a file of at most 4 MiB inside its bundle");
    if !path.starts_with(directory) || !path.is_file() {
        return Err(too_large());
    }
    let mut text = String::new();
    std::fs::File::open(&path)
        .and_then(|file| file.take(MAX_CATALOG_BYTES + 1).read_to_string(&mut text))
        .map_err(|error| error.to_string())?;
    if text.len() as u64 > MAX_CATALOG_BYTES {
        return Err(too_large());
    }
    Ok(text)
}

/// main decodes optional manifest fields with `decodeIfPresent`, so `null`
/// means "not set". Free-form JSON (`defaultData`, `data`, `defaultValue`)
/// is kept as written.
fn without_nulls(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            fields.retain(|_, field| !field.is_null());
            for (key, field) in fields.iter_mut() {
                if !matches!(key.as_str(), "defaultData" | "data" | "defaultValue") {
                    without_nulls(field);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(without_nulls),
        _ => {}
    }
}

impl Extension {
    /// Loads a package folder, preferring its `dist/` build output like main does.
    pub fn load(package: &Path) -> Result<Self, String> {
        let load = || -> Result<Self, String> {
            let dist = package.join("dist");
            let manifest_path = if dist.join("package.json").is_file() {
                dist.join("package.json")
            } else {
                package.join("package.json")
            };
            let root = if dist.is_dir() {
                dist
            } else {
                package.to_path_buf()
            };
            let value = super::storage::read(&manifest_path).map_err(|e| e.to_string())?;
            let name = value["name"]
                .as_str()
                .filter(|name| identifier(name))
                .ok_or("invalid extension name")?;
            if name == "foundation" {
                return Err("extension name is reserved".into());
            }
            let version = value["version"]
                .as_str()
                .filter(|version| !version.is_empty())
                .ok_or("missing version")?;
            let mut manifest = value
                .get("muxy")
                .filter(|v| v.is_object())
                .ok_or("missing muxy manifest")?
                .clone();
            without_nulls(&mut manifest);
            let extension = Self {
                name: name.into(),
                version: version.into(),
                directory: root.canonicalize().map_err(|e| e.to_string())?,
                manifest: serde_json::from_value(manifest)
                    .map_err(|e| format!("invalid manifest: {e}"))?,
            };
            extension.validate()?;
            Ok(extension)
        };
        load().map_err(|error| format!("{}: {error}", package.display()))
    }

    fn located(&self, relative: &str) -> Result<PathBuf, String> {
        let path = Path::new(relative);
        if relative.is_empty()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err(format!("'{relative}' escapes the extension directory"));
        }
        let path = self
            .directory
            .join(path)
            .canonicalize()
            .map_err(|_| format!("'{relative}' was not found"))?;
        if !path.starts_with(&self.directory) {
            return Err(format!("'{relative}' escapes the extension directory"));
        }
        Ok(path)
    }

    /// Resolves a file inside the resource root, rejecting traversal and symlink escapes.
    pub fn resource(&self, relative: &str) -> Result<PathBuf, String> {
        let path = self.located(relative)?;
        if path.is_file() {
            Ok(path)
        } else {
            Err(format!("'{relative}' was not found"))
        }
    }

    pub fn allows(&self, permission: &str) -> bool {
        self.manifest.permissions.contains(permission)
    }

    /// Whether the manifest lets the extension subscribe to an event.
    pub fn allows_event(&self, event: &str) -> bool {
        if event.starts_with("extension.") {
            return local_event(event);
        }
        (self.manifest.events.contains(event)
            || event
                .strip_prefix("command.")
                .is_some_and(|id| self.manifest.command(id).is_some()))
            && super::event_permission(event).is_none_or(|permission| self.allows(permission))
    }

    fn validate(&self) -> Result<(), String> {
        if let Some(permission) = self
            .manifest
            .permissions
            .iter()
            .find(|permission| !PERMISSIONS.contains(&permission.as_str()))
        {
            return Err(format!("unknown permission '{permission}'"));
        }
        if let Some(background) = &self.manifest.background {
            self.resource(background)
                .map_err(|error| format!("background script {error}"))?;
        }
        self.validate_surfaces()?;
        self.validate_commands()?;
        self.validate_items()?;
        self.validate_localizations()?;
        unique(
            "setting",
            self.manifest.settings.iter().map(|s| s.key.as_str()),
        )?;
        unique(
            "remote method",
            self.manifest.remote_methods.iter().map(|m| m.id.as_str()),
        )?;
        if let Some(method) = self
            .manifest
            .remote_methods
            .iter()
            .find(|method| method.id.chars().any(|c| c == '|' || c.is_control()))
        {
            return Err(format!("invalid remote method id '{}'", method.id));
        }
        Ok(())
    }

    fn validate_entry(&self, kind: &str, id: &str, entry: &str) -> Result<(), String> {
        self.resource(entry)
            .map(|_| ())
            .map_err(|error| format!("{kind} '{id}' entry {error}"))
    }

    fn validate_icon(&self, kind: &str, id: &str, icon: Option<&Icon>) -> Result<(), String> {
        let Some(Icon::Svg(svg)) = icon else {
            return Ok(());
        };
        let path = self
            .resource(svg)
            .map_err(|error| format!("{kind} '{id}' icon {error}"))?;
        let size = std::fs::metadata(path).map_or(u64::MAX, |metadata| metadata.len());
        if !svg.to_ascii_lowercase().ends_with(".svg") || size > MAX_ICON_BYTES {
            return Err(format!(
                "{kind} '{id}' icon '{svg}' must be an SVG file of at most 256 KiB"
            ));
        }
        Ok(())
    }

    fn validate_surfaces(&self) -> Result<(), String> {
        let manifest = &self.manifest;
        distinct("tab type", manifest.tab_types.iter().map(|t| t.id.as_str()))?;
        for tab in &manifest.tab_types {
            self.validate_entry("tab type", &tab.id, &tab.entry)?;
        }
        unique(
            "home view",
            manifest.home_views.iter().map(|v| v.id.as_str()),
        )?;
        for view in &manifest.home_views {
            if view.title.is_empty() {
                return Err(format!("home view '{}' title must not be empty", view.id));
            }
            self.validate_entry("home view", &view.id, &view.entry)?;
            self.validate_icon("home view", &view.id, view.icon.as_ref())?;
        }
        distinct("panel", manifest.panels.iter().map(|p| p.id.as_str()))?;
        for panel in &manifest.panels {
            self.validate_entry("panel", &panel.id, &panel.entry)?;
            self.validate_icon("panel", &panel.id, panel.icon.as_ref())?;
            unique(
                &format!("panel '{}' header button", panel.id),
                panel.header_buttons.iter().map(|b| b.id.as_str()),
            )?;
        }
        distinct("popover", manifest.popovers.iter().map(|p| p.id.as_str()))?;
        for popover in &manifest.popovers {
            self.validate_entry("popover", &popover.id, &popover.entry)?;
        }
        if let Some(sidebar) = &manifest.sidebar {
            unique("sidebar", [sidebar.id.as_str()])?;
            self.validate_entry("sidebar", &sidebar.id, &sidebar.entry)?;
            self.validate_icon("sidebar", &sidebar.id, sidebar.icon.as_ref())?;
        }
        unique(
            "file opener",
            manifest.file_openers.iter().map(|o| o.id.as_str()),
        )?;
        for opener in &manifest.file_openers {
            if manifest.tab_type(&opener.tab_type).is_none() {
                return Err(format!(
                    "file opener '{}' references unknown tab type '{}'",
                    opener.id, opener.tab_type
                ));
            }
            if opener.patterns.iter().any(String::is_empty) {
                return Err(format!("file opener '{}' has an empty pattern", opener.id));
            }
        }
        Ok(())
    }

    fn validate_commands(&self) -> Result<(), String> {
        let manifest = &self.manifest;
        for command in &manifest.commands {
            let missing = |kind: &str, target: &str| {
                Err(format!(
                    "command '{}' references unknown {kind} '{target}'",
                    command.id
                ))
            };
            match &command.action {
                Action::OpenTab { tab_type, .. } if manifest.tab_type(tab_type).is_none() => {
                    return missing("tab type", tab_type);
                }
                Action::TogglePanel { panel, .. } if manifest.panel(panel).is_none() => {
                    return missing("panel", panel);
                }
                Action::OpenPopover { popover } if manifest.popover(popover).is_none() => {
                    return missing("popover", popover);
                }
                Action::OpenModal { entry, .. } => {
                    self.validate_entry("command", &command.id, entry)?;
                }
                Action::RunScript { script } => {
                    self.resource(script)
                        .map_err(|error| format!("command '{}' script {error}", command.id))?;
                }
                Action::Event
                | Action::OpenTab { .. }
                | Action::TogglePanel { .. }
                | Action::OpenPopover { .. } => {}
            }
        }
        Ok(())
    }

    fn validate_items(&self) -> Result<(), String> {
        let manifest = &self.manifest;
        let check = |kind: &str, id: &str, command: &str, icon: &Icon| {
            if manifest.command(command).is_none() {
                return Err(format!(
                    "{kind} '{id}' references unknown command '{command}'"
                ));
            }
            self.validate_icon(kind, id, Some(icon))
        };
        unique(
            "topbar item",
            manifest.topbar_items.iter().map(|i| i.id.as_str()),
        )?;
        for item in &manifest.topbar_items {
            check("topbar item", &item.id, &item.command, &item.icon)?;
        }
        unique(
            "status bar item",
            manifest.status_bar_items.iter().map(|i| i.id.as_str()),
        )?;
        for item in &manifest.status_bar_items {
            check("status bar item", &item.id, &item.command, &item.icon)?;
        }
        for panel in &manifest.panels {
            for button in &panel.header_buttons {
                check(
                    "panel header button",
                    &button.id,
                    &button.command,
                    &button.icon,
                )?;
            }
        }
        Ok(())
    }

    fn validate_localizations(&self) -> Result<(), String> {
        let localizations = &self.manifest.localizations;
        unique("localization", localizations.iter().map(|l| l.id.as_str()))?;
        for localization in localizations {
            let id = &localization.id;
            if !identifier(id) {
                return Err(format!("invalid localization id '{id}'"));
            }
            if !language_tag(&localization.language) {
                return Err(format!(
                    "localization '{id}' has an invalid language '{}'",
                    localization.language
                ));
            }
            if localization.title.trim().is_empty() {
                return Err(format!("localization '{id}' title must not be empty"));
            }
            let bundle = self
                .located(&localization.bundle)
                .ok()
                .filter(|path| path.is_dir())
                .ok_or_else(|| format!("localization '{id}' bundle was not found"))?;
            if bounded_text(&bundle, "Info.plist")?.contains("CFBundleExecutable") {
                return Err(format!(
                    "localization '{id}' bundle must not contain executable code"
                ));
            }
            let catalogs: Vec<_> = ["Localizable.strings", "Localizable.stringsdict"]
                .iter()
                .map(|name| {
                    bundle
                        .join(format!("{}.lproj", localization.language))
                        .join(name)
                })
                .filter(|path| path.is_file())
                .collect();
            if catalogs.is_empty()
                || catalogs.iter().any(|path| {
                    !std::fs::metadata(path).is_ok_and(|m| m.len() <= MAX_CATALOG_BYTES)
                })
            {
                return Err(format!(
                    "localization '{id}' needs a Localizable catalog of at most 4 MiB"
                ));
            }
        }
        Ok(())
    }
}
