pub(crate) mod defaults;
pub mod settings;

use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::repository_ai::{
    COMMIT_PROMPT, COMMIT_PROMPT_KEY, COMMIT_PROVIDER_KEY, CREATE_PULL_REQUEST_PROMPT,
    CREATE_PULL_REQUEST_PROMPT_KEY, CREATE_PULL_REQUEST_PROVIDER_KEY, RepositoryAiPreferences,
};

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

pub const COMPOSER_PANEL_MODE_KEY: &str = "muxy.panel.mode.builtin:richInput";
pub const COMPOSER_POSITION_KEY: &str = "muxy.richInput.position";
pub const COMPOSER_PANEL_WIDTH_KEY: &str = "muxy.richInputPanelWidth";
pub const COMPOSER_PANEL_HEIGHT_KEY: &str = "muxy.richInputPanelHeight";
pub const COMPOSER_BROADCAST_KEY: &str = "muxy.richInput.broadcast";
pub const COMPOSER_FONT_SIZE_KEY: &str = "muxy.richInput.fontSize";
pub const COMPOSER_RIGHT_WIDTH_MIN: f64 = 280.0;
pub const COMPOSER_RIGHT_WIDTH_MAX: f64 = 800.0;
pub const COMPOSER_RIGHT_WIDTH_DEFAULT: f64 = 380.0;
pub const COMPOSER_BOTTOM_HEIGHT_MIN: f64 = 120.0;
pub const COMPOSER_BOTTOM_HEIGHT_MAX: f64 = 600.0;
pub const COMPOSER_BOTTOM_HEIGHT_DEFAULT: f64 = 220.0;
pub const COMPOSER_FONT_SIZE_MIN: f64 = 9.0;
pub const COMPOSER_FONT_SIZE_MAX: f64 = 32.0;
pub const COMPOSER_FONT_SIZE_DEFAULT: f64 = 13.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerPanelMode {
    Floating,
    Pinned,
}

impl ComposerPanelMode {
    pub fn parse(value: &str) -> Self {
        match value {
            "pinned" => Self::Pinned,
            _ => Self::Floating,
        }
    }

