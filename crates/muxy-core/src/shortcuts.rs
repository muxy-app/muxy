use crate::prefs::app_support_dir;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const KEY_CONTEXT: &str = "WorkspaceTabs";

pub const SHIFT: u64 = 1 << 17;
pub const CONTROL: u64 = 1 << 18;
pub const OPTION: u64 = 1 << 19;
pub const COMMAND: u64 = 1 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutAction {
    NewTab,
    NewHomeTab,
    NewBrowserTab,
    CloseTab,
    RenameTab,
    PinUnpinTab,
    SplitRight,
    SplitDown,
    ClosePane,
    FocusPaneLeft,
    FocusPaneRight,
    FocusPaneUp,
    FocusPaneDown,
    MovePaneLeft,
    MovePaneRight,
    MovePaneUp,
    MovePaneDown,
    CycleNextTabAcrossPanes,
    CyclePreviousTabAcrossPanes,
    NextTab,
    PreviousTab,
    SelectTab1,
    SelectTab2,
    SelectTab3,
    SelectTab4,
    SelectTab5,
    SelectTab6,
    SelectTab7,
    SelectTab8,
    SelectTab9,
    ToggleMaximizePane,
    OpenProject,
    RecentlyRemovedProjects,
    RefreshWorktrees,
    CreateWorktree,
    NextProject,
    PreviousProject,
    SelectProject1,
    SelectProject2,
    SelectProject3,
    SelectProject4,
    SelectProject5,
    SelectProject6,
    SelectProject7,
    SelectProject8,
    SelectProject9,
    NavigateBack,
    NavigateForward,
    FindInTerminal,
    TerminalOmnibox,
    TerminalOmniboxProjects,
    TerminalOmniboxWorktrees,
    TerminalOmniboxWorkspaces,
    TerminalOmniboxCommands,
    ToggleSidebar,
    ToggleFullScreen,
    ToggleThemePicker,
    ReloadConfig,
}

pub const CATEGORIES: [&str; 10] = [
    "Tabs",
    "Panes",
    "Tab Navigation",
    "Project Navigation",
    "Navigation",
    "Browser",
    "Terminal",
    "Composer",
    "App",
    "Extensions",
];

