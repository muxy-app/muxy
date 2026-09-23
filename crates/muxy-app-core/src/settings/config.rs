use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::settings::{Appearance, Error, Keymap, Result};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub ai: AiSettings,
    pub composer: super::ComposerSettings,
    pub panel_pins: BTreeMap<String, BTreeMap<String, bool>>,
    pub quick_terminal: crate::settings::QuickTerminalSettings,
    pub appearance: Appearance,
    pub window: WindowSettings,
    pub keymap: Keymap,
    pub projects: ProjectSettings,
    pub panes: PaneSettings,
    pub clipboard: ClipboardSettings,
    pub openers: OpenerSettings,
}

/// Per-action AI choices. Hand edits are expected here, so unknown keys and
/// non-text values are ignored instead of stopping the app from starting.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    #[serde(deserialize_with = "text_entries")]
    pub providers: BTreeMap<String, String>,
    #[serde(deserialize_with = "text_entries")]
    pub prompts: BTreeMap<String, String>,
    #[serde(deserialize_with = "text_entries")]
    pub project_pr_prompts: BTreeMap<String, String>,
}

fn text_entries<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<BTreeMap<String, String>, D::Error> {
    let value = toml::Value::deserialize(deserializer)?;
    Ok(value
        .as_table()
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
        .collect())
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaneSettings {
    pub new_pane_directory: NewPaneDirectory,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewPaneDirectory {
    #[default]
    Project,
    Current,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectSettings {
    pub search_root: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WindowSettings {
    pub default_size: [f32; 2],
    pub confirm_running_process: bool,
    pub close_behavior: CloseBehavior,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseBehavior {
    #[default]
    CloseSession,
    Detach,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            default_size: [1200.0, 800.0],
            confirm_running_process: true,
            close_behavior: CloseBehavior::CloseSession,
        }
    }
}

impl Settings {
    /// Saves one AI entry over the file's current contents, keeping hand edits,
    /// and returns the AI settings as saved.
    pub fn save_ai_entry(
        path: &Path,
        table: &str,
        key: &str,
        value: Option<&str>,
    ) -> Result<AiSettings> {
        crate::settings::appearance::save_entry(path, "ai", table, key, value)
            .and_then(|saved| saved.try_into().map_err(Into::into))
            .map_err(|error| Error::new("ai", error))
    }

    pub fn panel_pinned(&self, owner: &str, panel: &str, default: bool) -> bool {
        self.panel_pins
            .get(owner)
            .and_then(|panels| panels.get(panel))
            .copied()
            .unwrap_or(default)
    }

    pub fn set_panel_pinned(
        &mut self,
        owner: &str,
        panel: &str,
        pinned: bool,
        path: &Path,
    ) -> Result<()> {
        let mut pins = self.panel_pins.clone();
        pins.entry(owner.into())
            .or_default()
            .insert(panel.into(), pinned);
        crate::settings::appearance::save_section(path, "panel_pins", &pins)
            .map_err(|error| Error::new("panel_pins", error))?;
        self.panel_pins = pins;
        Ok(())
    }

    pub fn set_confirm_running_process(&mut self, enabled: bool, path: &Path) -> Result<()> {
        let values = toml::Table::from_iter([(
            "confirm_running_process".into(),
            toml::Value::Boolean(enabled),
        )]);
        crate::settings::appearance::save_section(path, "window", &values)
            .map_err(|error| Error::new("window.confirm_running_process", error))?;
        self.window.confirm_running_process = enabled;
        Ok(())
    }

    pub fn set_project_search_root(&mut self, root: PathBuf, path: &Path) -> Result<()> {
        let values = toml::Table::from_iter([(
            "search_root".into(),
            toml::Value::String(root.to_string_lossy().into_owned()),
        )]);
        crate::settings::appearance::save_section(path, "projects", &values)
            .map_err(|error| Error::new("projects.search_root", error))?;
        self.projects.search_root = Some(root);
        Ok(())
    }

    pub fn default_path() -> Result<PathBuf> {
        muxy_core::dirs::muxy_dir()
            .map(|directory| directory.join("settings.toml"))
            .map_err(|error| Error::new("settings path", error))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let source = read_or_create(
            path,
            &toml::to_string_pretty(&Self::default())
                .map_err(|error| Error::new("settings defaults", error))?,
        )?;
        let settings: Self = toml::from_str(&source)
            .map_err(|error| Error::new(path.display().to_string(), error))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<()> {
        self.quick_terminal.validate()?;
        self.composer.validate()?;
        for (name, value, minimum) in [
            ("width", self.window.default_size[0], 640.0),
            ("height", self.window.default_size[1], 400.0),
        ] {
            if !value.is_finite() || !(minimum..=16384.0).contains(&value) {
                return Err(Error::new(
                    format!("window.default_size.{name}"),
                    format!("must be between {minimum} and 16384"),
                ));
            }
        }
        Ok(())
    }

    pub fn save_window(&self, path: &Path) -> Result<()> {
        self.validate()?;
        crate::settings::appearance::save_section(path, "window", &self.window)
            .map_err(|error| Error::new("window", error))
    }

    pub fn save_clipboard(&self, path: &Path) -> Result<()> {
        crate::settings::appearance::save_section(path, "clipboard", &self.clipboard)
            .map_err(|error| Error::new("clipboard", error))
    }

    pub fn save_composer(&self, path: &Path) -> Result<()> {
        self.composer.validate()?;
        crate::settings::appearance::save_section(path, "composer", &self.composer)
            .map_err(|error| Error::new("composer", error))
    }

    pub fn save_panes(&self, path: &Path) -> Result<()> {
        crate::settings::appearance::save_section(path, "panes", &self.panes)
            .map_err(|error| Error::new("panes", error))
    }
}

pub(crate) fn read_or_create(path: &Path, defaults: &str) -> Result<String> {
    let context = || path.display().to_string();
    match fs::read_to_string(path) {
        Ok(source) => return Ok(source),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::new(context(), error)),
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| Error::new(context(), error))?;
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(defaults.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|error| Error::new(context(), error))?;
            Ok(defaults.into())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::read_to_string(path).map_err(|error| Error::new(context(), error))
        }
        Err(error) => Err(Error::new(context(), error)),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClipboardSettings {
    pub copy_on_select: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OpenerSettings {
    pub url: String,
    pub file: String,
    pub project_target: Option<String>,
}

impl Default for OpenerSettings {
    fn default() -> Self {
        Self {
            url: "system.browser".into(),
            file: "system.editor".into(),
            project_target: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_settings_ignore_hand_edit_mistakes_and_saves_keep_hand_edits() -> Result<()> {
        let directory = tempfile::tempdir().map_err(|error| Error::new("test", error))?;
        let path = directory.path().join("settings.toml");
        fs::write(
            &path,
            "[ai]\nprompt = \"typo\"\n[ai.prompts]\ncommit = \"Use Conventional Commits\"\ncreate_pr = 5\n",
        )
        .map_err(|error| Error::new("test", error))?;
        let loaded = Settings::load(&path)?;
        assert_eq!(
            loaded.ai.prompts,
            BTreeMap::from([("commit".into(), "Use Conventional Commits".into())])
        );

        let saved = Settings::save_ai_entry(&path, "providers", "commit", Some("claude"))?;
        assert_eq!(saved.providers["commit"], "claude");
        assert_eq!(saved.prompts["commit"], "Use Conventional Commits");
        let saved = Settings::save_ai_entry(
            &path,
            "project_pr_prompts",
            "project-id",
            Some("Project instructions"),
        )?;
        assert_eq!(
            saved.project_pr_prompts["project-id"],
            "Project instructions"
        );
        let saved = Settings::save_ai_entry(&path, "prompts", "commit", None)?;
        assert!(saved.prompts.is_empty());
        assert_eq!(
            saved.project_pr_prompts["project-id"],
            "Project instructions"
        );
        assert_eq!(Settings::load(&path)?.ai, saved);
        Ok(())
    }
}
