use gpui::{AppContext, Context, Entity, Subscription, Window};
use muxy_app_core::ServerId;
use muxy_app_core::settings::TerminalEdit;
use muxy_ui::tr;

use super::{AppModel, Overlay, Quitting};
use crate::boot::Work;
use crate::views::settings::tab::SettingsTab;
use crate::views::settings::{SettingsView, Snapshot};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug)]
pub(crate) enum Change {
    Composer(ComposerToggle, bool),
    QuickTerminal(muxy_app_core::settings::QuickTerminalSettings),
    /// One Quick Terminal number, such as `quick-width`, set on the saved settings.
    QuickNumber(&'static str, u16),
    Theme(bool, String),
    Sidebar(bool),
    SidebarVibrancy(bool),
    /// The extension whose sidebar replaces the built-in one; empty for built-in.
    ExtensionSidebar(String),
    /// The app language as `<extension>:<localization>`; empty for English.
    Language(String),
    Worktrees(WorktreeToggle, bool),
    SidebarCollapsedStyle(muxy_app_core::settings::SidebarCollapsedStyle),
    InterfaceSize(muxy_app_core::settings::InterfaceSize),
    AgentsPanel(bool),
    AgentsPanelGrouping(muxy_app_core::settings::AgentsPanelGrouping),
    AgentsPanelIcon(muxy_app_core::settings::AgentsPanelIcon),
    AgentsPanelDensity(muxy_app_core::settings::AgentsPanelDensity),
    AgentsPanelOption(AgentsPanelToggle, bool),
    StatusBar(bool),
    Rail(bool),
    PaneHeaders(bool),
    Tips(bool),
    RememberProjectsWithoutTabs(bool),
    SidebarServers(bool),
    ConfirmProcess(bool),
    UsagePing(bool),
    CloseBehavior(muxy_app_core::settings::CloseBehavior),
    Terminal(&'static str, String),
    TerminalEdit(TerminalEdit),
    ResetTerminal,
    /// `[openers] file`: a built-in opener, or `<extension>:<opener>`.
    FileOpener(String),
    /// `[openers] project_target`: the app the Open in IDE control opens.
    ProjectTarget(String),
    Directory(muxy_app_core::settings::NewPaneDirectory),
    Field(&'static str, String),
    Binding(String, Option<muxy_app_core::settings::KeyChord>),
    Command(muxy_app_core::settings::CustomCommand),
    RemoveCommand(String),
    /// Removes an extension command's shortcut.
    Unassign(String),
    ShellIntegration(bool),
    Browser(BrowserChange),
}

#[derive(Clone, Debug)]
pub(crate) enum BrowserChange {
    Enabled(bool),
    TerminalLinks(bool),
    SearchEngine(muxy_app_core::browser::SearchEngine),
    DefaultProfile(String),
    AddProfile(String),
    RenameProfile(String, String),
    ClearProfile(String),
    DeleteProfile(String),
    /// Imports from the listed browser at `source` into the default profile.
    Import {
        source: usize,
        cookies: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum WorktreeToggle {
    AutoExpand,
    Unread,
    RecentOrder,
}

impl WorktreeToggle {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::AutoExpand => "auto-expand-worktrees",
            Self::Unread => "worktree-unread",
            Self::RecentOrder => "worktree-recent-order",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum AgentsPanelToggle {
    Grouped,
    ProjectColors,
    IdleTime,
}

impl AgentsPanelToggle {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Grouped => "agents-panel-grouped",
            Self::ProjectColors => "agents-panel-project-colors",
            Self::IdleTime => "agents-panel-idle-time",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ComposerToggle {
    ClearAfter,
    ClearClose,
    Images,
}

impl ComposerToggle {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::ClearAfter => "composer-clear-after",
            Self::ClearClose => "composer-clear-close",
            Self::Images => "composer-images",
        }
    }
}

#[derive(Default)]
pub(super) struct ServerPreferences {
    pub(super) document: Option<muxy_protocol::ServerSettingsDoc>,
    pub(super) busy: bool,
    pub(super) control_busy: bool,
    pub(super) row: String,
    pub(super) pending: std::collections::VecDeque<Change>,
}

pub(crate) struct SettingsTabState {
    pub(crate) root: Entity<SettingsTab>,
    pub(crate) view: Entity<SettingsView>,
    pub(crate) overlay: Entity<crate::views::cached::CachedView<SettingsTab>>,
    overlay_open: bool,
    _subscription: Subscription,
}

pub(super) struct TerminalPreview {
    settings: muxy_app_core::settings::TerminalSettings,
    zoom: std::collections::HashMap<super::PaneId, f32>,
}

#[derive(Default)]
pub(super) struct TerminalBackground {
    pub(super) available: bool,
    #[cfg(all(target_os = "macos", not(test)))]
    native: Option<muxy_ui::vibrancy::TerminalVibrancy>,
}

impl AppModel {
    pub(crate) fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_settings(window, cx);
    }

    pub(crate) fn show_extensions_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.show_settings(window, cx) {
            return;
        }
        if let Some(settings) = &self.settings_tab {
            settings.view.update(cx, SettingsView::show_extensions);
        }
    }

    pub(crate) fn show_about_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.show_settings(window, cx) {
            return;
        }
        if let Some(settings) = &self.settings_tab {
            settings.view.update(cx, SettingsView::show_about);
        }
    }

    pub(crate) fn show_keyboard_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.show_settings(window, cx) {
            return;
        }
        if let Some(settings) = &self.settings_tab {
            settings.view.update(cx, SettingsView::show_keyboard);
        }
    }

    /// Opens Settings at Browser, where profiles are managed.
    pub(crate) fn show_browser_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_settings(window, cx);
        if let Some(settings) = &self.settings_tab {
            settings.view.update(cx, SettingsView::show_browser);
        }
    }

