use crate::environment::{BuildMode, MobileSettingsPolicy};
use crate::fold::fold;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    General,
    Projects,
    RemoteDevices,
    Appearance,
    Terminal,
    QuickTerminal,
    Browser,
    RichInput,
    Shortcuts,
    Commands,
    Ai,
    Voice,
    Notifications,
    Mobile,
    Backup,
    Json,
}

impl Category {
    pub const ALL: [Self; 16] = [
        Self::General,
        Self::Projects,
        Self::RemoteDevices,
        Self::Appearance,
        Self::Terminal,
        Self::QuickTerminal,
        Self::Browser,
        Self::RichInput,
        Self::Shortcuts,
        Self::Commands,
        Self::Ai,
        Self::Voice,
        Self::Notifications,
        Self::Mobile,
        Self::Backup,
        Self::Json,
    ];

    pub fn raw(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Projects => "projects",
            Self::RemoteDevices => "remoteDevices",
            Self::Appearance => "appearance",
            Self::Terminal => "terminal",
            Self::QuickTerminal => "quickTerminal",
            Self::Browser => "browser",
            Self::RichInput => "richInput",
            Self::Shortcuts => "shortcuts",
            Self::Commands => "commands",
            Self::Ai => "ai",
            Self::Voice => "voice",
            Self::Notifications => "notifications",
            Self::Mobile => "mobile",
            Self::Backup => "backup",
            Self::Json => "json",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::General => "App",
            Self::Projects => "Projects",
            Self::RemoteDevices => "Remote Devices",
            Self::Appearance => "Interface",
            Self::Terminal => "Terminal",
            Self::QuickTerminal => "Quick Terminal",
            Self::Browser => "Browser",
            Self::RichInput => "Composer",
            Self::Shortcuts => "Shortcuts",
            Self::Commands => "Commands",
            Self::Ai => "AI",
            Self::Voice => "Voice",
            Self::Notifications => "Notifications",
            Self::Mobile => "Mobile",
            Self::Backup => "Backup",
            Self::Json => "JSON",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::General => "gearshape",
            Self::Projects => "folder",
            Self::RemoteDevices => "server.rack",
            Self::Appearance => "macwindow",
            Self::Terminal => "terminal",
            Self::QuickTerminal => "bolt.horizontal.circle",
            Self::Browser => "globe",
            Self::RichInput => "text.cursor",
            Self::Shortcuts => "keyboard",
            Self::Commands => "command",
            Self::Ai => "sparkles",
            Self::Voice => "mic",
            Self::Notifications => "bell",
            Self::Mobile => "iphone",
            Self::Backup => "externaldrive",
            Self::Json => "curlybraces",
        }
    }

    pub fn parse_route(stored: &str) -> Option<Self> {
        let raw = stored.strip_prefix("builtin.")?;
        Self::ALL.into_iter().find(|category| category.raw() == raw)
    }

    pub fn route(self) -> String {
        format!("builtin.{}", self.raw())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub category: Category,
    pub section: &'static str,
    pub aliases: &'static [&'static str],
}

const fn item(
    key: &'static str,
    title: &'static str,
    description: &'static str,
    category: Category,
    section: &'static str,
    aliases: &'static [&'static str],
) -> Item {
    Item {
        key,
        title,
        description,
        category,
        section,
        aliases,
    }
}

