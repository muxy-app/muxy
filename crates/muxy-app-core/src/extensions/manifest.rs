use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Icon {
    Name(String),
    Symbol { symbol: String },
    Svg { svg: String },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Surface {
    pub id: String,
    pub title: String,
    pub entry: String,
    pub icon: Option<Icon>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Panel {
    #[serde(flatten)]
    pub surface: Surface,
    #[serde(default = "right")]
    pub position: String,
    #[serde(default = "pinned")]
    pub mode: String,
    #[serde(default)]
    pub hidden_controls: Vec<String>,
    #[serde(default)]
    pub header_buttons: Vec<BarItem>,
}

fn right() -> String {
    "right".into()
}
fn pinned() -> String {
    "pinned".into()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub default_shortcut: Option<String>,
    pub action: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BarItem {
    pub id: String,
    pub icon: Icon,
    pub tooltip: Option<String>,
    pub command: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub description: String,
    pub permissions: BTreeSet<String>,
    pub events: BTreeSet<String>,
    pub tab_types: Vec<Surface>,
    pub panels: Vec<Panel>,
    pub commands: Vec<Command>,
    pub topbar_items: Vec<BarItem>,
    pub file_openers: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct Extension {
    pub name: String,
    pub version: String,
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

impl Extension {
    pub fn load(directory: &Path) -> Result<Self, String> {
        let load = || -> Result<Self, String> {
            let value =
                super::storage::read(&directory.join("package.json")).map_err(|e| e.to_string())?;
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
            let manifest = value
                .get("muxy")
                .filter(|v| v.is_object())
                .ok_or("missing muxy manifest")?;
            for field in [
                "background",
                "sidebar",
                "popovers",
                "homeViews",
                "remoteMethods",
                "localizations",
                "statusBarItems",
                "settings",
            ] {
                if manifest
                    .get(field)
                    .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
                {
                    return Err(format!("unsupported extension capability: {field}"));
                }
            }
            let extension = Self {
                name: name.into(),
                version: version.into(),
                directory: directory.canonicalize().map_err(|e| e.to_string())?,
                manifest: serde_json::from_value(manifest.clone()).map_err(|e| e.to_string())?,
            };
            extension.validate()?;
            Ok(extension)
        };
        load().map_err(|error| format!("{}: {error}", directory.display()))
    }

    pub fn resource(&self, relative: &str) -> Result<PathBuf, String> {
        let path = Path::new(relative);
        if relative.is_empty()
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err("resource must be a relative path inside the extension".into());
        }
        let path = self
            .directory
            .join(path)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !path.starts_with(&self.directory) || !path.is_file() {
            return Err("resource escapes the extension directory".into());
        }
        Ok(path)
    }

    pub fn allows(&self, permission: &str) -> bool {
        self.manifest.permissions.contains(permission)
    }

    pub fn allows_event(&self, event: &str) -> bool {
        if event.starts_with("extension.") {
            return event.len() <= 200;
        }
        self.manifest.events.contains(event)
            || self
                .manifest
                .commands
                .iter()
                .any(|command| event == format!("command.{}", command.id))
    }

    fn validate(&self) -> Result<(), String> {
        for surfaces in [
            self.manifest.tab_types.iter().collect::<Vec<_>>(),
            self.manifest.panels.iter().map(|p| &p.surface).collect(),
        ] {
            let mut ids = BTreeSet::new();
            for surface in surfaces {
                if !identifier(&surface.id) || !ids.insert(&surface.id) {
                    return Err("invalid or duplicate surface ID".into());
                }
                self.resource(&surface.entry)?;
                if let Some(Icon::Svg { svg }) = &surface.icon {
                    self.resource(svg)?;
                }
            }
        }
        let mut ids = BTreeSet::new();
        for command in &self.manifest.commands {
            if !identifier(&command.id) || !ids.insert(&command.id) {
                return Err("invalid or duplicate command ID".into());
            }
            if let Some(action) = &command.action {
                match action["kind"].as_str() {
                    Some("runScript" | "openModal") => {
                        let field = if action["kind"] == "runScript" {
                            "script"
                        } else {
                            "entry"
                        };
                        self.resource(action[field].as_str().ok_or("missing command resource")?)?;
                    }
                    Some("togglePanel" | "openPanel")
                        if self
                            .manifest
                            .panels
                            .iter()
                            .any(|p| Some(p.surface.id.as_str()) == action["panel"].as_str()) => {}
                    Some("openTab")
                        if self
                            .manifest
                            .tab_types
                            .iter()
                            .any(|p| Some(p.id.as_str()) == action["tabType"].as_str()) => {}
                    _ => {
                        return Err(format!(
                            "unsupported or invalid command action: {}",
                            command.id
                        ));
                    }
                }
            }
        }
        for opener in &self.manifest.file_openers {
            if !self
                .manifest
                .tab_types
                .iter()
                .any(|surface| Some(surface.id.as_str()) == opener["tabType"].as_str())
            {
                return Err("file opener references an unknown tab type".into());
            }
        }
        for item in self
            .manifest
            .topbar_items
            .iter()
            .chain(self.manifest.panels.iter().flat_map(|p| &p.header_buttons))
        {
            if !ids.contains(&item.command) {
                return Err(format!("unknown command: {}", item.command));
            }
            if let Icon::Svg { svg } = &item.icon {
                self.resource(svg)?;
            }
        }
        Ok(())
    }
}