impl ShortcutAction {
    pub fn display_name(self) -> &'static str {
        self.metadata().0
    }

    pub fn category(self) -> &'static str {
        self.metadata().1
    }

    fn metadata(self) -> (&'static str, &'static str) {
        use ShortcutAction::*;
        match self {
            NewTab => ("New Tab", "Tabs"),
            NewHomeTab => ("New Home Tab", "Tabs"),
            NewBrowserTab => ("New Browser Tab", "Tabs"),
            CloseTab => ("Close Tab", "Tabs"),
            RenameTab => ("Rename Tab", "Tabs"),
            PinUnpinTab => ("Pin/Unpin Tab", "Tabs"),
            SplitRight => ("Split Right", "Panes"),
            SplitDown => ("Split Down", "Panes"),
            ClosePane => ("Close Pane", "Panes"),
            FocusPaneLeft => ("Focus Pane Left", "Panes"),
            FocusPaneRight => ("Focus Pane Right", "Panes"),
            FocusPaneUp => ("Focus Pane Up", "Panes"),
            FocusPaneDown => ("Focus Pane Down", "Panes"),
            MovePaneLeft => ("Move Pane Left", "Panes"),
            MovePaneRight => ("Move Pane Right", "Panes"),
            MovePaneUp => ("Move Pane Up", "Panes"),
            MovePaneDown => ("Move Pane Down", "Panes"),
            ToggleMaximizePane => ("Toggle Maximize Pane", "Panes"),
            CycleNextTabAcrossPanes => ("Cycle Next Tab (All Panes)", "Tab Navigation"),
            CyclePreviousTabAcrossPanes => ("Cycle Previous Tab (All Panes)", "Tab Navigation"),
            NextTab => ("Next Tab", "Tab Navigation"),
            PreviousTab => ("Previous Tab", "Tab Navigation"),
            SelectTab1 => ("Tab 1", "Tab Navigation"),
            SelectTab2 => ("Tab 2", "Tab Navigation"),
            SelectTab3 => ("Tab 3", "Tab Navigation"),
            SelectTab4 => ("Tab 4", "Tab Navigation"),
            SelectTab5 => ("Tab 5", "Tab Navigation"),
            SelectTab6 => ("Tab 6", "Tab Navigation"),
            SelectTab7 => ("Tab 7", "Tab Navigation"),
            SelectTab8 => ("Tab 8", "Tab Navigation"),
            SelectTab9 => ("Tab 9", "Tab Navigation"),
            NextProject => ("Next Project", "Project Navigation"),
            PreviousProject => ("Previous Project", "Project Navigation"),
            SelectProject1 => ("Project 1", "Project Navigation"),
            SelectProject2 => ("Project 2", "Project Navigation"),
            SelectProject3 => ("Project 3", "Project Navigation"),
            SelectProject4 => ("Project 4", "Project Navigation"),
            SelectProject5 => ("Project 5", "Project Navigation"),
            SelectProject6 => ("Project 6", "Project Navigation"),
            SelectProject7 => ("Project 7", "Project Navigation"),
            SelectProject8 => ("Project 8", "Project Navigation"),
            SelectProject9 => ("Project 9", "Project Navigation"),
            NavigateBack => ("Navigate Back", "Navigation"),
            NavigateForward => ("Navigate Forward", "Navigation"),
            FindInTerminal => ("Find", "Terminal"),
            TerminalOmnibox => ("Terminal Omnibox Open Tabs", "Terminal"),
            TerminalOmniboxProjects => ("Terminal Omnibox Projects", "Terminal"),
            TerminalOmniboxWorktrees => ("Terminal Omnibox Worktrees", "Terminal"),
            TerminalOmniboxWorkspaces => ("Terminal Omnibox Workspaces", "Terminal"),
            TerminalOmniboxCommands => ("Terminal Omnibox Custom Commands", "Terminal"),
            OpenProject => ("Open Project", "App"),
            RecentlyRemovedProjects => ("Recently Removed Projects", "App"),
            RefreshWorktrees => ("Refresh Worktrees", "App"),
            CreateWorktree => ("New Worktree", "App"),
            ToggleSidebar => ("Toggle Sidebar", "App"),
            ToggleFullScreen => ("Toggle Full Screen", "App"),
            ToggleThemePicker => ("Theme Picker", "App"),
            ReloadConfig => ("Reload Configuration", "App"),
        }
    }
}

pub fn modelled_actions() -> Vec<ShortcutAction> {
    defaults().into_iter().map(|(action, _)| action).collect()
}

pub const UNMODELLED_DEFAULTS: [(&str, &str, u64); 9] = [
    ("removeCurrentWorktree", "", 0),
    ("toggleRichInput", "i", COMMAND),
    ("toggleComposerVoice", "", 0),
    ("submitRichInput", "return", COMMAND),
    ("submitRichInputWithoutReturn", "return", COMMAND | SHIFT),
    ("toggleAppLayout", "l", COMMAND | SHIFT),
    ("toggleVoiceRecording", "i", COMMAND | SHIFT),
    ("toggleExtensionConsole", "`", COMMAND),
    ("inspectElement", "i", COMMAND | OPTION),
];

pub fn default_bindings() -> Vec<(ShortcutAction, KeyCombo)> {
    defaults()
}