pub const fn items(mode: BuildMode) -> [Item; 73] {
    let mobile_keys = MobileSettingsPolicy::new(mode).keys();
    [
        item(
            "muxy.update.channel",
            "Update Channel",
            "Controls whether Muxy receives stable releases or beta builds.",
            Category::General,
            "Updates",
            &["release", "beta"],
        ),
        item(
            "diagnostics.profiler.enabled",
            "Record Anonymous Performance Samples",
            "Records local CPU and memory samples for diagnosing long-running performance issues.",
            Category::General,
            "Diagnostics",
            &["profile", "profiler", "CPU", "memory", "performance"],
        ),
        item(
            "diagnostics.profiler.reveal",
            "Profiler Data",
            "Reveals the local JSONL performance profile in Finder for manual sharing.",
            Category::General,
            "Diagnostics",
            &["JSONL", "file", "share", "Finder"],
        ),
        item(
            "muxy.localization",
            "App Language",
            "Chooses built-in English or a language provided by an enabled extension.",
            Category::Appearance,
            "Language",
            &["localization", "translation", "locale", "i18n"],
        ),
        item(
            "muxy.general.autoExpandWorktreesOnProjectSwitch",
            "Auto-expand Worktrees",
            "Automatically reveals worktrees when switching projects.",
            Category::Appearance,
            "Sidebar",
            &[],
        ),
        item(
            "muxy.showHomeProject",
            "Show Home",
            "Shows the permanent Home project at the top of the sidebar.",
            Category::Appearance,
            "Sidebar",
            &[],
        ),
        item(
            "muxy.tips.visible",
            "Show Tips",
            "Shows Muxy tips at the bottom of the built-in sidebar.",
            Category::Appearance,
            "Sidebar",
            &["hints", "help", "sidebar card", "lightbulb"],
        ),
        item(
            "muxy.showProjectSearch",
            "Always Show Project Search",
            "Shows the project search field whenever the project-focused sidebar is expanded.",
            Category::Appearance,
            "Sidebar",
            &["find projects", "search bar", "search box"],
        ),
        item(
            "muxy.activeSidebar",
            "Active Sidebar",
            "Chooses the built-in sidebar or one provided by an extension.",
            Category::Appearance,
            "Sidebar",
            &["extension sidebar", "webview sidebar"],
        ),
        item(
            "muxy.worktrees.groupWorktrees",
            "Nest Worktrees Inside Projects",
            "Places worktrees under their project in Tab Focused and Agents Focused layouts.",
            Category::Appearance,
            "Sidebar",
            &[
                "group worktrees",
                "nested",
                "folders",
                "tab focused",
                "agents focused",
            ],
        ),
        item(
            "muxy.worktrees.showUnreadIndicator",
            "Show Worktree Unread Indicator",
            "Shows a dot on worktrees with unread notifications in the worktree switcher.",
            Category::Appearance,
            "Worktrees",
            &["unread", "badge", "notification dot", "omnibox"],
        ),
        item(
            "muxy.worktrees.orderByMRU",
            "Order Worktrees by Recent Use",
            "Sorts the worktree switcher with the active worktree first, then by most-recently-used.",
            Category::Appearance,
            "Worktrees",
            &["mru", "recent", "sort", "order", "omnibox"],
        ),
        item(
            "muxy.projectPicker.mode",
            "Project Picker",
            "Chooses the picker used when opening projects.",
            Category::Projects,
            "Projects",
            &[],
        ),
        item(
            "muxy.remoteDevices.manage",
            "Remote Devices",
            "Adds and manages reusable SSH connections used by remote workspaces.",
            Category::RemoteDevices,
            "Remote Devices",
            &["ssh", "server", "host", "remote", "connection", "device"],
        ),
        item(
            "muxy.projectPicker.defaultDirectory",
            "Project Picker Search Location",
            "Sets where Muxy's project picker searches for folders.",
            Category::Projects,
            "Projects",
            &["folder", "path", "directory", "search root"],
        ),
        item(
            "muxy.projects.keepOpenWhenNoTabs",
            "Keep Projects Open",
            "Keeps projects in the sidebar after closing the last tab.",
            Category::Projects,
            "Projects",
            &[],
        ),
        item(
            "muxy.defaultFileOpener",
            "Default Opener",
            "Uses the separately selected top-bar project target or an extension opener for terminal file links.",
            Category::Projects,
            "Open Files With",
            &[
                "file opener",
                "terminal links",
                "editor",
                "extension opener",
                "top bar",
            ],
        ),
        item(
            "muxy.general.defaultWorktreePathTemplate",
            "Default Worktree Path Template",
            "Builds new worktree paths with the required branch variable and optional project variables.",
            Category::Projects,
            "Worktrees",
            &["relative", "branch", "project name", "base dir"],
        ),
        item(
            "muxy.general.defaultWorktreeParentPath",
            "Default Worktree Parent Folder",
            "Keeps the legacy project and worktree subfolder layout inside a selected folder.",
            Category::Projects,
            "Worktrees",
            &["folder", "path", "legacy"],
        ),
        item(
            "muxy.app.transparency",
            "App Transparency",
            "Controls how much of the desktop shows through terminal panes, the top bar, and the status bar.",
            Category::Appearance,
            "Appearance",
            &["opacity", "glass", "background", "transparent", "terminal"],
        ),
        item(
            "muxy.app.blur",
            "App Vibrancy",
            "Controls the native macOS material intensity behind the transparent app background.",
            Category::Appearance,
            "Appearance",
            &["blur", "glass", "frost", "background", "terminal"],
        ),
        item(
            "muxy.general.autoCopyTerminalSelection",
            "Auto-copy Terminal Selection",
            "Copies terminal selections when the mouse is released.",
            Category::Terminal,
            "Selection",
            &[],
        ),
        item(
            "muxy.tabs.confirmCloseRunningProcess",
            "Confirm Running Process Tab Close",
            "Asks before closing a terminal tab with a running process.",
            Category::Terminal,
            "Tabs",
            &[],
        ),
        item(
            "muxy.app.confirmQuit",
            "Confirm Quit",
            "Asks before quitting Muxy.",
            Category::General,
            "Quit",
            &[],
        ),
        item(
            "SUAutomaticallyUpdate",
            "Install Downloaded Updates on Quit",
            "Downloads updates in the background and installs them when Muxy quits.",
            Category::General,
            "Updates",
            &[],
        ),
        item(
            "muxy.sentry.consent",
            "Crash Reports",
            "Controls anonymous crash report consent when diagnostics are available.",
            Category::General,
            "Diagnostics",
            &[],
        ),
        item(
            "muxy.browser.searchEngine",
            "Search Engine",
            "Chooses the search engine used when you type a query in the browser address bar.",
            Category::Browser,
            "Browsing",
            &[
                "google",
                "duckduckgo",
                "bing",
                "brave",
                "startpage",
                "search",
            ],
        ),
        item(
            "muxy.browser.homePageURL",
            "Home Page",
            "Sets the page new browser tabs open to. Blank by default, or a website you choose.",
            Category::Browser,
            "Browsing",
            &["homepage", "new tab", "start page", "blank"],
        ),
        item(
            "muxy.ui.scale",
            "Interface Size",
            "Controls the scale of the app interface.",
            Category::Appearance,
            "Interface",
            &["zoom", "density"],
        ),
        item(
            "muxy.tabs.maxWidth",
            "Tab header width",
            "Sets the maximum tab header width in pixels; the widest setting lets tabs fill the titlebar.",
            Category::Appearance,
            "Interface",
            &["tabs", "tab width", "full-width"],
        ),
        item(
            "muxy.showTopBarActions",
            "Show Top Bar Actions",
            "Shows or hides the window-level controls on the right side of the top bar.",
            Category::Appearance,
            "Interface",
            &[
                "topbar",
                "title bar",
                "tab strip controls",
                "hide top bar icons",
            ],
        ),
        item(
            "muxy.showStatusBar",
            "Show Status Bar",
            "Shows or hides the status bar.",
            Category::Appearance,
            "Interface",
            &[],
        ),
        item(
            "muxy.showResourceUsageInStatusBar",
            "Show Resource Usage in Status Bar",
            "Shows app and subprocess CPU and memory usage in the status bar. Disabling it stops the sampling.",
            Category::Appearance,
            "Interface",
            &[],
        ),
        item(
            "muxy.theme.light",
            "Light Terminal Theme",
            "Chooses the terminal theme for light appearance.",
            Category::Appearance,
            "Theme",
            &[],
        ),
        item(
            "muxy.theme.dark",
            "Dark Terminal Theme",
            "Chooses the terminal theme for dark appearance.",
            Category::Appearance,
            "Theme",
            &[],
        ),
        item(
            "muxy.appBackgroundStyle",
            "Sidebar Vibrancy",
            "Uses tinted native macOS vibrancy for the sidebar and its left title strip. Turn off for a solid background.",
            Category::Appearance,
            "Sidebar",
            &[
                "vibrancy",
                "material",
                "transparency",
                "background",
                "sidebar",
            ],
        ),
        item(
            "muxy.sidebarCollapsedStyle",
            "Collapsed Sidebar Style",
            "Controls the sidebar appearance when collapsed.",
            Category::Appearance,
            "Sidebar",
            &[],
        ),
        item(
            "muxy.sidebarExpandedStyle",
            "Expanded Sidebar Style",
            "Controls the sidebar appearance when expanded.",
            Category::Appearance,
            "Sidebar",
            &[],
        ),
        item(
            "muxy.richInput.presentationMode",
            "Composer Presentation",
            "Chooses whether the composer opens as a workspace panel or a floating modal.",
            Category::RichInput,
            "Composer",
            &["rich input", "panel", "floating"],
        ),
        item(
            "muxy.richInput.clearAfterSending",
            "Clear After Sending",
            "Clears text and attachments after a successful Composer submission.",
            Category::RichInput,
            "Composer",
            &["rich input", "draft", "send"],
        ),
        item(
            "muxy.richInput.clearOnClose",
            "Clear on Close",
            "Clears text and attachments whenever the Composer closes.",
            Category::RichInput,
            "Composer",
            &["rich input", "draft", "dismiss"],
        ),
        item(
            "editor.richInputImageStrategy",
            "Composer Image Submission",
            "Chooses how the composer submits images.",
            Category::RichInput,
            "Composer",
            &["rich input"],
        ),
        item(
            "editor.richInputFontFamily",
            "Composer Font Family",
            "Controls the composer editor font family.",
            Category::RichInput,
            "Composer",
            &["rich input"],
        ),
        item(
            "editor.richInputLineHeightMultiplier",
            "Composer Line Height",
            "Controls line height in the composer.",
            Category::RichInput,
            "Composer",
            &["rich input"],
        ),
        item(
            "muxy.terminalOffline.enabled",
            "Free Idle Background Terminals",
            "Frees a background tab's terminal after it stays idle, reclaiming memory.",
            Category::Terminal,
            "Memory",
            &[],
        ),
        item(
            "muxy.terminalOffline.idleThresholdSeconds",
            "Idle Timeout (seconds)",
            "How long a background tab stays idle before its terminal is freed.",
            Category::Terminal,
            "Memory",
            &[],
        ),
        item(
            "muxy.terminalPersistentSession.enabled",
            "Run New Terminals in the Background",
            "Runs new terminals in a background process so they survive quitting Muxy.",
            Category::Terminal,
            "Background sessions",
            &[],
        ),
        item(
            "shortcuts.app",
            "App Shortcuts",
            "Configures Muxy keyboard shortcuts.",
            Category::Shortcuts,
            "App Shortcuts",
            &["keybindings", "hotkeys"],
        ),
        item(
            "muxy.quickTerminal.enabled",
            "Enable Quick Terminal",
            "Controls whether the Quick Terminal shortcut listener and shell can run.",
            Category::QuickTerminal,
            "General",
            &["disable", "off", "global terminal"],
        ),
        item(
            "shortcuts.quickTerminal",
            "Quick Terminal",
            "Configures the system-wide shortcut for the quick terminal.",
            Category::QuickTerminal,
            "Shortcut",
            &[
                "double shift",
                "quick terminal",
                "global shortcut",
                "hotkey",
            ],
        ),
        item(
            "muxy.quickTerminal.width",
            "Quick Terminal Width",
            "Sets the width of the quick terminal in points.",
            Category::QuickTerminal,
            "Size",
            &["size", "panel", "window"],
        ),
        item(
            "muxy.quickTerminal.height",
            "Quick Terminal Height",
            "Sets the height of the quick terminal in points.",
            Category::QuickTerminal,
            "Size",
            &["size", "panel", "window"],
        ),
        item(
            "muxy.quickTerminal.transparency",
            "Quick Terminal Transparency",
            "Controls how much of the desktop shows through the terminal background.",
            Category::QuickTerminal,
            "Appearance",
            &["opacity", "glass", "background", "appearance"],
        ),
        item(
            "muxy.quickTerminal.blur",
            "Quick Terminal Vibrancy",
            "Controls the native macOS material intensity behind the terminal.",
            Category::QuickTerminal,
            "Appearance",
            &["blur", "glass", "frost", "background", "appearance"],
        ),
        item(
            "shortcuts.customCommands",
            "Commands",
            "Configures shortcuts that open command tabs.",
            Category::Commands,
            "Commands",
            &["command layer", "custom commands", "shortcuts"],
        ),
        item(
            "muxy.ai.repositoryActions.commit.provider",
            "Commit Provider",
            "Chooses the AI CLI used by the Commit top-bar action.",
            Category::Ai,
            "Commit and Push",
            &["agent", "git", "push"],
        ),
        item(
            "muxy.ai.repositoryActions.commit.prompt",
            "Commit Prompt",
            "Controls how the AI provider generates commit-message metadata.",
            Category::Ai,
            "Commit and Push",
            &["agent", "git", "push", "instructions"],
        ),
        item(
            "muxy.ai.repositoryActions.createPullRequest.provider",
            "Create Pull Request Provider",
            "Chooses the AI CLI used by the Create PR top-bar action.",
            Category::Ai,
            "Create Pull Request",
            &["agent", "github", "pr"],
        ),
        item(
            "muxy.ai.repositoryActions.createPullRequest.prompt",
            "Create Pull Request Prompt",
            "Controls how the AI provider generates pull request metadata.",
            Category::Ai,
            "Create Pull Request",
            &["agent", "github", "pr", "instructions"],
        ),
        item(
            "muxy.recording.autoSend",
            "Press Return After Inserting",
            "Presses Return after voice transcription is inserted.",
            Category::Voice,
            "Voice Recording",
            &[],
        ),
        item(
            "muxy.recording.language",
            "Recording Language",
            "Chooses the on-device speech recognition language.",
            Category::Voice,
            "Language",
            &[],
        ),
        item(
            "muxy.notifications.toastEnabled",
            "Toast Notifications",
            "Shows toast notifications.",
            Category::Notifications,
            "Delivery",
            &[],
        ),
        item(
            "muxy.notifications.desktopEnabled",
            "Desktop Notifications",
            "Shows a macOS notification when Muxy is not frontmost.",
            Category::Notifications,
            "Delivery",
            &[],
        ),
        item(
            "muxy.notifications.sound",
            "Notification Sound",
            "Chooses the notification sound.",
            Category::Notifications,
            "Sound",
            &[],
        ),
        item(
            "muxy.notifications.toastPosition",
            "Toast Position",
            "Controls where toast notifications appear.",
            Category::Notifications,
            "Toast",
            &[],
        ),
        item(
            "ai.providers",
            "AI Provider Notifications",
            "Controls AI provider notification integrations.",
            Category::Notifications,
            "AI Providers",
            &[],
        ),
        item(
            mobile_keys.enabled,
            "Allow Mobile Connections",
            "Allows mobile devices to connect to this Mac.",
            Category::Mobile,
            "Mobile",
            &[],
        ),
        item(
            mobile_keys.port,
            "Mobile Port",
            "Controls the local server port for mobile pairing.",
            Category::Mobile,
            "Mobile",
            &[],
        ),
        item(
            mobile_keys.scrollback_cap,
            "Scrollback Buffer Cap",
            "Scrollback history kept per terminal, in MB, for replay to mobile devices.",
            Category::Mobile,
            "Mobile",
            &["scrollback", "buffer", "history", "terminal history"],
        ),
        item(
            "mobile.pairing",
            "Pair Mobile Device",
            "Shows the QR code used to pair a mobile device.",
            Category::Mobile,
            "Pair Mobile Device",
            &[],
        ),
        item(
            "mobile.approvedDevices",
            "Approved Devices",
            "Manages mobile devices that can connect.",
            Category::Mobile,
            "Approved Devices",
            &[],
        ),
        item(
            "backup.export",
            "Export Muxy",
            "Saves settings, projects, remote devices and customizations to a file.",
            Category::Backup,
            "Export",
            &["backup", "migrate", "transfer"],
        ),
        item(
            "backup.import",
            "Import Muxy",
            "Restores a backup and replaces all current Muxy data.",
            Category::Backup,
            "Import",
            &["backup", "restore", "migrate"],
        ),
    ]
}

