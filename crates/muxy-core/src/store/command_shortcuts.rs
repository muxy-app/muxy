use crate::prefs::app_support_dir;
use crate::shortcuts::{COMMAND, CONTROL, KeyCombo, OPTION, SHIFT};
use serde::{Deserialize, Serialize};
use std::path::Path;

const SUPPORTED_MODIFIERS: u64 = COMMAND | SHIFT | CONTROL | OPTION;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandShortcut {
    pub id: String,
    pub name: String,
    pub command: String,
    pub combo: KeyCombo,
}

impl CommandShortcut {
    pub fn display_name(&self) -> String {
        let trimmed = self.name.trim();
        if trimmed.is_empty() {
            "Command".to_owned()
        } else {
            trimmed.to_owned()
        }
    }

    pub fn trimmed_command(&self) -> String {
        self.command.trim().to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct CommandShortcuts {
    pub prefix_combo: KeyCombo,
    pub shortcuts: Vec<CommandShortcut>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConfiguration {
    prefix_combo: Option<KeyCombo>,
    shortcuts: Option<Vec<CommandShortcut>>,
}

impl Default for CommandShortcuts {
    fn default() -> Self {
        Self {
            prefix_combo: KeyCombo::new("g", COMMAND),
            shortcuts: Vec::new(),
        }
    }
}

impl CommandShortcuts {
    pub fn load() -> Self {
        Self::read(&app_support_dir().join("command-shortcuts.json"))
    }

    fn read(path: &Path) -> Self {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
            return Self::default();
        };
        if let Ok(shortcuts) = serde_json::from_value::<Vec<CommandShortcut>>(value.clone()) {
            return Self::normalized(Self::default().prefix_combo, shortcuts);
        }
        let Ok(stored) = serde_json::from_value::<StoredConfiguration>(value) else {
            return Self::default();
        };
        Self::normalized(
            stored.prefix_combo.unwrap_or(Self::default().prefix_combo),
            stored.shortcuts.unwrap_or_default(),
        )
    }

    fn normalized(prefix_combo: KeyCombo, shortcuts: Vec<CommandShortcut>) -> Self {
        Self {
            prefix_combo,
            shortcuts: shortcuts
                .into_iter()
                .map(|mut shortcut| {
                    shortcut.combo = combo_with_default_modifier(shortcut.combo);
                    shortcut
                })
                .collect(),
        }
    }

    pub fn shortcut(&self, id: &str) -> Option<&CommandShortcut> {
        self.shortcuts.iter().find(|shortcut| shortcut.id == id)
    }

    pub fn mirror_value(&self) -> serde_json::Value {
        let mut root = serde_json::Map::new();
        root.insert(
            "prefixCombo".to_owned(),
            serde_json::to_value(&self.prefix_combo).unwrap_or(serde_json::Value::Null),
        );
        root.insert(
            "shortcuts".to_owned(),
            serde_json::to_value(&self.shortcuts).unwrap_or(serde_json::Value::Null),
        );
        serde_json::Value::Object(root)
    }

    pub fn add(&mut self) -> String {
        let id = crate::store::new_uuid();
        self.shortcuts.push(CommandShortcut {
            id: id.clone(),
            name: String::new(),
            command: String::new(),
            combo: KeyCombo::new("t", COMMAND | OPTION),
        });
        id
    }

    pub fn update(&mut self, id: &str, name: Option<String>, command: Option<String>) {
        let Some(shortcut) = self.shortcuts.iter_mut().find(|entry| entry.id == id) else {
            return;
        };
        if let Some(name) = name {
            shortcut.name = name;
        }
        if let Some(command) = command {
            shortcut.command = command;
        }
    }

    pub fn set_combo(&mut self, id: &str, combo: KeyCombo) {
        let Some(shortcut) = self.shortcuts.iter_mut().find(|entry| entry.id == id) else {
            return;
        };
        shortcut.combo = combo_with_default_modifier(combo);
    }

    pub fn remove(&mut self, id: &str) {
        self.shortcuts.retain(|shortcut| shortcut.id != id);
    }

    pub fn remove_all(&mut self) {
        self.shortcuts.clear();
    }

    pub fn set_prefix_combo(&mut self, combo: KeyCombo) {
        self.prefix_combo = combo_with_default_modifier(combo);
    }

    pub fn conflicting(&self, combo: &KeyCombo, excluding: &str) -> Option<&CommandShortcut> {
        if !combo.is_assigned() {
            return None;
        }
        let combo = combo_with_default_modifier(combo.clone());
        self.shortcuts
            .iter()
            .find(|shortcut| shortcut.id != excluding && shortcut.combo.conflicts_with(&combo))
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&app_support_dir().join("command-shortcuts.json"))?;
        crate::prefs::settings::sync();
        Ok(())
    }

    fn save_to(&self, path: &Path) -> std::io::Result<()> {
        let contents =
            crate::prefs::settings::to_foundation_json(&self.mirror_value(), true, false);
        crate::store::write_private(path, contents.as_bytes())
    }
}

fn combo_with_default_modifier(combo: KeyCombo) -> KeyCombo {
    if combo.modifiers & SUPPORTED_MODIFIERS != 0 {
        return combo;
    }
    KeyCombo::new(&combo.key, COMMAND)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, contents: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("muxy-command-shortcuts-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("command-shortcuts.json");
        std::fs::write(&path, contents).expect("write fixture");
        path
    }

    #[test]
    fn reads_the_object_form() {
        let path = write(
            "object",
            r#"{
                "prefixCombo": { "key": "j", "modifiers": 1572864 },
                "shortcuts": [
                    { "id": "A", "name": "setup", "command": "echo hi",
                      "combo": { "key": "s", "modifiers": 1048576 } }
                ]
            }"#,
        );
        let config = CommandShortcuts::read(&path);
        assert_eq!(config.prefix_combo, KeyCombo::new("j", COMMAND | OPTION));
        assert_eq!(config.shortcuts.len(), 1);
        assert_eq!(config.shortcuts[0].display_name(), "setup");
        assert_eq!(config.shortcuts[0].trimmed_command(), "echo hi");
        assert_eq!(config.shortcuts[0].combo, KeyCombo::new("s", COMMAND));
    }