    /// Opens Settings at Servers and reads which clients may reach this computer.
    pub(crate) fn show_servers_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.show_settings(window, cx) {
            return;
        }
        if let Some(settings) = &self.settings_tab {
            settings.view.update(cx, SettingsView::show_servers);
        }
        self.client_access.show_page();
        self.sync_servers_page(true, cx);
    }

    /// Opens Settings at `category`.
    pub(crate) fn show_settings_category(
        &mut self,
        category: crate::views::settings::Category,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::views::settings::Category;
        match category {
            Category::Servers => self.show_servers_settings(window, cx),
            Category::Extensions => self.show_extensions_settings(window, cx),
            Category::About => self.show_about_settings(window, cx),
            category => {
                if !self.show_settings(window, cx) {
                    return;
                }
                if let Some(settings) = &self.settings_tab {
                    settings.view.update(cx, |view, cx| {
                        view.select_category(category, None, window, cx);
                    });
                }
            }
        }
    }

    pub(super) fn show_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.quitting != Quitting::Idle || self.dialogs.is_open() {
            return false;
        }
        let project = self.state.current_project().id;
        match self.transact(
            |state| state.show_settings_tab(project, &tr!("Settings")),
            cx,
        ) {
            Ok(Some(_)) => {}
            Ok(None) => return false,
            Err(error) => {
                self.fail(error.to_string(), cx);
                return false;
            }
        }
        if let Some(server) = self
            .state
            .settings_tab()
            .and_then(|(project, _)| self.state.project_server(project))
        {
            self.reveal_server(server, cx);
        }
        self.dismiss_overlay(cx);
        self.changed(cx);
        self.ensure_settings_tab(window, cx);
        self.focus_active(window, cx);
        self.focus_owner.settle();
        window.activate_window();
        true
    }

    pub(crate) fn ensure_settings_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.settings_tab().is_none() {
            return;
        }
        if let Some(settings) = &self.settings_tab {
            let active = self
                .active_pane()
                .is_some_and(|pane| self.state.is_settings_pane(pane));
            settings
                .root
                .update(cx, |root, cx| root.set_active(active, cx));
            return;
        }
        self.refresh_browser_profiles();
        self.browsers.import_sources = Self::browser_import_sources();
        let snapshot = self.preferences_snapshot();
        self.refresh_theme(cx);
        let model = cx.weak_entity();
        let extensions = cx.new(|cx| {
            crate::views::settings::extensions::ExtensionsView::new(
                model,
                self.theme.clone(),
                crate::views::settings::content_metrics(self.metrics),
                cx,
            )
        });
        let servers = crate::views::cached::CachedView::new(
            |model, _, cx| crate::views::servers_page::render(model, cx),
            cx,
        );
        let view = cx.new(|cx| {
            let mut view = SettingsView::new(snapshot, self.theme.clone(), self.metrics, cx);
            view.extensions = Some(extensions);
            view.servers = Some(servers);
            view.set_backup_pending(self.path.with_file_name("pending-import.muxy").exists());
            view
        });
        let weak = cx.weak_entity();
        let content = view.clone();
        let active = self
            .active_pane()
            .is_some_and(|pane| self.state.is_settings_pane(pane));
        let root = cx.new(|cx| SettingsTab::new(weak, content, window, cx));
        root.update(cx, |root, cx| root.set_active(active, cx));
        let overlay = root.read(cx).overlay_view.clone();
        let subscription = cx.observe(&root, |model, root, cx| {
            let open = root.read(cx).overlay.is_some();
            if let Some(settings) = &mut model.settings_tab
                && settings.overlay_open != open
            {
                settings.overlay_open = open;
                cx.notify();
            }
        });
        self.settings_tab = Some(SettingsTabState {
            root,
            view,
            overlay,
            overlay_open: false,
            _subscription: subscription,
        });
        if active {
            self.focus_owner.request_active_pane();
        }
        self.read_server_settings(cx);
    }

    pub(super) fn sync_settings_tab(&mut self, cx: &mut Context<Self>) {
        if self.settings_tab.is_some() && self.state.settings_tab().is_none() {
            self.record_quick_shortcut(false, cx);
            self.flush_preferences(cx);
            self.settings_tab = None;
            self.client_access.hide_page();
        }
    }

    pub(crate) fn settings_picker_open(&self) -> bool {
        self.settings_tab
            .as_ref()
            .is_some_and(|settings| settings.overlay_open)
            && self
                .active_pane()
                .is_some_and(|pane| self.state.is_settings_pane(pane))
    }

    fn preferences_snapshot(&self) -> Snapshot {
        let mut settings = self.settings.clone();
        settings.appearance = self.appearance.clone();
        Snapshot {
            quick_shortcut_status: self.quick_shortcut_status(),
            capabilities: self.capabilities.clone(),
            settings,
            terminal: self.terminal.clone(),
            server: self.server_preferences.document.clone(),
            server_update: self.server_update_description(),
            connected: self.ready(ServerId::local()),
            server_busy: self.server_preferences.busy
                || self.server_preferences.control_busy
                || self.updates.replacing(),
            pending_server_fields: self.pending_server_fields(),
            ai_installed: self
                .ai
                .installed
                .iter()
                .map(|provider| provider.id)
                .collect(),
            extensions_loaded: self.extensions_loaded(),
            sidebars: self.extension_sidebars(),
            file_openers: self.extension_file_openers(),
            languages: self.app_languages(),
            extension_shortcuts: self.extension_shortcuts(),
            browser_profiles: self.settings_browser_profiles(),
            browser_sources: self
                .browsers
                .import_sources
                .iter()
                .map(|source| {
                    (
                        format!("{} — {}", source.browser(), source.name()),
                        source.browser().supports_cookies(),
                    )
                })
                .collect(),
        }
    }

    pub(super) fn pending_server_fields(&self) -> std::collections::HashSet<String> {
        self.server_preferences
            .pending
            .iter()
            .map(|change| change_id(change).to_owned())
            .chain(
                self.server_preferences
                    .busy
                    .then(|| self.server_preferences.row.clone()),
            )
            .collect()
    }

    pub(crate) fn flush_preferences(&mut self, cx: &mut Context<Self>) {
        for change in self.take_pending_preferences(cx) {
            self.change_preference(change, cx);
        }
    }

    pub(super) fn preferences_before_quit(&mut self, cx: &mut Context<Self>) -> bool {
        self.flush_preferences(cx);
        if self.server_preferences.busy || self.server_preferences.control_busy {
            self.fail(
                tr!("Wait for the server settings operation to finish before quitting").to_string(),
                cx,
            );
            return false;
        }
        true
    }

    pub(super) fn sync_preferences(&self, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings_tab {
            settings.view.update(cx, |view, cx| {
                view.sync(
                    self.preferences_snapshot(),
                    self.theme.clone(),
                    self.metrics,
                    cx,
                );
            });
        }
    }

    pub(crate) fn preference_result(&self, id: &str, error: Option<&str>, cx: &mut Context<Self>) {
        if let Some(settings) = &self.settings_tab {
            settings
                .view
                .update(cx, |view, cx| view.set_error(id, error, cx));
        }
    }

    pub(crate) fn configuration_path(&self, filename: &str) -> std::path::PathBuf {
        self.path.with_file_name(filename)
    }

    /// Applies a change made outside Settings and answers whether it saved.
    pub(crate) fn try_change_preference(
        &mut self,
        change: Change,
        cx: &mut Context<Self>,
    ) -> std::result::Result<(), String> {
        if self.quitting != Quitting::Idle {
            return Err("the app is quitting".into());
        }
        let id = change_id(&change).to_owned();
        let result = self
            .apply_preference(change, cx)
            .map_err(|error| error.to_string());
        self.preference_result(&id, result.as_ref().err().map(String::as_str), cx);
        self.sync_preferences(cx);
        cx.notify();
        result
    }

    pub(crate) fn change_preference(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle {
            return;
        }
        if let Change::Field(id, prompt) = &change
            && let Some(action) = crate::views::settings::prompt_action(id)
        {
            self.set_ai_prompt(action, prompt, cx);
            return;
        }
        let id = change_id(&change).to_owned();
        if matches!(
            &change,
            Change::ShellIntegration(_) | Change::Field("default-shell" | "history-budget", _)
        ) {
            if self.server_preferences.busy {
                self.server_preferences
                    .pending
                    .retain(|pending| change_id(pending) != id);
                self.server_preferences.pending.push_back(change);
            } else if let Err(error) = self.write_server_preference(change, cx) {
                self.preference_result(&id, Some(&error.to_string()), cx);
            }
        } else {
            let theme_change = matches!(change, Change::Theme(..));
            let target_change = matches!(change, Change::ProjectTarget(_));
            let result = self.apply_preference(change, cx);
            let error = result.err().map(|error| error.to_string());
            self.preference_result(&id, error.as_deref(), cx);
            if let Some(error) = error {
                if theme_change {
                    self.fail(tr!("Could not save theme: %@", &error).to_string(), cx);
                } else if target_change {
                    self.fail(
                        tr!("Could not save the default app: %@", &error).to_string(),
                        cx,
                    );
                }
            }
        }
        self.sync_preferences(cx);
        cx.notify();
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Exhaustive preference change dispatch"
    )]
    fn apply_preference(&mut self, change: Change, cx: &mut Context<Self>) -> Result<()> {
        if matches!(
            &change,
            Change::Composer(..) | Change::Field("composer-font" | "composer-line-height", _)
        ) {
            return self.apply_composer_preference(change, cx);
        }
        let path = self.path.with_file_name("settings.toml");
        let mut settings = self.settings.clone();
        settings.appearance = self.appearance.clone();
        let theme_changed = matches!(change, Change::Theme(..));
        let bindings_changed = matches!(
            change,
            Change::Binding(..)
                | Change::Unassign(_)
                | Change::Command(_)
                | Change::RemoveCommand(_)
        );
        match change {
            Change::QuickNumber(id, value) => {
                self.apply_quick_settings(
                    quick_number(&self.settings.quick_terminal, id, value),
                    cx,
                )?;
                return Ok(());
            }
            Change::QuickTerminal(quick) => {
                self.apply_quick_settings(quick, cx)?;
                return Ok(());
            }
            Change::Browser(change) => {
                if !self.apply_browser_profile_change(&change, cx)? {
                    settings.browser = browser_settings(settings.browser, change);
                    settings.save_browser(&path)?;
                }
            }
            Change::Field("browser-home-page", value) => {
                settings.browser.home_page = value;
                settings.save_browser(&path)?;
            }
            Change::Field("browser-profile-name", _) => return Ok(()),
            Change::Field(id @ ("quick-width" | "quick-height"), value) => {
                let mut quick = settings.quick_terminal;
                if id == "quick-width" {
                    quick.width = value.parse()?;
                } else {
                    quick.height = value.parse()?;
                }
                self.apply_quick_settings(quick, cx)?;
                return Ok(());
            }
            Change::Theme(dark, name) => {
                if dark {
                    settings.appearance.dark_theme = name;
                } else {
                    settings.appearance.light_theme = name;
                }
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Worktrees(toggle, value) => {
                match toggle {
                    WorktreeToggle::AutoExpand => settings.appearance.auto_expand_worktrees = value,
                    WorktreeToggle::Unread => settings.appearance.worktree_show_unread = value,
                    WorktreeToggle::RecentOrder => {
                        settings.appearance.worktree_recent_order = value;
                    }
                }
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Sidebar(value) => {
                settings.appearance.sidebar_expanded = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::SidebarCollapsedStyle(style) => {
                settings.appearance.sidebar_collapsed_style = style;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::InterfaceSize(size) => {
                settings.appearance.interface_size = size;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::AgentsPanel(value) => {
                settings.appearance.agents_panel_visible = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
                if !settings.appearance.agents_panel_visible {
                    self.agents_panel.drag = None;
                    self.focus_owner.request_active_pane();
                    if matches!(self.overlay, Some(Overlay::Agents)) {
                        self.dismiss_overlay(cx);
                    }
                }
            }
            Change::AgentsPanelGrouping(grouping) => {
                settings.appearance.agents_panel_grouping = grouping;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::AgentsPanelIcon(icon) => {
                settings.appearance.agents_panel_icon = icon;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::AgentsPanelDensity(density) => {
                settings.appearance.agents_panel_density = density;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::AgentsPanelOption(toggle, value) => {
                match toggle {
                    AgentsPanelToggle::Grouped => {
                        settings.appearance.agents_panel_grouped = value;
                    }
                    AgentsPanelToggle::ProjectColors => {
                        settings.appearance.agents_panel_project_colors = value;
                    }
                    AgentsPanelToggle::IdleTime => {
                        settings.appearance.agents_panel_idle_time = value;
                    }
                }
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::SidebarVibrancy(value) => {
                settings.appearance.sidebar_vibrancy = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::ExtensionSidebar(owner) => {
                settings.appearance.extension_sidebar = owner;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Language(selection) => {
                settings.appearance.language = selection;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Field("sidebar-vibrancy-level", value) => {
                let level: u8 = value
                    .parse()
                    .map_err(|_| tr!("Enter a whole number from 0 to 100").to_string())?;
                if level > 100 {
                    return Err(tr!("Enter a whole number from 0 to 100").to_string().into());
                }
                settings.appearance.sidebar_vibrancy_level = level;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::StatusBar(value) => {
                settings.appearance.status_bar_visible = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Rail(value) => {
                settings.appearance.rail_enabled = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::PaneHeaders(value) => {
                settings.appearance.pane_headers = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::Tips(value) => {
                settings.appearance.tips_visible = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
                if !value {
                    self.hide_tip(cx);
                }
            }
            Change::RememberProjectsWithoutTabs(value) => {
                settings.appearance.remember_projects_without_tabs = value;
                settings.appearance.projects_without_tabs_shown =
                    value && self.projects_without_tabs_shown;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::SidebarServers(value) => {
                settings.appearance.sidebar_remote_servers = value;
                settings.appearance = settings.appearance.save_changes(&self.appearance, &path)?;
            }
            Change::ConfirmProcess(value) => {
                settings.window.confirm_running_process = value;
                settings.save_window(&path)?;
            }
            Change::CloseBehavior(value) => {
                settings.window.close_behavior = value;
                settings.save_window(&path)?;
            }
            Change::UsagePing(value) => {
                settings.privacy.usage_ping = value;
                settings.save_privacy(&path)?;
            }
            Change::FileOpener(value) => {
                settings.openers.file = value;
                settings.save_openers(&path)?;
            }
            Change::ProjectTarget(value) => {
                settings.openers.project_target = Some(value);
                settings.save_openers(&path)?;
            }
            Change::Directory(value) => {
                settings.panes.new_pane_directory = value;
                settings.save_panes(&path)?;
            }
            Change::Command(command) => {
                command.validate()?;
                if let Some(existing) = settings
                    .commands
                    .iter_mut()
                    .find(|existing| existing.id == command.id)
                {
                    *existing = command;
                } else {
                    settings.commands.push(command);
                }
                settings.validate_command_shortcuts(&self.terminal)?;
                settings.save_commands(&path)?;
            }
            Change::RemoveCommand(id) => {
                settings.commands.retain(|command| command.id != id);
                settings.keymap = settings
                    .keymap
                    .with_binding(&format!("command.{id}"), None)?;
                settings.save_commands(&path)?;
            }
            Change::Binding(id, chord) => {
                if let Some(chord) = &chord {
                    self.validate_quick_conflict(chord)?;
                    if let Some(conflict) = self.custom_shortcut_conflict(&id, chord) {
                        return Err(conflict.into());
                    }
                    if (id.starts_with("extension.") || id.starts_with("command."))
                        && let Some(conflict) = self.extension_shortcut_conflict(&id, chord)
                    {
                        return Err(conflict.into());
                    }
                }
                settings.keymap = settings.keymap.with_binding(&id, chord)?;
                settings.validate_command_shortcuts(&self.terminal)?;
                settings.keymap.save(&path)?;
                cx.set_menus(crate::menus());
            }
            Change::Unassign(id) => {
                settings.keymap = settings.keymap.with_unassigned(&id)?;
                settings.keymap.save(&path)?;
            }
            Change::Field(id @ ("worktree-template" | "worktree-folder"), value) => {
                if id == "worktree-template" {
                    settings.worktrees.default_location.path_template = value;
                } else {
                    settings.worktrees.default_location.parent_path = value;
                }
                settings.save_worktrees(&path)?;
            }
            Change::Field(id @ ("width" | "height"), value) => {
                let index = usize::from(id == "height");
                settings.window.default_size[index] = value.parse()?;
                settings.save_window(&path)?;
            }
            Change::Terminal(id, value)
            | Change::Field(
                id @ ("font-family"
                | "font-size"
                | "adjust-cell-height"
                | "adjust-cell-width"
                | "background-opacity"
                | "window-padding-x"
                | "window-padding-y"
                | "adjust-cursor-thickness"
                | "scroll-precision"
                | "scroll-discrete"),
                value,
            ) => {
                self.save_terminal(cx, |terminal| terminal.set_preference(id, &value))?;
            }
            Change::Field(id, value) if terminal_field(id) => {
                self.save_terminal(cx, |terminal| terminal.set_preference(id, &value))?;
            }
            Change::TerminalEdit(edit) => {
                self.save_terminal(cx, |terminal| terminal.edit(edit))?;
            }
            Change::ResetTerminal => {
                self.save_terminal(cx, |terminal| {
                    *terminal = muxy_app_core::settings::TerminalSettings::default();
                    Ok(())
                })?;
                let directory = muxy_app_core::settings::NewPaneDirectory::default();
                if settings.panes.new_pane_directory != directory {
                    settings.panes.new_pane_directory = directory;
                    settings.save_panes(&path)?;
                }
            }
            _ => return Err(tr!("Unknown app setting").to_string().into()),
        }
        self.adopt_settings(settings, theme_changed, bindings_changed, cx);
        Ok(())
    }

    /// Makes `settings`, already saved, the ones the app runs with.
    fn adopt_settings(
        &mut self,
        settings: muxy_app_core::settings::Settings,
        theme_changed: bool,
        bindings_changed: bool,
        cx: &mut Context<Self>,
    ) {
        let theme_changed = theme_changed
            || self.appearance.dark_theme != settings.appearance.dark_theme
            || self.appearance.light_theme != settings.appearance.light_theme
            || self.appearance.interface_size != settings.appearance.interface_size;
        let language_changed = self.appearance.language != settings.appearance.language;
        if settings.appearance.remember_projects_without_tabs
            && settings.appearance.projects_without_tabs_shown
                != self.appearance.projects_without_tabs_shown
        {
            self.projects_without_tabs_shown = settings.appearance.projects_without_tabs_shown;
        }
        self.appearance = settings.appearance.clone();
        self.settings = settings;
        self.sync_agents_panel(cx);
        self.sync_agent_sessions(cx);
        self.filter_agent_sessions(cx);
        self.invalidate_webview_shortcuts();
        if bindings_changed {
            self.bind_extension_keys(cx);
        }
        if theme_changed {
            self.refresh_theme(cx);
        }
        if language_changed {
            self.sync_language(cx);
        }
        for pane in self.grids.values() {
            pane.view.update(cx, |pane, cx| {
                pane.copy_on_select = self.settings.clipboard.copy_on_select;
                cx.notify();
            });
        }
    }

    /// Applies `settings.toml` and `terminal.toml` as they are on disk now,
    /// after something other than this app changed them, such as
    /// `muxy config`. Remote servers keep the list the app connected.
    pub(crate) fn reload_settings(
        &mut self,
        cx: &mut Context<Self>,
    ) -> std::result::Result<(), String> {
        if self.quitting != Quitting::Idle {
            return Err("Muxy is closing".to_owned());
        }
        if self.composer.preferences_dirty {
            self.settings
                .save_composer(&self.path.with_file_name("settings.toml"))
                .map_err(|error| error.to_string())?;
            self.composer.preferences_dirty = false;
        }
        let (sliders, edits): (Vec<_>, Vec<_>) = self
            .take_pending_preferences(cx)
            .into_iter()
            .partition(|change| matches!(change, Change::Terminal(..)));
        for change in sliders {
            self.change_preference(change, cx);
        }
        let reloaded = self.adopt_saved_settings(cx);
        for change in edits {
            self.change_preference(change, cx);
        }
        reloaded
    }

    /// Settings edits not saved yet, such as text still being typed.
    fn take_pending_preferences(&mut self, cx: &mut Context<Self>) -> Vec<Change> {
        self.settings_tab
            .as_ref()
            .map_or_else(Vec::new, |settings| {
                settings.view.update(cx, |view, cx| view.take_changes(cx))
            })
    }

    fn adopt_saved_settings(&mut self, cx: &mut Context<Self>) -> std::result::Result<(), String> {
        let loaded =
            muxy_app_core::settings::Settings::load(&self.path.with_file_name("settings.toml"))
                .and_then(|loaded| {
                    let terminal = muxy_app_core::settings::TerminalSettings::load_native(
                        &self.path.with_file_name("terminal.toml"),
                    )?;
                    loaded.validate_command_shortcuts(&terminal)?;
                    Ok((loaded, terminal))
                })
                .map_err(|error| error.to_string())
                .and_then(|(loaded, terminal)| {
                    Self::quick_keymap_conflict(&loaded.keymap, &loaded.quick_terminal)?;
                    Ok((loaded, terminal))
                });
        let (mut loaded, terminal) = match loaded {
            Ok(loaded) => loaded,
            Err(error) => {
                self.configuration_failed(&error, cx);
                return Err(error);
            }
        };
        if self.terminal_preview.is_none() {
            self.adopt_terminal(terminal, cx);
        }
        loaded.servers.clone_from(&self.settings.servers);
        let mut current = self.settings.clone();
        current.appearance = self.appearance.clone();
        if loaded == current {
            return Ok(());
        }
        let quick = std::mem::replace(&mut loaded.quick_terminal, current.quick_terminal.clone());
        let bindings_changed =
            loaded.keymap != current.keymap || loaded.commands != current.commands;
        let composer_changed = loaded.composer != current.composer;
        self.adopt_settings(loaded, false, bindings_changed, cx);
        if bindings_changed {
            cx.set_menus(crate::menus());
        }
        if composer_changed {
            self.composer.preferences_dirty = false;
            if let Some(view) = &self.composer.view {
                view.update(cx, |view, cx| {
                    view.settings = self.settings.composer.clone();
                    view.refresh_style(cx);
                });
            }
        }
        if quick != current.quick_terminal {
            self.adopt_quick_settings(quick, false, cx)?;
        }
        self.sync_preferences(cx);
        cx.notify();
        Ok(())
    }

    fn apply_composer_preference(&mut self, change: Change, cx: &mut Context<Self>) -> Result<()> {
        let path = self.path.with_file_name("settings.toml");
        let mut settings = self.settings.clone();
        match change {
            Change::Composer(toggle, value) => {
                match toggle {
                    ComposerToggle::ClearAfter => settings.composer.clear_after_sending = value,
                    ComposerToggle::ClearClose => settings.composer.clear_on_close = value,
                    ComposerToggle::Images => {
                        settings.composer.image_strategy = if value {
                            muxy_app_core::composer::submission::ImageSubmissionStrategy::Clipboard
                        } else {
                            muxy_app_core::composer::submission::ImageSubmissionStrategy::InlinePath
                        }
                    }
                }
                settings.save_composer(&path)?;
            }
            Change::Field(id @ ("composer-font" | "composer-line-height"), value) => {
                match id {
                    "composer-font" => settings.composer.font_family = value,
                    _ => settings.composer.line_height = value.parse()?,
                }
                settings.save_composer(&path)?;
            }
            _ => return Err(tr!("Unknown composer preference").to_string().into()),
        }
        self.settings.composer = settings.composer;
        self.composer.preferences_dirty = false;
        if let Some(view) = &self.composer.view {
            view.update(cx, |view, cx| {
                view.settings = self.settings.composer.clone();
                view.refresh_style(cx);
            });
        }
        Ok(())
    }

    #[cfg(all(target_os = "macos", not(test)))]
    pub(crate) fn sync_background_effects(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.sync_sidebar_vibrancy(window);
        let options = &self.terminal.options;
        if options.background_vibrancy == 0 || !self.terminal_effects_allowed() {
            self.terminal_background.native = None;
            self.set_terminal_vibrancy_available(false, cx);
            return;
        }
        let offset = self.sidebar_width();
        let width = (f32::from(window.viewport_size().width) - offset).max(0.0);
        let background = self.palette.background;
        let background = gpui::rgb(background).into();
        if let Some(effect) = &mut self.terminal_background.native {
            effect.update(offset, width, background);
        } else {
            self.terminal_background.native =
                muxy_ui::vibrancy::TerminalVibrancy::new(window, offset, width, background);
        }
        self.set_terminal_vibrancy_available(self.terminal_background.native.is_some(), cx);
    }

    #[cfg(all(target_os = "macos", not(test)))]
    pub(super) fn set_terminal_vibrancy_available(
        &mut self,
        available: bool,
        cx: &mut Context<Self>,
    ) {
        if self.terminal_background.available == available {
            return;
        }
        self.terminal_background.available = available;
        for (id, pane) in &self.grids {
            let enabled = available && !self.is_quick_terminal(*id);
            pane.view.update(cx, |pane, cx| {
                if pane.native_vibrancy != enabled {
                    pane.native_vibrancy = enabled;
                    cx.notify();
                }
            });
        }
    }

    pub(crate) fn confirm_terminal_reset(
        &mut self,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.quitting != Quitting::Idle {
            return;
        }
        self.ask_once(
            super::dialogs::Question::TerminalReset,
            window,
            async |parent, cx| crate::views::confirm::prompt_reset_terminal(parent, cx).await,
            |model, response, cx| match response {
                Ok(true) => model.change_preference(Change::ResetTerminal, cx),
                Ok(false) => {}
                Err(error) => model.preference_result("terminal-reset", Some(&error), cx),
            },
            cx,
        );
    }

    pub(crate) fn preview_terminal_preference(
        &mut self,
        id: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        let mut next = self.terminal.clone();
        if let Err(error) = next.set_preference(id, value) {
            self.preference_result(id, Some(&error.to_string()), cx);
            return;
        }
        if next == self.terminal {
            return;
        }
        self.terminal_preview
            .get_or_insert_with(|| TerminalPreview {
                settings: self.terminal.clone(),
                zoom: self
                    .grids
                    .iter()
                    .map(|(id, pane)| (*id, pane.view.read(cx).terminal.font_size))
                    .collect(),
            });
        self.preview_terminal_preferences(next, cx);
    }

    /// Shows `next` in the terminals while a slider moves, without the work a
    /// saved change does: the theme stays resolved and Settings keeps the
    /// value it shows itself until the slider is released.
    fn preview_terminal_preferences(
        &mut self,
        next: muxy_app_core::settings::TerminalSettings,
        cx: &mut Context<Self>,
    ) {
        let changed_size = self.terminal.font_size.to_bits() != next.font_size.to_bits();
        self.terminal = next;
        let previous_colors = self.palette.terminal_colors();
        let palette = self.palette.with_options(&self.terminal.options);
        let palette_changed = palette != self.palette;
        self.palette = palette;
        if previous_colors != self.palette.terminal_colors() {
            for server in self.servers.ids() {
                if self.ready(server) {
                    self.send(server, Work::Colors(self.palette.terminal_colors()), cx);
                }
            }
        }
        for (id, pane) in &self.grids {
            let quick = self.is_quick_terminal(*id);
            pane.view.update(cx, |pane, cx| {
                let zoom = pane.terminal.font_size;
                pane.terminal = self.terminal.clone();
                pane.configured_font_size = self.terminal.font_size;
                if !changed_size {
                    pane.terminal.font_size = zoom;
                }
                if palette_changed && !quick {
                    pane.palette = self.palette;
                }
                cx.notify();
            });
        }
        if palette_changed {
            self.refresh_quick_terminal(cx);
        }
        cx.notify();
    }

    fn save_terminal(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(
            &mut muxy_app_core::settings::TerminalSettings,
        ) -> muxy_app_core::settings::Result<()>,
    ) -> Result<()> {
        let result = muxy_app_core::settings::TerminalSettings::edit_native(
            &self.path.with_file_name("terminal.toml"),
            |requested| {
                edit(requested)?;
                self.settings.validate_command_shortcuts(requested)
            },
        );
        let preview = self.terminal_preview.take();
        match result {
            Ok(requested) => {
                let original = preview
                    .as_ref()
                    .map_or(&self.terminal, |preview| &preview.settings);
                let changed_size = original.font_size.to_bits() != requested.font_size.to_bits();
                self.apply_terminal_preferences(requested, changed_size, cx);
                if !changed_size && let Some(preview) = preview {
                    self.restore_terminal_zoom(preview.zoom, cx);
                }
                self.set_configuration_error(None);
                Ok(())
            }
            Err(error) => {
                if let Some(preview) = preview {
                    self.apply_terminal_preferences(preview.settings, false, cx);
                    self.restore_terminal_zoom(preview.zoom, cx);
                }
                Err(error.into())
            }
        }
    }

    fn restore_terminal_zoom(
        &mut self,
        zoom: std::collections::HashMap<super::PaneId, f32>,
        cx: &mut Context<Self>,
    ) {
        for (id, size) in zoom {
            if let Some(pane) = self.grids.get(&id) {
                pane.view.update(cx, |pane, cx| {
                    pane.terminal.font_size = size;
                    cx.notify();
                });
            }
        }
    }

    fn apply_terminal_preferences(
        &mut self,
        effective: muxy_app_core::settings::TerminalSettings,
        reset_zoom: bool,
        cx: &mut Context<Self>,
    ) {
        let changed_size =
            reset_zoom || self.terminal.font_size.to_bits() != effective.font_size.to_bits();
        self.terminal = effective;
        if reset_zoom {
            self.clear_pane_zooms();
        }
        for pane in self.grids.values() {
            pane.view.update(cx, |pane, cx| {
                let zoom = pane.terminal.font_size;
                pane.terminal = self.terminal.clone();
                pane.configured_font_size = self.terminal.font_size;
                if !changed_size {
                    pane.terminal.font_size = zoom;
                }
                cx.notify();
            });
        }
        self.refresh_theme(cx);
    }

    pub(crate) fn read_server_settings(&mut self, cx: &mut Context<Self>) {
        if self.quitting == Quitting::Idle
            && self.ready(ServerId::local())
            && !self.server_preferences.busy
            && !self.server_preferences.control_busy
        {
            self.server_preferences.busy =
                self.send(ServerId::local(), Work::ReadServerSettings, cx);
            self.server_preferences.row = "server".into();
            self.sync_preferences(cx);
        }
    }

    fn write_server_preference(&mut self, change: Change, cx: &mut Context<Self>) -> Result<()> {
        if !self.ready(ServerId::local()) || self.server_preferences.control_busy {
            return Err(tr!("Connect to the server before editing its settings")
                .to_string()
                .into());
        }
        let mut settings = self
            .server_preferences
            .document
            .clone()
            .ok_or_else(|| tr!("Load server settings first").to_string())?;
        let id = change_id(&change).to_owned();
        match change {
            Change::ShellIntegration(value) => settings.shell_integration = value,
            Change::Field("default-shell", value) => {
                settings.default_shell =
                    (!value.is_empty()).then(|| muxy_protocol::ServerPath::from(value));
            }
            Change::Field("history-budget", value) => {
                settings.history_budget_bytes = value
                    .parse::<u64>()?
                    .checked_mul(1024 * 1024)
                    .ok_or_else(|| tr!("History budget is too large").to_string())?;
            }
            _ => return Err(tr!("Unknown server setting").to_string().into()),
        }
        settings.validate().map_err(|_| {
            tr!("Use an absolute shell path and a history budget between 0 and 65536 MiB")
                .to_string()
        })?;
        self.server_preferences.busy =
            self.send(ServerId::local(), Work::WriteServerSettings(settings), cx);
        self.server_preferences.row = id;
        if !self.server_preferences.busy {
            return Err(tr!("Could not send server settings").to_string().into());
        }
        Ok(())
    }

    pub(super) fn receive_server_settings(
        &mut self,
        result: std::result::Result<muxy_protocol::ServerSettingsDoc, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.server_preferences.busy = false;
        match result {
            Ok(settings) => {
                self.server_preferences.document = Some(settings);
                self.preference_result(&self.server_preferences.row, None, cx);
            }
            Err(error) => {
                self.preference_result(&self.server_preferences.row, Some(&error.to_string()), cx);
            }
        }
        while !self.server_preferences.busy {
            let Some(change) = self.server_preferences.pending.pop_front() else {
                break;
            };
            self.change_preference(change, cx);
        }
        self.sync_preferences(cx);
    }

    /// A stopped server counts as disconnected. Restarting connects again,
    /// which starts it, over SSH too.
    pub(super) fn receive_server_stopped(
        &mut self,
        server: ServerId,
        restart: bool,
        result: std::result::Result<(), muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        if server.is_local() {
            self.server_preferences.control_busy = false;
            self.server_update_control_finished(&result, cx);
        } else if let Some(runtime) = self.servers.get_mut(server) {
            runtime.stopping = false;
        }
        match result {
            Ok(()) => {
                self.disconnect(server, cx);
                if restart {
                    self.connect_server(server, cx);
                } else if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.retry.held = true;
                }
            }
            Err(error) => {
                let message = self
                    .server_message(server, &tr!("Could not stop server: %@", error.to_string()));
                if server.is_local() {
                    self.preference_result("server", Some(&message), cx);
                }
                self.fail(message, cx);
            }
        }
        self.sync_preferences(cx);
    }

    /// Asks before stopping or restarting `server`, which ends its sessions.
    pub(crate) fn confirm_server_control(
        &mut self,
        server: ServerId,
        restart: bool,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !self.control_enabled(server) {
            return;
        }
        self.dismiss_overlay(cx);
        let generation = self.generation(server);
        let name = (!server.is_local()).then(|| self.server_label(server));
        self.ask_once(
            super::dialogs::Question::ServerControl(server, restart),
            window,
            async move |parent, cx| {
                crate::views::confirm::prompt_server(parent, restart, name.as_deref(), cx).await
            },
            move |model, response, cx| {
                model.focus_owner.request_active_pane();
                if generation == model.generation(server) {
                    match response {
                        Ok(true) => model.stop_server(server, restart, cx),
                        Ok(false) => {}
                        Err(error) if server.is_local() => {
                            model.preference_result("server", Some(&error), cx);
                        }
                        Err(error) => model.fail(error, cx),
                    }
                }
            },
            cx,
        );
    }

    fn stop_server(&mut self, server: ServerId, restart: bool, cx: &mut Context<Self>) {
        let sent = self.send(server, Work::StopServer { restart }, cx);
        if server.is_local() {
            self.server_preferences.control_busy = sent;
            if sent {
                self.server_update_control_started(restart);
            }
        } else if let Some(runtime) = self.servers.get_mut(server) {
            runtime.stopping = sent;
        }
        self.sync_preferences(cx);
    }
}

fn terminal_field(id: &str) -> bool {
    matches!(
        id,
        "font-feature"
            | "selection-word-chars"
            | "background"
            | "foreground"
            | "cursor-color"
            | "cursor-text"
            | "selection-foreground"
            | "selection-background"
    ) || id.starts_with("palette-")
}

fn change_id(change: &Change) -> &str {
    match change {
        Change::Command(_) | Change::RemoveCommand(_) => "commands",
        Change::QuickTerminal(_) => "quick-shortcut",
        Change::Theme(false, _) => "light-theme",
        Change::Theme(true, _) => "dark-theme",
        Change::Sidebar(_) => "sidebar",
        Change::SidebarVibrancy(_) => "sidebar-vibrancy",
        Change::ExtensionSidebar(_) => "extension-sidebar",
        Change::Language(_) => "app-language",
        Change::Worktrees(toggle, _) => toggle.id(),
        Change::AgentsPanelOption(toggle, _) => toggle.id(),
        Change::SidebarCollapsedStyle(_) => "sidebar-collapsed-style",
        Change::InterfaceSize(_) => "interface-size",
        Change::AgentsPanel(_) => "agents-panel",
        Change::AgentsPanelGrouping(_) => "agents-panel-grouping",
        Change::AgentsPanelIcon(_) => "agents-panel-icon",
        Change::AgentsPanelDensity(_) => "agents-panel-density",
        Change::StatusBar(_) => "status-bar",
        Change::Rail(_) => "rail",
        Change::PaneHeaders(_) => "pane-headers",
        Change::Tips(_) => "tips",
        Change::RememberProjectsWithoutTabs(_) => "remember-projects-without-tabs",
        Change::SidebarServers(_) => "sidebar-servers",
        Change::ConfirmProcess(_) => "confirm-process",
        Change::UsagePing(_) => "usage-ping",
        Change::CloseBehavior(_) => "close-behavior",
        Change::FileOpener(_) => "file-opener",
        Change::ProjectTarget(_) => "project-target",
        Change::Directory(_) => "directory",
        Change::Terminal("padding-left" | "padding-right", _) => "window-padding-x",
        Change::Terminal("padding-top" | "padding-bottom", _) => "window-padding-y",
        Change::Terminal("dismiss-import-notes", _) => "terminal-import-notes",
        Change::Composer(toggle, _) => toggle.id(),
        Change::Terminal(id, _) | Change::Field(id, _) | Change::QuickNumber(id, _) => id,
        Change::ResetTerminal => "terminal-reset",
        Change::TerminalEdit(edit) => match edit {
            TerminalEdit::Fallbacks(_) => "font-fallbacks",
            TerminalEdit::CodepointMap { .. } => "font-codepoint-map",
            TerminalEdit::Binding { .. } => "terminal-keybindings",
        },
        Change::Binding(id, _) | Change::Unassign(id) => id,
        Change::ShellIntegration(_) => "shell-integration",
        Change::Browser(change) => match change {
            BrowserChange::Enabled(_) => "browser-enabled",
            BrowserChange::TerminalLinks(_) => "browser-terminal-links",
            BrowserChange::SearchEngine(_) => "browser-search-engine",
            BrowserChange::Import { .. } => "browser-import",
            BrowserChange::DefaultProfile(_)
            | BrowserChange::AddProfile(_)
            | BrowserChange::RenameProfile(..)
            | BrowserChange::ClearProfile(_)
            | BrowserChange::DeleteProfile(_) => "browser-profiles",
        },
    }
}

/// `browser` with a change that lives in `settings.toml`.
fn browser_settings(
    mut browser: muxy_app_core::settings::BrowserSettings,
    change: BrowserChange,
) -> muxy_app_core::settings::BrowserSettings {
    match change {
        BrowserChange::Enabled(enabled) => browser.enabled = enabled,
        BrowserChange::TerminalLinks(enabled) => browser.open_terminal_links = enabled,
        BrowserChange::SearchEngine(engine) => browser.search_engine = engine,
        BrowserChange::DefaultProfile(id) => browser.default_profile = id,
        _ => {}
    }
    browser
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Transparency and vibrancy stay between 0 and 100"
)]
fn quick_number(
    current: &muxy_app_core::settings::QuickTerminalSettings,
    id: &str,
    value: u16,
) -> muxy_app_core::settings::QuickTerminalSettings {
    let mut next = current.clone();
    match id {
        "quick-width" => next.width = value,
        "quick-height" => next.height = value,
        "quick-transparency" => next.transparency = value.min(255) as u8,
        _ => next.blur = value.min(255) as u8,
    }
    next
}