pub const ITEMS: [Item; 73] = items(crate::build_mode!());

fn haystack(item: &Item) -> String {
    let mut parts = vec![
        fold(item.title.trim()),
        fold(item.description.trim()),
        fold(item.category.title().trim()),
        fold(item.section.trim()),
        fold(item.key.trim()),
    ];
    parts.extend(item.aliases.iter().map(|alias| fold(alias.trim())));
    parts.join(" ")
}

fn matching(query: &str) -> Vec<&'static Item> {
    let normalized = fold(query.trim());
    if normalized.is_empty() {
        return ITEMS.iter().collect();
    }
    ITEMS
        .iter()
        .filter(|item| haystack(item).contains(&normalized))
        .collect()
}

pub fn category_matches(category: Category, query: &str) -> bool {
    let normalized = fold(query.trim());
    if normalized.is_empty() {
        return true;
    }
    fold(category.title()).contains(&normalized)
        || matching(query).iter().any(|item| item.category == category)
}

pub fn match_count_summary(category: Category, query: &str) -> Option<String> {
    if fold(query.trim()).is_empty() {
        return None;
    }
    let count = matching(query)
        .iter()
        .filter(|item| item.category == category)
        .count();
    if count == 1 {
        return Some("1 match".to_owned());
    }
    Some(format!("{count} matches"))
}

