pub(crate) mod defaults;
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
    pub const ALL: [Self; 5] = [
        Self::Manual,
        Self::NameAscending,
        Self::NameDescending,
        Self::RecentlyActive,
        Self::DateCreated,
    ];

    pub fn parse(raw: &str) -> Self {
        match raw {
            "nameAscending" => Self::NameAscending,
            "nameDescending" => Self::NameDescending,
            "recentlyActive" => Self::RecentlyActive,
            "dateCreated" => Self::DateCreated,
            _ => Self::Manual,
        }
    }

    pub fn raw(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::NameAscending => "nameAscending",
            Self::NameDescending => "nameDescending",
            Self::RecentlyActive => "recentlyActive",
            Self::DateCreated => "dateCreated",
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
        prefs.apply_portable_preferences();
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

    fn apply_portable_preferences(&mut self) {
        if let Some(id) = defaults::read_string("muxy.activeProjectID") {
            self.active_project_id = Some(id);
        }
        if let Some(id) = defaults::read_string("muxy.ide.selectedBundleIdentifier") {
            self.ide_bundle_identifier = Some(id);
        }
        if let Some(expanded) = defaults::read_bool("muxy.sidebarExpanded") {
            self.sidebar_expanded = expanded;
        }
        if let Some(width) = defaults::read_f64("muxy.sidebarExpandedCustomWidth") {
            self.sidebar_expanded_custom_width = Some(width as f32);
        }
        if let Some(mode) = defaults::read_string("muxy.projectSortMode") {
            self.sort_mode = SortMode::parse(&mode);
        }
        if let Some(value) = defaults::read_bool("muxy.browser.enabled") {
            self.browser_enabled = value;
        }
        if let Some(value) = defaults::read_bool("muxy.projects.keepOpenWhenNoTabs") {
            self.keep_projects_open = value;
        }
        if let Some(value) = defaults::read_f64("muxy.tabs.maxWidth")
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            self.tab_max_width = value as f32;
        }
        if let Some(root) = defaults::read_string("muxy.projectPicker.defaultDirectory")
            .filter(|value| !value.trim().is_empty())
        {
            self.project_search_root = Some(root);
        }
        if let Some(id) = defaults::read_string("muxy.activeProjectGroupID")
            .filter(|value| !value.trim().is_empty())
        {
            self.active_group_id = Some(id);
        }
        if let Some(entries) = defaults::read_dictionary("muxy.activeWorktreeIDs") {
            self.active_worktree_ids = entries;
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

    pub fn try_store_active_worktree_ids(
        value: &std::collections::HashMap<String, String>,
    ) -> std::io::Result<()> {
        defaults::try_store_dictionary("muxy.activeWorktreeIDs", value)
    }
}

const TEST_APP_SUPPORT_DIRECTORY: &str = "MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY";

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub fn app_support_dir() -> PathBuf {
    let is_test = is_test_process();
    let override_directory = is_test
        .then(|| std::env::var_os(TEST_APP_SUPPORT_DIRECTORY))
        .flatten();
    resolve_app_support_dir(
        &home_dir(),
        crate::build_mode!(),
        is_test,
        override_directory.as_deref(),
        &std::env::temp_dir(),
        std::process::id(),
    )
}

pub fn is_test_process() -> bool {
    cfg!(test)
        || std::env::current_exe()
            .ok()
            .as_deref()
            .is_some_and(executable_is_test_process)
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
    mode: crate::environment::BuildMode,
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
    crate::environment::StoragePathPolicy::new(mode).root(home)
}

pub fn read_json(path: &PathBuf) -> Option<Value> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;

    use crate::environment::BuildMode;

    use super::{ScalePreset, SortMode, executable_is_test_process, resolve_app_support_dir};

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
    fn sort_modes_round_trip_all_five_raw_values() {
        assert_eq!(SortMode::ALL.len(), 5);
        assert_eq!(
            SortMode::ALL.map(SortMode::raw),
            [
                "manual",
                "nameAscending",
                "nameDescending",
                "recentlyActive",
                "dateCreated",
            ]
        );
        for mode in SortMode::ALL {
            assert_eq!(SortMode::parse(mode.raw()), mode);
        }
    }

    #[test]
    fn normal_storage_path_uses_mode_specific_roots_and_ignores_test_override() {
        let home = Path::new("/Users/example");
        let override_directory = Some(OsStr::new("/project/test-state"));
        assert_eq!(
            resolve_app_support_dir(
                home,
                BuildMode::Development,
                false,
                override_directory,
                Path::new("/tmp"),
                42,
            ),
            Path::new("/Users/example/.muxy-dev")
        );
        assert_eq!(
            resolve_app_support_dir(
                home,
                BuildMode::Production,
                false,
                override_directory,
                Path::new("/tmp"),
                42,
            ),
            Path::new("/Users/example/.muxy")
        );
    }

    #[test]
    fn storage_path_uses_a_nonempty_test_override() {
        assert_eq!(
            resolve_app_support_dir(
                Path::new("/Users/example"),
                BuildMode::Development,
                true,
                Some(OsStr::new("/project/test-state")),
                Path::new("/tmp"),
                42,
            ),
            Path::new("/project/test-state")
        );
    }

    #[test]
    fn storage_path_falls_back_to_a_process_specific_test_directory() {
        for override_directory in [None, Some(OsStr::new(""))] {
            assert_eq!(
                resolve_app_support_dir(
                    Path::new("/Users/example"),
                    BuildMode::Production,
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
