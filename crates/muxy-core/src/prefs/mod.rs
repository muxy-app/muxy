mod defaults;
pub mod settings;

use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

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
        let Some(domain_name) = defaults::domain_name() else {
            return;
        };
        let path = home_dir().join(format!("Library/Preferences/{domain_name}.plist"));
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

const TEST_APP_SUPPORT_DIRECTORY: &str = "MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY";

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn app_support_dir() -> PathBuf {
    let executable_is_test = std::env::current_exe()
        .ok()
        .as_deref()
        .is_some_and(executable_is_test_process);
    let is_test = cfg!(test) || executable_is_test;
    let override_directory = is_test
        .then(|| std::env::var_os(TEST_APP_SUPPORT_DIRECTORY))
        .flatten();
    resolve_app_support_dir(
        &home_dir(),
        is_test,
        override_directory.as_deref(),
        &std::env::temp_dir(),
        std::process::id(),
    )
}

fn executable_is_test_process(path: &Path) -> bool {
    let Some(name) = path.file_stem().and_then(OsStr::to_str) else {
        return false;
    };
    if name.ends_with("Tests") {
        return true;
    }
    let Some((target, hash)) = name.rsplit_once('-') else {
        return false;
    };
    !target.is_empty()
        && hash.len() == 16
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        && path.parent().and_then(Path::file_name) == Some(OsStr::new("deps"))
}

fn resolve_app_support_dir(
    home: &Path,
    is_test: bool,
    override_directory: Option<&OsStr>,
    temporary_directory: &Path,
    process_id: u32,
) -> PathBuf {
    if is_test {
        if let Some(override_directory) = override_directory.filter(|path| !path.is_empty()) {
            return PathBuf::from(override_directory);
        }
        return temporary_directory.join(format!("MuxyTests-{process_id}"));
    }
    home.join("Library/Application Support/Muxy")
}

pub fn read_json(path: &PathBuf) -> Option<Value> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    use super::{ScalePreset, executable_is_test_process, resolve_app_support_dir};

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

    #[test]
    fn normal_app_support_uses_the_user_home_and_ignores_test_override() {
        assert_eq!(
            resolve_app_support_dir(
                Path::new("/Users/example"),
                false,
                Some(OsStr::new("/project/test-state")),
                Path::new("/tmp"),
                42,
            ),
            Path::new("/Users/example/Library/Application Support/Muxy")
        );
    }

    #[test]
    fn test_app_support_uses_a_nonempty_override() {
        assert_eq!(
            resolve_app_support_dir(
                Path::new("/Users/example"),
                true,
                Some(OsStr::new("/project/test-state")),
                Path::new("/tmp"),
                42,
            ),
            Path::new("/project/test-state")
        );
    }

    #[test]
    fn test_app_support_falls_back_to_a_process_specific_temporary_directory() {
        for override_directory in [None, Some(OsStr::new(""))] {
            assert_eq!(
                resolve_app_support_dir(
                    Path::new("/Users/example"),
                    true,
                    override_directory,
                    Path::new("/tmp"),
                    42,
                ),
                Path::new("/tmp/MuxyTests-42")
            );
        }
    }

    #[test]
    fn only_staged_and_cargo_test_executables_are_recognized() {
        assert!(executable_is_test_process(Path::new(
            "/project/target/p1/MuxyTests.app/Contents/MacOS/MuxyTests"
        )));
        assert!(executable_is_test_process(Path::new(
            "/project/target/debug/deps/muxy-0123456789abcdef"
        )));
        assert!(executable_is_test_process(Path::new(
            "/project/target/release/deps/integration_test-fedcba9876543210.exe"
        )));
        for path in [
            "/project/target/debug/muxy",
            "/project/target/release/Muxy",
            "/project/target/debug/MuxyTest",
            "/project/target/debug/TestsMuxy",
            "/project/target/debug/muxy-0123456789abcdef",
            "/project/target/debug/deps/muxy-not-a-hash",
            "/project/target/debug/deps/muxy-0123456789abcde",
        ] {
            assert!(!executable_is_test_process(Path::new(path)), "{path}");
        }
    }
}
