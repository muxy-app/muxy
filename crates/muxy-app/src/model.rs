mod banners;
mod catalog;
mod composer;
mod diagnostics;
pub(crate) mod extensions;
pub(crate) mod git;
mod links;
mod preferences;
mod quick_terminal;
mod server_status;
pub(crate) use server_status::ServerStatus;
mod tabs;
mod updates;
pub(crate) use updates::UpdateAction;
mod voice;
mod webviews;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{AppContext, Context, Entity, FocusHandle, Subscription, Task, Window};
use muxy_app_core::{
    AppState, Direction, PaneContent, PaneId, ProjectId, ProjectStatus, TabId, WindowBounds,
    restore, store,
};
use muxy_client::{ClientEvent, RunGrid};
use muxy_protocol::{SessionId, SessionInfo, Size};

use crate::boot::{Boot, Update, Work, Worker, missing_session};
use crate::views::overlays::Overlay;
use crate::views::terminal::colors::Palette;
use crate::views::terminal::pane::{PaneEvent, PaneState, TerminalPane};
use muxy_ui::theme::{Metrics, Theme};

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
    catalog: catalog::Synchronization,
    pub(crate) existing_sessions: crate::views::session_picker::ExistingSessions,
    pub(crate) quick: quick_terminal::QuickTerminalRuntime,
    pub(crate) window: gpui::AnyWindowHandle,
    pub(crate) state: AppState,
    pub(crate) grids: HashMap<PaneId, PaneView>,
    pub(crate) error: Option<String>,
    pub(crate) focus: FocusHandle,
    pub(crate) appearance: muxy_app_core::settings::Appearance,
    pub(crate) settings: muxy_app_core::settings::Settings,
    terminal: muxy_app_core::settings::TerminalSettings,
    server_preferences: preferences::ServerPreferences,
    server_anchor: muxy_ui::popover::PopoverAnchor,
    pub(crate) settings_window: Option<preferences::SettingsWindowState>,
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
    pub(crate) project_logo_task: Option<Task<()>>,
    pub(crate) project_logos: crate::views::project_editor::logo::Cache,
    pub(crate) expanded_worktrees: HashSet<ProjectId>,
    pub(crate) tab_sidebar_selection: Option<(ProjectId, Option<TabId>)>,
    #[cfg(target_os = "macos")]
    pub(crate) window_drag: Option<muxy_ui::window_drag::WindowDrag>,
    #[cfg(target_os = "macos")]
    pub(crate) sidebar_vibrancy: Option<muxy_ui::vibrancy::SidebarVibrancy>,
    pub(crate) overlay_subscription: Option<Subscription>,
    pub(crate) picker_search: crate::picker::search::SearchService,
    pub(crate) navigation: crate::navigation::Navigation,
    pub(crate) configuration_error: Option<String>,
    dismissed_banners: [Option<String>; 2],
    path: PathBuf,
    bounds_save: Option<Task<()>>,
    webview_shortcuts: Option<Rc<Vec<gpui::Keystroke>>>,
    pub(crate) spinners: crate::views::tab_activity::Spinners,
    work: Worker,
    pending: HashSet<PaneId>,
    detached_pending: HashSet<PaneId>,
    pending_close: Option<TabId>,
    close_request: Option<CloseRequest>,
    pub(crate) close_prompt: Option<Task<()>>,
    retained: HashSet<PaneId>,
    loaded: HashSet<PaneId>,
    snapshots: HashMap<PaneId, RunGrid>,
    pub(crate) activity: activity::ActivityView,
    pub(crate) window_active: bool,
    #[cfg(target_os = "macos")]
    notifications: Option<muxy_ui::notifications::Notifications>,
    pub(crate) progress: HashMap<SessionId, muxy_protocol::SessionProgress>,
    pub(crate) completions: HashSet<PaneId>,
    discarding: HashSet<SessionId>,
    references: Option<Vec<SessionId>>,
    generation: u64,
    connection: ConnectionState,
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
        let (theme, fallback) = self.themes.resolve(&self.appearance, self.dark);
        self.theme = theme;
        match self.themes.terminal_palette(
            &fallback,
            &self.terminal.options,
            self.dark,
            self.path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
        ) {
            Ok(palette) => self.palette = palette,
            Err(error) => self.set_configuration_error(Some(error)),
        }
        if self.connection == ConnectionState::Ready
            && previous_colors != self.palette.terminal_colors()
        {
            self.send(Work::Colors(self.palette.terminal_colors()), cx);
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
        );
        match result {
            Ok(terminal) => {
                let themes = crate::theme::Catalog::load(&self.path.with_file_name("themes"));
                let fallback = themes.resolve(&self.appearance, self.dark).1;
                if let Err(error) = themes.terminal_palette(
                    &fallback,
                    &terminal.options,
                    self.dark,
                    self.path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new(".")),
                ) {
                    self.set_configuration_error(Some(error));
                    cx.notify();
                    return;
                }
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
                self.set_configuration_error(Some(format!(
                    "Could not reload configuration: {error}"
                )));
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
            self.fail(format!("Could not save search location: {error}"), cx);
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
                self.fail(format!("Could not save appearance: {error}"), cx);
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
        cx.notify();
        self.acknowledge_focused_activity(cx);
        if window.is_window_active() {
            if self.path.with_file_name("ghostty.conf").exists() {
                self.reload_configuration(cx);
            }
            self.refresh_git(cx);
            self.refresh_project_statuses(cx);
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
            model.close_voice(cx);
            model.save(cx);
            model.flush_composer(cx)
        });
        let themes = crate::theme::Catalog::load(&boot.state_path.with_file_name("themes"));
        let (theme, fallback) = themes.resolve(&boot.settings.appearance, dark);
        let mut configuration_error = None;
        let palette = themes
            .terminal_palette(
                &fallback,
                &boot.terminal.options,
                dark,
                boot.state_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
            )
            .unwrap_or_else(|error| {
                configuration_error = Some(error);
                fallback
            });
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
            catalog: catalog::Synchronization::default(),
            existing_sessions: crate::views::session_picker::ExistingSessions::default(),
            quick: quick_terminal::QuickTerminalRuntime::new(&boot.settings.quick_terminal, cx),
            window: window.window_handle(),
            state: boot.state,
            grids: HashMap::new(),
            error: theme_error,
            focus: cx.focus_handle(),
            appearance: boot.settings.appearance.clone(),
            project_logo_task: None,
            project_logos: HashMap::new(),
            expanded_worktrees: HashSet::new(),
            tab_sidebar_selection: None,
            settings: boot.settings,
            terminal: boot.terminal,
            server_preferences: preferences::ServerPreferences::default(),
            server_anchor: Rc::default(),
            settings_window: None,
            font_sizes: HashMap::new(),
            initial_directories: HashMap::new(),
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
            #[cfg(target_os = "macos")]
            window_drag: muxy_ui::window_drag::WindowDrag::new(&window.window_title()),
            #[cfg(target_os = "macos")]
            sidebar_vibrancy: None,
            overlay_subscription: None,
            picker_search: crate::picker::search::SearchService::default(),
            navigation: crate::navigation::Navigation::default(),
            configuration_error,
            dismissed_banners: [None, None],
            path: boot.state_path,
            bounds_save: None,
            webview_shortcuts: None,
            spinners: crate::views::tab_activity::Spinners::default(),
            work: boot.work,
            pending: HashSet::new(),
            detached_pending: HashSet::new(),
            pending_close: None,
            close_request: None,
            close_prompt: None,
            retained: HashSet::new(),
            loaded: HashSet::new(),
            snapshots: HashMap::new(),
            activity: activity::ActivityView::default(),
            window_active: window.is_window_active(),
            #[cfg(target_os = "macos")]
            notifications: Self::start_notifications(cx),
            progress: HashMap::new(),
            completions: HashSet::new(),
            discarding: HashSet::new(),
            references: None,
            generation: 1,
            connection: ConnectionState::Connecting,
            quitting: Quitting::Idle,
            updates: updates::Updater::default(),
            _events: events,
            _appearance: appearance,
            _bounds: bounds,
            _activation: activation,
            _panes: panes,
            _quit: quit,
        };
        model.refresh_installed_extensions(cx);
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

    pub(crate) fn visible_panes(&self) -> Vec<PaneId> {
        self.active_tab()
            .and_then(|id| {
                self.state
                    .current_project()
                    .tabs
                    .iter()
                    .find(|tab| tab.id == id)
            })
            .map_or_else(Vec::new, muxy_app_core::Tab::visible_panes)
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
        if self.connection == ConnectionState::Ready {
            self.start_attach(pane, size, cx);
        } else if self.connection == ConnectionState::Disconnected {
            self.connect(cx);
        }
        cx.notify();
    }

    pub(crate) fn focus_pane(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle
            || self.pane_tab(pane) != self.active_tab()
            || self.active_pane() == Some(pane)
        {
            return;
        }
        if self.state.focus_pane(pane).is_ok() {
            self.focus_requested = true;
            self.changed(cx);
        }
    }

    pub(crate) fn focus_direction(&mut self, direction: Direction, cx: &mut Context<Self>) {
        if let Some(neighbor) = self
            .active_pane()
            .and_then(|pane| self.state.neighbor(pane, direction))
        {
            self.focus_pane(neighbor, cx);
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
        }
    }

    pub(crate) fn cycle_project(&mut self, forward: bool, cx: &mut Context<Self>) {
        let projects: Vec<_> = self
            .sidebar_projects()
            .into_iter()
            .filter(|project| project.status() == ProjectStatus::Available)
            .map(|project| project.id)
            .collect();
        if projects.is_empty() {
            return;
        }
        let next = projects
            .iter()
            .position(|id| {
                *id == self.state.current_project().id
                    || (self.appearance.layout
                        == muxy_app_core::settings::AppLayout::ProjectFocused
                        && Some(*id) == self.state.current_project().parent_id)
            })
            .map_or(0, |index| {
                if forward {
                    (index + 1) % projects.len()
                } else {
                    (index + projects.len() - 1) % projects.len()
                }
            });
        let target = if self.appearance.layout == muxy_app_core::settings::AppLayout::ProjectFocused
        {
            self.preferred_worktree(projects[next])
        } else {
            projects[next]
        };
        self.select_project(target, cx);
    }

    pub(crate) fn add_project(&mut self, directory: PathBuf, cx: &mut Context<Self>) -> bool {
        self.edit_project(|state| state.add_project(directory).map(|_| ()), cx)
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
        if self.edit_project(
            |state| {
                for session in state.remove_project(project)? {
                    state.queue_discard(session);
                }
                Ok(())
            },
            cx,
        ) {
            self.discard_pending(cx);
        }
    }

    pub(crate) fn refresh_project_statuses(&mut self, cx: &mut Context<Self>) {
        self.state.refresh_project_statuses();
        self.sync_visible(cx);
        cx.notify();
    }

    pub(crate) fn new_tab(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle {
            return;
        }
        match self
            .state
            .open_terminal_tab(self.state.current_project().id)
        {
            Ok(_) => self.changed(cx),
            Err(error) => self.fail(error.to_string(), cx),
        }
        if self.connection == ConnectionState::Disconnected {
            self.connect(cx);
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
        if self.pending.contains(&pane) {
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
            if let Some(session) = self.pane_session(pane)
                && self.session_used_outside(session, &request.panes)
            {
                if let Some(request) = &mut self.close_request {
                    request.checking += 1;
                }
                continue;
            }
            if self.connection == ConnectionState::Ready
                && !self.retained.contains(&pane)
                && let Some(session) = self.pane_session(pane)
            {
                let size = self
                    .terminal(&pane)
                    .and_then(|pane| pane.view.read(cx).viewport())
                    .unwrap_or(Size { cols: 80, rows: 24 });
                self.pending_close = Some(tab);
                if !self.send(Work::CheckClose { tab, session, size }, cx) {
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

    fn session_used_outside(&self, session: SessionId, closing: &[PaneId]) -> bool {
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .chain(self.state.quick_terminal())
            .any(|pane| !closing.contains(&pane.id) && self.pane_session(pane.id) == Some(session))
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
                        format!("Could not save close confirmation preference: {error}"),
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
                self.fail(format!("Could not show close confirmation: {error}"), cx);
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
        let project_id = project.id;
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
            if !detach && !self.state.session_references().contains(&session) {
                self.state.queue_discard(session);
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
                    .filter(|pane| self.pending.contains(pane))
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
        self.discard_pending(cx);
        cx.notify();
    }

    pub(crate) fn connect(&mut self, cx: &mut Context<Self>) {
        self.connect_to_server(false, cx);
    }

    fn connect_to_server(&mut self, after_update: bool, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Disconnected
            || self.quitting != Quitting::Idle
            || self.server_preferences.control_busy
        {
            return;
        }
        let Some(generation) = self.generation.checked_add(1) else {
            return;
        };
        self.generation = generation;
        self.detached_pending.clear();
        self.connection = ConnectionState::Connecting;
        self.sync_preferences(cx);
        self.pending.clear();
        self.pending_close = None;
        if self.close_prompt.is_none() {
            self.close_request = None;
        }
        self.discarding.clear();
        self.catalog.cancelling.clear();
        for pane in self.grids.values() {
            pane.view
                .update(cx, |pane, cx| pane.set_state(PaneState::Connecting, cx));
        }
        let work = if after_update {
            Work::ReconnectAfterUpdate(
                self.updates
                    .server
                    .as_ref()
                    .map_or(0, |server| server.instance),
            )
        } else {
            Work::Connect
        };
        if self.work.send((generation, work)).is_err() {
            self.disconnect(cx);
            self.fail("The server connection worker stopped".into(), cx);
        }
    }

    pub(crate) fn quit(&mut self, cx: &mut Context<Self>) {
        crate::diagnostics::event(
            "quit.request",
            format_args!(
                "quitting={:?} pending={:?} discarding={:?}",
                self.quitting, self.pending, self.discarding
            ),
        );
        if self.quitting != Quitting::Idle || !self.preferences_before_quit(cx) {
            return;
        }
        self.quitting = Quitting::Preserve;
        if !self.send(Work::Flush, cx) {
            self.quitting = Quitting::Idle;
        }
    }

    pub(crate) fn end_all_and_quit(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle || !self.preferences_before_quit(cx) {
            return;
        }
        if self.connection != ConnectionState::Ready {
            self.fail(
                "Connect to the server before ending all sessions".into(),
                cx,
            );
            return;
        }
        let mut sessions: Vec<_> = self
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .filter_map(|pane| self.pane_session(pane.id))
            .collect();
        sessions.extend(
            self.state
                .quick_terminal()
                .and_then(|pane| self.pane_session(pane.id)),
        );
        sessions.extend(self.state.pending_discards());
        self.quitting = Quitting::EndAll;
        if !self.send(Work::EndAll(sessions), cx) {
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

    fn discard_pending(&mut self, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready {
            return;
        }
        for operation in self.state.pending_cancellations().to_vec() {
            if self.catalog.cancelling.insert(operation)
                && !self.send(Work::CancelCreation(operation), cx)
            {
                self.catalog.cancelling.remove(&operation);
                break;
            }
        }
        self.sync_references(cx);
        self.state.prepare_closes();
        if !self.state.pending_discards().is_empty() && !self.save(cx) {
            return;
        }
        for session in self.state.pending_discards().to_vec() {
            let Some(operation) = self.state.close_operation(session) else {
                continue;
            };
            if self.discarding.insert(session) && !self.send(Work::Discard(session, operation), cx)
            {
                self.discarding.remove(&session);
                break;
            }
        }
    }

    fn discard_created(&mut self, session: SessionId, cx: &mut Context<Self>) {
        self.state.queue_discard(session);
        if self.save(cx) {
            self.discard_pending(cx);
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
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
    }

    fn save(&mut self, cx: &mut Context<Self>) -> bool {
        self.bounds_save = None;
        match store::save(&self.path, &self.state) {
            Ok(()) => true,
            Err(error) => {
                self.fail(format!("Could not save tabs: {error}"), cx);
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

    fn sync_references(&mut self, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready {
            return;
        }
        let references = self.state.session_references();
        if self.references.as_ref() != Some(&references)
            && self.send(Work::References(references.clone()), cx)
        {
            self.references = Some(references);
        }
    }

    fn sync_visible(&mut self, cx: &mut Context<Self>) {
        self.sync_composer(cx);
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
                if let Some(channel) = pane.view.read(cx).channel()
                    && self.connection == ConnectionState::Ready
                {
                    self.send(Work::Detach(channel), cx);
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
        let sessions = self.state.session_references();
        self.progress.retain(|id, _| sessions.contains(id));
        self.sync_activity_panes(sessions);
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
                cx.notify();
            }
            PaneEvent::OpenLink(target) => model.open_terminal_link(id, target.clone(), cx),
            PaneEvent::ContextMenu(position) => {
                if model.is_quick_terminal(id) {
                    model.quick_terminal_menu(*position, cx);
                } else {
                    model.terminal_menu(id, *position, cx);
                }
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
                if model.quitting == Quitting::Idle && !model.retained.contains(&id) {
                    model.send(Work::Input(*channel, bytes.clone()), cx);
                }
            }
            PaneEvent::CellSize(channel, cell) => {
                if model.quitting == Quitting::Idle && !model.retained.contains(&id) {
                    model.send(Work::CellSize(*channel, *cell), cx);
                }
            }
            PaneEvent::Mouse(channel, event) => {
                if model.quitting == Quitting::Idle && !model.retained.contains(&id) {
                    model.send(Work::Mouse(*channel, *event), cx);
                }
            }
        });
        self.grids.insert(
            id,
            PaneView {
                view,
                _subscription: subscription,
            },
        );
        if !self.is_quick_terminal(id) {
            self.focus_requested = true;
        }
    }

    fn pane_state(&self, id: PaneId) -> PaneState {
        match self.connection {
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
        match self.connection {
            ConnectionState::Connecting => PaneState::Connecting,
            ConnectionState::Disconnected => PaneState::Disconnected,
            ConnectionState::Ready => self
                .active_pane()
                .and_then(|id| self.terminal(&id))
                .map_or(PaneState::Live, |pane| pane.view.read(cx).state),
        }
    }

    fn apply_restore(&mut self, sessions: &[SessionInfo], cx: &mut Context<Self>) {
        self.restore_quick_terminal(sessions, cx);
        let plan = restore::plan(&self.state, sessions);
        let active = self.active_pane();
        for (pane, _) in plan.close {
            if let Err(error) = self.state.close_session_pane(pane) {
                self.fail(error.to_string(), cx);
                return;
            }
        }
        self.focus_requested |= active != self.active_pane();
        self.save(cx);
        self.discard_pending(cx);
        self.loaded.clear();
        self.pending.clear();
        for (id, pane) in &self.grids {
            let state = self.pane_state(*id);
            pane.view.update(cx, |pane, cx| pane.set_state(state, cx));
        }
        for pane in plan.create {
            let size = self
                .terminal(&pane)
                .and_then(|pane| pane.view.read(cx).viewport())
                .unwrap_or(Size { cols: 80, rows: 24 });
            self.start_attach(pane, size, cx);
        }
        self.sync_visible(cx);
        self.refresh_quick_terminal(cx);
    }

    fn ensure_visible(&mut self, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready || self.quitting != Quitting::Idle {
            return;
        }
        for id in self.attached_panes() {
            if self.pending.contains(&id) {
                continue;
            }
            if self.retained.contains(&id) {
                if !self.loaded.contains(&id)
                    && let Some(session) = self.pane_session(id)
                {
                    self.pending.insert(id);
                    self.send(Work::ReadSaved { pane: id, session }, cx);
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
        crate::diagnostics::event(
            "terminal.attach",
            format_args!(
                "pane={pane} size={size:?} pending={} intents={} restore={} replacing={}",
                self.pending.contains(&pane),
                self.state.project_intents().len(),
                self.catalog.restore.is_some(),
                self.updates.replacing()
            ),
        );
        if self.pending.contains(&pane)
            || !self.state.project_intents().is_empty()
            || self.catalog.restore.is_some()
        {
            return;
        }
        if self.updates.replacing() {
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
        if !self.pending.insert(pane) {
            return;
        }
        self.send(
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
        crate::diagnostics::event(
            "terminal.viewport",
            format_args!(
                "pane={id} size={size:?} connection={:?} quitting={:?}",
                self.connection, self.quitting
            ),
        );
        if self.connection != ConnectionState::Ready || self.quitting != Quitting::Idle {
            return;
        }
        if let Some(channel) = self
            .terminal(&id)
            .and_then(|pane| pane.view.read(cx).channel())
        {
            self.send(Work::Resize(channel, size), cx);
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
        if self.quitting != Quitting::Idle || self.connection != ConnectionState::Ready {
            return;
        }
        let Some(session) = self.pane_session(pane) else {
            return;
        };
        let channel = self
            .terminal(&pane)
            .and_then(|pane| pane.view.read(cx).channel());
        if channel.is_none() && !self.retained.contains(&pane) {
            return;
        }
        self.send(
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
        if self.quitting != Quitting::Idle || self.connection != ConnectionState::Ready {
            return;
        }
        let Some(session) = self.pane_session(pane) else {
            return;
        };
        let channel = self
            .terminal(&pane)
            .and_then(|pane| pane.view.read(cx).channel());
        let source = match channel {
            Some(channel) => muxy_protocol::SearchSource::Live(channel),
            None if self.retained.contains(&pane) => muxy_protocol::SearchSource::Saved(session),
            None => return,
        };
        self.send(
            Work::Search {
                pane,
                source,
                request,
            },
            cx,
        );
    }

    fn receive_connected(&mut self, sessions: &[SessionInfo], cx: &mut Context<Self>) {
        self.git.reset_context();
        self.connection = ConnectionState::Ready;
        let navigation = self.activity.navigation.take();
        self.activity = activity::ActivityView::default();
        self.activity.navigation = navigation;
        self.references = None;
        self.existing_sessions = crate::views::session_picker::ExistingSessions::default();
        self.sync_preferences(cx);
        self.set_banner_error(None);
        if !self.send(Work::Colors(self.palette.terminal_colors()), cx) {
            return;
        }
        self.extension_client(cx);
        self.refresh_activity(cx);
        self.sync_references(cx);
        self.catalog.pending = false;
        self.catalog.replaying = false;
        self.catalog.restore = Some(sessions.to_vec());
        self.discard_pending(cx);
        self.refresh_catalog(cx);
        if self.settings_window.is_some() {
            self.read_server_settings(cx);
        }
        cx.notify();
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Exhaustive routing of client worker results"
    )]
    fn receive(&mut self, (generation, update): (u64, Update), cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        match update {
            Update::Activity(result) => self.receive_activity(result, cx),
            Update::ActivityClaimed(result) => self.deliver_activity(result),
            Update::ActivityAcknowledged { ids, result } => {
                self.activity_acknowledged(&ids, result, cx);
            }
            Update::Git { request, result } => self.receive_git(&request, result, cx),
            Update::ProjectSessions { project, result } => {
                self.receive_session_page(project, result, cx);
            }
            Update::CreationCancelled { operation, result } => {
                self.receive_creation_cancelled(operation, result, cx);
            }
            Update::Catalog(result) => self.receive_catalog(result, cx),
            Update::ProjectMutated { operation, result } => {
                self.receive_project_mutation(operation, result, cx);
            }
            Update::ServerInfo(server) => self.receive_server_info(server, cx),
            Update::ServerChecked(result) => self.receive_server_update(result, cx),
            Update::ServerSettings(result) => self.receive_server_settings(result, cx),
            Update::ServerStopped { restart, result } => {
                self.receive_server_stopped(restart, result, cx);
            }
            Update::PreparedForInstall(result) => self.receive_update_prepared(result, cx),
            Update::Search {
                pane,
                request,
                result,
            } => self.receive_search(pane, &request, result, cx),
            Update::Connected(sessions) => {
                self.updates.sessions = sessions.len();
                self.receive_connected(&sessions, cx);
                self.reconcile_server_update(cx);
            }
            Update::ConnectFailed(error) => {
                self.update_connect_failed();
                self.disconnect(cx);
                self.fail(error, cx);
            }
            Update::Attached {
                pane,
                session,
                attachment,
                created,
            } => {
                self.receive_attached(pane, session, attachment, created, cx);
            }
            Update::AttachFailed {
                pane,
                session,
                created,
                error,
            } => self.receive_attach_failed(pane, session, created, &error, cx),
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
                self.disconnect(cx);
                self.fail(format!("Could not register open terminals: {error}"), cx);
            }
            Update::Discarded { session, result } => self.receive_discarded(session, result, cx),
            Update::CloseChecked {
                tab,
                session,
                result,
            } => {
                if self
                    .quick
                    .closing
                    .is_some_and(|(closing, _)| closing == tab)
                {
                    self.check_quick_close(result, cx);
                } else {
                    self.receive_close_checked(tab, session, result, cx);
                }
            }
            Update::EndedAll(result) => self.finish_end_all(result, cx),
            Update::Flushed if self.quitting == Quitting::Preserve => {
                if !self.pending.is_empty() || !self.discarding.is_empty() {
                    self.send(Work::Flush, cx);
                } else if self.save(cx) {
                    cx.quit();
                } else {
                    self.quitting = Quitting::Idle;
                }
            }
            Update::Flushed if self.quitting == Quitting::Update => self.flush_before_update(cx),
            Update::ReferencesSynced(Ok(())) | Update::Flushed => {}
            Update::CloseSessionPanes(session) => self.close_ended_session(session, cx),
            Update::Event(event) => self.receive_event(event, cx),
            Update::Error(error) => self.fail(error, cx),
        }
        cx.notify();
    }

    fn receive_attached(
        &mut self,
        pane: PaneId,
        session: SessionId,
        attachment: muxy_client::Attachment,
        created: bool,
        cx: &mut Context<Self>,
    ) {
        crate::diagnostics::event(
            "terminal.attached",
            format_args!(
                "pane={pane} session={session:?} channel={:?} created={created} visible={} detached_pending={}",
                attachment.channel,
                self.terminal(&pane).is_some(),
                self.detached_pending.contains(&pane)
            ),
        );
        self.pending.remove(&pane);
        self.initial_directories.remove(&pane);
        let detached = self.detached_pending.remove(&pane);
        if detached {
            self.references = None;
            self.sync_references(cx);
        }
        if self.state.set_pane_session(pane, Some(session)).is_err() {
            if created && !detached {
                self.discard_created(session, cx);
            } else {
                self.send(Work::Detach(attachment.channel), cx);
            }
            return;
        }
        self.save(cx);
        self.sync_references(cx);
        if let Some(view) = self.terminal(&pane).map(|pane| pane.view.clone()) {
            let channel = attachment.channel;
            let size = view.update(cx, |pane, cx| pane.attach(attachment, cx));
            if let Some(size) = size {
                self.send(Work::Resize(channel, size), cx);
            }
        } else {
            self.send(Work::Detach(attachment.channel), cx);
            self.snapshots.insert(pane, attachment.grid);
        }
    }

    fn receive_attach_failed(
        &mut self,
        pane: PaneId,
        session: Option<SessionId>,
        created: bool,
        error: &muxy_client::ClientError,
        cx: &mut Context<Self>,
    ) {
        crate::diagnostics::event(
            "terminal.attach_failed",
            format_args!(
                "pane={pane} session={session:?} created={created} error_kind={:?}",
                std::mem::discriminant(error)
            ),
        );
        self.pending.remove(&pane);
        let detached = self.detached_pending.remove(&pane);
        if detached {
            self.references = None;
            self.sync_references(cx);
        }
        if self.is_quick_terminal(pane) && missing_session(error) {
            self.close_quick_terminal(cx);
            return;
        }
        if let Some(session) = session {
            self.initial_directories.remove(&pane);
            if self.state.set_pane_session(pane, Some(session)).is_err() {
                if created && !detached {
                    self.discard_created(session, cx);
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
                self.close_ended_session(session, cx);
            }
        } else {
            self.fail(error.to_string(), cx);
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
                self.fail(format!("Could not check the running process: {error}"), cx);
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
                        self.fail(format!("Could not restore terminal output: {error}"), cx);
                    }
                }
            }
        } else {
            self.loaded.remove(&pane);
        }
    }

    fn receive_progress(
        &mut self,
        session: SessionId,
        progress: muxy_protocol::SessionProgress,
        cx: &mut Context<Self>,
    ) {
        if !self.state.session_references().contains(&session) {
            return;
        }
        let previous = self.progress.insert(session, progress).unwrap_or_default();
        if progress.completed > previous.completed
            && !self
                .activity
                .snapshot
                .agents
                .iter()
                .any(|agent| agent.session == session)
        {
            let active = self.active_pane();
            for pane in self
                .state
                .projects()
                .iter()
                .flat_map(|project| &project.tabs)
                .flat_map(|tab| &tab.panes)
            {
                if Some(pane.id) != active
                    && matches!(pane.content, PaneContent::Terminal { session: Some(id) } if id == session)
                {
                    self.completions.insert(pane.id);
                }
            }
        }
        cx.notify();
    }

    fn receive_session_metadata(
        &mut self,
        session: SessionId,
        metadata: &muxy_protocol::SessionMetadata,
        cx: &mut Context<Self>,
    ) {
        let title = muxy_app_core::title::derive(
            &metadata.title,
            metadata.process.as_ref(),
            &metadata.directory,
        );
        let panes: Vec<_> = self
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .filter(|pane| {
                matches!(pane.content,
                PaneContent::Terminal { session: Some(id) } if id == session)
            })
            .map(|pane| pane.id)
            .collect();
        for pane in panes {
            let _ = self.state.set_pane_title(pane, title.clone());
        }
        cx.notify();
    }

    fn receive_event(&mut self, event: ClientEvent, cx: &mut Context<Self>) {
        match event {
            ClientEvent::SessionMetadata { session, metadata } => {
                self.receive_session_metadata(session, &metadata, cx);
            }
            ClientEvent::ActivityChanged { revision } => {
                self.activity.dirty = self.activity.dirty.max(revision);
                self.refresh_activity(cx);
            }
            ClientEvent::Progress { session, progress } => {
                self.receive_progress(session, progress, cx);
            }
            ClientEvent::FilesChanged { project, changes } => {
                self.extension_files_changed(project, changes, cx);
            }
            ClientEvent::GitChanged { project } => self.git_invalidated(project, cx),
            ClientEvent::SessionsChanged { revision } => {
                self.existing_sessions.revision = self.existing_sessions.revision.max(revision);
                self.refresh_existing_sessions(cx);
            }
            ClientEvent::CatalogChanged { revision } => {
                self.catalog.dirty = self.catalog.dirty.max(revision);
                self.refresh_catalog(cx);
                self.refresh_session_picker(cx);
            }
            ClientEvent::Frame { channel, frame } => {
                let pane = self
                    .grids
                    .values()
                    .find(|pane| pane.view.read(cx).channel() == Some(channel))
                    .map(|pane| pane.view.clone());
                if let Some(pane) = pane {
                    let seq = frame.seq;
                    pane.update(cx, |pane, cx| pane.apply(&frame, cx));
                    self.send(Work::Ack(channel, seq), cx);
                }
            }
            ClientEvent::SessionEnded { session, .. } => {
                self.updates.sessions = self.updates.sessions.saturating_sub(1);
                self.close_ended_session(session, cx);
                self.refresh_session_picker(cx);
                self.reconcile_server_update(cx);
            }
            ClientEvent::ServerRestarting => self.expect_server_restart(),
            ClientEvent::Disconnected => self.receive_disconnect(cx),
            ClientEvent::Metadata { channel, event } => {
                if let Some(pane) = self
                    .grids
                    .values()
                    .find(|pane| pane.view.read(cx).channel() == Some(channel))
                    .map(|pane| pane.view.clone())
                {
                    pane.update(cx, |pane, cx| pane.metadata(event, cx));
                }
            }
        }
    }

    fn close_ended_session(&mut self, session: SessionId, cx: &mut Context<Self>) {
        self.forget_session_activity(session, cx);
        if !self.state.session_references().contains(&session) {
            return;
        }
        if self
            .state
            .quick_terminal()
            .is_some_and(|pane| self.pane_session(pane.id) == Some(session))
        {
            self.close_quick_terminal(cx);
        }
        let active = self.active_pane();
        if let Err(error) = self.state.close_session_panes(session) {
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
        self.discard_pending(cx);
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
            self.fail(format!("Could not end all sessions: {error}"), cx);
            self.ensure_visible(cx);
            return;
        }
        let previous = self.state.clone();
        if let Err(error) = self.state.clear_terminal_panes() {
            self.state = previous;
            self.fail(format!("Could not clear terminal panes: {error}"), cx);
            return;
        }
        for session in self.state.pending_discards().to_vec() {
            self.state.complete_discard(session);
        }
        if self.save(cx) {
            cx.quit();
        } else {
            self.state = previous;
            self.sync_visible(cx);
        }
    }

    fn disconnect(&mut self, cx: &mut Context<Self>) {
        self.stop_extension_tasks(cx);
        self.extensions.disconnect();
        self.git.reset_context();
        for repository in self.git.projects.values_mut() {
            repository.disconnect();
        }
        self.connection = ConnectionState::Disconnected;
        for progress in self.progress.values_mut() {
            progress.progress = None;
        }
        self.completions.clear();
        self.activity.snapshot.agents.clear();
        self.activity.loaded = false;
        self.activity.pending = false;
        self.references = None;
        self.existing_sessions = crate::views::session_picker::ExistingSessions::default();
        self.update_session_picker(cx);
        self.quick.closing = None;
        self.refresh_quick_terminal(cx);
        if self.server_preferences.busy || !self.server_preferences.pending.is_empty() {
            let message = "Disconnected before settings were confirmed. Reconnect and reload before retrying.";
            self.preference_result("server", Some(message), cx);
            for id in self.pending_server_fields() {
                self.preference_result(&id, Some(message), cx);
            }
        }
        self.server_preferences.busy = false;
        self.server_preferences.pending.clear();
        self.sync_preferences(cx);
        self.pending.clear();
        self.pending_close = None;
        if self.close_prompt.is_none() {
            self.close_request = None;
        }
        self.discarding.clear();
        self.loaded.clear();
        for pane in self.grids.values() {
            pane.view
                .update(cx, |pane, cx| pane.set_state(PaneState::Disconnected, cx));
        }
        cx.notify();
    }

    fn send(&mut self, work: Work, cx: &mut Context<Self>) -> bool {
        if !matches!(
            work,
            Work::Input(..) | Work::Mouse(..) | Work::CellSize(..) | Work::Ack(..) | Work::Flush
        ) {
            crate::diagnostics::event(
                "model.send",
                format_args!(
                    "generation={} kind={} connection={:?}",
                    self.generation,
                    work.name(),
                    self.connection
                ),
            );
        }
        if self.connection != ConnectionState::Ready && !matches!(work, Work::Flush) {
            self.fail("Server disconnected".into(), cx);
            return false;
        }
        if self.work.send((self.generation, work)).is_err() {
            self.disconnect(cx);
            self.fail("The server connection worker stopped".into(), cx);
            return false;
        }
        true
    }

    pub(crate) fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.set_banner_error(Some(error));
        cx.notify();
    }
}

impl Drop for AppModel {
    fn drop(&mut self) {
        let _ = store::save(&self.path, &self.state);
        let _ = self.work.send((self.generation, Work::Stop));
    }
}

#[cfg(test)]
mod tests {
    pub(super) mod extensions;
    mod webviews;
    use muxy_protocol::ExitReason;
    mod activity;
    mod banners;
    mod clipboard;
    mod colors;
    mod command_palette;
    mod composer;
    mod detach;
    mod find;
    mod git;
    mod links;
    mod mouse;
    mod preferences;
    mod progress;
    mod project_artwork;
    mod projects;
    mod quick_terminal;
    mod rendering;
    mod scrollback;
    mod server_status;
    mod session_ownership;
    mod sidebar;
    mod splits;
    mod tab_menu;
    mod tab_sidebar;
    mod tab_strip;
    mod tui;
    mod updates;
    mod window_bounds;

    use muxy_client::Client;
    use std::io::{self, Write};
    use std::thread;
    use std::time::{Duration, Instant};

    use gpui::{Keystroke, Modifiers, TestAppContext, VisualTestContext, px, size};

    use super::*;

    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    fn acknowledge_catalog(model: &mut AppModel, cx: &mut Context<AppModel>) {
        if let Some(sessions) = &mut model.catalog.restore {
            for session in sessions {
                if let Some(project) = model.state.projects().iter().find(|project| project.tabs.iter().flat_map(|tab| &tab.panes).any(|pane| matches!(pane.content, PaneContent::Terminal { session: Some(id) } if id == session.id))) {
                    session.project = project.id;
                }
            }
        }

        while let Some(intent) = model.state.project_intents().first().cloned() {
            model
                .state
                .complete_project_intent(intent.operation)
                .expect("fixture server acknowledged project");
        }
        let page = muxy_protocol::CatalogPage {
            server: muxy_protocol::ServerIdentity::from_u128(1),
            home: model.state.home().id,
            revision: model.state.catalog_revision() + 1,
            projects: model
                .state
                .projects()
                .iter()
                .map(muxy_app_core::Project::descriptor)
                .collect(),
            next: None,
            legacy_home: None,
        };
        model.receive_catalog(Ok(page), cx);
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
                    model.generation,
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
        let (work, requests) = std::sync::mpsc::channel();
        let (_, updates) = async_channel::unbounded();
        let directory = std::env::temp_dir().join(format!("muxy-app-restore-{}", ProjectId::new()));
        (
            Boot {
                composer: muxy_app_core::composer::ComposerStore::load_from(&directory),
                state,
                state_path: directory.join("state.json"),
                settings: muxy_app_core::settings::Settings::default(),
                terminal: muxy_app_core::settings::TerminalSettings::default(),
                work,
                updates,
            },
            requests,
        )
    }

    #[gpui::test]
    #[allow(clippy::float_cmp)]
    fn terminal_config_reload_applies_live_preserves_zoom_and_rejects_invalid_files(
        cx: &mut TestAppContext,
    ) {
        let mut state = AppState::bootstrap().expect("state");
        state.open_terminal_tab(state.home().id).expect("tab");
        let (boot, _requests) = stub_boot(state);
        let path = boot.state_path.with_file_name("ghostty.conf");
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            let id = model.active_pane().expect("pane");
            model.grids[&id].view.update(cx, |pane, _| pane.terminal.zoom(2.0));
            std::fs::write(&path, "font-size = 21\nbackground = 123456\nmacos-option-as-alt = false\nkeybind = shift+enter=text:\\x1b\\r\nwindow-save-state = always\n").expect("config");
            model.reload_configuration(cx);
            assert_eq!(model.terminal.font_size, 21.0);
            assert_eq!(model.palette.background, 0x12_34_56);
            let pane = model.grids[&id].view.read(cx);
            assert_eq!(pane.terminal.font_size, 15.0);
            assert_eq!(pane.configured_font_size, 21.0);
            assert_eq!(pane.terminal.macos_option_as_alt, muxy_app_core::settings::OptionAsAlt::False);
            assert!(model.configuration_error.is_none());
            assert!(model.terminal.diagnostics.iter().any(|message| message.contains("window-save-state")));
            for source in ["background = broken\n", "theme = missing-theme-123\n"] {
                std::fs::write(&path, source).expect("config");
                model.reload_configuration(cx);
                assert_eq!(model.palette.background, 0x12_34_56);
                assert_eq!(model.terminal.font_size, 21.0);
                assert!(model.configuration_error.is_some());
                let error = model.configuration_error.clone();
                model.refresh_theme(cx);
                assert_eq!(model.configuration_error, error);
            }
            std::fs::write(&path, "font-size = 19\n").expect("config");
            model.reload_configuration(cx);
            assert!(model.configuration_error.is_none());
            assert_eq!(model.terminal.font_size, 19.0);
            assert!(model.terminal.options.background.is_none());
        });
    }

    #[gpui::test]
    fn terminal_reload_updates_hidden_tabs_and_preserves_only_explicit_zoom(
        cx: &mut TestAppContext,
    ) {
        let mut state = AppState::bootstrap().expect("state");
        let first = state.open_terminal_tab(state.home().id).expect("tab");
        let second = state.open_terminal_tab(state.home().id).expect("tab");
        let (boot, _requests) = stub_boot(state);
        let path = boot.state_path.with_file_name("ghostty.conf");
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.select_tab(first, cx);
            model.select_tab(second, cx);
            assert!(model.font_sizes.is_empty());
            std::fs::write(&path, "font-size = 21\n").expect("config");
            model.reload_configuration(cx);
            for tab in [first, second] {
                model.select_tab(tab, cx);
                let pane = &model.grids[&model.active_pane().expect("pane")].view;
                assert_eq!(
                    pane.read(cx).terminal.font_size.to_bits(),
                    21.0_f32.to_bits()
                );
            }
            let pane = &model.grids[&model.active_pane().expect("pane")].view;
            pane.update(cx, |pane, _| pane.terminal.zoom(2.0));
            model.select_tab(first, cx);
            std::fs::write(&path, "font-size = 19\n").expect("config");
            model.reload_configuration(cx);
            model.select_tab(second, cx);
            let pane = &model.grids[&model.active_pane().expect("pane")].view;
            assert_eq!(
                pane.read(cx).terminal.font_size.to_bits(),
                23.0_f32.to_bits()
            );
            pane.update(cx, |pane, _| {
                pane.terminal.font_size = pane.configured_font_size;
            });
            model.reload_configuration(cx);
            let pane = &model.grids[&model.active_pane().expect("pane")].view;
            assert_eq!(
                pane.read(cx).terminal.font_size.to_bits(),
                19.0_f32.to_bits()
            );
            model.select_tab(first, cx);
            model.select_tab(second, cx);
            assert!(model.font_sizes.is_empty());
        });
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
            model.receive((1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            model.new_tab(cx);
            let pane = model.active_pane().expect("pane");
            let session = SessionId::new(42).expect("session");
            model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
            model.receive(
                (
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
                restore::plan(&restored, &[]),
                restore::RestorePlan::default()
            );
        });
    }

    #[gpui::test]
    fn close_during_initial_exit_removes_tab_and_discards_the_created_session(
        cx: &mut TestAppContext,
    ) {
        let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive((1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            model.new_tab(cx);
            let pane = model.active_pane().expect("pane");
            let tab = model.active_tab().expect("tab");
            let session = SessionId::new(42).expect("session");
            model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
            model.close_tab(tab, cx);
            model.receive(
                (
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
            assert_eq!(model.state.pending_discards(), &[session]);
            assert!(
                requests
                    .try_iter()
                    .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
            );
            model.receive(
                (
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
            model.receive((1, Update::Event(ClientEvent::Disconnected)), cx);
            model.close_tab(tab, cx);
            assert!(model.state.home().tabs.is_empty());
            assert!(model.error.is_none());
            assert_eq!(model.state.pending_discards(), &[session]);
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
                    .pending_discards()
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
            model.receive((1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            model.close_tab(tab, cx);
            assert_eq!(model.state.home().tabs.len(), 1);
            assert_eq!(model.active_tab(), Some(other));
            assert_eq!(
                store::load(&model.path)
                    .expect("persisted close")
                    .pending_discards(),
                &[session]
            );
            assert!(
                requests
                    .try_iter()
                    .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
            );
            model.receive(
                (
                    1,
                    Update::Discarded {
                        session,
                        result: Err(muxy_client::ClientError::Disconnected),
                    },
                ),
                cx,
            );
            assert_eq!(model.state.home().tabs.len(), 1);
            assert_eq!(model.state.pending_discards(), &[session]);
            assert!(model.error.is_none());
            model.connect(cx);
            model.receive((2, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            assert!(
                requests
                    .try_iter()
                    .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
            );
            model.receive(
                (
                    2,
                    Update::Discarded {
                        session,
                        result: Ok(()),
                    },
                ),
                cx,
            );
            assert!(model.state.pending_discards().is_empty());
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
            model.receive((1, Update::Connected(vec![])), cx);
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
            assert!(model.state.pending_discards().is_empty());
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
    fn suppressing_the_native_close_prompt_persists_and_skips_future_process_checks(
        cx: &mut TestAppContext,
    ) {
        let mut state = AppState::bootstrap().expect("state");
        let first = state.open_terminal_tab(state.home().id).expect("first");
        let first_pane = state.home().tabs[0].panes[0].id;
        let first_session = SessionId::new(42).expect("session");
        state
            .set_pane_session(first_pane, Some(first_session))
            .expect("session");
        let second = state.open_terminal_tab(state.home().id).expect("second");
        let second_pane = state.home().tabs[1].panes[0].id;
        let second_session = SessionId::new(43).expect("session");
        state
            .set_pane_session(second_pane, Some(second_session))
            .expect("session");
        state.select_tab(state.home().id, first).expect("select");
        let (boot, requests) = stub_boot(state);
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive(
                (
                    1,
                    Update::Connected(
                        [first_session, second_session]
                            .map(|id| SessionInfo {
                                project: ProjectId::from_u128(1),
                                id,
                                directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                            })
                            .to_vec(),
                    ),
                ),
                cx,
            );
            acknowledge_catalog(model, cx);
            let mut attachment = attachment();
            attachment.process = Some(muxy_protocol::ForegroundProcess {
                name: "sleep".into(),
                is_shell: false,
            });
            model.receive(
                (
                    1,
                    Update::Attached {
                        pane: first_pane,
                        session: first_session,
                        attachment,
                        created: false,
                    },
                ),
                cx,
            );
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("cmd-w");
        view.update(cx, acknowledge_close);
        cx.run_until_parked();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer(crate::views::confirm::CLOSE_WITHOUT_ASKING);
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        view.update(cx, |model, cx| {
            assert_eq!(model.state.home().tabs.len(), 1);
            assert_eq!(model.active_tab(), Some(second));
            assert!(!model.settings.window.confirm_running_process);
            let saved = muxy_app_core::settings::Settings::load(
                &model.path.with_file_name("settings.toml"),
            )
            .expect("saved settings");
            assert!(!saved.window.confirm_running_process);
            model.close_tab(second, cx);
            assert!(model.state.home().tabs.is_empty());
        });
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
        assert_eq!(
            work.iter()
                .filter(|work| matches!(work, Work::CheckClose { .. }))
                .count(),
            1
        );
        for session in [first_session, second_session] {
            assert!(
                work.iter()
                    .any(|work| matches!(work, Work::Discard(id, _) if *id == session))
            );
        }
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
                    model.receive(
                        (
                            1,
                            Update::Attached {
                                pane,
                                session,
                                attachment,
                                created: false,
                            },
                        ),
                        cx,
                    );
                    model.receive((1, Update::Event(ClientEvent::Disconnected)), cx);
                    model.connect(cx);
                }
                model.receive(
                    (
                        generation,
                        Update::Connected(vec![SessionInfo {
                            project: ProjectId::from_u128(1),
                            id: session,
                            directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                        }]),
                    ),
                    cx,
                );
                acknowledge_catalog(model, cx);
                model.select_tab(first, cx);
                let terminal = model.terminal(&pane).expect("terminal");
                assert!(terminal.view.read(cx).channel().is_none());
                model.close_tab(first, cx);
                model.close_tab(first, cx);
                assert_eq!(model.pending_close, Some(first));
                assert_eq!(model.state.home().tabs.len(), 2);
                assert!(model.state.pending_discards().is_empty());
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
                assert!(model.state.pending_discards().is_empty());
                assert!(model.close_prompt.is_none());
            });
        }
    }

    #[gpui::test]
    fn closing_an_inactive_live_tab_checks_fresh_metadata_once(cx: &mut TestAppContext) {
        let mut state = AppState::bootstrap().expect("state");
        let first = state.open_terminal_tab(state.home().id).expect("first");
        let pane = state.home().tabs[0].panes[0].id;
        let session = SessionId::new(42).expect("session");
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
        let second = state.open_terminal_tab(state.home().id).expect("second");
        let (boot, requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive(
                (
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
            model.close_tab(first, cx);
            model.close_tab(first, cx);
            assert_eq!(model.pending_close, Some(first));
            assert_eq!(model.state.home().tabs.len(), 2);
            assert_eq!(model.active_tab(), Some(second));
            assert_eq!(
                requests
                    .try_iter()
                    .filter(|(_, work)| matches!(work, Work::CheckClose { .. }))
                    .count(),
                1
            );
            model.receive(
                (
                    1,
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
        view.update(cx, |model, cx| {
            assert!(model.close_prompt.is_none());
            assert_eq!(model.state.home().tabs.len(), 2);
            model.close_tab(first, cx);
            model.disconnect(cx);
            assert!(model.pending_close.is_none());
            model.receive(
                (
                    1,
                    Update::CloseChecked {
                        tab: first,
                        session,
                        result: Ok(None),
                    },
                ),
                cx,
            );
            assert_eq!(model.state.home().tabs.len(), 2);
        });
    }

    #[gpui::test]
    fn repeated_bells_extend_the_flash_and_expiry_notifies_the_tab_strip(cx: &mut TestAppContext) {
        let executor = cx.executor();
        let (boot, _) = stub_boot(AppState::bootstrap().expect("state"));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        let pane = view.update(cx, |model, cx| {
            model.new_tab(cx);
            model
                .terminal(&model.active_pane().expect("pane"))
                .expect("terminal")
                .view
                .clone()
        });
        pane.update(cx, |pane, cx| {
            pane.metadata(muxy_protocol::MetadataEvent::Bell, cx);
        });
        cx.run_until_parked();
        assert!(pane.read_with(cx, |pane, _| pane.bell_flashing));
        executor.advance_clock(Duration::from_secs(1));
        pane.update(cx, |pane, cx| {
            pane.metadata(muxy_protocol::MetadataEvent::Bell, cx);
        });
        cx.run_until_parked();
        executor.advance_clock(Duration::from_millis(300));
        cx.run_until_parked();
        assert!(pane.read_with(cx, |pane, _| pane.bell_flashing));
        executor.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert!(!pane.read_with(cx, |pane, _| pane.bell_flashing));
    }

    fn attachment() -> muxy_client::Attachment {
        let screen = saved_screen();
        muxy_client::Attachment {
            channel: muxy_protocol::ChannelId(1),
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
            model.receive((1, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            assert!(model.state.home().tabs.is_empty());
            assert!(
                store::load(&model.path)
                    .expect("saved state")
                    .home()
                    .tabs
                    .is_empty()
            );
            model.receive((1, Update::Event(ClientEvent::Disconnected)), cx);
            model.connect(cx);
            model.receive((1, Update::ConnectFailed("stale".into())), cx);
            assert!(model.connection == ConnectionState::Connecting);
            model.receive((2, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            assert!(model.connection == ConnectionState::Ready);
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
        let other = state.add_project(std::env::temp_dir()).expect("project");
        state.open_terminal_tab(other).expect("tab");
        let invalid = state.window().active_pane.expect("pane");
        state
            .set_pane_session(invalid, Some(session))
            .expect("session");
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.apply_restore(
                &[SessionInfo {
                    project: home,
                    id: session,
                    directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                }],
                cx,
            );
            assert_eq!(model.pane_session(valid), Some(session));
            assert!(model.pane_tab(invalid).is_none());
            assert!(!model.state.pending_discards().contains(&session));
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
        let hidden = state.add_project(std::env::temp_dir()).expect("project");
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
            model.receive((1, Update::CloseSessionPanes(dead)), cx);
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
            assert_eq!(model.state.session_references(), vec![live]);
            assert_eq!(
                store::load(&model.path)
                    .expect("saved state")
                    .session_references(),
                vec![live]
            );
            model.receive(
                (
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
            model.receive((1, Update::ConnectFailed("offline".into())), cx);
            assert_eq!(model.state.home().tabs.len(), 1);
            model.connect(cx);
            model.receive(
                (
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
            model.receive((1, Update::Connected(vec![])), cx);
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

    #[gpui::test]
    fn empty_startup_stays_empty_on_success_and_failure(cx: &mut TestAppContext) {
        let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.receive((1, Update::ConnectFailed("offline".into())), cx);
            model.connect(cx);
            model.receive((2, Update::Connected(vec![])), cx);
            acknowledge_catalog(model, cx);
            assert!(model.state.home().tabs.is_empty());
        });
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Attach { .. }))
        );
    }

    #[gpui::test]
    #[ignore = "requires a built muxy CLI and a fresh MUXY_DIR under /tmp/muxy-phase12-"]
    fn phase12_settings_walkthrough(cx: &mut TestAppContext) {
        let result = run_settings_walkthrough(cx);
        assert!(result.is_ok(), "{result:?}");
    }

    #[gpui::test]
    #[ignore = "requires an isolated MUXY_DIR under /tmp/muxy-phase13- and a server PID wrapper"]
    fn phase13_restore_walkthrough(cx: &mut TestAppContext) {
        let result = run_restore_walkthrough(cx);
        assert!(result.is_ok(), "{result:?}");
    }

    #[gpui::test]
    #[ignore = "requires a server binary and an isolated MUXY_DIR under /tmp/muxy-phase13-"]
    fn phase13_close_walkthrough(cx: &mut TestAppContext) {
        let result = run_close_walkthrough(cx);
        assert!(result.is_ok(), "{result:?}");
    }

    #[gpui::test]
    #[ignore = "requires a server binary and a fresh MUXY_DIR under /tmp/muxy-phase14-"]
    fn phase14_metadata_walkthrough(cx: &mut TestAppContext) {
        let result = run_metadata_walkthrough(cx);
        assert!(result.is_ok(), "{result:?}");
    }

    fn run_metadata_walkthrough(cx: &mut TestAppContext) -> Result {
        let directory = PathBuf::from(std::env::var("MUXY_DIR")?);
        assert!(
            directory
                .to_string_lossy()
                .starts_with("/tmp/muxy-phase14-")
        );
        assert!(!directory.join("state.json").exists());
        let boot = Boot::load()?;
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, AppModel::new_tab);
        wait_live(cx, &view)?;
        shell(cx, "cd /tmp");
        wait_title(cx, &view, "tmp")?;
        report("14.1: cd /tmp -> tab title = tmp")?;
        shell(cx, "vim -u NONE -i NONE");
        wait_title(cx, &view, "vim")?;
        shell(cx, ":q");
        wait_title(cx, &view, "tmp")?;
        report("14.2: vim (clean configuration) -> vim; :q -> tmp")?;
        shell(cx, "printf '\\033]0;hello\\007'");
        wait_title(cx, &view, "hello")?;
        report("14.3: OSC title hello -> tab title = hello")?;
        for command in ["sleep 60", "echo x | sleep 60"] {
            shell(cx, command);
            wait(cx, &view, |model, cx| {
                active_process(model, cx)
                    .is_some_and(|process| process.name == "sleep" && !process.is_shell)
            })?;
            cx.simulate_keystrokes("cmd-w cmd-w");
            wait(cx, &view, |model, _| model.close_prompt.is_some())?;
            assert_eq!(
                cx.pending_prompt(),
                Some((
                    crate::views::confirm::PANE_TITLE.into(),
                    crate::views::confirm::PANE_MESSAGE.into()
                ))
            );
            cx.simulate_prompt_answer("Cancel");
            wait(cx, &view, |model, _| model.close_prompt.is_none())?;
            assert!(!cx.has_pending_prompt());
            assert_eq!(
                view.read_with(cx, |model, _| model.state.home().tabs.len()),
                1
            );
            let session = active_session(&view, cx)?;
            let probe = Client::connect(&directory.join("server.sock"))?;
            let current = probe.attach(session, Size { cols: 80, rows: 24 })?;
            assert!(
                current
                    .process
                    .is_some_and(|process| process.name == "sleep" && !process.is_shell)
            );
            probe.detach(current.channel)?;
            report(&format!(
                "14.4: {command} + Cmd-W twice -> one native prompt; Cancel -> tab kept, sleep still running",
            ))?;
            cx.simulate_keystrokes("ctrl-c");
            wait(cx, &view, |model, cx| {
                active_process(model, cx).is_some_and(|process| process.is_shell)
            })?;
        }
        shell(cx, "printf '\\007'");
        wait(cx, &view, |model, cx| {
            model
                .active_pane()
                .and_then(|pane| model.terminal(&pane))
                .is_some_and(|pane| pane.view.read(cx).bell_flashing)
        })?;
        assert!(cx.debug_bounds("tab-bell").is_some());
        cx.executor().advance_clock(Duration::from_millis(1250));
        cx.run_until_parked();
        assert!(view.read_with(cx, |model, cx| {
            model
                .active_pane()
                .and_then(|pane| model.terminal(&pane))
                .is_some_and(|pane| !pane.view.read(cx).bell_flashing)
        }));
        assert!(cx.debug_bounds("tab-terminal").is_some());
        report("14.5: BEL -> accent bell icon rendered; icon cleared after 1,250 ms")?;
        cx.simulate_keystrokes("cmd-w");
        wait(cx, &view, |model, _| {
            model.state.home().tabs.is_empty() && model.state.pending_discards().is_empty()
        })?;
        report("Phase 14 GPUI/live-server walkthrough: PASS")
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

    fn wait_title(cx: &mut VisualTestContext, view: &Entity<AppModel>, title: &str) -> Result {
        wait(cx, view, |model, _| {
            model
                .state
                .home()
                .tabs
                .iter()
                .find(|tab| Some(tab.id) == model.active_tab())
                .is_some_and(|tab| tab.title(model.state.window().active_pane) == title)
        })
    }

    fn run_close_walkthrough(cx: &mut TestAppContext) -> Result {
        let directory = PathBuf::from(std::env::var("MUXY_DIR")?);
        assert!(
            directory
                .to_string_lossy()
                .starts_with("/tmp/muxy-phase13-")
        );
        assert!(!directory.join("state.json").exists());
        let boot = Boot::load()?;
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, AppModel::new_tab);
        wait_live(cx, &view)?;
        let closed = active_session(&view, cx)?;
        let closed_pid = record_shell_pid(cx, &view, &directory, "closed")?;
        shell(cx, "sleep 60");
        wait(cx, &view, |model, cx| {
            active_process(model, cx).is_some_and(|process| process.name == "sleep")
        })?;
        cx.simulate_keystrokes("cmd-t");
        wait_live(cx, &view)?;
        let other = active_session(&view, cx)?;
        let other_pid = record_shell_pid(cx, &view, &directory, "other")?;
        let probe = Client::connect(&directory.join("server.sock"))?;
        assert_eq!(probe.list_sessions()?.len(), 2);
        cx.simulate_keystrokes("cmd-1");
        wait_live(cx, &view)?;
        cx.simulate_keystrokes("cmd-w");
        wait(cx, &view, |model, _| model.close_prompt.is_some())?;
        cx.simulate_prompt_answer("Close");
        wait(cx, &view, |model, _| model.state.home().tabs.len() == 1)?;
        assert_eq!(
            view.read_with(cx, |model, _| model.state.home().tabs.len()),
            1
        );
        wait(cx, &view, |model, _| {
            model.state.pending_discards().is_empty()
        })?;
        assert!(!process_exists(closed_pid));
        assert!(process_exists(other_pid));
        assert_eq!(
            probe
                .list_sessions()?
                .iter()
                .map(|session| session.id)
                .collect::<Vec<_>>(),
            vec![other]
        );
        wait_live(cx, &view)?;
        shell(cx, "printf 'OTHER_%s\\n' STILL_RUNNING");
        wait_text(cx, &view, "OTHER_STILL_RUNNING")?;
        probe.ping()?;
        assert!(view.read_with(cx, |model, cx| model.error.is_none()
            && crate::views::disconnected::label(model.status(cx)).is_none()));
        report(&format!(
            "close: session {} / shell PID {closed_pid} terminated; session {} / shell PID {other_pid} stayed alive, accepted input, and connection remained usable",
            closed.get(),
            other.get()
        ))?;
        view.update(cx, AppModel::disconnect);
        cx.simulate_keystrokes("cmd-w");
        assert!(view.read_with(cx, |model, _| model.state.home().tabs.is_empty()));
        assert!(process_exists(other_pid));
        assert_eq!(
            store::load(directory.join("desktop-state.json"))?.pending_discards(),
            &[other]
        );
        reload_model(cx, &view)?;
        wait(cx, &view, |model, _| {
            model.connection == ConnectionState::Ready && model.state.pending_discards().is_empty()
        })?;
        assert!(!process_exists(other_pid));
        assert!(probe.list_sessions()?.is_empty());
        assert!(view.read_with(cx, |model, _| model.state.home().tabs.is_empty()));
        report(&format!(
            "offline close: tab removed immediately; pending termination persisted; Boot reload killed shell PID {other_pid}; list_sessions = []; no tab restored"
        ))?;
        cx.update(|window, _| window.remove_window());
        Ok(())
    }

    fn active_session(view: &Entity<AppModel>, cx: &VisualTestContext) -> Result<SessionId> {
        view.read_with(cx, |model, _| {
            model
                .active_pane()
                .and_then(|pane| model.pane_session(pane))
        })
        .ok_or_else(|| io::Error::other("no active session").into())
    }

    fn record_shell_pid(
        cx: &mut VisualTestContext,
        view: &Entity<AppModel>,
        directory: &std::path::Path,
        name: &str,
    ) -> Result<u32> {
        let path = directory.join(format!("{name}.pid"));
        shell(cx, &format!("printf '%s\\n' \"$$\" > '{}'", path.display()));
        wait(cx, view, |_, _| {
            std::fs::read_to_string(&path).is_ok_and(|text| text.trim().parse::<u32>().is_ok())
        })?;
        let pid = std::fs::read_to_string(path)?.trim().parse()?;
        assert!(process_exists(pid));
        Ok(pid)
    }

    fn process_exists(pid: u32) -> bool {
        std::process::Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()
            .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
    }

    fn run_restore_walkthrough(cx: &mut TestAppContext) -> Result {
        let directory = PathBuf::from(std::env::var("MUXY_DIR")?);
        assert!(
            directory
                .to_string_lossy()
                .starts_with("/tmp/muxy-phase13-")
        );
        assert!(!directory.join("state.json").exists());
        let boot = Boot::load()?;
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        wait(cx, &view, |model, _| {
            model.connection == ConnectionState::Ready
        })?;
        assert!(view.read_with(cx, |model, _| model.state.home().tabs.is_empty()));
        cx.simulate_keystrokes("cmd-t");
        wait_live(cx, &view)?;
        shell(cx, "top -l 1000 -s 1 -n 3");
        wait_text(cx, &view, "Processes:")?;
        cx.simulate_keystrokes("cmd-t");
        wait_live(cx, &view)?;
        shell(cx, "cd /tmp; printf 'DIRECTORY_%s\\n' \"$PWD\"");
        wait_text(cx, &view, "DIRECTORY_/tmp")?;
        cx.simulate_keystrokes("cmd-t");
        wait_live(cx, &view)?;
        shell(cx, "printf 'phase-13 history marker\\n'; seq 1 5000");
        wait_text(cx, &view, "5000")?;
        cx.simulate_resize(size(px(1000.0), px(700.0)));
        cx.run_until_parked();
        let before = view.read_with(cx, |model, _| model.state.clone());
        report(&format!(
            "13.1: three tabs/session references {:?}; selected {:?}; bounds {:?}",
            before
                .home()
                .tabs
                .iter()
                .map(|tab| (tab.id, tab.panes[0].content.clone()))
                .collect::<Vec<_>>(),
            before.window().selected_tab,
            before.window().bounds
        ))?;
        shell(cx, "sleep 1; printf 'WHILE_%s\\n' CLOSED");
        cx.simulate_keystrokes("cmd-q");
        thread::sleep(Duration::from_millis(150));
        cx.run_until_parked();
        verify_output_while_closed(cx, &view, &directory)?;
        reload_model(cx, &view)?;
        wait_live(cx, &view)?;
        assert_eq!(view.read_with(cx, |model, _| model.state.clone()), before);
        wait_text(cx, &view, "WHILE_CLOSED")?;
        let probe = Client::connect(&directory.join("server.sock"))?;
        assert_eq!(probe.list_sessions()?.len(), 3);
        cx.simulate_keystrokes("cmd-1");
        wait_live(cx, &view)?;
        wait_text(cx, &view, "Processes:")?;
        cx.simulate_keystrokes("cmd-2");
        wait_live(cx, &view)?;
        shell(cx, "printf 'RESTORED_%s\\n' \"$PWD\"");
        wait_text(cx, &view, "RESTORED_/tmp")?;
        report(
            "13.2: Cmd-Q action, model teardown and Boot reload preserved all IDs, order, selection and bounds; top continued; shell stayed in /tmp; WHILE_CLOSED was saved with no app connection",
        )?;
        verify_automatic_exit(cx, &view)?;
        verify_server_restarts(cx, &view, &directory)?;
        verify_discard_and_end_all(cx, &view, &directory)?;
        Ok(())
    }

    fn verify_output_while_closed(
        cx: &mut VisualTestContext,
        view: &Entity<AppModel>,
        directory: &std::path::Path,
    ) -> Result {
        let session = view
            .read_with(cx, |model, _| {
                model.pane_session(model.active_pane().expect("pane"))
            })
            .expect("session");
        let (boot, _) = stub_boot(AppState::bootstrap()?);
        cx.update(|window, cx| {
            view.update(cx, |model, cx| *model = AppModel::new(boot, window, cx));
        });
        let probe = Client::connect(&directory.join("server.sock"))?;
        wait(cx, view, |_, _| {
            probe.read_saved_screen(session).is_ok_and(|screen| {
                screen
                    .rows
                    .iter()
                    .flat_map(|row| &row.runs)
                    .any(|run| run.text.contains("WHILE_CLOSED"))
            })
        })
    }

    fn reload_model(cx: &mut VisualTestContext, view: &Entity<AppModel>) -> Result {
        let boot = Boot::load()?;
        cx.update(|window, cx| {
            view.update(cx, |model, cx| *model = AppModel::new(boot, window, cx));
        });
        Ok(())
    }

    fn wait_live(cx: &mut VisualTestContext, view: &Entity<AppModel>) -> Result {
        wait(cx, view, |model, cx| {
            model
                .active_pane()
                .and_then(|id| model.terminal(&id))
                .is_some_and(|pane| pane.view.read(cx).channel().is_some())
        })
    }

    fn wait_empty(cx: &mut VisualTestContext, view: &Entity<AppModel>) -> Result {
        wait(cx, view, |model, _| {
            model.connection == ConnectionState::Ready && model.state.home().tabs.is_empty()
        })
    }

    fn verify_automatic_exit(cx: &mut VisualTestContext, view: &Entity<AppModel>) -> Result {
        let exited = view
            .read_with(cx, |model, _| model.active_tab())
            .ok_or("tab")?;
        shell(cx, "exit");
        wait(cx, view, |model, _| {
            !model.state.home().tabs.iter().any(|tab| tab.id == exited)
        })?;
        reload_model(cx, view)?;
        wait_live(cx, view)?;
        assert!(!view.read_with(cx, |model, _| {
            model.state.home().tabs.iter().any(|tab| tab.id == exited)
        }));
        report(
            "13.3: Session exit closed its tab immediately and it stayed closed after Boot reload",
        )
    }

    fn signal_test_server(directory: &std::path::Path, signal: &str) -> Result {
        let pid = std::fs::read_to_string(directory.join("server.pid"))?;
        let pid: u32 = pid.trim().parse()?;
        assert!(pid > 1);
        let status = std::process::Command::new("/bin/kill")
            .args([signal, &pid.to_string()])
            .status()?;
        assert!(status.success());
        Ok(())
    }

    fn verify_server_restarts(
        cx: &mut VisualTestContext,
        view: &Entity<AppModel>,
        directory: &std::path::Path,
    ) -> Result {
        signal_test_server(directory, "-TERM")?;
        wait(cx, view, |model, _| {
            model.connection == ConnectionState::Disconnected
        })?;
        view.update(cx, AppModel::connect);
        wait_empty(cx, view)?;
        let probe = Client::connect(&directory.join("server.sock"))?;
        assert!(probe.list_sessions()?.is_empty());
        report("13.4: SIGTERM and reconnect closed every ended terminal tab")?;
        cx.simulate_keystrokes("cmd-t");
        wait_live(cx, view)?;
        shell(cx, "printf 'CRASH_%s\\n' CHECKPOINT");
        wait_text(cx, view, "CRASH_CHECKPOINT")?;
        let id = view
            .read_with(cx, |model, _| {
                model.pane_session(model.active_pane().expect("pane"))
            })
            .expect("session");
        wait(cx, view, |_, _| {
            probe.read_saved_screen(id).is_ok_and(|screen| {
                screen
                    .rows
                    .iter()
                    .flat_map(|row| &row.runs)
                    .any(|run| run.text.contains("CRASH_CHECKPOINT"))
            })
        })?;
        signal_test_server(directory, "-KILL")?;
        wait(cx, view, |model, _| {
            model.connection == ConnectionState::Disconnected
        })?;
        reload_model(cx, view)?;
        wait_empty(cx, view)?;
        report(
            "13.5: SIGKILL and Boot/server restart removed the dead terminal without a replacement process",
        )
    }

    fn verify_discard_and_end_all(
        cx: &mut VisualTestContext,
        view: &Entity<AppModel>,
        directory: &std::path::Path,
    ) -> Result {
        let probe = Client::connect(&directory.join("server.sock"))?;
        cx.simulate_keystrokes("cmd-t");
        wait_live(cx, view)?;
        wait(cx, view, |model, cx| {
            active_process(model, cx).is_some_and(|process| process.is_shell)
        })?;
        let live = view
            .read_with(cx, |model, _| {
                model.pane_session(model.active_pane().expect("pane"))
            })
            .expect("session");
        cx.simulate_keystrokes("cmd-w");
        wait(cx, view, |model, _| {
            model.state.home().tabs.is_empty() && model.state.pending_discards().is_empty()
        })?;
        assert!(probe.list_sessions()?.is_empty());
        assert!(probe.read_saved_screen(live).is_err());
        reload_model(cx, view)?;
        wait_empty(cx, view)?;
        assert_eq!(
            view.read_with(cx, |model, _| model.state.home().tabs.len()),
            0
        );
        report(
            "13.7: closing a live tab discarded its record, terminated the process, and the tab stayed closed after Boot reload",
        )?;
        for _ in 0..2 {
            cx.simulate_keystrokes("cmd-t");
            wait_live(cx, view)?;
        }
        assert_eq!(probe.list_sessions()?.len(), 2);
        cx.dispatch_action(crate::views::workspace::EndAllSessionsAndQuit);
        wait(cx, view, |model, _| model.state.home().tabs.is_empty())?;
        assert!(probe.list_sessions()?.is_empty());
        assert!(
            store::load(directory.join("desktop-state.json"))?
                .home()
                .tabs
                .is_empty()
        );
        reload_model(cx, view)?;
        wait(cx, view, |model, _| {
            model.connection == ConnectionState::Ready
        })?;
        assert!(view.read_with(cx, |model, _| model.state.home().tabs.is_empty()));
        assert!(probe.list_sessions()?.is_empty());
        report(
            "13.8: End All Sessions and Quit completed termination; phase-9 list empty; Boot reload left Home empty without creating a session",
        )?;
        cx.simulate_keystrokes("cmd-t");
        wait_live(cx, view)?;
        assert_eq!(probe.list_sessions()?.len(), 1);
        cx.dispatch_action(crate::views::workspace::EndAllSessionsAndQuit);
        wait(cx, view, |model, _| model.state.home().tabs.is_empty())?;
        report("13.8: explicit New Tab still starts a shell; final cleanup list_sessions = []")
    }

    #[allow(
        clippy::float_cmp,
        reason = "Configuration values must reach the app unchanged"
    )]
    fn run_settings_walkthrough(cx: &mut TestAppContext) -> Result {
        let directory = PathBuf::from(std::env::var("MUXY_DIR")?);
        assert!(
            directory
                .to_string_lossy()
                .starts_with("/tmp/muxy-phase12-")
        );
        assert!(!directory.join("state.json").exists());
        std::fs::create_dir_all(&directory)?;
        std::fs::write(
            directory.join("settings.toml"),
            "[window]\ndefault_size = [1000, 700]\n[keymap]\nnew_tab = 'cmd-n'\nnext_tab = 'ctrl-alt-right'\n",
        )?;
        let config = "font-family = Menlo\nfont-size = 16\nadjust-cell-height = 10%\n";
        std::fs::write(directory.join("ghostty.conf"), config)?;
        let boot = Boot::load()?;
        assert_eq!(boot.settings.window.default_size, [1000.0, 700.0]);
        assert_eq!(boot.terminal.font_size, 16.0);
        let probe = crate::server::ensure_server_running(&directory.join("server.sock"))?;
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, AppModel::new_tab);
        wait(cx, &view, |model, cx| active_grid(model, cx).is_some())?;
        shell(
            cx,
            "unset RPROMPT; PROMPT='muxy> '; printf 'PHASE12_%s\\n' READY",
        );
        wait_text(cx, &view, "PHASE12_READY")?;
        report("12: boot loaded 1000x700 window settings, Menlo 16 pt and +10% cell height")?;
        let original = view.read_with(cx, |model, cx| active_grid(model, cx).map(|grid| grid.size));
        cx.simulate_keystrokes("cmd-+");
        wait(cx, &view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| Some(grid.size) != original)
        })?;
        assert_eq!(active_font_size(&view, cx), Some(17.0));
        report("12: Cmd-Plus changed active font 16 -> 17 and resized the terminal grid")?;
        cx.simulate_keystrokes("cmd-n");
        wait(cx, &view, |model, cx| {
            model.state.home().tabs.len() == 2 && active_grid(model, cx).is_some()
        })?;
        assert_eq!(active_font_size(&view, cx), Some(16.0));
        cx.simulate_keystrokes("ctrl-alt-right");
        wait(cx, &view, |model, cx| {
            active_grid(model, cx).is_some()
                && model.active_tab() == model.state.home().tabs.first().map(|tab| tab.id)
        })?;
        assert_eq!(active_font_size(&view, cx), Some(17.0));
        cx.simulate_keystrokes("cmd--");
        wait(cx, &view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| Some(grid.size) == original)
        })?;
        assert_eq!(active_font_size(&view, cx), Some(16.0));
        cx.simulate_keystrokes("cmd-t");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model.state.home().tabs.len()),
            2
        );
        assert_eq!(
            std::fs::read_to_string(directory.join("ghostty.conf"))?,
            config
        );
        report(
            "12: Cmd-N opened one tab at 16 pt; Ctrl-Alt-Right switched tabs; first tab retained 17 pt; Cmd-Minus restored 16 pt; old Cmd-T was unbound; ghostty.conf stayed unchanged",
        )?;
        for session in probe.list_sessions()? {
            probe.end_session(session.id)?;
        }
        cx.update(|window, _| window.remove_window());
        Ok(())
    }

    fn active_font_size(view: &Entity<AppModel>, cx: &VisualTestContext) -> Option<f32> {
        view.read_with(cx, |model, cx| {
            Some(
                model
                    .terminal(&model.active_pane()?)?
                    .view
                    .read(cx)
                    .terminal
                    .font_size,
            )
        })
    }

    #[gpui::test]
    #[ignore = "requires a built muxy CLI and a fresh MUXY_DIR under /tmp/muxy-phase11-"]
    fn live_server_walkthrough(cx: &mut TestAppContext) {
        let result = run_walkthrough(cx);
        assert!(result.is_ok(), "{result:?}");
    }

    fn run_walkthrough(cx: &mut TestAppContext) -> Result {
        let directory = PathBuf::from(std::env::var("MUXY_DIR")?);
        assert!(
            directory
                .to_string_lossy()
                .starts_with("/tmp/muxy-phase11-")
        );
        assert!(!directory.join("state.json").exists());
        let boot = Boot::load()?;
        let probe = crate::server::ensure_server_running(&directory.join("server.sock"))?;
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, AppModel::new_tab);
        wait(cx, &view, |model, cx| active_grid(model, cx).is_some())?;
        shell(
            cx,
            "unset RPROMPT; PROMPT='muxy> '; printf 'PHASE11_%s\\n' READY",
        );
        wait_text(cx, &view, "PHASE11_READY")?;
        report(&format!(
            "11a: boot, server handshake, Home session and prompt: {}",
            screen(&view, cx)
        ))?;
        verify_terminal(cx, &view, &directory)?;
        verify_appearance(cx, &view, &directory)?;
        verify_tabs(cx, &view, &probe)?;
        let sessions = probe.list_sessions()?;
        assert_eq!(sessions.len(), 1);
        cx.update(|window, _| window.remove_window());
        drop(view);
        cx.run_until_parked();
        assert_eq!(probe.list_sessions()?.len(), 1);
        report("11a: closing the GPUI window leaves the server and shell session alive")?;
        for session in sessions {
            probe.end_session(session.id)?;
        }
        Ok(())
    }

    fn verify_appearance(
        cx: &mut VisualTestContext,
        view: &Entity<AppModel>,
        directory: &std::path::Path,
    ) -> Result {
        std::fs::write(
            directory.join("themes/Walkthrough.conf"),
            "background = 123456\nforeground = abcdef\npalette = 4=112233\n",
        )?;
        cx.simulate_keystrokes("cmd-shift-k");
        cx.run_until_parked();
        cx.simulate_input("Walkthrough");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(view.read_with(cx, |model, _| model.overlay.is_some()));
        assert_eq!(
            view.read_with(cx, |model, _| model.palette.background),
            0x12_34_56
        );
        let settings = muxy_app_core::settings::Appearance::load(&directory.join("settings.toml"))?;
        assert!(settings.dark_theme == "Walkthrough" || settings.light_theme == "Walkthrough");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        cx.update(|window, cx| {
            let model = view.read(cx);
            assert!(
                model
                    .terminal(&model.active_pane().expect("active pane"))
                    .expect("terminal")
                    .view
                    .read(cx)
                    .focus
                    .is_focused(window)
            );
        });
        std::fs::write(
            directory.join("themes/Walkthrough.conf"),
            "background = 234567\nforeground = abcdef\n",
        )?;
        cx.simulate_keystrokes("cmd-shift-k");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model.palette.background),
            0x23_45_67
        );
        cx.simulate_keystrokes("escape cmd-b");
        cx.run_until_parked();
        assert!(
            muxy_app_core::settings::Appearance::load(&directory.join("settings.toml"))?
                .sidebar_expanded
        );
        cx.simulate_keystrokes("cmd-b");
        report(
            "11 UI: custom theme discovered, searched, applied and persisted; picker stays open; Escape restores terminal focus; reopening reloads changed colors; sidebar preference persists",
        )?;
        Ok(())
    }

    fn verify_terminal(
        cx: &mut VisualTestContext,
        view: &Entity<AppModel>,
        directory: &std::path::Path,
    ) -> Result {
        shell(
            cx,
            "printf '\\033[2J\\033[H'; CLICOLOR_FORCE=1 /bin/ls -G /bin",
        );
        wait(cx, view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| {
                grid.rows.iter().flatten().any(|run| {
                    run.text.contains("bash") && run.style.fg != muxy_protocol::Color::Default
                })
            })
        })?;
        report("11c: ls -G /bin produced colored style runs")?;
        let fixture = directory.join("lines.txt");
        let mut file = std::fs::File::create(&fixture)?;
        for line in 1..=120 {
            writeln!(file, "line{line:03}")?;
        }
        shell(
            cx,
            &format!("/usr/bin/vim -u NONE -i NONE -n {}", fixture.display()),
        );
        wait(cx, view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| {
                grid.row_text(0).starts_with("line001") && grid.modes.application_cursor_keys
            })
        })?;
        cx.simulate_keystrokes("down right");
        wait(cx, view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| grid.cursor.row == 1 && grid.cursor.col == 1)
        })?;
        shell(cx, ":q");
        wait_text(cx, view, "muxy>")?;
        report(
            "11c/11d: vim entered the alternate screen, arrows moved to row 2 / column 2, :q restored the shell",
        )?;
        let before = view.read_with(cx, |model, cx| active_grid(model, cx).map(|grid| grid.size));
        cx.simulate_resize(size(px(800.0), px(600.0)));
        wait(cx, view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| Some(grid.size) != before)
        })?;
        let dimensions = view
            .read_with(cx, |model, cx| active_grid(model, cx).map(|grid| grid.size))
            .ok_or_else(|| io::Error::other("no grid after resize"))?;
        shell(cx, "printf 'SIZE_%s\\n' \"$(stty size)\"");
        wait_text(
            cx,
            view,
            &format!("SIZE_{} {}", dimensions.rows, dimensions.cols),
        )?;
        report(&format!(
            "11c: viewport resize reached PTY: {} rows × {} columns",
            dimensions.rows, dimensions.cols
        ))?;
        shell(cx, "sleep 10");
        thread::sleep(Duration::from_millis(150));
        cx.simulate_keystrokes("ctrl-c");
        shell(cx, "printf 'INTERRUPT_%s\\n' OK");
        wait_text(cx, view, "INTERRUPT_OK")?;
        cx.simulate_keystrokes("up");
        wait(cx, view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| {
                grid.row_text(usize::from(grid.cursor.row))
                    .contains("printf 'INTERRUPT_%s")
            })
        })?;
        cx.simulate_keystrokes("ctrl-c");
        shell(cx, &format!("/usr/bin/less {}", fixture.display()));
        wait(cx, view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| grid.row_text(0).starts_with("line001"))
        })?;
        cx.simulate_keystrokes("pagedown");
        wait(cx, view, |model, cx| {
            active_grid(model, cx).is_some_and(|grid| {
                grid.row_text(0).starts_with("line") && !grid.row_text(0).starts_with("line001")
            })
        })?;
        type_text(cx, "q");
        wait_text(cx, view, "muxy>")?;
        report("11d: typing, Ctrl-C during sleep 10, zsh history Up, and less Page Down passed")
    }

    fn verify_tabs(cx: &mut VisualTestContext, view: &Entity<AppModel>, probe: &Client) -> Result {
        let first = view
            .read_with(cx, |model, _| model.active_tab())
            .ok_or_else(|| io::Error::other("no initial tab"))?;
        cx.simulate_keystrokes("cmd-t");
        wait(cx, view, |model, cx| {
            model.state.home().tabs.len() == 2 && active_grid(model, cx).is_some()
        })?;
        let second = view
            .read_with(cx, |model, _| model.active_tab())
            .ok_or_else(|| io::Error::other("no second tab"))?;
        assert_ne!(first, second);
        assert_eq!(probe.list_sessions()?.len(), 2);
        cx.simulate_keystrokes("cmd-[");
        wait(cx, view, |model, cx| {
            model.active_tab() == Some(first) && active_grid(model, cx).is_some()
        })?;
        cx.simulate_keystrokes("ctrl-tab");
        wait(cx, view, |model, cx| {
            model.active_tab() == Some(second) && active_grid(model, cx).is_some()
        })?;
        view.update(cx, |model, cx| model.move_tab(second, first, cx));
        view.read_with(cx, |model, _| {
            assert_eq!(model.state.home().tabs[0].id, second);
            assert_eq!(model.grids.len(), 1);
            assert!(
                model
                    .state
                    .home()
                    .tabs
                    .iter()
                    .all(|tab| tab.title(model.state.window().active_pane) == "Terminal")
            );
        });
        cx.simulate_keystrokes("cmd-w");
        wait(cx, view, |model, cx| {
            model.state.home().tabs.len() == 1 && active_grid(model, cx).is_some()
        })?;
        wait(cx, view, |_, _| {
            probe
                .list_sessions()
                .is_ok_and(|sessions| sessions.len() == 1)
        })?;
        let saved = store::load(view.read_with(cx, |model, _| model.path.clone()))?;
        assert_eq!(saved.home().tabs.len(), 1);
        shell(cx, "exit");

        wait(cx, view, |model, _| model.state.home().tabs.is_empty())?;
        cx.simulate_keystrokes("cmd-t");
        wait(cx, view, |model, cx| {
            model.state.home().tabs.len() == 1 && active_grid(model, cx).is_some()
        })?;
        report(
            "11b/11c: new/select/cycle/reorder/close, Terminal titles, state persistence, single visible grid, and automatic SessionEnded tab closure passed",
        )
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