pub fn section_matches(query: &str, category: Category, section: &str) -> bool {
    if fold(query.trim()).is_empty() {
        return true;
    }
    matching(query)
        .iter()
        .any(|item| item.section == section && item.category == category)
}

#[cfg(test)]
mod tests {
    use super::Category;

    #[test]
    fn an_empty_query_matches_every_category_and_reports_no_count() {
        for category in Category::ALL {
            assert!(super::category_matches(category, "  "));
            assert_eq!(super::match_count_summary(category, ""), None);
            assert!(super::section_matches("", category, "anything"));
        }
        assert_eq!(super::matching("").len(), super::ITEMS.len());
    }

    #[test]
    fn a_substring_of_a_category_title_matches_it() {
        assert!(super::category_matches(Category::Terminal, "erm"));
    }

    #[test]
    fn an_alias_only_query_surfaces_its_category() {
        assert!(super::category_matches(Category::Shortcuts, "hotkeys"));
        assert!(
            super::matching("hotkeys")
                .iter()
                .any(|item| item.key == "shortcuts.app")
        );
    }

    #[test]
    fn a_category_matching_only_by_title_reports_zero_matches() {
        assert!(super::category_matches(Category::Json, "json"));
        assert_eq!(
            super::match_count_summary(Category::Json, "json"),
            Some("0 matches".to_owned())
        );
    }

