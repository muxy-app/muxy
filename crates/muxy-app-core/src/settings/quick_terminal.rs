use std::path::Path;

use muxy_core::quick_terminal::QuickTerminalShortcut;
use serde::{Deserialize, Serialize};

use crate::settings::{Error, Result};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QuickTerminalSettings {
    pub enabled: bool,
    pub width: u16,
    pub height: u16,
    pub transparency: u8,
    pub blur: u8,
    pub shortcut: QuickTerminalShortcut,
}

impl Default for QuickTerminalSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 720,
            height: 430,
            transparency: 18,
            blur: 70,
            shortcut: QuickTerminalShortcut::Unassigned,
        }
    }
}

impl QuickTerminalSettings {
    pub fn validate(&self) -> Result<()> {
        for (name, valid) in [
            ("width", (480..=1200).contains(&self.width)),
            ("height", (280..=800).contains(&self.height)),
            ("transparency", self.transparency <= 55),
            ("blur", self.blur <= 100),
            (
                "shortcut",
                !matches!(self.shortcut, QuickTerminalShortcut::KeyCombo { .. })
                    || self.shortcut.registration_identity().is_some(),
            ),
        ] {
            if !valid {
                return Err(Error::new(
                    format!("quick_terminal.{name}"),
                    "invalid value",
                ));
            }
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        crate::settings::appearance::save_section(path, "quick_terminal", self)
            .map_err(|error| Error::new("quick_terminal", error))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn removed_double_shift_loads_as_unassigned_without_losing_settings() {
        let directory =
            std::env::temp_dir().join(format!("muxy-quick-shortcut-{}", crate::ProjectId::new()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("settings.toml");
        std::fs::write(
            &path,
            "[quick_terminal]\nenabled = true\nwidth = 900\nheight = 600\ntransparency = 25\nblur = 40\n[quick_terminal.shortcut]\ntype = 'doubleShift'\n",
        ).unwrap();
        let settings = crate::settings::Settings::load(&path).unwrap();
        assert_eq!(
            settings.quick_terminal,
            QuickTerminalSettings {
                enabled: true,
                width: 900,
                height: 600,
                transparency: 25,
                blur: 40,
                shortcut: QuickTerminalShortcut::Unassigned,
            }
        );
        settings.quick_terminal.save(&path).unwrap();
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("doubleShift")
        );
        assert_eq!(
            crate::settings::Settings::load(&path)
                .unwrap()
                .quick_terminal,
            settings.quick_terminal
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