    #[test]
    fn reads_the_legacy_bare_array_with_the_default_prefix() {
        let path = write(
            "legacy",
            r#"[{ "id": "A", "name": "", "command": "ls",
                 "combo": { "key": "l", "modifiers": 1048576 } }]"#,
        );
        let config = CommandShortcuts::read(&path);
        assert_eq!(config.prefix_combo, KeyCombo::new("g", COMMAND));
        assert_eq!(config.shortcuts.len(), 1);
        assert_eq!(config.shortcuts[0].display_name(), "Command");
    }

    #[test]
    fn a_missing_shortcuts_key_yields_an_empty_list() {
        let path = write(
            "no-shortcuts",
            r#"{ "prefixCombo": { "key": "k", "modifiers": 1048576 } }"#,
        );
        let config = CommandShortcuts::read(&path);
        assert_eq!(config.prefix_combo, KeyCombo::new("k", COMMAND));
        assert!(config.shortcuts.is_empty());
    }

    #[test]
    fn a_missing_file_yields_the_defaults() {
        let config = CommandShortcuts::read(Path::new("/nonexistent/command-shortcuts.json"));
        assert_eq!(config.prefix_combo, KeyCombo::new("g", COMMAND));
        assert!(config.shortcuts.is_empty());
    }

    #[test]
    fn a_shortcut_without_modifiers_defaults_to_command() {
        let path = write(
            "modifier-default",
            r#"{ "shortcuts": [
                { "id": "A", "name": "n", "command": "c",
                  "combo": { "key": "s", "modifiers": 0 } }
            ] }"#,
        );
        let config = CommandShortcuts::read(&path);
        assert_eq!(config.shortcuts[0].combo, KeyCombo::new("s", COMMAND));
    }

    #[test]
    fn a_save_round_trips_and_matches_the_swift_format() {
        let path = write(
            "round-trip",
            r#"{ "prefixCombo": { "key": "g", "modifiers": 1048576 }, "shortcuts": [] }"#,
        );
        let mut config = CommandShortcuts::read(&path);
        let id = config.add();
        config.update(
            &id,
            Some("setup".to_owned()),
            Some("scripts/setup.sh".to_owned()),
        );
        config.set_combo(&id, KeyCombo::new("s", 0));
        config.save_to(&path).expect("saves");

        let contents = std::fs::read_to_string(&path).expect("read");
        assert!(contents.contains("\"prefixCombo\" : {"));
        assert!(contents.contains("scripts\\/setup.sh"));
        assert!(!contents.ends_with('\n'));

        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let reloaded = CommandShortcuts::read(&path);
        assert_eq!(reloaded.shortcuts.len(), 1);
        assert_eq!(reloaded.shortcuts[0].id, id);
        assert_eq!(reloaded.shortcuts[0].display_name(), "setup");
        assert_eq!(reloaded.shortcuts[0].trimmed_command(), "scripts/setup.sh");
        assert_eq!(reloaded.shortcuts[0].combo, KeyCombo::new("s", COMMAND));
        assert_eq!(reloaded.prefix_combo, KeyCombo::new("g", COMMAND));
    }

    #[test]
    fn conflicting_finds_a_duplicate_and_ignores_the_excluded_row() {
        let mut config = CommandShortcuts::default();
        let first = config.add();
        let second = config.add();
        config.set_combo(&first, KeyCombo::new("s", COMMAND));
        config.set_combo(&second, KeyCombo::new("d", COMMAND));

        let combo = KeyCombo::new("s", COMMAND);
        assert_eq!(
            config
                .conflicting(&combo, &second)
                .map(|row| row.id.clone()),
            Some(first.clone())
        );
        assert!(config.conflicting(&combo, &first).is_none());
        assert!(config.conflicting(&KeyCombo::new("", 0), &second).is_none());

        config.remove(&second);
        assert_eq!(config.shortcuts.len(), 1);
        config.remove_all();
        assert!(config.shortcuts.is_empty());
    }
}