pub fn default_combo(action: ShortcutAction) -> KeyCombo {
    defaults()
        .into_iter()
        .find(|(candidate, _)| *candidate == action)
        .map(|(_, combo)| combo)
        .unwrap_or_else(|| KeyCombo::new("", 0))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct KeyCombo {
    pub key: String,
    pub modifiers: u64,
}

impl KeyCombo {
    pub fn new(key: &str, modifiers: u64) -> Self {
        Self {
            key: key.to_owned(),
            modifiers,
        }
    }

    pub fn is_assigned(&self) -> bool {
        !self.key.is_empty()
    }

    pub fn keystroke(&self) -> Option<String> {
        if !self.is_assigned() {
            return None;
        }
        let mut parts = Vec::new();
        if self.modifiers & CONTROL != 0 {
            parts.push("ctrl");
        }
        if self.modifiers & OPTION != 0 {
            parts.push("alt");
        }
        if self.modifiers & SHIFT != 0 {
            parts.push("shift");
        }
        if self.modifiers & COMMAND != 0 {
            parts.push(if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            });
        }
        parts.push(match self.key.as_str() {
            "leftarrow" => "left",
            "rightarrow" => "right",
            "uparrow" => "up",
            "downarrow" => "down",
            "return" => "enter",
            key => key,
        });
        Some(parts.join("-"))
    }

    pub fn display(&self) -> String {
        if !self.is_assigned() {
            return "Unassigned".to_owned();
        }
        if cfg!(target_os = "macos") {
            let mut value = String::new();
            if self.modifiers & CONTROL != 0 {
                value.push('⌃');
            }
            if self.modifiers & OPTION != 0 {
                value.push('⌥');
            }
            if self.modifiers & SHIFT != 0 {
                value.push('⇧');
            }
            if self.modifiers & COMMAND != 0 {
                value.push('⌘');
            }
            value.push_str(match self.key.as_str() {
                "leftarrow" => "←",
                "rightarrow" => "→",
                "uparrow" => "↑",
                "downarrow" => "↓",
                "tab" => "⇥",
                "return" => "↩",
                key => key,
            });
            return value;
        }
        self.keystroke().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct StoredBinding {
    action: ShortcutAction,
    combo: KeyCombo,
}

#[derive(Debug, Clone)]
pub struct ShortcutMap {
    bindings: HashMap<ShortcutAction, KeyCombo>,
    unknown: Vec<serde_json::Value>,
}

impl ShortcutMap {
    pub fn load() -> Self {
        Self::load_from(&app_support_dir().join("keybindings.json"))
    }

    fn load_from(path: &std::path::Path) -> Self {
        let entries = std::fs::read_to_string(path)
            .ok()
            .and_then(|contents| serde_json::from_str::<Vec<serde_json::Value>>(&contents).ok())
            .unwrap_or_default();
        let mut saved = HashMap::new();
        let mut unknown = Vec::new();
        for entry in entries {
            match serde_json::from_value::<StoredBinding>(entry.clone()) {
                Ok(binding) => {
                    saved.insert(binding.action, binding.combo);
                }
                Err(_) => unknown.push(entry),
            }
        }
        Self::merge_with_unknown(saved, unknown)
    }

    fn merge_with_unknown(
        saved: HashMap<ShortcutAction, KeyCombo>,
        unknown: Vec<serde_json::Value>,
    ) -> Self {
        let mut claimed = saved
            .values()
            .filter(|combo| combo.is_assigned())
            .cloned()
            .collect::<HashSet<_>>();
        claimed.extend(
            unknown
                .iter()
                .filter_map(|entry| {
                    serde_json::from_value::<KeyCombo>(entry.get("combo")?.clone()).ok()
                })
                .filter(KeyCombo::is_assigned),
        );
        let mut bindings = HashMap::new();
        for (action, default) in defaults() {
            let combo = if let Some(saved) = saved.get(&action) {
                saved.clone()
            } else if !default.is_assigned() || claimed.insert(default.clone()) {
                default
            } else {
                KeyCombo::new("", 0)
            };
            bindings.insert(action, combo);
        }
        Self { bindings, unknown }
    }

    pub fn set(&mut self, action: ShortcutAction, combo: KeyCombo) {
        self.bindings.insert(action, combo);
    }

    pub fn reset_to_defaults(&mut self) {
        self.bindings = defaults().into_iter().collect();
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&app_support_dir().join("keybindings.json"))?;
        crate::prefs::settings::sync();
        Ok(())
    }

    fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut entries: Vec<serde_json::Value> = Vec::new();
        for (action, _) in defaults() {
            let stored = StoredBinding {
                action,
                combo: self.combo(action).clone(),
            };
            entries.push(serde_json::to_value(&stored)?);
        }
        entries.extend(self.unknown.iter().cloned());
        let contents = crate::prefs::settings::to_foundation_json(
            &serde_json::Value::Array(entries),
            true,
            false,
        );
        crate::store::write_private(path, contents.as_bytes())
    }

    pub fn mirror_object(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut object = serde_json::Map::new();
        for (action, _) in defaults() {
            let Ok(serde_json::Value::String(name)) = serde_json::to_value(action) else {
                continue;
            };
            let Ok(combo) = serde_json::to_value(self.combo(action)) else {
                continue;
            };
            object.insert(name, combo);
        }
        for entry in &self.unknown {
            let Some(name) = entry.get("action").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let Some(combo) = entry.get("combo") else {
                continue;
            };
            object.insert(name.to_owned(), combo.clone());
        }
        object
    }

    pub fn combo(&self, action: ShortcutAction) -> &KeyCombo {
        self.bindings
            .get(&action)
            .expect("shortcut action has a default")
    }

    pub fn tooltip(&self, label: &str, action: ShortcutAction) -> String {
        let combo = self.combo(action);
        if combo.is_assigned() {
            format!("{label} ({})", combo.display())
        } else {
            label.to_owned()
        }
    }

    pub fn bindings(&self) -> &HashMap<ShortcutAction, KeyCombo> {
        &self.bindings
    }

    pub fn assigned_combos(&self) -> Vec<KeyCombo> {
        self.bindings
            .values()
            .filter(|combo| combo.is_assigned())
            .cloned()
            .collect()
    }
}

