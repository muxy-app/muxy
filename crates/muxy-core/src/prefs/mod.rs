mod defaults;
pub mod settings;

use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalePreset {
    Regular,
    Large,
    ExtraLarge,
    Huge,
}

impl ScalePreset {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "large" => Self::Large,
            "extraLarge" => Self::ExtraLarge,
            "huge" => Self::Huge,
            _ => Self::Regular,
        }
    }

    pub fn raw(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Large => "large",
            Self::ExtraLarge => "extraLarge",
            Self::Huge => "huge",
        }
    }

    pub fn multiplier(self) -> f32 {
        match self {
            Self::Regular => 1.00,
            Self::Large => 1.12,
            Self::ExtraLarge => 1.24,
            Self::Huge => 1.40,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollapsedStyle {
    Hidden,
    Icons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Manual,
    NameAscending,
    NameDescending,
    RecentlyActive,
    DateCreated,
}

impl SortMode {
    fn parse(raw: &str) -> Self {
        match raw {
            "nameAscending" => Self::NameAscending,
            "nameDescending" => Self::NameDescending,
            "recentlyActive" => Self::RecentlyActive,
            "dateCreated" => Self::DateCreated,
            _ => Self::Manual,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandedStyle {
    Icons,
    Wide,
}

#[derive(Debug, Clone)]
pub struct Prefs {
    pub scale: ScalePreset,
    pub dark_theme: String,
    pub light_theme: String,
    pub show_home_project: bool,
    pub show_status_bar: bool,
    pub show_topbar_actions: bool,
    pub show_project_search: bool,
    pub show_tips: bool,
    pub browser_enabled: bool,
    pub keep_projects_open: bool,
    pub tab_max_width: f32,
    pub collapsed_style: CollapsedStyle,
    pub expanded_style: ExpandedStyle,
    pub sidebar_expanded: bool,
    pub sidebar_expanded_custom_width: Option<f32>,
    pub active_project_id: Option<String>,
    pub ide_bundle_identifier: Option<String>,
    pub sort_mode: SortMode,
    pub project_search_root: Option<String>,
    pub active_worktree_ids: std::collections::HashMap<String, String>,
    pub active_group_id: Option<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            scale: ScalePreset::Regular,
            dark_theme: "Muxy".into(),
            light_theme: "Muxy Light".into(),
            show_home_project: true,
            show_status_bar: true,
            show_topbar_actions: true,
            show_project_search: false,
            show_tips: true,
            browser_enabled: true,
            keep_projects_open: false,
            tab_max_width: 200.0,
            collapsed_style: CollapsedStyle::Icons,
            expanded_style: ExpandedStyle::Wide,
            sidebar_expanded: false,
            sidebar_expanded_custom_width: None,
            active_project_id: None,
            ide_bundle_identifier: None,
            sort_mode: SortMode::Manual,
            project_search_root: None,
            active_worktree_ids: std::collections::HashMap::new(),
            active_group_id: None,
        }
    }
}

impl Prefs {
    pub fn load() -> Self {
        let mut prefs = Self::default();
        prefs.apply_settings_json();
        prefs.apply_ui_scale_json();
        prefs.apply_user_defaults();
        prefs
    }

    fn apply_settings_json(&mut self) {
        let Some(root) = read_json(&app_support_dir().join("settings.json")) else {
            return;
        };
        let string = |key: &str| root.get(key).and_then(Value::as_str).map(str::to_owned);
        let flag = |key: &str| root.get(key).and_then(Value::as_bool);
        let number = |key: &str| root.get(key).and_then(Value::as_f64);

        if let Some(theme) = string("muxy.theme.dark") {
            self.dark_theme = theme;
        }
        if let Some(theme) = string("muxy.theme.light") {
            self.light_theme = theme;
        }
        if let Some(value) = flag("muxy.showHomeProject") {
            self.show_home_project = value;
        }
        if let Some(value) = flag("muxy.showStatusBar") {
            self.show_status_bar = value;
        }
        if let Some(value) = flag("muxy.showTopBarActions") {
            self.show_topbar_actions = value;
        }
        if let Some(value) = flag("muxy.showProjectSearch") {
            self.show_project_search = value;
        }
        if let Some(value) = flag("muxy.showTips") {
            self.show_tips = value;
        }
        if let Some(value) = flag("muxy.browser.enabled") {
            self.browser_enabled = value;
        }
        if let Some(value) = flag("muxy.projects.keepOpenWhenNoTabs") {
            self.keep_projects_open = value;
        }
        if let Some(value) =
            number("muxy.tabs.maxWidth").filter(|value| value.is_finite() && *value >= 0.0)
        {
            self.tab_max_width = value as f32;
        }
        if let Some(style) = string("muxy.sidebarCollapsedStyle") {
            self.collapsed_style = match style.as_str() {
                "hidden" => CollapsedStyle::Hidden,
                _ => CollapsedStyle::Icons,
            };
        }
        if let Some(style) = string("muxy.sidebarExpandedStyle") {
            self.expanded_style = match style.as_str() {
                "icons" => ExpandedStyle::Icons,
                _ => ExpandedStyle::Wide,
            };
        }
    }

    fn apply_ui_scale_json(&mut self) {
        let Some(root) = read_json(&app_support_dir().join("ui-scale.json")) else {
            return;
        };
        let Some(preset) = root.get("preset").and_then(Value::as_str) else {
            return;
        };
        self.scale = ScalePreset::parse(preset);
    }

    fn apply_user_defaults(&mut self) {
        let path = home_dir().join("Library/Preferences/com.muxy.app.plist");
        let Ok(plist::Value::Dictionary(defaults)) = plist::Value::from_file(&path) else {
            return;
        };
        if let Some(id) = defaults
            .get("muxy.activeProjectID")
            .and_then(|v| v.as_string())
        {
            self.active_project_id = Some(id.to_owned());
        }
        if let Some(id) = defaults
            .get("muxy.ide.selectedBundleIdentifier")
            .and_then(|value| value.as_string())
        {
            self.ide_bundle_identifier = Some(id.to_owned());
        }
        if let Some(expanded) = defaults.get("muxy.sidebarExpanded").and_then(as_bool) {
            self.sidebar_expanded = expanded;
        }
        if let Some(width) = defaults
            .get("muxy.sidebarExpandedCustomWidth")
            .and_then(|value| value.as_real())
        {
            self.sidebar_expanded_custom_width = Some(width as f32);
        }
        if let Some(mode) = defaults
            .get("muxy.projectSortMode")
            .and_then(|value| value.as_string())
        {
            self.sort_mode = SortMode::parse(mode);
        }
        if let Some(value) = defaults.get("muxy.browser.enabled").and_then(as_bool) {
            self.browser_enabled = value;
        }
        if let Some(value) = defaults
            .get("muxy.projects.keepOpenWhenNoTabs")
            .and_then(as_bool)
        {
            self.keep_projects_open = value;
        }
        if let Some(value) = defaults
            .get("muxy.tabs.maxWidth")
            .and_then(|value| value.as_real())
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            self.tab_max_width = value as f32;
        }
        if let Some(root) = defaults
            .get("muxy.projectPicker.defaultDirectory")
            .and_then(|value| value.as_string())
            .filter(|value| !value.trim().is_empty())
        {
            self.project_search_root = Some(root.to_owned());
        }
        if let Some(id) = defaults
            .get("muxy.activeProjectGroupID")
            .and_then(|value| value.as_string())
            .filter(|value| !value.trim().is_empty())
        {
            self.active_group_id = Some(id.to_owned());
        }
        if let Some(plist::Value::Dictionary(entries)) = defaults.get("muxy.activeWorktreeIDs") {
            self.active_worktree_ids = entries
                .iter()
                .filter_map(|(project_id, worktree_id)| {
                    Some((project_id.clone(), worktree_id.as_string()?.to_owned()))
                })
                .collect();
        }
    }

    pub fn store_settings_value(key: &str, value: Value) {
        settings::set(key, value);
    }

    pub fn store_default(key: &str, value: Option<&str>) {
        defaults::store_string(key, value);
    }

    pub fn store_active_worktree_ids(value: &std::collections::HashMap<String, String>) {
        defaults::store_dictionary("muxy.activeWorktreeIDs", value);
    }
}

fn as_bool(value: &plist::Value) -> Option<bool> {
    value
        .as_boolean()
        .or_else(|| value.as_signed_integer().map(|number| number != 0))
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn app_support_dir() -> PathBuf {
    home_dir().join("Library/Application Support/Muxy")
}

pub fn read_json(path: &PathBuf) -> Option<Value> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

#[cfg(test)]
mod tests {
    use super::ScalePreset;

    #[test]
    fn parsing_a_raw_preset_is_the_identity() {
        for preset in [
            ScalePreset::Regular,
            ScalePreset::Large,
            ScalePreset::ExtraLarge,
            ScalePreset::Huge,
        ] {
            assert_eq!(ScalePreset::parse(preset.raw()), preset);
        }
    }
}
