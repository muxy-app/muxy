pub(crate) mod ai;
pub(crate) mod banners;
mod catalog;
pub(crate) mod commands;
mod composer;
mod diagnostics;
pub(crate) mod extensions;
pub(crate) mod git;
mod language;
mod links;
mod mobile;
mod preferences;
pub(crate) mod project_layouts;
mod quick_terminal;
mod reconnect;
pub(crate) mod remote_files;
mod remote_servers;
pub(crate) use remote_servers::{
    ConnectionTest, DeviceForm, Install, PendingRemote, RemoteIntent, RemoteServer,
    join_destination, released_version, split_destination,
};
mod server_status;
mod servers;
pub(crate) use server_status::ServerStatus;
mod tabs;
pub(crate) mod tips;
mod updates;
pub(crate) use updates::UpdateAction;
mod voice;
mod webviews;
mod workspaces;
mod worktrees;
pub(crate) use worktrees::removes_worktree;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{AppContext, Context, Entity, FocusHandle, Subscription, Task, Window};
use muxy_app_core::{
    AppState, Direction, PaneContent, PaneId, ProjectId, ProjectStatus, ServerId, TabId,
    WindowBounds, restore, store,
};
use muxy_client::{ClientEvent, RunGrid};
use muxy_protocol::{SessionId, SessionInfo, Size};

use crate::boot::{Boot, Update, Work, missing_session};
use crate::views::overlays::Overlay;
use crate::views::terminal::colors::Palette;
use crate::views::terminal::pane::{PaneEvent, PaneState, TerminalPane};
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::tr;

pub(crate) struct PaneView {
    pub(crate) view: Entity<TerminalPane>,
    _subscription: Subscription,
}