    #[test]
    fn a_query_spanning_two_joined_fields_matches() {
        assert!(
            super::matching("app quit")
                .iter()
                .any(|item| item.key == "muxy.app.confirmQuit")
        );
    }

    #[test]
    fn one_match_is_reported_in_the_singular() {
        assert_eq!(
            super::match_count_summary(Category::General, "beta"),
            Some("1 match".to_owned())
        );
    }

    #[test]
    fn explicit_catalogs_differ_only_at_the_three_mobile_items() {
        let development = super::items(crate::environment::BuildMode::Development);
        let production = super::items(crate::environment::BuildMode::Production);
        let differences: Vec<usize> = development
            .iter()
            .zip(production.iter())
            .enumerate()
            .filter_map(|(index, (left, right))| (left != right).then_some(index))
            .collect();
        assert_eq!(development.len(), 73);
        assert_eq!(production.len(), 73);
        assert_eq!(differences.len(), 3);
        for index in differences {
            assert!(development[index].key.ends_with(".dev"));
            assert_eq!(
                development[index].key.strip_suffix(".dev"),
                Some(production[index].key)
            );
            let mut normalized = development[index];
            normalized.key = production[index].key;
            assert_eq!(normalized, production[index]);
        }
        assert_eq!(super::ITEMS, super::items(crate::build_mode!()));
    }

