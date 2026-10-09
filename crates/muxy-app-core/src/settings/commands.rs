use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Error, Result, Settings};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomCommand {
    pub id: String,
    pub name: String,
    pub command: String,
}

impl CustomCommand {
    pub fn new(name: String, command: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            command,
        }
    }

    pub fn shortcut_id(&self) -> String {
        format!("command.{}", self.id)
    }

    pub fn validate(&self) -> Result<()> {
        if !valid_id(&self.id) {
            return Err(Error::new(
                "commands",
                "command IDs must contain 1–64 letters, digits, hyphens, or underscores",
            ));
        }
        if self.name.trim().is_empty()
            || self.name.len() > 256
            || self.name.chars().any(char::is_control)
        {
            return Err(Error::new(
                "commands",
                "enter a name of 1–256 bytes without control characters",
            ));
        }
        if self.command.trim().is_empty()
            || self.command.len() >= 64 * 1024
            || self.command.chars().any(char::is_control)
        {
            return Err(Error::new(
                "commands",
                "enter a single-line command without control characters, shorter than 64 KiB",
            ));
        }
        Ok(())
    }
}

pub(super) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

impl Settings {
    pub(super) fn validate_commands(&self) -> Result<()> {
        if self.commands.len() > 256 {
            return Err(Error::new("commands", "at most 256 commands are supported"));
        }
        let quick = self
            .quick_terminal
            .shortcut
            .key_combo()
            .and_then(muxy_core::quick_terminal::keys::KeyCombo::keystroke)
            .and_then(|key| key.parse::<super::KeyChord>().ok());
        let mut ids = HashSet::new();
        for command in &self.commands {
            command.validate()?;
            if !ids.insert(&command.id) {
                return Err(Error::new("commands", "command IDs must be unique"));
            }
            if let Some(quick) = &quick
                && self.keymap.binding(&command.shortcut_id()) == Some(quick)
            {
                return Err(Error::new(
                    "commands",
                    "shortcut conflicts with Quick Terminal",
                ));
            }
        }
        Ok(())
    }

    pub fn validate_command_shortcuts(&self, terminal: &super::TerminalSettings) -> Result<()> {
        for command in &self.commands {
            let id = command.shortcut_id();
            if let Some(chord) = self.keymap.binding(&id)
                && terminal.keybindings.bindings.contains_key(chord)
            {
                return Err(Error::new(
                    format!("keymap.{id}"),
                    format!(
                        "{chord} for command '{}' is also bound in terminal settings; choose a different shortcut",
                        command.name
                    ),
                ));
            }
        }
        Ok(())
    }

    pub fn save_commands(&self, path: &Path) -> Result<()> {
        self.validate_commands()?;
        let values = (|| -> std::result::Result<_, toml::ser::Error> {
            Ok([
                ("commands", toml::Value::try_from(&self.commands)?),
                ("keymap", toml::Value::try_from(self.keymap.overrides())?),
            ])
        })()
        .map_err(|error| Error::new("commands", error))?;
        super::appearance::replace_sections(path, values)
            .map_err(|error| Error::new("commands", error))
    }
}