    pub const fn raw(self) -> &'static str {
        match self {
            Self::Floating => "floating",
            Self::Pinned => "pinned",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerPanelPosition {
    Right,
    Bottom,
}

impl ComposerPanelPosition {
    pub fn parse(value: &str) -> Self {
        match value {
            "bottom" => Self::Bottom,
            _ => Self::Right,
        }
    }

    pub const fn raw(self) -> &'static str {
        match self {
            Self::Right => "right",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComposerPreferences {
    pub panel_mode: ComposerPanelMode,
    pub position: ComposerPanelPosition,
    pub panel_width: f64,
    pub panel_height: f64,
    pub broadcast: bool,
    pub font_size: f64,
}

impl Default for ComposerPreferences {
    fn default() -> Self {
        Self {
            panel_mode: ComposerPanelMode::Floating,
            position: ComposerPanelPosition::Right,
            panel_width: COMPOSER_RIGHT_WIDTH_DEFAULT,
            panel_height: COMPOSER_BOTTOM_HEIGHT_DEFAULT,
            broadcast: false,
            font_size: COMPOSER_FONT_SIZE_DEFAULT,
        }
    }
}

impl ComposerPreferences {
    pub fn load() -> Self {
        Self::from_values(
            defaults::read_string(COMPOSER_PANEL_MODE_KEY).as_deref(),
            defaults::read_string(COMPOSER_POSITION_KEY).as_deref(),
            defaults::read_f64(COMPOSER_PANEL_WIDTH_KEY),
            defaults::read_f64(COMPOSER_PANEL_HEIGHT_KEY),
            defaults::read_bool(COMPOSER_BROADCAST_KEY),
            defaults::read_f64(COMPOSER_FONT_SIZE_KEY),
        )
    }

    fn from_values(
        panel_mode: Option<&str>,
        position: Option<&str>,
        panel_width: Option<f64>,
        panel_height: Option<f64>,
        broadcast: Option<bool>,
        font_size: Option<f64>,
    ) -> Self {
        let fallback = Self::default();
        Self {
            panel_mode: panel_mode
                .map(ComposerPanelMode::parse)
                .unwrap_or(fallback.panel_mode),
            position: position
                .map(ComposerPanelPosition::parse)
                .unwrap_or(fallback.position),
            panel_width: finite_clamped(
                panel_width,
                COMPOSER_RIGHT_WIDTH_MIN,
                COMPOSER_RIGHT_WIDTH_MAX,
                fallback.panel_width,
            ),
            panel_height: finite_clamped(
                panel_height,
                COMPOSER_BOTTOM_HEIGHT_MIN,
                COMPOSER_BOTTOM_HEIGHT_MAX,
                fallback.panel_height,
            ),
            broadcast: broadcast.unwrap_or(fallback.broadcast),
            font_size: finite_clamped(
                font_size,
                COMPOSER_FONT_SIZE_MIN,
                COMPOSER_FONT_SIZE_MAX,
                fallback.font_size,
            ),
        }
    }

    pub fn try_store_panel_mode(mode: ComposerPanelMode) -> std::io::Result<()> {
        defaults::try_store_string(COMPOSER_PANEL_MODE_KEY, Some(mode.raw()))
    }

    pub fn try_store_position(position: ComposerPanelPosition) -> std::io::Result<()> {
        defaults::try_store_string(COMPOSER_POSITION_KEY, Some(position.raw()))
    }

    pub fn try_store_panel_width(width: f64) -> std::io::Result<()> {
        defaults::try_store_f64(
            COMPOSER_PANEL_WIDTH_KEY,
            finite_clamped(
                Some(width),
                COMPOSER_RIGHT_WIDTH_MIN,
                COMPOSER_RIGHT_WIDTH_MAX,
                COMPOSER_RIGHT_WIDTH_DEFAULT,
            ),
        )
    }

    pub fn try_store_panel_height(height: f64) -> std::io::Result<()> {
        defaults::try_store_f64(
            COMPOSER_PANEL_HEIGHT_KEY,
            finite_clamped(
                Some(height),
                COMPOSER_BOTTOM_HEIGHT_MIN,
                COMPOSER_BOTTOM_HEIGHT_MAX,
                COMPOSER_BOTTOM_HEIGHT_DEFAULT,
            ),
        )
    }

    pub fn try_store_broadcast(broadcast: bool) -> std::io::Result<()> {
        defaults::try_store_bool(COMPOSER_BROADCAST_KEY, broadcast)
    }

    pub fn try_store_font_size(font_size: f64) -> std::io::Result<()> {
        defaults::try_store_f64(
            COMPOSER_FONT_SIZE_KEY,
            finite_clamped(
                Some(font_size),
                COMPOSER_FONT_SIZE_MIN,
                COMPOSER_FONT_SIZE_MAX,
                COMPOSER_FONT_SIZE_DEFAULT,
            ),
        )
    }
}

fn finite_clamped(value: Option<f64>, minimum: f64, maximum: f64, fallback: f64) -> f64 {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(minimum, maximum))
        .unwrap_or(fallback)
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
    pub default_worktree_path_template: Option<String>,
    pub default_worktree_parent_path: Option<String>,
    pub active_worktree_ids: std::collections::HashMap<String, String>,
    pub active_group_id: Option<String>,
    pub composer: ComposerPreferences,
    pub repository_ai: RepositoryAiPreferences,
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
            default_worktree_path_template: None,
            default_worktree_parent_path: None,
            active_worktree_ids: std::collections::HashMap::new(),
            active_group_id: None,
            composer: ComposerPreferences::default(),
            repository_ai: RepositoryAiPreferences::default(),
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
        self.apply_settings_root(&root);
    }

    fn apply_settings_root(&mut self, root: &Value) {
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
        self.default_worktree_path_template = string("muxy.general.defaultWorktreePathTemplate")
            .filter(|value| !value.trim().is_empty());
        self.default_worktree_parent_path = string("muxy.general.defaultWorktreeParentPath")
            .filter(|value| !value.trim().is_empty());
        self.repository_ai.commit.provider = string(COMMIT_PROVIDER_KEY).unwrap_or_default();
        self.repository_ai.commit.prompt =
            string(COMMIT_PROMPT_KEY).unwrap_or_else(|| COMMIT_PROMPT.to_owned());
        self.repository_ai.create_pull_request.provider =
            string(CREATE_PULL_REQUEST_PROVIDER_KEY).unwrap_or_default();
        self.repository_ai.create_pull_request.prompt = string(CREATE_PULL_REQUEST_PROMPT_KEY)
            .unwrap_or_else(|| CREATE_PULL_REQUEST_PROMPT.to_owned());
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
        self.composer = ComposerPreferences::load();
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

    use super::{
        ComposerPanelMode, ComposerPanelPosition, ComposerPreferences, ScalePreset, SortMode,
        executable_is_test_process, resolve_app_support_dir,
    };

    #[test]
    fn composer_preferences_use_exact_keys_and_defaults() {
        assert_eq!(
            [
                super::COMPOSER_PANEL_MODE_KEY,
                super::COMPOSER_POSITION_KEY,
                super::COMPOSER_PANEL_WIDTH_KEY,
                super::COMPOSER_PANEL_HEIGHT_KEY,
                super::COMPOSER_BROADCAST_KEY,
                super::COMPOSER_FONT_SIZE_KEY,
            ],
            [
                "muxy.panel.mode.builtin:richInput",
                "muxy.richInput.position",
                "muxy.richInputPanelWidth",
                "muxy.richInputPanelHeight",
                "muxy.richInput.broadcast",
                "muxy.richInput.fontSize",
            ]
        );
        assert_eq!(
            ComposerPreferences::default(),
            ComposerPreferences {
                panel_mode: ComposerPanelMode::Floating,
                position: ComposerPanelPosition::Right,
                panel_width: 380.0,
                panel_height: 220.0,
                broadcast: false,
                font_size: 13.0,
            }
        );
    }

    #[test]
    fn composer_preferences_fallback_and_clamp_malformed_values() {
        assert_eq!(
            ComposerPreferences::from_values(
                Some("standalone"),
                Some("left"),
                Some(f64::NAN),
                Some(f64::INFINITY),
                None,
                Some(f64::NEG_INFINITY),
            ),
            ComposerPreferences::default()
        );
        assert_eq!(
            ComposerPreferences::from_values(
                Some("pinned"),
                Some("bottom"),
                Some(100.0),
                Some(900.0),
                Some(true),
                Some(40.0),
            ),
            ComposerPreferences {
                panel_mode: ComposerPanelMode::Pinned,
                position: ComposerPanelPosition::Bottom,
                panel_width: 280.0,
                panel_height: 600.0,
                broadcast: true,
                font_size: 32.0,
            }
        );
    }

    #[test]
    fn composer_preference_enums_round_trip_accepted_values() {
        for mode in [ComposerPanelMode::Floating, ComposerPanelMode::Pinned] {
            assert_eq!(ComposerPanelMode::parse(mode.raw()), mode);
        }
        for position in [ComposerPanelPosition::Right, ComposerPanelPosition::Bottom] {
            assert_eq!(ComposerPanelPosition::parse(position.raw()), position);
        }
    }

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
    fn default_worktree_locations_load_from_portable_settings() {
        let mut prefs = super::Prefs::default();
        prefs.apply_settings_root(&serde_json::json!({
            "muxy.general.defaultWorktreePathTemplate": "../{base-dir}.{branch}",
            "muxy.general.defaultWorktreeParentPath": "/worktrees"
        }));

        assert_eq!(
            prefs.default_worktree_path_template.as_deref(),
            Some("../{base-dir}.{branch}")
        );
        assert_eq!(
            prefs.default_worktree_parent_path.as_deref(),
            Some("/worktrees")
        );
    }

    #[test]
    fn repository_ai_preferences_load_as_typed_runtime_fields() {
        let mut prefs = super::Prefs::default();
        prefs.apply_settings_root(&serde_json::json!({
            "muxy.ai.repositoryActions.commit.provider": "codex",
            "muxy.ai.repositoryActions.commit.prompt": "Commit prompt",
            "muxy.ai.repositoryActions.createPullRequest.provider": "claude",
            "muxy.ai.repositoryActions.createPullRequest.prompt": "PR prompt"
        }));

        assert_eq!(prefs.repository_ai.commit.provider, "codex");
        assert_eq!(prefs.repository_ai.commit.prompt, "Commit prompt");
        assert_eq!(prefs.repository_ai.create_pull_request.provider, "claude");
        assert_eq!(prefs.repository_ai.create_pull_request.prompt, "PR prompt");
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