    #[test]
    fn mobile_catalog_keys_match_the_current_artifact() {
        for key in [
            crate::prefs::settings::MOBILE_KEYS.enabled,
            crate::prefs::settings::MOBILE_KEYS.port,
            crate::prefs::settings::MOBILE_KEYS.scrollback_cap,
        ] {
            assert!(super::ITEMS.iter().any(|item| item.key == key));
            assert_eq!(key.ends_with(".dev"), crate::build_mode!().is_development());
        }
    }

    #[test]
    fn every_item_key_is_mirrored_or_deliberately_excluded() {
        const EXCLUDED: [&str; 5] = [
            "diagnostics.profiler.reveal",
            "muxy.remoteDevices.manage",
            "mobile.pairing",
            "backup.export",
            "backup.import",
        ];
        for mode in [
            crate::environment::BuildMode::Development,
            crate::environment::BuildMode::Production,
        ] {
            let mirror = crate::prefs::settings::mirror(mode);
            let items = super::items(mode);
            let mirrored: Vec<&str> = mirror.iter().map(|entry| entry.key).collect();
            for item in &items {
                assert!(
                    mirrored.contains(&item.key) || EXCLUDED.contains(&item.key),
                    "catalog key {} is neither mirrored nor excluded",
                    item.key
                );
            }
            for key in &mirrored {
                assert!(
                    items.iter().any(|item| item.key == *key),
                    "mirrored key {key} is missing from the catalog"
                );
            }
            assert_eq!(mirrored.len() + EXCLUDED.len(), items.len());
        }
    }
}