impl PaneView {
    pub(crate) fn element(&self) -> gpui::AnyElement {
        use gpui::{IntoElement, Styled};
        gpui::AnyView::from(self.view.clone())
            .cached(
                gpui::div()
                    .size_full()
                    .min_w(gpui::px(0.0))
                    .min_h(gpui::px(0.0))
                    .style()
                    .clone(),
            )
            .into_any_element()
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &gpui::App) {
        self.view.read(cx).focus.focus(window);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnectionState {
    Connecting,
    Ready,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Quitting {
    Idle,
    Preserve,
    EndAll,
    Update,
}

struct CloseRequest {
    tab: TabId,
    tabs: Vec<TabId>,
    panes: Vec<PaneId>,
    checking: usize,
    whole_tab: bool,
    behavior: muxy_app_core::settings::CloseBehavior,
}

pub(crate) struct AppModel {
    pub(crate) extensions: extensions::Runtime,
    pub(crate) sidebar_view: Entity<crate::views::cached::CachedView<Self>>,
    pub(crate) webviews: webviews::Webviews,
    pub(crate) panels: muxy_ui::panel::PanelHost,
    pub(crate) composer: composer::ComposerRuntime,
    pub(crate) voice: voice::VoiceRuntime,
    pub(crate) git: git::GitState,
    pub(crate) ai: ai::Runtime,
    pub(crate) servers: servers::Servers,
    project_layouts: project_layouts::ProjectLayouts,
    pub(crate) existing_sessions: crate::views::session_picker::ExistingSessions,
    pub(crate) quick: quick_terminal::QuickTerminalRuntime,
    pub(crate) window: gpui::AnyWindowHandle,
    pub(crate) state: AppState,
    pub(crate) grids: HashMap<PaneId, PaneView>,
    pub(crate) error: Option<String>,
    error_detail: Option<String>,
    problem_serial: u64,
    pub(crate) notice: Option<String>,
    notice_body: Option<String>,
    notice_serial: u64,
    pub(crate) focus: FocusHandle,
    pub(crate) appearance: muxy_app_core::settings::Appearance,
    pub(crate) settings: muxy_app_core::settings::Settings,
    terminal: muxy_app_core::settings::TerminalSettings,
    server_preferences: preferences::ServerPreferences,
    mobile: mobile::MobileAccess,
    /// The last Test Connection in Settings, with the destination it tried.
    server_probe: Option<(String, ConnectionTest)>,
    server_probe_generation: u64,
    server_anchor: muxy_ui::popover::PopoverAnchor,
    pub(crate) remote_anchor: muxy_ui::popover::PopoverAnchor,
    pub(crate) pending_remote: Option<PendingRemote>,
    pub(crate) pending_folders: Vec<PathBuf>,
    pub(crate) remote_links: remote_files::RemoteLinks,
    pub(crate) tips: tips::Tips,
    pub(crate) settings_window: Option<preferences::SettingsWindowState>,
    pending_extension_updates: Option<Entity<crate::views::settings::extensions::ExtensionsView>>,
    font_sizes: HashMap<PaneId, f32>,
    initial_directories: HashMap<PaneId, PathBuf>,
    pub(crate) theme: Theme,
    pub(crate) themes: crate::theme::Catalog,
    pub(crate) metrics: Metrics,
    pub(crate) palette: Palette,
    pub(crate) dark: bool,
    pub(crate) overlay: Option<Overlay>,
    pub(crate) overlay_focus: FocusHandle,
    pub(crate) focus_requested: bool,
    pub(crate) split_resize: crate::views::splits::SplitResizeState,
    pub(crate) sidebar_resize: Option<crate::views::sidebar::SidebarResize>,
    pub(crate) tab_drag: crate::views::tab_strip::TabDragState,
    pub(crate) layout_drag: crate::views::splits::drag::LayoutDragState,
    pub(crate) project_logo_task: Option<Task<()>>,
    pub(crate) project_logos: crate::views::project_editor::logo::Cache,
    pub(crate) expanded_worktrees: HashSet<ProjectId>,
    pub(crate) tab_sidebar_selection: Option<(ProjectId, Option<TabId>)>,
    pub(crate) shortcut_hints: crate::views::shortcut_hints::ShortcutHints,
    #[cfg(target_os = "macos")]
    pub(crate) window_drag: Option<muxy_ui::window_drag::WindowDrag>,
    #[cfg(target_os = "macos")]
    pub(crate) sidebar_vibrancy: Option<muxy_ui::vibrancy::SidebarVibrancy>,
    pub(crate) overlay_subscription: Option<Subscription>,
    pub(crate) picker_search: crate::picker::search::SearchService,
    pub(crate) navigation: crate::navigation::Navigation,
    pub(crate) configuration_error: Option<String>,
    dismissed_banners: [Option<String>; 2],
    pub(crate) problem_toast: Option<banners::Toast>,
    pub(crate) notice_toast: Option<banners::Toast>,
    problem_task: Option<Task<()>>,
    notice_task: Option<Task<()>>,
    language_task: Option<Task<()>>,
    path: PathBuf,
    bounds_save: Option<Task<()>>,
    webview_shortcuts: Option<Rc<Vec<gpui::Keystroke>>>,
    pub(crate) spinners: crate::views::tab_activity::Spinners,
    /// Panes waiting for an attachment or saved output, with the server asked.
    pending: HashMap<PaneId, ServerId>,
    detached_pending: HashSet<PaneId>,
    pending_close: Option<TabId>,
    close_request: Option<CloseRequest>,
    pub(crate) close_prompt: Option<Task<()>>,
    retained: HashSet<PaneId>,
    loaded: HashSet<PaneId>,
    snapshots: HashMap<PaneId, RunGrid>,
    pub(crate) window_active: bool,
    #[cfg(target_os = "macos")]
    notifications: Option<muxy_ui::notifications::Notifications>,
    pub(crate) completions: HashSet<PaneId>,
    /// Servers still finishing their work before the app quits.
    flushing: HashSet<ServerId>,
    quitting: Quitting,
    updates: updates::Updater,
    _events: Task<()>,
    _appearance: Subscription,
    _bounds: Subscription,
    _activation: Subscription,
    _panes: Subscription,
    _quit: Subscription,
}

impl AppModel {
    pub(crate) fn terminal(&self, id: &PaneId) -> Option<&PaneView> {
        self.grids.get(id)
    }
    pub(crate) fn refresh_theme(&mut self, cx: &mut Context<Self>) {
        let previous_colors = self.palette.terminal_colors();
        let (theme, palette) = self.themes.resolve(&self.appearance, self.dark);
        self.theme = theme;
        self.palette = palette.with_options(&self.terminal.options);
        if previous_colors != self.palette.terminal_colors() {
            for server in self.servers.ids() {
                if self.ready(server) {
                    self.send(server, Work::Colors(self.palette.terminal_colors()), cx);
                }
            }
        }
        for pane in self.grids.values() {
            pane.view.update(cx, |pane, cx| {
                pane.palette = self.palette;
                pane.update_find_theme(&self.theme, cx);
                cx.notify();
            });
        }
        self.refresh_quick_terminal(cx);
        if let Some(view) = &self.composer.view {
            view.update(cx, |view, cx| {
                view.appearance(self.theme.clone(), self.metrics, cx);
            });
        }
        if let Some(view) = &self.voice.view {
            view.update(cx, |view, cx| {
                view.appearance(self.theme.clone(), self.metrics, cx);
            });
        }
        self.sync_preferences(cx);
        if let Some(Overlay::Commands { palette, dark }) = &self.overlay {
            let active = self.themes.active_name(&self.appearance, *dark);
            palette.update(cx, |palette, cx| {
                palette.set_appearance(self.theme.clone(), self.metrics, cx);
                if palette.is_page(crate::views::theme_picker::PAGE_ID) {
                    palette.set_current(&active, cx);
                }
            });
        }
        cx.notify();
    }

    pub(crate) fn reload_configuration(&mut self, cx: &mut Context<Self>) {
        let result = muxy_app_core::settings::TerminalSettings::load(
            &self.path.with_file_name("ghostty.conf"),
        )
        .and_then(|terminal| {
            self.settings.validate_command_shortcuts(&terminal)?;
            Ok(terminal)
        });
        match result {
            Ok(terminal) => {
                let themes = crate::theme::Catalog::load(&self.path.with_file_name("themes"));
                self.terminal = terminal;
                self.themes = themes;
                self.set_configuration_error(None);
                for pane in self.grids.values() {
                    let old = pane.view.read(cx);
                    let zoom = (old.terminal.font_size.to_bits()
                        != old.configured_font_size.to_bits())
                    .then_some(old.terminal.font_size);
                    pane.view.update(cx, |pane, cx| {
                        pane.terminal = self.terminal.clone();
                        pane.configured_font_size = self.terminal.font_size;
                        if let Some(size) = zoom {
                            pane.terminal.font_size = size;
                        }
                        cx.notify();
                    });
                }
                self.refresh_theme(cx);
            }
            Err(error) => {
                self.set_configuration_error(Some(
                    tr!("Could not reload configuration: %@", error.to_string()).to_string(),
                ));
                cx.notify();
            }
        }
    }

    pub(crate) fn reload_themes(&mut self, cx: &mut Context<Self>) {
        let was_theme_error = self.error.as_deref() == Some(self.themes.errors.join("; ").as_str());
        self.themes = crate::theme::Catalog::load(&self.path.with_file_name("themes"));
        if !self.themes.errors.is_empty() {
            self.fail(self.themes.errors.join("; "), cx);
        } else if was_theme_error {
            self.set_banner_error(None);
        }
        self.refresh_theme(cx);
    }

    pub(crate) fn save_project_search_root(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        if let Err(error) = self
            .settings
            .set_project_search_root(root, &self.path.with_file_name("settings.toml"))
        {
            self.fail(
                tr!("Could not save search location: %@", error.to_string()).to_string(),
                cx,
            );
        }
    }

    pub(crate) fn save_appearance(&mut self, cx: &mut Context<Self>) {
        match self.appearance.save_changes(
            &self.settings.appearance,
            &self.path.with_file_name("settings.toml"),
        ) {
            Ok(saved) => {
                let theme_changed = self.appearance.dark_theme != saved.dark_theme
                    || self.appearance.light_theme != saved.light_theme;
                self.appearance = saved;
                self.settings.appearance = self.appearance.clone();
                if theme_changed {
                    self.refresh_theme(cx);
                }
            }
            Err(error) => {
                self.appearance = self.settings.appearance.clone();
                self.refresh_theme(cx);
                self.fail(
                    tr!("Could not save appearance: %@", error.to_string()).to_string(),
                    cx,
                );
            }
        }
        self.sync_preferences(cx);
    }

    pub(crate) fn navigate(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some((index, (project, tab))) = self.navigation.target(forward, |(project, tab)| {
            self.navigation_target_exists(project, tab)
        }) && self.state.select_tab(project, tab).is_ok()
        {
            self.navigation.commit(index);
            self.changed(cx);
            self.focus_requested = true;
            self.connect_on_demand(self.state.current_project().server_id, cx);
        }
    }

    fn navigation_target_exists(&self, project: ProjectId, tab: TabId) -> bool {
        self.state.project(project).is_some_and(|project| {
            project.status() == ProjectStatus::Available
                && project.tabs.iter().any(|candidate| candidate.id == tab)
        })
    }

    pub(crate) fn can_navigate(&self, forward: bool) -> bool {
        self.navigation
            .target(forward, |(project, tab)| {
                self.navigation_target_exists(project, tab)
            })
            .is_some()
    }

    fn activation_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window_active = window.is_window_active();
        self.update_shortcut_hints(
            if self.window_active {
                window.modifiers()
            } else {
                gpui::Modifiers::default()
            },
            cx,
        );
        cx.notify();
        self.acknowledge_focused_activity(cx);
        if window.is_window_active() {
            if self.path.with_file_name("ghostty.conf").exists() {
                self.reload_configuration(cx);
            }
            self.refresh_git(cx);
            self.refresh_project_statuses(cx);
            self.reconnect_waiting(cx);
        } else {
            self.cancel_titlebar_drag(cx);
            self.finish_sidebar_resize(cx);
        }
        if let Some(pane) = self.active_pane().and_then(|id| self.terminal(&id)) {
            pane.view.update(cx, |pane, cx| {
                pane.focus_changed(window.is_window_active(), cx);
            });
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Application startup wires independent state and subscriptions"
    )]
    pub(crate) fn new(boot: Boot, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let events = cx.spawn(async move |this, cx| {
            while let Ok(update) = boot.updates.recv().await {
                if this
                    .update(cx, |model, cx| model.receive(update, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let appearance = cx.observe_window_appearance(window, |model: &mut Self, window, cx| {
            model.dark = crate::theme::is_dark(window);
            model.refresh_theme(cx);
        });
        let dark = crate::theme::is_dark(window);
        let bounds = cx.observe_window_bounds(window, |model: &mut Self, window, cx| {
            model.save_bounds(window, cx);
        });
        let activation = cx.observe_window_activation(window, Self::activation_changed);
        let panes = cx.observe(&cx.entity(), |model: &mut Self, _, cx| {
            model.sync_pane_focus(cx);
        });
        let quit = cx.on_app_quit(|model: &mut Self, cx| {
            #[cfg(not(test))]
            crate::ai::terminate_all();
            model.close_voice(cx);
            model.save(cx);
            model.flush_composer(cx)
        });
        let themes = crate::theme::Catalog::load(&boot.state_path.with_file_name("themes"));
        let (theme, palette) = themes.resolve(&boot.settings.appearance, dark);
        let palette = palette.with_options(&boot.terminal.options);
        let configuration_error = boot.import_message;
        let theme_error = (!themes.errors.is_empty()).then(|| themes.errors.join("; "));
        cx.on_release(|model: &mut Self, cx| {
            for (_, (_, image)) in model.project_logos.drain() {
                image.remove_asset(cx);
            }
        })
        .detach();
        let mut model = Self {
            extensions: extensions::Runtime::new(
                boot.state_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
            ),
            sidebar_view: crate::views::cached::CachedView::new(
                |model, window, cx| crate::views::sidebar::sidebar(model, window, cx),
                cx,
            ),
            webviews: webviews::Webviews::default(),
            panels: muxy_ui::panel::PanelHost::default(),
            composer: composer::ComposerRuntime::new(boot.composer),
            voice: voice::VoiceRuntime::default(),
            git: git::GitState::default(),
            ai: ai::Runtime::default(),
            servers: servers::Servers::new(
                boot.workers,
                boot.work,
                boot.state_path
                    .with_file_name(format!("askpass-{}.sock", std::process::id())),
            ),
            existing_sessions: crate::views::session_picker::ExistingSessions::default(),
            quick: quick_terminal::QuickTerminalRuntime::new(&boot.settings.quick_terminal, cx),
            window: window.window_handle(),
            state: boot.state,
            grids: HashMap::new(),
            error: theme_error,
            error_detail: None,
            problem_serial: 0,
            notice: None,
            notice_body: None,
            notice_serial: 0,
            focus: cx.focus_handle(),
            appearance: boot.settings.appearance.clone(),
            project_logo_task: None,
            project_logos: HashMap::new(),
            expanded_worktrees: HashSet::new(),
            tab_sidebar_selection: None,
            shortcut_hints: crate::views::shortcut_hints::ShortcutHints::default(),
            settings: boot.settings,
            terminal: boot.terminal,
            server_preferences: preferences::ServerPreferences::default(),
            mobile: mobile::MobileAccess::default(),
            server_probe: None,
            server_probe_generation: 0,
            server_anchor: Rc::default(),
            remote_anchor: Rc::default(),
            pending_remote: None,
            pending_folders: Vec::new(),
            remote_links: remote_files::RemoteLinks::default(),
            tips: tips::Tips::default(),
            settings_window: None,
            pending_extension_updates: None,
            font_sizes: HashMap::new(),
            initial_directories: HashMap::new(),
            project_layouts: project_layouts::ProjectLayouts::default(),
            theme,
            themes,
            metrics: Metrics::new(1.0),
            palette,
            dark,
            overlay: None,
            overlay_focus: cx.focus_handle(),
            focus_requested: false,
            split_resize: crate::views::splits::SplitResizeState::default(),
            sidebar_resize: None,
            tab_drag: crate::views::tab_strip::TabDragState::default(),
            layout_drag: crate::views::splits::drag::LayoutDragState::default(),
            #[cfg(target_os = "macos")]
            window_drag: muxy_ui::window_drag::WindowDrag::new(&window.window_title()),
            #[cfg(target_os = "macos")]
            sidebar_vibrancy: None,
            overlay_subscription: None,
            picker_search: crate::picker::search::SearchService::default(),
            navigation: crate::navigation::Navigation::default(),
            configuration_error,
            dismissed_banners: [None, None],
            problem_toast: None,
            notice_toast: None,
            problem_task: None,
            notice_task: None,
            language_task: None,
            path: boot.state_path,
            bounds_save: None,
            webview_shortcuts: None,
            spinners: crate::views::tab_activity::Spinners::default(),
            pending: HashMap::new(),
            detached_pending: HashSet::new(),
            pending_close: None,
            close_request: None,
            close_prompt: None,
            retained: HashSet::new(),
            loaded: HashSet::new(),
            snapshots: HashMap::new(),
            window_active: window.is_window_active(),
            #[cfg(target_os = "macos")]
            notifications: Self::start_notifications(cx),
            completions: HashSet::new(),
            flushing: HashSet::new(),
            quitting: Quitting::Idle,
            updates: updates::Updater::default(),
            _events: events,
            _appearance: appearance,
            _bounds: bounds,
            _activation: activation,
            _panes: panes,
            _quit: quit,
        };
        if !model.project_shown(model.state.current_project()) {
            let home = model.state.home().id;
            let _ = model.state.select_project(home);
        }
        model.bind_extension_keys(cx);
        model.refresh_installed_extensions(cx);
        #[cfg(not(test))]
        Self::discover_ai_providers(cx);
        model.sync_visible(cx);
        model.sync_pane_focus(cx);
        model.save_bounds(window, cx);
        model.save(cx);
        model.focus.focus(window);
        if let Some(tab) = model.active_tab() {
            model
                .navigation
                .record((model.state.current_project().id, tab));
        }
        Self::start_diagnostics(cx);
        model.start_update_checks(cx);
        model.start_servers(cx);
        model
    }

    pub(crate) fn active_tab(&self) -> Option<TabId> {
        let project = self.state.current_project();
        let selected = self.state.window().selected_tab.get(&project.id).copied()?;
        if project.status() == ProjectStatus::Missing {
            return None;
        }
        Some(selected)
    }

    pub(crate) fn active_pane(&self) -> Option<PaneId> {
        self.active_tab()?;
        self.state.window().active_pane
    }

    /// The tab each of the current project's groups shows.
    pub(crate) fn visible_tabs(&self) -> Vec<TabId> {
        if self.active_tab().is_none() {
            return Vec::new();
        }
        self.state.visible_tabs(self.state.current_project().id)
    }

    pub(crate) fn visible_panes(&self) -> Vec<PaneId> {
        self.visible_tabs()
            .into_iter()
            .filter_map(|id| self.tab(id))
            .flat_map(muxy_app_core::Tab::visible_panes)
            .collect()
    }

    pub(crate) fn split_pane(&mut self, edge: Direction, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle || self.close_request.is_some() {
            return;
        }
        let Some(source) = self.active_pane() else {
            return;
        };
        let directory = if self.settings.panes.new_pane_directory
            == muxy_app_core::settings::NewPaneDirectory::Current
        {
            self.terminal(&source)
                .and_then(|pane| pane.view.read(cx).directory())
                .unwrap_or_else(|| self.state.current_project().directory.clone())
        } else {
            self.state.current_project().directory.clone()
        };
        let mut size = self
            .terminal(&source)
            .and_then(|pane| pane.view.read(cx).viewport())
            .unwrap_or(Size { cols: 80, rows: 24 });
        match edge.axis() {
            muxy_app_core::Axis::Horizontal => size.cols = (size.cols / 2).max(1),
            muxy_app_core::Axis::Vertical => size.rows = (size.rows / 2).max(1),
        }
        let previous = self.state.clone();
        let pane = match self.state.split_pane(source, edge) {
            Ok(pane) => pane,
            Err(error) => {
                self.fail(error.to_string(), cx);
                return;
            }
        };
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        self.initial_directories.insert(pane, directory);
        self.split_resize.end();
        self.sync_visible(cx);
        self.focus_requested = true;
        let server = self.state.pane_server(pane).unwrap_or_else(ServerId::local);
        match self.connection(server) {
            ConnectionState::Ready => self.start_attach(pane, size, cx),
            ConnectionState::Disconnected => self.connect_server(server, cx),
            ConnectionState::Connecting => {}
        }
        cx.notify();
    }

    pub(crate) fn focus_pane(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle
            || self
                .pane_tab(pane)
                .is_none_or(|tab| !self.visible_tabs().contains(&tab))
            || self.active_pane() == Some(pane)
        {
            return;
        }
        if self.state.focus_pane(pane).is_ok() {
            self.focus_requested = true;
            self.changed(cx);
        }
    }

    /// Moves focus to the neighboring pane, or into the neighboring group.
    pub(crate) fn focus_direction(&mut self, direction: Direction, cx: &mut Context<Self>) {
        let Some(pane) = self.active_pane() else {
            return;
        };
        if let Some(neighbor) = self.state.neighbor(pane, direction) {
            self.focus_pane(neighbor, cx);
            return;
        }
        let project = self.state.current_project();
        if let Some(tab) = project.groups().and_then(|groups| {
            let group = groups.group_of(self.active_tab()?)?.id();
            let neighbor = groups.layout().neighbor(group, direction)?;
            groups
                .group(neighbor)
                .map(muxy_app_core::TabGroup::selected)
        }) {
            self.select_tab(tab, cx);
        }
    }

    pub(crate) fn toggle_zoom_pane(&mut self, cx: &mut Context<Self>) {
        if self.quitting == Quitting::Idle
            && let Some(pane) = self.active_pane()
            && self.state.toggle_zoom(pane).is_ok()
        {
            self.split_resize.end();
            self.focus_requested = true;
            self.changed(cx);
        }
    }

    fn tab_project(&self, tab: TabId) -> Option<&muxy_app_core::Project> {
        self.state
            .projects()
            .iter()
            .find(|project| project.tabs.iter().any(|candidate| candidate.id == tab))
    }

    pub(crate) fn select_project(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        crate::diagnostics::event(
            "project.select",
            format_args!(
                "from={:?} to={project:?} quitting={:?}",
                self.state.current_project().id,
                self.quitting
            ),
        );
        if self.quitting != Quitting::Idle {
            return;
        }
        if self.state.select_project(project).is_ok() {
            self.dismiss_overlay(cx);
            self.changed(cx);
            self.focus_requested = true;
            self.connect_on_demand(self.state.current_project().server_id, cx);
        }
    }

    pub(crate) fn navigation_projects(&self) -> Vec<ProjectId> {
        self.listed_parents()
            .into_iter()
            .filter(|project| project.status() == ProjectStatus::Available)
            .map(|project| project.id)
            .collect()
    }

    pub(crate) fn cycle_project(&mut self, forward: bool, cx: &mut Context<Self>) {
        let projects = self.navigation_projects();
        if projects.is_empty() {
            return;
        }
        let next = projects
            .iter()
            .position(|id| {
                *id == self.state.current_project().id
                    || Some(*id) == self.state.current_project().parent_id
            })
            .map_or(0, |index| {
                if forward {
                    (index + 1) % projects.len()
                } else {
                    (index + projects.len() - 1) % projects.len()
                }
            });
        let target = self.preferred_worktree(projects[next]);
        self.select_project(target, cx);
    }

    pub(crate) fn add_project(
        &mut self,
        directory: PathBuf,
        cx: &mut Context<Self>,
    ) -> Option<ProjectId> {
        self.add_project_on(ServerId::local(), directory, cx)
    }

    /// Adds `directory` on `server` as a project; another computer's must be
    /// an absolute path there.
    pub(crate) fn add_project_on(
        &mut self,
        server: ServerId,
        directory: PathBuf,
        cx: &mut Context<Self>,
    ) -> Option<ProjectId> {
        let mut added = None;
        if !self.edit_project(
            |state| {
                added = Some(state.add_project(server, directory)?);
                Ok(())
            },
            cx,
        ) {
            return None;
        }
        let project = added?;
        self.hide_new_project_worktrees(project, cx);
        Some(project)
    }

    pub(crate) fn edit_project(
        &mut self,
        edit: impl FnOnce(&mut AppState) -> Result<(), muxy_app_core::AppError>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.quitting != Quitting::Idle {
            return false;
        }
        let previous = self.state.clone();
        if let Err(error) = edit(&mut self.state) {
            self.state = previous;
            self.fail(error.to_string(), cx);
            return false;
        }
        if !self.save(cx) {
            self.state = previous;
            return false;
        }
        if let Some(tab) = self.active_tab() {
            self.navigation
                .record((self.state.current_project().id, tab));
        }
        self.replay_projects(cx);
        self.sync_visible(cx);
        self.focus_requested = true;
        cx.notify();
        true
    }

    pub(crate) fn move_project(&mut self, from: ProjectId, to: ProjectId, cx: &mut Context<Self>) {
        if let Some(index) = self
            .state
            .projects()
            .iter()
            .position(|project| project.id == to)
        {
            self.edit_project(|state| state.move_project(from, index), cx);
        }
    }

    pub(crate) fn remove_project_confirmed(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(server) = self.state.project_server(project) else {
            return;
        };
        if self.edit_project(
            |state| {
                for session in state.remove_project(project)? {
                    state.queue_discard(server, session);
                }
                Ok(())
            },
            cx,
        ) {
            self.discard_pending(server, cx);
        }
    }

    pub(crate) fn refresh_project_statuses(&mut self, cx: &mut Context<Self>) {
        self.state.refresh_project_statuses();
        for server in self.servers.ids() {
            self.check_remote_projects(server, cx);
        }
        self.sync_visible(cx);
        cx.notify();
    }

    pub(crate) fn new_tab(&mut self, cx: &mut Context<Self>) {
        self.new_tab_in(self.state.current_project().id, cx);
    }

    /// Opens a terminal tab in `project` and shows it, switching the sidebar's
    /// workspace when it doesn't list the project.
    pub(crate) fn new_tab_in(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle {
            return;
        }
        match self.state.open_terminal_tab(project) {
            Ok(_) => {
                self.changed(cx);
                self.focus_requested = true;
            }
            Err(error) => self.fail(error.to_string(), cx),
        }
        let server = self.state.current_project().server_id;
        if self.connection(server) == ConnectionState::Disconnected {
            self.connect_server(server, cx);
        }
    }

    pub(crate) fn select_tab(&mut self, tab: TabId, cx: &mut Context<Self>) {
        if self.active_tab() == Some(tab) {
            return;
        }
        let Some(project) = self.tab_project(tab).map(|project| project.id) else {
            return;
        };
        match self.state.select_tab(project, tab) {
            Ok(()) => {
                self.changed(cx);
                self.focus_requested = true;
                self.connect_on_demand(self.state.current_project().server_id, cx);
            }
            Err(error) => self.fail(error.to_string(), cx),
        }
    }

    pub(crate) fn cycle_tab(&mut self, forward: bool, cx: &mut Context<Self>) {
        let tabs = self.navigation_tabs();
        if tabs.is_empty() {
            return;
        }
        let current = tabs.iter().position(|tab| Some(*tab) == self.active_tab());
        let next = match (current, forward) {
            (Some(current), true) => (current + 1) % tabs.len(),
            (Some(current), false) => (current + tabs.len() - 1) % tabs.len(),
            (None, true) => 0,
            (None, false) => tabs.len() - 1,
        };
        self.select_tab(tabs[next], cx);
    }

    pub(crate) fn move_tab(&mut self, from: TabId, to: TabId, cx: &mut Context<Self>) {
        let tabs = &self.state.current_project().tabs;
        let positions = (
            tabs.iter().position(|tab| tab.id == from),
            tabs.iter().position(|tab| tab.id == to),
        );
        if let (Some(from), Some(to)) = positions {
            match self
                .state
                .move_tab(self.state.current_project().id, from, to)
            {
                Ok(()) => self.changed(cx),
                Err(error) => self.fail(error.to_string(), cx),
            }
        }
    }

    pub(crate) fn move_agent_tab(&mut self, from: TabId, to: TabId, cx: &mut Context<Self>) {
        let project = self.state.current_project();
        let id = project.id;
        let visible: Vec<_> = self.sidebar_tabs(project).map(|tab| tab.id).collect();
        if !visible.contains(&from) || !visible.contains(&to) {
            return;
        }
        match self.state.move_visible_tab(id, from, to, &visible) {
            Ok(()) => self.changed(cx),
            Err(error) => self.fail(error.to_string(), cx),
        }
    }

    pub(crate) fn close_tab(&mut self, tab: TabId, cx: &mut Context<Self>) {
        self.begin_close_tabs(vec![tab], None, cx);
    }

    pub(crate) fn close_pane(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if let Some(tab) = self.pane_tab(pane) {
            self.begin_close_tabs(vec![tab], Some(pane), cx);
        }
    }

    pub(crate) fn can_detach_terminal(&self, pane: PaneId) -> bool {
        self.quitting == Quitting::Idle
            && self.close_request.is_none()
            && self.pending_close.is_none()
            && self.close_prompt.is_none()
            && self.pane_tab(pane).is_some()
            && self.pane_session(pane).is_some()
    }

    pub(crate) fn detach_terminal(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if !self.can_detach_terminal(pane) {
            return;
        }
        let previous = self.state.clone();
        if let Err(error) = self.state.detach_pane(pane) {
            self.state = previous;
            self.fail(error.to_string(), cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        if self.pending.contains_key(&pane) {
            self.detached_pending.insert(pane);
        }
        if let Some(tab) = self.active_tab() {
            self.navigation
                .record((self.state.current_project().id, tab));
        }
        self.split_resize.end();
        self.dismiss_overlay(cx);
        self.focus_requested = true;
        self.sync_visible(cx);
        cx.notify();
    }

    fn begin_close_tabs(&mut self, tabs: Vec<TabId>, pane: Option<PaneId>, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle
            || self.close_request.is_some()
            || self.pending_close.is_some()
            || self.close_prompt.is_some()
        {
            return;
        }
        let Some(tab) = tabs.first().copied() else {
            return;
        };
        let Some(project) = self.tab_project(tab) else {
            return;
        };
        if project.status() == ProjectStatus::Missing {
            return;
        }
        let targets: Vec<_> = project
            .tabs
            .iter()
            .filter(|target| tabs.contains(&target.id))
            .collect();
        if targets.len() != tabs.len()
            || targets
                .iter()
                .any(|target| target.pinned && (pane.is_none() || target.panes.len() == 1))
        {
            return;
        }
        let panes = pane.map_or_else(
            || {
                targets
                    .iter()
                    .flat_map(|target| target.layout.leaves())
                    .collect()
            },
            |pane| vec![pane],
        );
        self.close_request = Some(CloseRequest {
            tab,
            tabs,
            panes,
            checking: 0,
            whole_tab: pane.is_none(),
            behavior: self.settings.window.close_behavior,
        });
        self.check_webview_closes(cx);
    }

    fn check_next_close(&mut self, cx: &mut Context<Self>) {
        while let Some(request) = &self.close_request {
            if request.behavior == muxy_app_core::settings::CloseBehavior::Detach
                || !self.settings.window.confirm_running_process
                || request.checking == request.panes.len()
            {
                self.close_confirmed(cx);
                return;
            }
            let tab = request.tab;
            let pane = request.panes[request.checking];
            let server = self.state.pane_server(pane).unwrap_or_else(ServerId::local);
            if let Some(session) = self.pane_session(pane)
                && self.session_used_outside(server, session, &request.panes)
            {
                if let Some(request) = &mut self.close_request {
                    request.checking += 1;
                }
                continue;
            }
            if self.ready(server)
                && !self.retained.contains(&pane)
                && let Some(session) = self.pane_session(pane)
            {
                let size = self
                    .terminal(&pane)
                    .and_then(|pane| pane.view.read(cx).viewport())
                    .unwrap_or(Size { cols: 80, rows: 24 });
                self.pending_close = Some(tab);
                if !self.send(server, Work::CheckClose { tab, session, size }, cx) {
                    self.close_request = None;
                    self.pending_close = None;
                }
                return;
            }
            if let Some(view) = self.terminal(&pane).map(|pane| pane.view.read(cx))
                && view.channel().is_some()
                && view.state == PaneState::Live
                && view
                    .process
                    .as_ref()
                    .is_some_and(|process| !process.is_shell)
            {
                self.confirm_close(tab, cx);
                return;
            }
            if let Some(request) = &mut self.close_request {
                request.checking += 1;
            }
        }
    }

    fn session_used_outside(
        &self,
        server: ServerId,
        session: SessionId,
        closing: &[PaneId],
    ) -> bool {
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .chain(self.state.quick_terminal())
            .any(|pane| {
                !closing.contains(&pane.id)
                    && self.pane_session(pane.id) == Some(session)
                    && self.state.pane_server(pane.id) == Some(server)
            })
    }

    pub(crate) fn closing_one_pane(&self) -> bool {
        self.close_request
            .as_ref()
            .is_some_and(|request| !request.whole_tab)
    }

    pub(crate) fn closing_multiple_tabs(&self) -> bool {
        self.close_request
            .as_ref()
            .is_some_and(|request| request.tabs.len() > 1)
    }

    pub(crate) fn finish_close_prompt(
        &mut self,
        tab: TabId,
        response: Result<muxy_ui::dialog::ConfirmationResponse, String>,
        cx: &mut Context<Self>,
    ) {
        self.close_prompt = None;
        self.focus_requested = true;
        if self
            .close_request
            .as_ref()
            .is_none_or(|request| request.tab != tab)
        {
            return;
        }
        match response {
            Ok(muxy_ui::dialog::ConfirmationResponse::Confirmed { dont_ask_again }) => {
                if dont_ask_again
                    && let Err(error) = self.settings.set_confirm_running_process(
                        false,
                        &self.path.with_file_name("settings.toml"),
                    )
                {
                    self.fail(
                        tr!(
                            "Could not save close confirmation preference: %@",
                            error.to_string()
                        )
                        .to_string(),
                        cx,
                    );
                }
                if let Some(request) = &mut self.close_request {
                    request.checking = request.panes.len();
                }
                self.check_next_close(cx);
            }
            Ok(muxy_ui::dialog::ConfirmationResponse::Cancelled) => {
                self.close_request = None;
            }
            Err(error) => {
                self.close_request = None;
                self.fail(
                    tr!("Could not show close confirmation: %@", &error).to_string(),
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn close_confirmed(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.close_request.take() else {
            return;
        };
        if self.quitting != Quitting::Idle {
            return;
        }
        let Some(project) = self.tab_project(request.tab) else {
            return;
        };
        if project.status() == ProjectStatus::Missing
            || request.tabs.iter().any(|id| {
                self.tab(*id)
                    .is_none_or(|tab| tab.pinned && (request.whole_tab || tab.panes.len() == 1))
            })
        {
            return;
        }
        let (project_id, server) = (project.id, project.server_id);
        let previous = self.state.clone();
        let closing: Vec<_> = request
            .panes
            .iter()
            .filter_map(|pane| self.pane_session(*pane))
            .collect();
        let detach = request.behavior == muxy_app_core::settings::CloseBehavior::Detach;
        let result = if detach {
            request
                .panes
                .iter()
                .try_for_each(|pane| self.state.detach_pane(*pane))
        } else if request.whole_tab {
            request
                .tabs
                .iter()
                .try_for_each(|tab| self.state.close_tab(project_id, *tab))
        } else {
            self.state.close_pane(request.panes[0])
        };
        for session in closing {
            if !detach && !self.state.session_references(server).contains(&session) {
                self.state.queue_discard(server, session);
            }
        }
        if result.is_err() || !self.save(cx) {
            self.state = previous;
            return;
        }
        if detach {
            self.detached_pending.extend(
                request
                    .panes
                    .iter()
                    .filter(|pane| self.pending.contains_key(pane))
                    .copied(),
            );
        }
        if let Some(tab) = self.active_tab() {
            self.navigation
                .record((self.state.current_project().id, tab));
        }
        self.split_resize.end();
        self.focus_requested = true;
        self.sync_visible(cx);
        self.discard_pending(server, cx);
        cx.notify();
    }

    pub(crate) fn quit(&mut self, cx: &mut Context<Self>) {
        crate::diagnostics::event(
            "quit.request",
            format_args!(
                "quitting={:?} pending={:?} discarding={:?}",
                self.quitting, self.pending, self.servers.local.discarding
            ),
        );
        if self.quitting != Quitting::Idle || !self.preferences_before_quit(cx) {
            return;
        }
        self.quitting = Quitting::Preserve;
        if !self.flush_servers(cx) {
            self.quitting = Quitting::Idle;
        }
    }

    pub(crate) fn end_all_and_quit(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle || !self.preferences_before_quit(cx) {
            return;
        }
        let local = ServerId::local();
        if !self.ready(local) {
            self.fail(
                tr!("Connect to the server before ending all sessions").to_string(),
                cx,
            );
            return;
        }
        let mut sessions = self.state.session_references(local);
        sessions.extend(self.state.pending_discards(local));
        self.quitting = Quitting::EndAll;
        if !self.send(local, Work::EndAll(sessions), cx) {
            self.quitting = Quitting::Idle;
        }
    }

    fn is_terminal(&self, id: PaneId) -> bool {
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .chain(self.state.quick_terminal())
            .any(|pane| pane.id == id && matches!(pane.content, PaneContent::Terminal { .. }))
    }

    pub(crate) fn pane_session(&self, id: PaneId) -> Option<SessionId> {
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .chain(self.state.quick_terminal())
            .find(|pane| pane.id == id)
            .and_then(|pane| match pane.content {
                PaneContent::Terminal { session } => session,
                PaneContent::Settings | PaneContent::Webview(_) => None,
            })
    }

    fn pane_tab(&self, id: PaneId) -> Option<TabId> {
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .find(|tab| tab.panes.iter().any(|pane| pane.id == id))
            .map(|tab| tab.id)
    }

    fn discard_pending(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !self.ready(server) || !self.confirmed(server) {
            return;
        }
        for operation in self.state.pending_cancellations(server).to_vec() {
            let cancelling = self
                .servers
                .get_mut(server)
                .is_some_and(|runtime| runtime.catalog.cancelling.insert(operation));
            if cancelling && !self.send(server, Work::CancelCreation(operation), cx) {
                if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.catalog.cancelling.remove(&operation);
                }
                break;
            }
        }
        self.sync_references(cx);
        self.state.prepare_closes(server);
        if !self.state.pending_discards(server).is_empty() && !self.save(cx) {
            return;
        }
        for session in self.state.pending_discards(server).to_vec() {
            let Some(operation) = self.state.close_operation(server, session) else {
                continue;
            };
            let discarding = self
                .servers
                .get_mut(server)
                .is_some_and(|runtime| runtime.discarding.insert(session));
            if discarding && !self.send(server, Work::Discard(session, operation), cx) {
                if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.discarding.remove(&session);
                }
                break;
            }
        }
    }

    fn discard_created(&mut self, server: ServerId, session: SessionId, cx: &mut Context<Self>) {
        self.state.queue_discard(server, session);
        if self.save(cx) {
            self.discard_pending(server, cx);
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.state.reveal_current_project();
        self.sync_extension_events(cx);
        self.split_resize.end();
        if let Some(tab) = self.active_tab() {
            self.navigation
                .record((self.state.current_project().id, tab));
        }
        self.save(cx);
        self.sync_visible(cx);
        cx.notify();
    }

    pub(crate) fn save_split_resize(&mut self, cx: &mut Context<Self>) {
        self.save(cx);
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) -> bool {
        self.bounds_save = None;
        match store::save(&self.path, &self.state) {
            Ok(()) => true,
            Err(error) => {
                self.fail(
                    tr!("Could not save tabs: %@", error.to_string()).to_string(),
                    cx,
                );
                false
            }
        }
    }

    fn save_bounds(&mut self, window: &Window, cx: &mut Context<Self>) {
        let bounds = window.window_bounds().get_bounds();
        let bounds = WindowBounds {
            x: f64::from(f32::from(bounds.origin.x)),
            y: f64::from(f32::from(bounds.origin.y)),
            width: f64::from(f32::from(bounds.size.width)),
            height: f64::from(f32::from(bounds.size.height)),
        };
        if self.state.window().bounds != Some(bounds)
            && self.state.set_window_bounds(Some(bounds)).is_ok()
        {
            self.bounds_save = Some(cx.spawn(async move |model, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(200))
                    .await;
                let _ = model.update(cx, AppModel::save);
            }));
        }
    }

    /// Tells each connected server which of its sessions this app shows, so
    /// a server whose last pane closed hears an empty list.
    fn sync_references(&mut self, cx: &mut Context<Self>) {
        for server in self.servers.ids() {
            if !self.ready(server) {
                continue;
            }
            let references = self.state.session_references(server);
            if self
                .servers
                .get(server)
                .is_some_and(|runtime| runtime.references.as_ref() != Some(&references))
                && self.send(server, Work::References(references.clone()), cx)
                && let Some(runtime) = self.servers.get_mut(server)
            {
                runtime.references = Some(references);
            }
        }
    }

    /// The project shown is always one the sidebar lists, never a removed
    /// server's project or another computer's Home outside Remote Servers, so
    /// new tabs never open somewhere the user can't see.
    fn sync_visible(&mut self, cx: &mut Context<Self>) {
        if self.state.remote_servers_selected() && !self.remote_servers_offered() {
            let _ = self.state.select_workspace(None);
        }
        if !self.project_listed(self.state.current_project()) {
            let home = self.state.home().id;
            let _ = self.state.select_project(home);
            self.state.reveal_current_project();
        }
        self.sync_composer(cx);
        self.sync_project_layouts(cx);
        self.sync_voice(cx);
        self.sync_git(cx);
        self.sync_references(cx);
        self.refresh_existing_sessions(cx);
        let visible = self.attached_panes();
        let hidden: Vec<_> = self
            .grids
            .keys()
            .copied()
            .filter(|id| !visible.contains(id))
            .collect();
        for id in hidden {
            if let Some(pane) = self.grids.remove(&id) {
                let terminal = pane.view.read(cx);
                if terminal.terminal.font_size.to_bits() == terminal.configured_font_size.to_bits()
                {
                    self.font_sizes.remove(&id);
                } else {
                    self.font_sizes.insert(id, terminal.terminal.font_size);
                }
                pane.view.update(cx, |pane, cx| {
                    pane.set_focused(false, cx);
                    pane.native_visible = false;
                    #[cfg(target_os = "macos")]
                    if let Some(scroll) = &pane.native_scroll {
                        scroll.set_visible(false);
                    }
                });
                if let Some((server, channel)) = pane.view.read(cx).attachment()
                    && self.ready(server)
                {
                    self.send(server, Work::Detach(channel), cx);
                }
                if let Some(grid) = pane.view.update(cx, |pane, _| pane.grid.take()) {
                    self.snapshots.insert(id, grid);
                }
                self.loaded.remove(&id);
            }
        }
        let panes: HashSet<_> = self
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .chain(self.state.quick_terminal())
            .map(|pane| pane.id)
            .collect();
        self.completions.retain(|id| panes.contains(id));
        for server in self.servers.ids() {
            let sessions = self.state.session_references(server);
            if let Some(runtime) = self.servers.get_mut(server) {
                runtime.progress.retain(|id, _| sessions.contains(id));
            }
            self.sync_activity_panes(server, sessions);
        }
        self.font_sizes.retain(|id, _| panes.contains(id));
        self.initial_directories.retain(|id, _| panes.contains(id));
        self.snapshots.retain(|id, _| panes.contains(id));
        self.retained.retain(|id| panes.contains(id));
        self.loaded.retain(|id| panes.contains(id));
        for id in visible {
            if self.grids.contains_key(&id) {
                continue;
            }
            if !self.is_terminal(id) {
                continue;
            }
            self.create_terminal_view(id, cx);
        }
        self.ensure_visible(cx);
    }

    fn create_terminal_view(&mut self, id: PaneId, cx: &mut Context<Self>) {
        let mut terminal = self.terminal.clone();
        if let Some(font_size) = self.font_sizes.get(&id) {
            terminal.font_size = *font_size;
        }
        let open_context = self.opener_context(id);
        let snapshot = self.snapshots.remove(&id);
        let state = self.pane_state(id);
        let view = cx.new(|cx| {
            let mut pane = TerminalPane::new(self.palette, terminal, cx);
            pane.configured_font_size = self.terminal.font_size;
            pane.copy_on_select = self.settings.clipboard.copy_on_select;
            pane.open_context = open_context;
            pane.grid = snapshot;
            pane.set_state(state, cx);
            pane
        });
        let subscription = cx.subscribe(&view, move |model, _, event, cx| match event {
            PaneEvent::Title(title) => {
                let _ = model.state.set_pane_title(id, title.clone());
                model.sync_extension_events(cx);
                cx.notify();
            }
            PaneEvent::OpenLink(target) => model.open_terminal_link(id, target.clone(), cx),
            PaneEvent::ResolveLink(candidate) => {
                model.resolve_remote_link(id, candidate.clone(), cx);
            }
            PaneEvent::Upload(upload) => model.upload_to_pane(id, upload.clone(), cx),
            PaneEvent::ContextMenu(position) => {
                if model.is_quick_terminal(id) {
                    model.quick_terminal_menu(*position, cx);
                } else {
                    model.terminal_menu(id, *position, cx);
                }
            }
            PaneEvent::SelectionCopied => {
                model.show_notice(tr!("Copied to clipboard").to_string(), cx);
            }
            PaneEvent::Bell => cx.notify(),
            PaneEvent::Focused => {
                if !model.is_quick_terminal(id) {
                    model.focus_pane(id, cx);
                }
            }
            PaneEvent::History(request) => model.fetch_history(id, *request, cx),
            PaneEvent::Search(request) => model.search(id, request.clone(), cx),
            PaneEvent::Viewport(size) => model.viewport(id, *size, cx),
            PaneEvent::Input(channel, bytes) => {
                crate::diagnostics::event(
                    "terminal.input",
                    format_args!(
                        "pane={id} channel={channel:?} length={} quitting={:?} retained={}",
                        bytes.len(),
                        model.quitting,
                        model.retained.contains(&id)
                    ),
                );
                model.send_channel(id, Work::Input(*channel, bytes.clone()), cx);
            }
            PaneEvent::TerminalInput(channel, input) => {
                model.send_channel(id, Work::TerminalInput(*channel, input.clone()), cx);
            }
            PaneEvent::ClearScreen(channel) => {
                model.send_channel(id, Work::ClearScreen(*channel), cx);
            }
            PaneEvent::CellSize(channel, cell) => {
                model.send_channel(id, Work::CellSize(*channel, *cell), cx);
            }
            PaneEvent::Mouse(channel, event) => {
                model.send_channel(id, Work::Mouse(*channel, *event), cx);
            }
        });
        self.grids.insert(
            id,
            PaneView {
                view,
                _subscription: subscription,
            },
        );
        if !self.is_quick_terminal(id) && self.pane_tab(id) == self.active_tab() {
            self.focus_requested = true;
        }
    }

    /// Sends a pane's channel work to the server that attached it.
    fn send_channel(&mut self, pane: PaneId, work: Work, cx: &mut Context<Self>) {
        if self.quitting == Quitting::Idle
            && !self.retained.contains(&pane)
            && let Some((server, _)) = self.attachment(pane, cx)
        {
            self.send(server, work, cx);
        }
    }

    fn pane_state(&self, id: PaneId) -> PaneState {
        let server = self.state.pane_server(id).unwrap_or_else(ServerId::local);
        match self.connection(server) {
            ConnectionState::Connecting => PaneState::Connecting,
            ConnectionState::Disconnected => PaneState::Disconnected,
            ConnectionState::Ready if self.retained.contains(&id) => PaneState::Exited {
                reason: None,
                unavailable: false,
            },
            ConnectionState::Ready => PaneState::Live,
        }
    }

    pub(crate) fn status(&self, cx: &gpui::App) -> PaneState {
        match self.connection(self.state.current_project().server_id) {
            ConnectionState::Connecting => PaneState::Connecting,
            ConnectionState::Disconnected => PaneState::Disconnected,
            ConnectionState::Ready => self
                .active_pane()
                .and_then(|id| self.terminal(&id))
                .map_or(PaneState::Live, |pane| pane.view.read(cx).state),
        }
    }

    fn apply_restore(
        &mut self,
        server: ServerId,
        sessions: &[SessionInfo],
        cx: &mut Context<Self>,
    ) {
        if server.is_local() {
            self.restore_quick_terminal(sessions, cx);
        }
        let plan = restore::plan(&self.state, server, sessions);
        let active = self.active_pane();
        for (pane, _) in plan.close {
            if let Err(error) = self.state.close_session_pane(pane) {
                self.fail(error.to_string(), cx);
                return;
            }
        }
        self.focus_requested |= active != self.active_pane();
        self.save(cx);
        self.discard_pending(server, cx);
        self.loaded
            .retain(|pane| self.state.pane_server(*pane) != Some(server));
        self.pending.retain(|_, owner| *owner != server);
        for (id, pane) in &self.grids {
            if self.state.pane_server(*id) != Some(server) {
                continue;
            }
            let state = self.pane_state(*id);
            pane.view.update(cx, |pane, cx| pane.set_state(state, cx));
        }
        let mut starting = plan.create;
        starting.extend(
            plan.attach
                .into_iter()
                .filter_map(|(pane, _)| self.state.startup_command(pane).is_some().then_some(pane)),
        );
        for pane in starting {
            let size = self
                .terminal(&pane)
                .and_then(|pane| pane.view.read(cx).viewport())
                .unwrap_or(Size { cols: 80, rows: 24 });
            self.start_attach(pane, size, cx);
        }
        self.sync_visible(cx);
        if server.is_local() {
            self.refresh_quick_terminal(cx);
        }
    }

    fn ensure_visible(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle {
            return;
        }
        for id in self.attached_panes() {
            let server = self.state.pane_server(id).unwrap_or_else(ServerId::local);
            if self.pending.contains_key(&id) || !self.ready(server) || !self.confirmed(server) {
                continue;
            }
            if self.retained.contains(&id) {
                if !self.loaded.contains(&id)
                    && let Some(session) = self.pane_session(id)
                {
                    self.pending.insert(id, server);
                    self.send(server, Work::ReadSaved { pane: id, session }, cx);
                }
            } else if let Some(pane) = self.terminal(&id)
                && pane.view.read(cx).channel().is_none()
                && let Some(size) = pane.view.read(cx).viewport()
            {
                self.start_attach(id, size, cx);
            }
        }
    }

    fn start_attach(&mut self, pane: PaneId, size: Size, cx: &mut Context<Self>) {
        let Some(server) = self.state.pane_server(pane) else {
            return;
        };
        let restoring = self
            .servers
            .get(server)
            .is_none_or(|runtime| runtime.catalog.restore.is_some());
        crate::diagnostics::event(
            "terminal.attach",
            format_args!(
                "pane={pane} server={server} size={size:?} pending={} intents={} restore={restoring} replacing={}",
                self.pending.contains_key(&pane),
                self.state.project_intents(server).len(),
                self.updates.replacing()
            ),
        );
        if self.pending.contains_key(&pane)
            || !self.state.project_intents(server).is_empty()
            || restoring
        {
            return;
        }
        if server.is_local() && self.updates.replacing() {
            self.updates.queued_attaches.insert(pane, size);
            return;
        }
        if !self.is_terminal(pane) {
            return;
        }
        let Some(project) = self.state.projects().iter().find(|project| {
            (self.is_quick_terminal(pane) && project.id == self.state.home().id)
                || project
                    .tabs
                    .iter()
                    .any(|tab| tab.panes.iter().any(|candidate| candidate.id == pane))
        }) else {
            return;
        };
        if project.status() == ProjectStatus::Missing {
            return;
        }
        let directory = self
            .initial_directories
            .get(&pane)
            .unwrap_or(&project.directory)
            .clone();
        let project_id = project.id;
        let directory = self.state.prepare_creation(pane, &directory);
        if !self.save(cx) {
            return;
        }
        let directory = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(directory.0))
        };
        if self.pending.insert(pane, server).is_some() {
            return;
        }
        self.send(
            server,
            Work::Attach {
                pane,
                project: project_id,
                session: self.pane_session(pane),
                directory,
                size,
            },
            cx,
        );
    }

    fn viewport(&mut self, id: PaneId, size: Size, cx: &mut Context<Self>) {
        let attachment = self.attachment(id, cx);
        let server = attachment.map_or_else(
            || self.state.pane_server(id).unwrap_or_else(ServerId::local),
            |(server, _)| server,
        );
        crate::diagnostics::event(
            "terminal.viewport",
            format_args!(
                "pane={id} server={server} size={size:?} connection={:?} quitting={:?}",
                self.connection(server),
                self.quitting
            ),
        );
        if !self.ready(server) || self.quitting != Quitting::Idle {
            return;
        }
        if let Some((server, channel)) = attachment {
            self.send(server, Work::Resize(channel, size), cx);
        } else {
            self.ensure_visible(cx);
        }
    }

    fn fetch_history(
        &mut self,
        pane: PaneId,
        request: crate::views::terminal::scroll::HistoryRequest,
        cx: &mut Context<Self>,
    ) {
        let Some(server) = self.state.pane_server(pane) else {
            return;
        };
        if self.quitting != Quitting::Idle || !self.ready(server) {
            return;
        }
        let Some(session) = self.pane_session(pane) else {
            return;
        };
        let channel = self
            .attachment(pane, cx)
            .filter(|(owner, _)| *owner == server)
            .map(|(_, channel)| channel);
        if channel.is_none() && !self.retained.contains(&pane) {
            return;
        }
        self.send(
            server,
            Work::History {
                pane,
                session,
                channel,
                request,
            },
            cx,
        );
    }

    fn search(
        &mut self,
        pane: PaneId,
        request: crate::views::terminal::find::SearchRequest,
        cx: &mut Context<Self>,
    ) {
        let Some(server) = self.state.pane_server(pane) else {
            return;
        };
        if self.quitting != Quitting::Idle || !self.ready(server) {
            return;
        }
        let Some(session) = self.pane_session(pane) else {
            return;
        };
        let channel = self
            .attachment(pane, cx)
            .filter(|(owner, _)| *owner == server)
            .map(|(_, channel)| channel);
        let source = match channel {
            Some(channel) => muxy_protocol::SearchSource::Live(channel),
            None if self.retained.contains(&pane) => muxy_protocol::SearchSource::Saved(session),
            None => return,
        };
        self.send(
            server,
            Work::Search {
                pane,
                source,
                request,
            },
            cx,
        );
    }

    fn receive_connected(
        &mut self,
        server: ServerId,
        sessions: &[SessionInfo],
        cx: &mut Context<Self>,
    ) {
        if self.state.current_project().server_id == server {
            self.git.reset_context();
        }
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        runtime.connection = ConnectionState::Ready;
        runtime.catalog.identified = false;
        runtime.failure = None;
        runtime.install = None;
        runtime.retry.connected();
        let failure = runtime.error.take();
        let navigation = runtime.activity.navigation.take();
        runtime.activity = activity::ActivityView::default();
        runtime.activity.navigation = navigation;
        runtime.references = None;
        runtime.sessions_revision = 0;
        self.forget_server_sessions(server, cx);
        self.sync_preferences(cx);
        if server.is_local() || (failure.is_some() && self.error == failure) {
            self.set_banner_error(None);
        }
        if !self.send(server, Work::Colors(self.palette.terminal_colors()), cx) {
            return;
        }
        self.extension_client(server, cx);
        self.refresh_activity(server, cx);
        self.sync_references(cx);
        if let Some(runtime) = self.servers.get_mut(server) {
            runtime.catalog.pending = false;
            runtime.catalog.replaying = false;
            runtime.catalog.restore = Some(sessions.to_vec());
        }
        self.discard_pending(server, cx);
        self.refresh_catalog(server, cx);
        if server.is_local() && self.settings_window.is_some() {
            self.read_server_settings(cx);
            self.read_remote_access(cx);
        }
        cx.notify();
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Exhaustive routing of client worker results"
    )]
    fn receive(
        &mut self,
        (server, generation, update): (ServerId, u64, Update),
        cx: &mut Context<Self>,
    ) {
        if self.generation(server) != generation {
            return;
        }
        let local = server.is_local();
        match update {
            Update::ProjectLayouts {
                project,
                request,
                result,
            } => self.receive_project_layouts(project, request, result, cx),
            Update::ProjectLayout {
                project,
                request,
                layout,
                result,
            } => self.receive_project_layout(project, request, &layout, result, cx),
            Update::Activity(result) => self.receive_activity(server, result, cx),
            Update::ActivityClaimed(result) => self.deliver_activity(server, result),
            Update::ActivityAcknowledged { ids, result } => {
                self.activity_acknowledged(server, &ids, result, cx);
            }
            Update::Git { request, result } => self.receive_git(&request, result, cx),
            Update::ProjectSessions { project, result } => {
                self.receive_session_page(project, result, cx);
            }
            Update::CreationCancelled { operation, result } => {
                self.receive_creation_cancelled(server, operation, result, cx);
            }
            Update::Catalog(result) => self.receive_catalog(server, result, cx),
            Update::ProjectMutated { operation, result } => {
                self.receive_project_mutation(server, operation, result, cx);
            }
            Update::ServerInfo(info) => {
                if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.info = Some(info.clone());
                }
                if local {
                    self.receive_server_info(info, cx);
                }
            }
            Update::ServerChecked(result) if local => self.receive_server_update(result, cx),
            Update::ServerSettings(result) if local => self.receive_server_settings(result, cx),
            Update::RemoteAccess(result) if local => self.receive_remote_access(result, cx),
            Update::Pairing(result) if local => self.receive_pairing(result, cx),
            Update::ServerStopped { restart, result } => {
                self.receive_server_stopped(server, restart, result, cx);
            }
            Update::PreparedForInstall(result) if local => {
                self.receive_update_prepared(result, cx);
            }
            Update::Search {
                pane,
                request,
                result,
            } => self.receive_search(pane, &request, result, cx),
            Update::Connected(sessions) => {
                if local {
                    self.updates.sessions = sessions.len();
                }
                self.receive_connected(server, &sessions, cx);
                if local {
                    self.reconcile_server_update(cx);
                }
            }
            Update::ConnectFailed(error, reason) => {
                if local {
                    self.update_connect_failed();
                }
                self.disconnect(server, cx);
                if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.failure = reason;
                    runtime.error = Some(error.clone());
                }
                if reason == Some(muxy_client::RemoteReason::AuthenticationFailed) {
                    self.servers.passwords.forget(server);
                }
                if reason == Some(muxy_client::RemoteReason::Incompatible) {
                    self.explain_incompatible(server, cx);
                }
                self.schedule_reconnect(server, reason, cx);
                if self
                    .pending_remote
                    .is_some_and(|pending| pending.server == server)
                {
                    self.pending_remote = None;
                }
                self.resume_folders(server, cx);
                self.sync_preferences(cx);
                if local {
                    self.fail(error, cx);
                }
            }
            Update::Attached {
                pane,
                session,
                attachment,
                created,
            } => {
                self.receive_attached(server, pane, session, attachment, created, cx);
            }
            Update::AttachFailed {
                pane,
                session,
                created,
                error,
            } => self.receive_attach_failed(server, pane, session, created, &error, cx),
            Update::Saved { pane, result } => self.receive_saved(pane, result, cx),
            Update::History {
                pane,
                request,
                result,
            } => {
                if let Some(pane) = self.terminal(&pane) {
                    pane.view
                        .update(cx, |pane, cx| pane.receive_history(request, result, cx));
                }
            }
            Update::ReferencesSynced(Err(error)) => {
                self.disconnect(server, cx);
                self.server_problem(
                    server,
                    tr!("Could not register open terminals: %@", error.to_string()).to_string(),
                    cx,
                );
            }
            Update::Discarded { session, result } => {
                self.receive_discarded(server, session, result, cx);
            }
            Update::CloseChecked {
                tab,
                session,
                result,
            } => {
                if local
                    && self
                        .quick
                        .closing
                        .is_some_and(|(closing, _)| closing == tab)
                {
                    self.check_quick_close(result, cx);
                } else {
                    self.receive_close_checked(tab, session, result, cx);
                }
            }
            Update::EndedAll(result) if local => self.finish_end_all(result, cx),
            Update::Flushed if self.quitting == Quitting::Preserve => {
                if self.flushed(server, cx) {
                    if self.save(cx) {
                        cx.quit();
                    } else {
                        self.quitting = Quitting::Idle;
                    }
                }
            }
            Update::Flushed if self.quitting == Quitting::Update => {
                self.flush_before_update(server, cx);
            }
            Update::ReferencesSynced(Ok(()))
            | Update::Flushed
            | Update::ServerChecked(_)
            | Update::ServerSettings(_)
            | Update::RemoteAccess(_)
            | Update::Pairing(_)
            | Update::PreparedForInstall(_)
            | Update::EndedAll(_) => {}
            Update::CloseSessionPanes(session) => self.close_ended_session(server, session, cx),
            Update::Event(event) => self.receive_event(server, event, cx),
            Update::Error(error) => self.server_problem(server, error, cx),
        }
        cx.notify();
    }

    fn receive_attached(
        &mut self,
        server: ServerId,
        pane: PaneId,
        session: SessionId,
        attachment: muxy_client::Attachment,
        created: bool,
        cx: &mut Context<Self>,
    ) {
        crate::diagnostics::event(
            "terminal.attached",
            format_args!(
                "pane={pane} server={server} session={session:?} channel={:?} created={created} visible={} detached_pending={}",
                attachment.channel,
                self.terminal(&pane).is_some(),
                self.detached_pending.contains(&pane)
            ),
        );
        self.pending.remove(&pane);
        self.initial_directories.remove(&pane);
        let detached = self.detached_pending.remove(&pane);
        if detached {
            if let Some(runtime) = self.servers.get_mut(server) {
                runtime.references = None;
            }
            self.sync_references(cx);
        }
        if self.state.pane_server(pane) != Some(server)
            || self.state.set_pane_session(pane, Some(session)).is_err()
        {
            if created && !detached {
                self.discard_created(server, session, cx);
            } else {
                self.send(server, Work::Detach(attachment.channel), cx);
            }
            return;
        }
        let command = self.state.take_startup_command(pane);
        let saved = self.save(cx);
        self.sync_references(cx);
        if let Some(command) = command
            && (!saved
                || !self.send(
                    server,
                    Work::Input(attachment.channel, format!("{command}\r").into_bytes()),
                    cx,
                ))
        {
            let _ = self.state.set_startup_command(pane, &command);
            self.save(cx);
        }
        if let Some(view) = self.terminal(&pane).map(|pane| pane.view.clone()) {
            let channel = attachment.channel;
            let size = view.update(cx, |pane, cx| pane.attach(server, attachment, cx));
            if let Some(size) = size {
                self.send(server, Work::Resize(channel, size), cx);
            }
        } else {
            self.send(server, Work::Detach(attachment.channel), cx);
            self.snapshots.insert(pane, attachment.grid);
        }
    }

    fn receive_attach_failed(
        &mut self,
        server: ServerId,
        pane: PaneId,
        session: Option<SessionId>,
        created: bool,
        error: &muxy_client::ClientError,
        cx: &mut Context<Self>,
    ) {
        crate::diagnostics::event(
            "terminal.attach_failed",
            format_args!(
                "pane={pane} server={server} session={session:?} created={created} error_kind={:?}",
                std::mem::discriminant(error)
            ),
        );
        self.pending.remove(&pane);
        let detached = self.detached_pending.remove(&pane);
        if detached {
            if let Some(runtime) = self.servers.get_mut(server) {
                runtime.references = None;
            }
            self.sync_references(cx);
        }
        if server.is_local() && self.is_quick_terminal(pane) && missing_session(error) {
            self.close_quick_terminal(cx);
            return;
        }
        if let Some(session) = session {
            self.initial_directories.remove(&pane);
            if self.state.pane_server(pane) != Some(server)
                || self.state.set_pane_session(pane, Some(session)).is_err()
            {
                if created && !detached {
                    self.discard_created(server, session, cx);
                }
                return;
            }
            self.save(cx);
        }
        if self.pane_tab(pane).is_none() && !self.is_quick_terminal(pane) {
            return;
        }
        if missing_session(error) {
            if let Some(session) = self.pane_session(pane) {
                self.close_ended_session(server, session, cx);
            }
        } else {
            self.fail(self.server_message(server, &error.to_string()), cx);
        }
    }

    fn receive_close_checked(
        &mut self,
        tab: TabId,
        session: SessionId,
        result: Result<Option<muxy_protocol::ForegroundProcess>, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        if self.pending_close != Some(tab) {
            return;
        }
        self.pending_close = None;
        let pane = self
            .close_request
            .as_ref()
            .filter(|request| request.tab == tab)
            .and_then(|request| request.panes.get(request.checking))
            .copied();
        if pane.and_then(|pane| self.pane_session(pane)) != Some(session) {
            self.close_request = None;
            return;
        }
        match result {
            Ok(Some(process)) if !process.is_shell => self.confirm_close(tab, cx),
            Ok(_) => self.advance_close(cx),
            Err(error) if missing_session(&error) => self.advance_close(cx),
            Err(error) => {
                self.close_request = None;
                self.fail(
                    tr!("Could not check the running process: %@", error.to_string()).to_string(),
                    cx,
                );
            }
        }
    }

    fn advance_close(&mut self, cx: &mut Context<Self>) {
        if let Some(request) = &mut self.close_request {
            request.checking += 1;
        }
        self.check_next_close(cx);
    }

    fn receive_saved(
        &mut self,
        pane: PaneId,
        result: Result<muxy_protocol::SavedScreen, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.pending.remove(&pane);
        if self.pane_tab(pane).is_none() {
            return;
        }
        self.loaded.insert(pane);
        if let Some(view) = self.terminal(&pane).map(|pane| pane.view.clone()) {
            match result {
                Ok(screen) => view.update(cx, |pane, cx| pane.restore(screen, cx)),
                Err(error) => {
                    view.update(cx, |pane, cx| {
                        pane.set_state(
                            PaneState::Exited {
                                reason: None,
                                unavailable: true,
                            },
                            cx,
                        );
                    });
                    if !matches!(&error, muxy_client::ClientError::Server(reply) if reply.code == muxy_protocol::ErrorCode::SavedContentUnavailable)
                    {
                        self.fail(
                            tr!("Could not restore terminal output: %@", error.to_string())
                                .to_string(),
                            cx,
                        );
                    }
                }
            }
        } else {
            self.loaded.remove(&pane);
        }
    }

    fn receive_progress(
        &mut self,
        server: ServerId,
        session: SessionId,
        progress: muxy_protocol::SessionProgress,
        cx: &mut Context<Self>,
    ) {
        if !self.state.session_references(server).contains(&session) {
            return;
        }
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        let previous = runtime
            .progress
            .insert(session, progress)
            .unwrap_or_default();
        if progress.completed > previous.completed
            && !runtime
                .activity
                .snapshot
                .agents
                .iter()
                .any(|agent| agent.session == session)
        {
            let active = self.active_pane();
            for pane in self.server_session_panes(server, session) {
                if Some(pane) != active {
                    self.completions.insert(pane);
                }
            }
        }
        cx.notify();
    }

    /// Panes in projects that show `server`'s `session`.
    fn server_session_panes(&self, server: ServerId, session: SessionId) -> Vec<PaneId> {
        self.state
            .projects()
            .iter()
            .filter(|project| project.server_id == server)
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .filter(|pane| {
                matches!(pane.content,
                PaneContent::Terminal { session: Some(id) } if id == session)
            })
            .map(|pane| pane.id)
            .collect()
    }

    fn receive_session_metadata(
        &mut self,
        server: ServerId,
        session: SessionId,
        metadata: &muxy_protocol::SessionMetadata,
        cx: &mut Context<Self>,
    ) {
        let title = muxy_app_core::title::derive(
            &metadata.title,
            metadata.process.as_ref(),
            &metadata.directory,
        );
        let panes = self.server_session_panes(server, session);
        for pane in panes {
            let _ = self.state.set_pane_title(pane, title.clone());
        }
        self.sync_extension_events(cx);
        cx.notify();
    }

    fn receive_event(&mut self, server: ServerId, event: ClientEvent, cx: &mut Context<Self>) {
        let local = server.is_local();
        match event {
            ClientEvent::SessionMetadata { session, metadata } => {
                self.receive_session_metadata(server, session, &metadata, cx);
            }
            ClientEvent::ActivityChanged { revision } => {
                if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.activity.dirty = runtime.activity.dirty.max(revision);
                }
                self.refresh_activity(server, cx);
            }
            ClientEvent::Progress { session, progress } => {
                self.receive_progress(server, session, progress, cx);
            }
            ClientEvent::FilesChanged { project, changes } => {
                self.extension_files_changed(project, changes, cx);
            }
            ClientEvent::GitChanged { project } => {
                self.extension_git_changed(project, cx);
                self.git_invalidated(project, cx);
            }
            ClientEvent::SessionsChanged { revision } => {
                if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.sessions_revision = runtime.sessions_revision.max(revision);
                }
                self.refresh_existing_sessions(cx);
            }
            ClientEvent::CatalogChanged { revision } => {
                if let Some(runtime) = self.servers.get_mut(server) {
                    runtime.catalog.dirty = runtime.catalog.dirty.max(revision);
                }
                self.refresh_catalog(server, cx);
                self.refresh_session_picker(cx);
            }
            ClientEvent::RemoteAccessChanged { .. } if local => self.remote_access_changed(cx),
            ClientEvent::Frame { channel, frame } => {
                if let Some(pane) = self.attached_view(server, channel, cx) {
                    let seq = frame.seq;
                    pane.update(cx, |pane, cx| pane.apply(&frame, cx));
                    self.send(server, Work::Ack(channel, seq), cx);
                }
            }
            ClientEvent::SessionEnded { session, .. } => {
                if local {
                    self.updates.sessions = self.updates.sessions.saturating_sub(1);
                }
                self.close_ended_session(server, session, cx);
                self.refresh_session_picker(cx);
                if local {
                    self.reconcile_server_update(cx);
                }
            }
            ClientEvent::ServerRestarting if local => self.expect_server_restart(),
            ClientEvent::RemoteAccessChanged { .. } | ClientEvent::ServerRestarting => {}
            ClientEvent::Disconnected if local => self.receive_disconnect(cx),
            ClientEvent::Disconnected => {
                self.disconnect(server, cx);
                self.schedule_reconnect(server, None, cx);
            }
            ClientEvent::Metadata { channel, event } => {
                if let Some(pane) = self.attached_view(server, channel, cx) {
                    pane.update(cx, |pane, cx| pane.metadata(event, cx));
                }
            }
        }
    }

    /// The pane `server` attached on `channel`. Each server numbers its own
    /// channels, so both have to match.
    fn attached_view(
        &self,
        server: ServerId,
        channel: muxy_protocol::ChannelId,
        cx: &gpui::App,
    ) -> Option<Entity<TerminalPane>> {
        self.grids
            .values()
            .find(|pane| pane.view.read(cx).attachment() == Some((server, channel)))
            .map(|pane| pane.view.clone())
    }

    fn close_ended_session(
        &mut self,
        server: ServerId,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        self.forget_session_activity(server, session, cx);
        if !self.state.session_references(server).contains(&session) {
            return;
        }
        if server.is_local()
            && self
                .state
                .quick_terminal()
                .is_some_and(|pane| self.pane_session(pane.id) == Some(session))
        {
            self.close_quick_terminal(cx);
        }
        let active = self.active_pane();
        if let Err(error) = self.state.close_session_panes(server, session) {
            self.fail(error.to_string(), cx);
            return;
        }
        if self.close_request.as_ref().is_some_and(|request| {
            request
                .panes
                .iter()
                .any(|pane| self.pane_tab(*pane).is_none())
        }) {
            self.close_request = None;
            self.pending_close = None;
            self.close_prompt = None;
        }
        self.focus_requested |= active != self.active_pane();
        self.changed(cx);
        self.discard_pending(server, cx);
    }

    fn receive_search(
        &mut self,
        pane: PaneId,
        request: &crate::views::terminal::find::SearchRequest,
        result: Result<muxy_protocol::SearchPage, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        if let Some(pane) = self.terminal(&pane) {
            pane.view
                .update(cx, |pane, cx| pane.receive_search(request, result, cx));
        }
    }

    fn finish_end_all(
        &mut self,
        result: Result<(), muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.quitting = Quitting::Idle;
        if let Err(error) = result {
            self.fail(
                tr!("Could not end all sessions: %@", error.to_string()).to_string(),
                cx,
            );
            self.ensure_visible(cx);
            return;
        }
        let local = ServerId::local();
        let previous = self.state.clone();
        if let Err(error) = self.state.clear_terminal_panes(local) {
            self.state = previous;
            self.fail(
                tr!("Could not clear terminal panes: %@", error.to_string()).to_string(),
                cx,
            );
            return;
        }
        for session in self.state.pending_discards(local).to_vec() {
            self.state.complete_discard(local, session);
        }
        if self.save(cx) {
            cx.quit();
        } else {
            self.state = previous;
            self.sync_visible(cx);
        }
    }

    pub(crate) fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.set_banner_error(Some(error));
        cx.notify();
    }
}

impl Drop for AppModel {
    fn drop(&mut self) {
        let _ = store::save(&self.path, &self.state);
        self.stop_workers();
    }
}

#[cfg(test)]
mod tests {
    pub(super) mod extensions;
    mod webviews;
    use muxy_protocol::ExitReason;
    mod activity;
    mod ai;
    mod composer;
    mod detach;
    mod find;
    mod git;
    mod layout_drag;
    mod links;
    mod preferences;
    mod project_layouts;
    mod projects;
    mod quick_terminal;
    mod remote_devices;
    mod remote_parity;
    mod servers;
    mod session_ownership;
    mod splits;
    mod tab_groups;
    mod tab_menu;
    mod tui;
    mod updates;
    mod worktrees;

    use muxy_client::Client;
    use std::io::{self, Write};
    use std::thread;
    use std::time::{Duration, Instant};

    use gpui::{Keystroke, Modifiers, TestAppContext, VisualTestContext, px, size};

    use super::*;

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    pub(in crate::model) fn acknowledge_catalog(model: &mut AppModel, cx: &mut Context<AppModel>) {
        if let Some(sessions) = &mut model.servers.local.catalog.restore {
            for session in sessions {
                if let Some(project) = model.state.projects().iter().find(|project| project.tabs.iter().flat_map(|tab| &tab.panes).any(|pane| matches!(pane.content, PaneContent::Terminal { session: Some(id) } if id == session.id))) {
                    session.project = project.id;
                }
            }
        }

        while let Some(intent) = model
            .state
            .project_intents(ServerId::local())
            .first()
            .cloned()
        {
            model
                .state
                .complete_project_intent(ServerId::local(), intent.operation)
                .expect("fixture server acknowledged project");
        }
        let page = muxy_protocol::CatalogPage {
            server: muxy_protocol::ServerIdentity::from_u128(1),
            home: model.state.home().id,
            revision: model.state.catalog_revision(ServerId::local()) + 1,
            projects: model
                .state
                .projects()
                .iter()
                .filter(|project| project.server_id.is_local())
                .map(muxy_app_core::Project::descriptor)
                .collect(),
            next: None,
            legacy_home: None,
        };
        model.receive_catalog(ServerId::local(), Ok(page), cx);
    }

    /// An update from this computer's worker.
    pub(in crate::model) fn local(generation: u64, update: Update) -> (ServerId, u64, Update) {
        (ServerId::local(), generation, update)
    }

    fn acknowledge_close(model: &mut AppModel, cx: &mut Context<AppModel>) {
        while let Some(tab) = model.pending_close {
            let request = model.close_request.as_ref().expect("close request");
            let pane = request.panes[request.checking];
            let session = model.pane_session(pane).expect("session");
            let process = model
                .terminal(&pane)
                .and_then(|pane| pane.view.read(cx).process.clone());
            model.receive(
                (
                    ServerId::local(),
                    model.servers.local.generation,
                    Update::CloseChecked {
                        tab,
                        session,
                        result: Ok(process),
                    },
                ),
                cx,
            );
        }
    }

    pub(super) fn stub_boot(state: AppState) -> (Boot, std::sync::mpsc::Receiver<(u64, Work)>) {
        let (boot, requests, _) = remote_boot(state, Vec::new());
        (boot, requests)
    }

    /// What stub workers of other computers' servers were asked to do.
    pub(in crate::model) type Remotes =
        Rc<std::cell::RefCell<HashMap<ServerId, std::sync::mpsc::Receiver<(u64, Work)>>>>;

    /// A stub boot whose settings list `servers`. Their workers are stubs too.
    pub(in crate::model) fn remote_boot(
        state: AppState,
        servers: Vec<muxy_app_core::settings::ServerEntry>,
    ) -> (Boot, std::sync::mpsc::Receiver<(u64, Work)>, Remotes) {
        let (work, requests) = std::sync::mpsc::channel();
        let (_, updates) = async_channel::unbounded();
        let directory = std::env::temp_dir().join(format!("muxy-app-restore-{}", ProjectId::new()));
        let remotes = Remotes::default();
        let started = Rc::clone(&remotes);
        let workers = crate::boot::Workers::with(move |server, _| {
            let (sender, receiver) = std::sync::mpsc::channel();
            started.borrow_mut().insert(server, receiver);
            Ok(sender)
        });
        let settings = muxy_app_core::settings::Settings {
            servers,
            ..muxy_app_core::settings::Settings::default()
        };
        (
            Boot {
                import_message: None,
                composer: muxy_app_core::composer::ComposerStore::load_from(&directory),
                state,
                state_path: directory.join("state.json"),
                settings,
                terminal: muxy_app_core::settings::TerminalSettings::default(),
                work,
                workers,
                updates,
            },
            requests,
            remotes,
        )
    }

    fn saved_screen() -> muxy_protocol::SavedScreen {
        muxy_protocol::SavedScreen {
            graphics: muxy_protocol::Graphics::default(),
            size: Size { cols: 80, rows: 24 },
            rows: (0..24)
                .map(|index| muxy_protocol::Row {
                    index,
                    runs: if index == 0 {
                        vec![muxy_protocol::Run {
                            text: "final marker".into(),
                            width: 12,
                            style: muxy_protocol::Style::default(),
                        }]
                    } else {
                        vec![]
                    },
                })
                .collect(),
            cursor: muxy_protocol::Cursor {
                shape: muxy_protocol::CursorShape::default(),
                row: 0,
                col: 12,
                visible: true,
            },
            reason: Some(ExitReason::Exited(0)),
        }
    }

    fn missing_session_error() -> muxy_client::ClientError {
        muxy_client::ClientError::Server(muxy_protocol::ErrorReply {
            code: muxy_protocol::ErrorCode::UnknownSession,
            message: "session exited before attachment".into(),
        })
    }

    #[gpui::test]
    fn exit_before_first_attachment_closes_and_persists_the_tab(cx: &mut TestAppContext) {
        let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            model.new_tab(cx);
            let pane = model.active_pane().expect("pane");
            let session = SessionId::new(42).expect("session");
            model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::AttachFailed {
                        pane,
                        session: Some(session),
                        created: true,
                        error: missing_session_error(),
                    },
                ),
                cx,
            );
            assert!(model.state.home().tabs.is_empty());
            assert!(model.terminal(&pane).is_none());
            let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
            assert!(
                !work
                    .iter()
                    .any(|work| matches!(work, Work::ReadSaved { .. }))
            );
            assert!(
                work.iter()
                    .any(|work| matches!(work, Work::Discard(id, _) if *id == session))
            );
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Saved {
                        pane,
                        result: Ok(saved_screen()),
                    },
                ),
                cx,
            );
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane,
                        session,
                        attachment: attachment(),
                        created: false,
                    },
                ),
                cx,
            );
            assert!(model.state.home().tabs.is_empty());
            let restored = store::load(&model.path).expect("saved state");
            assert!(restored.home().tabs.is_empty());
            assert_eq!(
                restore::plan(&restored, ServerId::local(), &[]),
                restore::RestorePlan::default()
            );
        });
    }

    #[gpui::test]
    fn disconnected_close_removes_the_tab_and_retries_cleanup_after_reload(
        cx: &mut TestAppContext,
    ) {
        let mut state = AppState::bootstrap().expect("state");
        let tab = state.open_terminal_tab(state.home().id).expect("tab");
        let pane = state.home().tabs[0].panes[0].id;
        let session = SessionId::new(42).expect("session");
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let (boot, requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        let saved = view.update(cx, |model, cx| {
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Event(ClientEvent::Disconnected),
                ),
                cx,
            );
            model.close_tab(tab, cx);
            assert!(model.state.home().tabs.is_empty());
            assert!(model.error.is_none());
            assert_eq!(model.state.pending_discards(ServerId::local()), &[session]);
            assert!(
                !requests
                    .try_iter()
                    .any(|(_, work)| matches!(work, Work::Discard(_, _)))
            );
            store::load(&model.path).expect("persisted close")
        });
        let (boot, requests) = stub_boot(saved);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Connected(vec![SessionInfo {
                        project: ProjectId::from_u128(1),
                        id: session,
                        directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                    }]),
                ),
                cx,
            );
            acknowledge_catalog(model, cx);
            assert!(model.state.home().tabs.is_empty());
            let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
            assert!(
                work.iter()
                    .any(|work| matches!(work, Work::Discard(id, _) if *id == session))
            );
            assert!(!work.iter().any(|work| matches!(work, Work::Attach { .. })));
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Discarded {
                        session,
                        result: Ok(()),
                    },
                ),
                cx,
            );
            assert!(
                store::load(&model.path)
                    .expect("cleanup acknowledged")
                    .pending_discards(ServerId::local())
                    .is_empty()
            );
        });
    }

    #[gpui::test]
    fn connected_close_removes_tab_immediately_and_survives_a_lost_reply(cx: &mut TestAppContext) {
        let mut state = AppState::bootstrap().expect("state");
        let tab = state.open_terminal_tab(state.home().id).expect("tab");
        let pane = state.home().tabs[0].panes[0].id;
        let session = SessionId::new(42).expect("session");
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let other = state.open_terminal_tab(state.home().id).expect("other tab");
        let (boot, requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            model.close_tab(tab, cx);
            assert_eq!(model.state.home().tabs.len(), 1);
            assert_eq!(model.active_tab(), Some(other));
            assert_eq!(
                store::load(&model.path)
                    .expect("persisted close")
                    .pending_discards(ServerId::local()),
                &[session]
            );
            assert!(
                requests
                    .try_iter()
                    .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
            );
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Discarded {
                        session,
                        result: Err(muxy_client::ClientError::Disconnected),
                    },
                ),
                cx,
            );
            assert_eq!(model.state.home().tabs.len(), 1);
            assert_eq!(model.state.pending_discards(ServerId::local()), &[session]);
            assert!(model.error.is_none());
            model.connect(cx);
            model.receive((ServerId::local(), 2, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            assert!(
                requests
                    .try_iter()
                    .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
            );
            model.receive(
                (
                    ServerId::local(),
                    2,
                    Update::Discarded {
                        session,
                        result: Ok(()),
                    },
                ),
                cx,
            );
            assert!(model.state.pending_discards(ServerId::local()).is_empty());
        });
    }

    #[gpui::test]
    fn session_status_changes_preserve_the_final_rows_in_the_viewport(cx: &mut TestAppContext) {
        let mut state = AppState::bootstrap().expect("state");
        state.open_terminal_tab(state.home().id).expect("tab");
        let pane = state.home().tabs[0].panes[0].id;
        let session = SessionId::new(42).expect("session");
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.simulate_resize(size(px(800.0), px(600.0)));
        view.update(cx, |model, cx| {
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Connected(vec![SessionInfo {
                        project: ProjectId::from_u128(1),
                        id: session,
                        directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                    }]),
                ),
                cx,
            );
            acknowledge_catalog(model, cx);
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane,
                        session,
                        attachment: attachment(),
                        created: false,
                    },
                ),
                cx,
            );
        });
        cx.run_until_parked();
        let terminal = view.read_with(cx, |model, _| {
            model.terminal(&pane).expect("terminal").view.clone()
        });
        let live = terminal.read_with(cx, |pane, _| pane.viewport().expect("viewport"));
        assert!(crate::views::disconnected::label(PaneState::Live).is_none());
        terminal.update(cx, |pane, cx| {
            let mut screen = saved_screen();
            screen.size = live;
            screen.cursor.row = live.rows - 1;
            screen.rows = (0..live.rows)
                .map(|index| muxy_protocol::Row {
                    index,
                    runs: vec![muxy_protocol::Run {
                        text: format!("row-{index:03}"),
                        width: 7,
                        style: muxy_protocol::Style::default(),
                    }],
                })
                .collect();
            pane.restore(screen, cx);
        });
        for state in [
            PaneState::Exited {
                reason: Some(ExitReason::Exited(0)),
                unavailable: false,
            },
            PaneState::Disconnected,
            PaneState::Connecting,
        ] {
            terminal.update(cx, |pane, cx| pane.set_state(state, cx));
            cx.run_until_parked();
            terminal.read_with(cx, |pane, _| {
                assert_eq!(pane.viewport(), Some(live));
                let grid = pane.grid.as_ref().expect("saved grid");
                assert_eq!(grid.size, live);
                assert_eq!(
                    grid.row_text(usize::from(live.rows - 1)),
                    format!("row-{:03}", live.rows - 1)
                );
            });
        }
    }

    #[gpui::test]
    fn metadata_titles_and_close_confirmation_follow_the_attached_process(cx: &mut TestAppContext) {
        let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            model.new_tab(cx);
            let pane = model.active_pane().expect("pane");
            let mut attachment = attachment();
            attachment.process = Some(muxy_protocol::ForegroundProcess {
                name: "sleep".into(),
                is_shell: false,
            });
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane,
                        session: SessionId::new(42).expect("session"),
                        attachment,
                        created: true,
                    },
                ),
                cx,
            );
        });
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model.state.home().tabs[0]
                .title(model.state.window().active_pane)
                .to_owned()),
            "sleep"
        );
        view.update(cx, |model, cx| {
            model.receive_event(
                ServerId::local(),
                ClientEvent::Metadata {
                    channel: muxy_protocol::ChannelId(1),
                    event: muxy_protocol::MetadataEvent::Title("hello".into()),
                },
                cx,
            );
        });
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model.state.home().tabs[0]
                .title(model.state.window().active_pane)
                .to_owned()),
            "hello"
        );
        cx.simulate_keystrokes("cmd-w");
        view.update(cx, acknowledge_close);
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert!(model.close_prompt.is_some());
            assert!(model.overlay.is_none());
            assert_eq!(model.state.home().tabs.len(), 1);
            assert!(model.state.pending_discards(ServerId::local()).is_empty());
        });
        assert_eq!(
            cx.pending_prompt(),
            Some((
                crate::views::confirm::PANE_TITLE.into(),
                crate::views::confirm::PANE_MESSAGE.into()
            ))
        );
        cx.simulate_keystrokes("cmd-w");
        view.update(cx, acknowledge_close);
        cx.run_until_parked();
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        assert!(view.read_with(cx, |model, _| model.close_prompt.is_none()));
        assert!(view.read_with(cx, |model, _| model.settings.window.confirm_running_process));
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Discard(_, _)))
        );
        cx.simulate_keystrokes("cmd-w");
        view.update(cx, acknowledge_close);
        cx.run_until_parked();
        cx.simulate_prompt_answer("Close");
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        assert!(view.read_with(cx, |model, _| model.state.home().tabs.is_empty()));
        assert_eq!(
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::Discard(_, _)))
                .count(),
            1
        );
    }

    #[gpui::test]
    fn closing_before_reattachment_checks_metadata_after_switch_and_reconnect(
        cx: &mut TestAppContext,
    ) {
        let mut state = AppState::bootstrap().expect("state");
        let first = state.open_terminal_tab(state.home().id).expect("first");
        let pane = state.home().tabs[0].panes[0].id;
        let session = SessionId::new(42).expect("session");
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
        state.open_terminal_tab(state.home().id).expect("second");
        let (boot, requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        for generation in 1..=2 {
            view.update(cx, |model, cx| {
                if generation == 2 {
                    let mut attachment = attachment();
                    attachment.process = Some(muxy_protocol::ForegroundProcess {
                        name: "sh".into(),
                        is_shell: true,
                    });
                    let attached = Update::Attached {
                        pane,
                        session,
                        attachment,
                        created: false,
                    };
                    model.receive(local(1, attached), cx);
                    model.receive(local(1, Update::Event(ClientEvent::Disconnected)), cx);
                    model.connect(cx);
                }
                let connected = Update::Connected(vec![SessionInfo {
                    project: ProjectId::from_u128(1),
                    id: session,
                    directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                }]);
                model.receive(local(generation, connected), cx);
                acknowledge_catalog(model, cx);
                model.select_tab(first, cx);
                let terminal = model.terminal(&pane).expect("terminal");
                assert!(terminal.view.read(cx).channel().is_none());
                model.close_tab(first, cx);
                model.close_tab(first, cx);
                assert_eq!(model.pending_close, Some(first));
                assert_eq!(model.state.home().tabs.len(), 2);
                assert!(model.state.pending_discards(ServerId::local()).is_empty());
                let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
                assert!(!work.iter().any(|work| matches!(work, Work::Discard(_, _))));
                assert_eq!(
                    work.iter()
                        .filter(
                            |work| matches!(work, Work::CheckClose { tab, .. } if *tab == first)
                        )
                        .count(),
                    1
                );
                model.receive(
                    (
                        ServerId::local(),
                        generation,
                        Update::CloseChecked {
                            tab: first,
                            session,
                            result: Ok(Some(muxy_protocol::ForegroundProcess {
                                name: "sleep".into(),
                                is_shell: false,
                            })),
                        },
                    ),
                    cx,
                );
                assert!(model.close_prompt.is_some());
            });
            cx.run_until_parked();
            assert_eq!(
                cx.pending_prompt(),
                Some((
                    crate::views::confirm::TITLE.into(),
                    crate::views::confirm::MESSAGE.into()
                ))
            );
            cx.simulate_prompt_answer("Cancel");
            cx.run_until_parked();
            assert!(!cx.has_pending_prompt());
            view.read_with(cx, |model, _| {
                assert_eq!(model.state.home().tabs.len(), 2);
                assert!(model.state.pending_discards(ServerId::local()).is_empty());
                assert!(model.close_prompt.is_none());
            });
        }
    }

    pub(in crate::model) fn attachment() -> muxy_client::Attachment {
        let screen = saved_screen();
        muxy_client::Attachment {
            channel: muxy_protocol::ChannelId(1),
            server_input: false,
            grid: RunGrid {
                graphics: muxy_protocol::Graphics::default(),
                prompts: std::collections::BTreeSet::default(),
                prompt_state: muxy_client::ScreenPrompts::default(),
                links: muxy_client::ScreenLinks::default(),
                size: screen.size,
                rows: screen.rows.into_iter().map(|row| row.runs).collect(),
                cursor: screen.cursor,
                modes: muxy_protocol::Modes::default(),
                history: std::collections::VecDeque::new(),
                history_cursor: None,
                history_total: 0,
                history_fresh: true,
            },
            title: String::new(),
            directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
            process: Some(muxy_protocol::ForegroundProcess {
                name: "sh".into(),
                is_shell: true,
            }),
        }
    }

    #[gpui::test]
    fn restore_closes_dead_tabs_and_rejects_stale_connections(cx: &mut TestAppContext) {
        let mut state = AppState::bootstrap().expect("state");
        state.open_terminal_tab(state.home().id).expect("tab");
        let pane = state.home().tabs[0].panes[0].id;
        let session = SessionId::new(42).expect("ID");
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let (boot, requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            assert!(model.state.home().tabs.is_empty());
            assert!(
                store::load(&model.path)
                    .expect("saved state")
                    .home()
                    .tabs
                    .is_empty()
            );
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Event(ClientEvent::Disconnected),
                ),
                cx,
            );
            model.connect(cx);
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::ConnectFailed("stale".into(), None),
                ),
                cx,
            );
            assert!(model.servers.local.connection == ConnectionState::Connecting);
            model.receive((ServerId::local(), 2, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            assert!(model.servers.local.connection == ConnectionState::Ready);
            assert!(model.state.home().tabs.is_empty());
            assert!(model.grids.is_empty());
        });
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Attach { .. } | Work::ReadSaved { .. }))
        );
    }

    #[gpui::test]
    fn restore_closes_only_the_pane_with_invalid_session_membership(cx: &mut TestAppContext) {
        let mut state = AppState::bootstrap().expect("state");
        let home = state.home().id;
        let session = SessionId::new(42).expect("session");
        state.open_terminal_tab(home).expect("tab");
        let valid = state.window().active_pane.expect("pane");
        state
            .set_pane_session(valid, Some(session))
            .expect("session");
        let other = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        state.open_terminal_tab(other).expect("tab");
        let invalid = state.window().active_pane.expect("pane");
        state
            .set_pane_session(invalid, Some(session))
            .expect("session");
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.apply_restore(
                ServerId::local(),
                &[SessionInfo {
                    project: home,
                    id: session,
                    directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                }],
                cx,
            );
            assert_eq!(model.pane_session(valid), Some(session));
            assert!(model.pane_tab(invalid).is_none());
            assert!(
                !model
                    .state
                    .pending_discards(ServerId::local())
                    .contains(&session)
            );
        });
    }

    #[gpui::test]
    fn session_exit_closes_duplicate_and_hidden_panes_preserving_live_splits(
        cx: &mut TestAppContext,
    ) {
        let mut state = AppState::bootstrap().expect("state");
        let home = state.home().id;
        let dead = SessionId::new(42).expect("ID");
        let live = SessionId::new(43).expect("ID");
        state.open_terminal_tab(home).expect("tab");
        let first = state.window().active_pane.expect("pane");
        state.set_pane_session(first, Some(dead)).expect("session");
        let neighbor = state.split_pane(first, Direction::Right).expect("split");
        state
            .set_pane_session(neighbor, Some(live))
            .expect("session");
        state.open_terminal_tab(home).expect("tab");
        state
            .set_pane_session(state.window().active_pane.expect("pane"), Some(dead))
            .expect("session");
        let hidden = state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project");
        state.open_terminal_tab(hidden).expect("tab");
        state
            .set_pane_session(state.window().active_pane.expect("pane"), Some(dead))
            .expect("session");
        state
            .select_tab(home, state.home().tabs[0].id)
            .expect("select");
        let (boot, _) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            let active = model.active_pane();
            model.receive((ServerId::local(), 1, Update::CloseSessionPanes(dead)), cx);
            assert_eq!(model.active_pane(), active);
            assert_eq!(model.state.home().tabs.len(), 1);
            assert_eq!(model.state.home().tabs[0].layout.leaves(), vec![neighbor]);
            assert!(
                model
                    .state
                    .project(hidden)
                    .expect("project")
                    .tabs
                    .is_empty()
            );
            assert_eq!(
                model.state.session_references(ServerId::local()),
                vec![live]
            );
            assert_eq!(
                store::load(&model.path)
                    .expect("saved state")
                    .session_references(ServerId::local()),
                vec![live]
            );
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Event(ClientEvent::SessionEnded {
                        session: dead,
                        reason: ExitReason::Exited(0),
                    }),
                ),
                cx,
            );
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Event(ClientEvent::SessionEnded {
                        session: live,
                        reason: ExitReason::Ended,
                    }),
                ),
                cx,
            );
            assert!(model.state.home().tabs.is_empty());
            assert!(model.active_pane().is_none());
        });
    }

    #[gpui::test]
    fn reconnect_surviving_session_and_failed_end_all_preserve_tabs(cx: &mut TestAppContext) {
        let mut state = AppState::bootstrap().expect("state");
        state.open_terminal_tab(state.home().id).expect("tab");
        let pane = state.home().tabs[0].panes[0].id;
        let session = SessionId::new(42).expect("ID");
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let (boot, requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::ConnectFailed("offline".into(), None),
                ),
                cx,
            );
            assert_eq!(model.state.home().tabs.len(), 1);
            model.connect(cx);
            model.receive(
                (
                    ServerId::local(),
                    2,
                    Update::Connected(vec![SessionInfo {
                        project: ProjectId::from_u128(1),
                        id: session,
                        directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                    }]),
                ),
                cx,
            );
            acknowledge_catalog(model, cx);
            model.viewport(pane, Size { cols: 80, rows: 24 }, cx);
            model.receive(
                (
                    ServerId::local(),
                    2,
                    Update::Attached {
                        pane,
                        session,
                        attachment: attachment(),
                        created: false,
                    },
                ),
                cx,
            );
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Event(ClientEvent::SessionEnded {
                        session,
                        reason: ExitReason::Ended,
                    }),
                ),
                cx,
            );
            assert!(!model.retained.contains(&pane));
            assert!(
                model
                    .terminal(&pane)
                    .expect("terminal")
                    .view
                    .read(cx)
                    .channel()
                    .is_some()
            );
            model.end_all_and_quit(cx);
            model.receive(
                (
                    ServerId::local(),
                    2,
                    Update::EndedAll(Err(io::Error::other("termination failed").into())),
                ),
                cx,
            );
            assert!(model.quitting == Quitting::Idle);
            assert_eq!(model.state.home().tabs.len(), 1);
            assert!(
                model
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("termination failed"))
            );
        });
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Attach { session: None, .. }))
        );
    }

    #[gpui::test]
    fn close_during_creation_removes_tab_before_discard_acknowledgement(cx: &mut TestAppContext) {
        let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            model.new_tab(cx);
            let tab = model.active_tab().expect("tab");
            let pane = model.active_pane().expect("pane");
            model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
            model.close_tab(tab, cx);
            assert!(model.state.home().tabs.is_empty());
            let session = SessionId::new(42).expect("ID");
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane,
                        session,
                        attachment: attachment(),
                        created: true,
                    },
                ),
                cx,
            );
            assert!(
                requests
                    .try_iter()
                    .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
            );
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Discarded {
                        session,
                        result: Ok(()),
                    },
                ),
                cx,
            );
            assert!(model.state.home().tabs.is_empty());
        });
    }

    fn active_process<'a>(
        model: &'a AppModel,
        cx: &'a gpui::App,
    ) -> Option<&'a muxy_protocol::ForegroundProcess> {
        model
            .active_pane()
            .and_then(|pane| model.terminal(&pane))
            .and_then(|pane| pane.view.read(cx).process.as_ref())
    }

    fn active_session(view: &Entity<AppModel>, cx: &VisualTestContext) -> Result<SessionId> {
        view.read_with(cx, |model, _| {
            model
                .active_pane()
                .and_then(|pane| model.pane_session(pane))
        })
        .ok_or_else(|| io::Error::other("no active session").into())
    }

    fn wait_live(cx: &mut VisualTestContext, view: &Entity<AppModel>) -> Result {
        wait(cx, view, |model, cx| {
            model
                .active_pane()
                .and_then(|id| model.terminal(&id))
                .is_some_and(|pane| pane.view.read(cx).channel().is_some())
        })
    }

    fn active_grid<'a>(model: &'a AppModel, cx: &'a gpui::App) -> Option<&'a RunGrid> {
        model
            .terminal(&model.active_pane()?)?
            .view
            .read(cx)
            .grid
            .as_ref()
    }

    fn screen(view: &Entity<AppModel>, cx: &VisualTestContext) -> String {
        view.read_with(cx, |model, cx| {
            active_grid(model, cx)
                .map(|grid| {
                    (0..grid.rows.len())
                        .map(|row| grid.row_text(row))
                        .filter(|row| !row.trim().is_empty())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default()
        })
    }

    fn wait(
        cx: &mut VisualTestContext,
        view: &Entity<AppModel>,
        condition: impl Fn(&AppModel, &gpui::App) -> bool,
    ) -> Result {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            if view.read_with(cx, &condition) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(io::Error::other(format!(
                    "verification timed out; error={:?}; screen={} ",
                    view.read_with(cx, |model, _| model.error.clone()),
                    screen(view, cx)
                ))
                .into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_text(cx: &mut VisualTestContext, view: &Entity<AppModel>, expected: &str) -> Result {
        wait(cx, view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| {
                (0..grid.rows.len()).any(|row| grid.row_text(row).contains(expected))
            })
        })
    }

    fn type_text(cx: &mut VisualTestContext, text: &str) {
        for ch in text.chars() {
            let key = Keystroke {
                key: ch.to_string(),
                key_char: Some(ch.to_string()),
                modifiers: Modifiers::default(),
            };
            cx.update(|window, cx| {
                window.dispatch_keystroke(key, cx);
            });
        }
        cx.run_until_parked();
    }

    fn shell(cx: &mut VisualTestContext, command: &str) {
        type_text(cx, command);
        cx.simulate_keystrokes("enter");
    }

    fn report(message: &str) -> Result {
        writeln!(io::stdout(), "{message}")?;
        Ok(())
    }
}

mod activity;