fn defaults() -> Vec<(ShortcutAction, KeyCombo)> {
    use ShortcutAction::*;
    vec![
        (NewTab, KeyCombo::new("t", COMMAND)),
        (NewHomeTab, KeyCombo::new("n", COMMAND)),
        (NewBrowserTab, KeyCombo::new("b", COMMAND | OPTION)),
        (CloseTab, KeyCombo::new("w", COMMAND)),
        (RenameTab, KeyCombo::new("", 0)),
        (PinUnpinTab, KeyCombo::new("", 0)),
        (SplitRight, KeyCombo::new("d", COMMAND)),
        (SplitDown, KeyCombo::new("d", COMMAND | SHIFT)),
        (ClosePane, KeyCombo::new("w", COMMAND | SHIFT)),
        (FocusPaneLeft, KeyCombo::new("leftarrow", COMMAND | OPTION)),
        (
            FocusPaneRight,
            KeyCombo::new("rightarrow", COMMAND | OPTION),
        ),
        (FocusPaneUp, KeyCombo::new("uparrow", COMMAND | OPTION)),
        (FocusPaneDown, KeyCombo::new("downarrow", COMMAND | OPTION)),
        (MovePaneLeft, KeyCombo::new("", 0)),
        (MovePaneRight, KeyCombo::new("", 0)),
        (MovePaneUp, KeyCombo::new("", 0)),
        (MovePaneDown, KeyCombo::new("", 0)),
        (CycleNextTabAcrossPanes, KeyCombo::new("tab", CONTROL)),
        (
            CyclePreviousTabAcrossPanes,
            KeyCombo::new("tab", CONTROL | SHIFT),
        ),
        (OpenProject, KeyCombo::new("o", COMMAND)),
        (RecentlyRemovedProjects, KeyCombo::new("", 0)),
        (RefreshWorktrees, KeyCombo::new("r", COMMAND | OPTION)),
        (CreateWorktree, KeyCombo::new("n", COMMAND | OPTION)),
        (NextTab, KeyCombo::new("]", COMMAND)),
        (PreviousTab, KeyCombo::new("[", COMMAND)),
        (SelectTab1, KeyCombo::new("1", COMMAND)),
        (SelectTab2, KeyCombo::new("2", COMMAND)),
        (SelectTab3, KeyCombo::new("3", COMMAND)),
        (SelectTab4, KeyCombo::new("4", COMMAND)),
        (SelectTab5, KeyCombo::new("5", COMMAND)),
        (SelectTab6, KeyCombo::new("6", COMMAND)),
        (SelectTab7, KeyCombo::new("7", COMMAND)),
        (SelectTab8, KeyCombo::new("8", COMMAND)),
        (SelectTab9, KeyCombo::new("9", COMMAND)),
        (NextProject, KeyCombo::new("]", CONTROL)),
        (PreviousProject, KeyCombo::new("[", CONTROL)),
        (SelectProject1, KeyCombo::new("1", CONTROL)),
        (SelectProject2, KeyCombo::new("2", CONTROL)),
        (SelectProject3, KeyCombo::new("3", CONTROL)),
        (SelectProject4, KeyCombo::new("4", CONTROL)),
        (SelectProject5, KeyCombo::new("5", CONTROL)),
        (SelectProject6, KeyCombo::new("6", CONTROL)),
        (SelectProject7, KeyCombo::new("7", CONTROL)),
        (SelectProject8, KeyCombo::new("8", CONTROL)),
        (SelectProject9, KeyCombo::new("9", CONTROL)),
        (NavigateBack, KeyCombo::new("leftarrow", COMMAND | CONTROL)),
        (
            NavigateForward,
            KeyCombo::new("rightarrow", COMMAND | CONTROL),
        ),
        (FindInTerminal, KeyCombo::new("f", COMMAND)),
        (TerminalOmnibox, KeyCombo::new("o", COMMAND | OPTION)),
        (
            TerminalOmniboxProjects,
            KeyCombo::new("p", COMMAND | OPTION),
        ),
        (
            TerminalOmniboxWorktrees,
            KeyCombo::new("w", COMMAND | OPTION),
        ),
        (
            TerminalOmniboxWorkspaces,
            KeyCombo::new("s", COMMAND | OPTION),
        ),
        (TerminalOmniboxCommands, KeyCombo::new("p", COMMAND | SHIFT)),
        (ToggleSidebar, KeyCombo::new("b", COMMAND)),
        (
            ToggleMaximizePane,
            KeyCombo::new("return", COMMAND | OPTION),
        ),
        (ToggleFullScreen, KeyCombo::new("f", COMMAND | CONTROL)),
        (ToggleThemePicker, KeyCombo::new("k", COMMAND | SHIFT)),
        (ReloadConfig, KeyCombo::new("r", COMMAND | SHIFT)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_swift_modifier_bits() {
        let combo = KeyCombo::new("d", COMMAND | SHIFT);
        let expected = if cfg!(target_os = "macos") {
            "shift-cmd-d"
        } else {
            "shift-ctrl-d"
        };
        assert_eq!(combo.keystroke().as_deref(), Some(expected));
    }

    #[test]
    fn saved_conflicts_disable_missing_defaults() {
        let mut saved = HashMap::new();
        saved.insert(ShortcutAction::NewTab, KeyCombo::new("w", COMMAND));
        let map = ShortcutMap::merge_with_unknown(saved, Vec::new());
        assert!(!map.combo(ShortcutAction::CloseTab).is_assigned());
    }

    #[test]
    fn new_defaults_do_not_shadow_saved_custom_bindings() {
        let mut saved = HashMap::new();
        saved.insert(
            ShortcutAction::TerminalOmniboxCommands,
            KeyCombo::new("n", COMMAND | OPTION),
        );
        let map = ShortcutMap::merge_with_unknown(saved, Vec::new());
        assert_eq!(
            map.combo(ShortcutAction::TerminalOmniboxCommands),
            &KeyCombo::new("n", COMMAND | OPTION)
        );

        let mut saved = HashMap::new();
        saved.insert(
            ShortcutAction::TerminalOmniboxCommands,
            KeyCombo::new("b", COMMAND | OPTION),
        );
        let map = ShortcutMap::merge_with_unknown(saved, Vec::new());
        assert_eq!(
            map.combo(ShortcutAction::TerminalOmniboxCommands),
            &KeyCombo::new("b", COMMAND | OPTION)
        );
        assert!(!map.combo(ShortcutAction::NewBrowserTab).is_assigned());
    }

    #[test]
    fn phase_three_navigation_defaults_match_swift() {
        use ShortcutAction::*;
        let map = ShortcutMap::merge_with_unknown(HashMap::new(), Vec::new());
        let expected = [
            (OpenProject, KeyCombo::new("o", COMMAND)),
            (RecentlyRemovedProjects, KeyCombo::new("", 0)),
            (NextProject, KeyCombo::new("]", CONTROL)),
            (PreviousProject, KeyCombo::new("[", CONTROL)),
            (SelectProject1, KeyCombo::new("1", CONTROL)),
            (SelectProject2, KeyCombo::new("2", CONTROL)),
            (SelectProject3, KeyCombo::new("3", CONTROL)),
            (SelectProject4, KeyCombo::new("4", CONTROL)),
            (SelectProject5, KeyCombo::new("5", CONTROL)),
            (SelectProject6, KeyCombo::new("6", CONTROL)),
            (SelectProject7, KeyCombo::new("7", CONTROL)),
            (SelectProject8, KeyCombo::new("8", CONTROL)),
            (SelectProject9, KeyCombo::new("9", CONTROL)),
            (FindInTerminal, KeyCombo::new("f", COMMAND)),
            (TerminalOmnibox, KeyCombo::new("o", COMMAND | OPTION)),
            (
                TerminalOmniboxProjects,
                KeyCombo::new("p", COMMAND | OPTION),
            ),
            (
                TerminalOmniboxWorktrees,
                KeyCombo::new("w", COMMAND | OPTION),
            ),
            (
                TerminalOmniboxWorkspaces,
                KeyCombo::new("s", COMMAND | OPTION),
            ),
            (TerminalOmniboxCommands, KeyCombo::new("p", COMMAND | SHIFT)),
            (ToggleSidebar, KeyCombo::new("b", COMMAND)),
            (ToggleFullScreen, KeyCombo::new("f", COMMAND | CONTROL)),
            (ToggleThemePicker, KeyCombo::new("k", COMMAND | SHIFT)),
            (ReloadConfig, KeyCombo::new("r", COMMAND | SHIFT)),
            (NavigateBack, KeyCombo::new("leftarrow", COMMAND | CONTROL)),
            (
                NavigateForward,
                KeyCombo::new("rightarrow", COMMAND | CONTROL),
            ),
        ];
        assert_eq!(modelled_actions().len(), 58);
        assert_eq!(UNMODELLED_DEFAULTS.len(), 9);
        assert_eq!(
            default_combo(ShortcutAction::RefreshWorktrees),
            KeyCombo::new("r", COMMAND | OPTION)
        );
        assert_eq!(
            default_combo(ShortcutAction::CreateWorktree),
            KeyCombo::new("n", COMMAND | OPTION)
        );
        assert_eq!(expected.len(), 25);
        for (action, combo) in expected {
            assert_eq!(map.combo(action), &combo, "{action:?}");
        }
    }

    #[test]
    fn saving_preserves_entries_this_build_does_not_model() {
        let dir = std::env::temp_dir().join("muxy-keybindings-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("keybindings.json");
        std::fs::write(
            &path,
            r#"[
                {"action":"newTab","combo":{"key":"t","modifiers":1048576}},
                {"action":"inspectElement","combo":{"key":"d","modifiers":1179648}},
                {"action":"submitRichInput","combo":{"key":"return","modifiers":1048576}}
            ]"#,
        )
        .expect("write fixture");

        let mut map = ShortcutMap::load_from(&path);
        assert_eq!(map.unknown.len(), 2);
        assert!(!map.combo(ShortcutAction::SplitDown).is_assigned());

        map.set(ShortcutAction::NewTab, KeyCombo::new("t", COMMAND | SHIFT));
        map.save_to(&path).expect("saves");

        let entries: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("json");
        let unmodelled: Vec<&serde_json::Value> = entries
            .iter()
            .filter(|entry| {
                matches!(
                    entry.get("action").and_then(serde_json::Value::as_str),
                    Some("inspectElement") | Some("submitRichInput")
                )
            })
            .collect();
        assert_eq!(unmodelled.len(), 2);
        assert_eq!(unmodelled[0]["combo"]["key"], "d");
        assert_eq!(unmodelled[0]["combo"]["modifiers"], 1179648);
        assert_eq!(unmodelled[1]["combo"]["key"], "return");

        let reloaded = ShortcutMap::load_from(&path);
        assert_eq!(
            reloaded.combo(ShortcutAction::NewTab),
            &KeyCombo::new("t", COMMAND | SHIFT)
        );
        assert!(!reloaded.combo(ShortcutAction::SplitDown).is_assigned());
    }

    #[test]
    fn the_mirror_object_carries_every_modelled_and_unmodelled_binding() {
        let dir = std::env::temp_dir().join("muxy-shortcuts-mirror");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("keybindings.json");
        std::fs::write(
            &path,
            r#"[
                {"action":"newTab","combo":{"key":"t","modifiers":1048576}},
                {"action":"inspectElement","combo":{"key":"i","modifiers":1572864}}
            ]"#,
        )
        .expect("write fixture");

        let map = ShortcutMap::load_from(&path);
        let object = map.mirror_object();
        assert_eq!(object.len(), modelled_actions().len() + 1);
        assert_eq!(
            object["inspectElement"],
            serde_json::json!({ "key": "i", "modifiers": 1572864 })
        );
        assert_eq!(
            object["renameTab"],
            serde_json::json!({ "key": "", "modifiers": 0 })
        );
    }

    #[test]
    fn keybindings_json_is_written_in_foundations_format() {
        let dir = std::env::temp_dir().join("muxy-shortcuts-format");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("keybindings.json");

        let map = ShortcutMap::merge_with_unknown(HashMap::new(), Vec::new());
        map.save_to(&path).expect("saves");
        let contents = std::fs::read_to_string(&path).expect("read");
        assert!(contents.starts_with("[\n  {\n    \"action\" : "));
        assert!(!contents.ends_with('\n'));

        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn action_names_match_swift_raw_values() {
        let cases = [
            ("openProject", ShortcutAction::OpenProject),
            (
                "recentlyRemovedProjects",
                ShortcutAction::RecentlyRemovedProjects,
            ),
            ("nextProject", ShortcutAction::NextProject),
            ("previousProject", ShortcutAction::PreviousProject),
            ("selectProject1", ShortcutAction::SelectProject1),
            ("selectProject9", ShortcutAction::SelectProject9),
            ("findInTerminal", ShortcutAction::FindInTerminal),
            ("terminalOmnibox", ShortcutAction::TerminalOmnibox),
            (
                "terminalOmniboxProjects",
                ShortcutAction::TerminalOmniboxProjects,
            ),
            (
                "terminalOmniboxWorktrees",
                ShortcutAction::TerminalOmniboxWorktrees,
            ),
            (
                "terminalOmniboxWorkspaces",
                ShortcutAction::TerminalOmniboxWorkspaces,
            ),
            (
                "terminalOmniboxCommands",
                ShortcutAction::TerminalOmniboxCommands,
            ),
            ("toggleSidebar", ShortcutAction::ToggleSidebar),
            ("toggleFullScreen", ShortcutAction::ToggleFullScreen),
            ("toggleThemePicker", ShortcutAction::ToggleThemePicker),
            ("reloadConfig", ShortcutAction::ReloadConfig),
            ("navigateBack", ShortcutAction::NavigateBack),
            ("navigateForward", ShortcutAction::NavigateForward),
        ];
        for (raw, action) in cases {
            let decoded: ShortcutAction =
                serde_json::from_value(serde_json::Value::String(raw.to_owned()))
                    .unwrap_or_else(|_| panic!("{raw} decodes"));
            assert_eq!(decoded, action);
        }
    }
}
