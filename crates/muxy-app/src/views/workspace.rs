use muxy_core::shortcuts::ShortcutId;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Context, FontWeight, InteractiveElement, IntoElement, KeyBinding, ParentElement, Render,
    Role, StatefulInteractiveElement, Styled, Window, actions, div, px, relative,
};
use muxy_app_core::settings::Keymap;
use muxy_ui::accessibility;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::{menu, overlays, sidebar, status_bar, tab_strip, titlebar};
use crate::model::AppModel;
use crate::model::focus::Next;
use crate::model::tab_windows::Surface;

pub(crate) mod commands;

actions!(
    muxy,
    [
        OpenSettings,
        OpenAbout,
        ShowWelcomeTour,
        CheckForUpdates,
        MoveTabToNewWindow,
        MovePaneToNewTab,
        MovePaneToNewWindow,
        InstallCommandLineTool,
        NewTab,
        ExistingTerminals,
        DetachTerminal,
        NewHomeTab,
        NewTabInProject,
        NewWorktree,
        SwitchWorktree,
        CloseTab,
        SplitRight,
        SplitDown,
        FocusPaneLeft,
        FocusPaneRight,
        FocusPaneUp,
        FocusPaneDown,
        ToggleZoomPane,
        EqualizeSplits,
        ClosePane,
        NextTab,
        PreviousTab,
        PreviousProject,
        NextProject,
        AddProject,
        ToggleSidebar,
        ToggleAgentsPanel,
        ToggleAgentSessions,
        ToggleFullScreen,
        ToggleThemePicker,
        ToggleCommandPalette,
        ToggleComposer,
        NavigateBack,
        NavigateForward,
        Quit,
        EndAllSessionsAndQuit,
        HideApp,
        HideOthers,
        ShowAll,
        Minimize,
        Zoom,
        OpenConfiguration,
        ReloadConfiguration,
        IncreaseFontSize,
        DecreaseFontSize,
        ScrollToBottom,
        Find,
        FindNext,
        FindPrevious,
        PreviousPrompt,
        NextPrompt,
        SelectCommandOutput,
        NewBrowserTab,
        BrowserFocusAddress,
        BrowserReload,
        BrowserBack,
        BrowserForward,
        BrowserZoomIn,
        BrowserZoomOut,
        BrowserZoomReset,
        BrowserCopyUrl,
        BrowserInspect
    ]
);

#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = muxy, no_json)]
pub(crate) struct SelectTab {
    pub(crate) index: usize,
}

#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = muxy, no_json)]
pub(crate) struct SelectProject {
    pub(crate) index: usize,
}

pub(crate) fn bind_keys(keymap: &Keymap, cx: &mut App) {
    cx.bind_keys(workspace_bindings(keymap));
}

fn workspace_bindings(keymap: &impl muxy_core::shortcuts::ShortcutSettings) -> Vec<KeyBinding> {
    let mut registry = muxy_ui::shortcuts::Registry::new(keymap);
    commands::bind_shortcuts(&mut registry);
    registry.register(ShortcutId::ComposerSubmit, &super::composer::Submit);
    registry.register(ShortcutId::ComposerInsert, &super::composer::Insert);
    registry.register(ShortcutId::ComposerClose, &super::composer::Close);
    for (index, id) in ShortcutId::TABS.into_iter().enumerate() {
        registry.register(id, &SelectTab { index });
    }
    for (index, id) in ShortcutId::PROJECTS.into_iter().enumerate() {
        registry.register(id, &SelectProject { index });
    }
    registry.register(ShortcutId::Copy, &muxy_ui::text_input::Copy);
    registry.register(ShortcutId::Paste, &muxy_ui::text_input::Paste);
    muxy_ui::text_input::register_shortcuts(&mut registry);
    muxy_ui::picker::register_shortcuts(&mut registry);
    muxy_ui::components::register_shortcuts(&mut registry);
    muxy_ui::form::register_shortcuts(&mut registry);
    menu::register_shortcuts(&mut registry);
    super::terminal::pane::register_shortcuts(&mut registry);
    super::settings::register_shortcuts(&mut registry);
    registry.into_bindings()
}

impl AppModel {
    fn detach_active_terminal(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.is_none()
            && let Some(pane) = self.active_pane()
        {
            self.detach_terminal(pane, cx);
        }
    }

    fn existing_terminals(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dialogs.is_open() {
            self.open_session_picker(self.state.current_project().id, window, cx);
        }
    }

    fn find_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.is_some() {
            return;
        }
        if let Some(view) = self.active_browser() {
            view.update(cx, |view, cx| view.open_find(window, cx));
            return;
        }
        if let Some(pane) = self.active_pane().and_then(|id| self.terminal(&id)) {
            pane.view.update(cx, |pane, cx| {
                pane.open_find(&self.theme, self.metrics, window, cx);
            });
        }
    }

    fn step_find(&mut self, previous: bool, cx: &mut Context<Self>) {
        if self.overlay.is_some() {
            return;
        }
        if let Some(view) = self.active_browser() {
            view.update(cx, |view, cx| view.step_find(previous, cx));
            return;
        }
        if let Some(pane) = self.active_pane().and_then(|id| self.terminal(&id)) {
            pane.view
                .update(cx, |pane, cx| pane.step_find(previous, cx));
        }
    }
    fn prompt_action(&mut self, previous: Option<bool>, cx: &mut Context<Self>) {
        if self.overlay.is_some() || self.dialogs.is_open() {
            return;
        }
        if let Some(pane) = self.active_pane().and_then(|id| self.terminal(&id)) {
            pane.view.update(cx, |pane, cx| match previous {
                Some(previous) => pane.jump_prompt(previous, cx),
                None => pane.select_command_output(None, cx),
            });
        }
    }
    fn zoom_terminal(&mut self, delta: f32, cx: &mut Context<Self>) {
        if let Some(pane) = self.active_pane().and_then(|pane| self.terminal(&pane)) {
            pane.view.update(cx, |pane, cx| {
                pane.terminal.zoom(delta);
                cx.notify();
            });
        }
    }

    pub(crate) fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.appearance.sidebar_expanded = !self.appearance.sidebar_expanded;
        self.save_appearance(cx);
        self.dismiss_overlay(cx);
        self.focus_active(window, cx);
        cx.notify();
    }

    pub(crate) fn focus_active(&self, window: &mut Window, cx: &App) {
        if let Some(view) = self.active_pane().and_then(|id| self.browser_view(id)) {
            let view = view.read(cx);
            if !view.focus.contains_focused(window, cx) {
                self.blur_webviews(cx);
                view.focus.focus(window);
                view.native.focus();
            }
            return;
        }
        self.blur_webviews(cx);
        if self
            .active_pane()
            .is_some_and(|pane| self.state.is_settings_pane(pane))
            && let Some(settings) = &self.settings_tab
        {
            settings.root.read(cx).focus(window, cx);
            return;
        }
        if let Some(surface) = self
            .active_pane()
            .and_then(|id| self.webviews.panes.get(&id))
        {
            surface.view.read(cx).focus.focus(window);
            surface.view.read(cx).native.focus();
            return;
        }
        if let Some(pane) = self.active_pane().and_then(|id| self.grids.get(&id)) {
            pane.focus(window, cx);
        } else {
            self.focus.focus(window);
        }
    }
}

pub(crate) fn action_handlers(cx: &mut Context<AppModel>) -> gpui::Div {
    commands::action_handlers(cx)
        .key_context("WorkspaceTabs")
        .on_modifiers_changed(
            cx.listener(|model, event: &gpui::ModifiersChangedEvent, _, cx| {
                model.update_shortcut_hints(event.modifiers, cx);
            }),
        )
        .on_action(cx.listener(
            |model, action: &crate::model::extensions::RunCommand, window, cx| {
                model.run_extension_command(&action.owner, &action.command, window, cx);
            },
        ))
        .on_action(cx.listener(
            |model, action: &crate::model::commands::RunCustomCommand, _, cx| {
                model.run_custom_command(&action.id, cx);
            },
        ))
        .on_mouse_down(
            gpui::MouseButton::Navigate(gpui::NavigationDirection::Back),
            cx.listener(|model, _, _, cx| model.navigate(false, cx)),
        )
        .on_mouse_down(
            gpui::MouseButton::Navigate(gpui::NavigationDirection::Forward),
            cx.listener(|model, _, _, cx| model.navigate(true, cx)),
        )
        .on_action(cx.listener(|model, action: &SelectProject, _, cx| {
            if let Some(project) = model.navigation_projects().get(action.index) {
                model.select_project(*project, cx);
            }
        }))
        .on_action(cx.listener(|model, action: &SelectTab, _, cx| {
            if let Some(tab) = model.navigation_tabs().get(action.index) {
                model.select_tab(*tab, cx);
            }
        }))
        .on_action(cx.listener(titlebar::begin_window_move))
        .on_action(
            cx.listener(|model, _: &ReloadConfiguration, _, cx| model.reload_configuration(cx)),
        )
}

impl AppModel {
    pub(crate) fn sync_pane_focus(&mut self, cx: &mut Context<Self>) {
        self.acknowledge_focused_activity(cx);
        let active = self.active_pane();
        if let Some(active) = active {
            self.set_pane_completed(active, false);
        }
        let zoomed: Vec<_> = self
            .visible_tabs()
            .into_iter()
            .filter_map(|tab| self.tab(tab)?.zoomed)
            .collect();
        let coverage = self.coverage();
        if coverage.modal() {
            self.split_resize.end();
        }
        let focused: Vec<_> = self
            .state
            .tab_windows()
            .filter_map(|(_, _, window)| window.active_pane())
            .chain(self.main_active_pane())
            .collect();
        for (id, pane) in &self.grids {
            if self.is_quick_terminal(*id) {
                continue;
            }
            pane.view.update(cx, |pane, cx| {
                pane.set_focused(focused.contains(id), cx);
                let radius = if zoomed.contains(id) {
                    (self.metrics.radius_lg() - px(1.0)).max(px(0.0))
                } else {
                    px(0.0)
                };
                let below_header = self.appearance.pane_headers;
                if pane.corner_radius != radius || pane.below_header != below_header {
                    pane.corner_radius = radius;
                    pane.below_header = below_header;
                    cx.notify();
                }
                pane.set_native_visible(!coverage.hides_terminal_scroll(), cx);
            });
        }
    }
}

impl AppModel {
    fn prepare_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _span = crate::profiler::span(crate::profiler::Metric::WorkspacePrepare);
        let overlay = self.overlay.is_some();
        let elsewhere = self.tab_windows.key_window().is_some() && !overlay;
        if elsewhere && self.focus_owner.wants_active_pane() {
            self.focus_owner.settle();
            if let Some(id) = self.tab_windows.key_window() {
                self.tab_windows.request_focus(id);
                cx.notify();
            }
        }
        match self.focus_owner.take(overlay) {
            Some(Next::Element(focus)) => focus.focus(window),
            Some(Next::ActivePane) => self.focus_active(window, cx),
            // With no pane, the workspace keeps the keyboard for its shortcuts.
            None if !overlay && self.active_pane().is_none() && window.focused(cx).is_none() => {
                self.focus_active(window, cx);
            }
            None => {}
        }
        self.hold_onboarding_focus(window, cx);
        let modal = self.coverage().modal();
        if modal && self.window_can_move() {
            self.cancel_tab_and_layout_drags(cx);
        } else if modal {
            self.cancel_titlebar_drag(cx);
        }
        if !self.appearance.sidebar_expanded || modal {
            self.finish_sidebar_resize(cx);
        }
        let pending = self.tab_drag.pending();
        self.tab_drag
            .cancel_unavailable(self.state.current_project(), false);
        if pending && !self.tab_drag.pending() {
            cx.notify();
        }
        self.validate_layout_drag(cx);
        self.sync_tab_sidebar(cx);
        let here = window.window_handle().window_id();
        self.layout_drag.geometry.clear(here);
        self.move_drag.zones.note_window(window);
        self.move_drag
            .zones
            .clear(here, super::move_drag::Owner::Strip);
        self.tab_drag.clear_bounds();
    }
}

impl Render for AppModel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drawing(Surface::Main, |model| model.render_workspace(window, cx))
    }
}

impl AppModel {
    #[allow(clippy::too_many_lines, reason = "One workspace layout")]
    fn render_workspace(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let _span = crate::profiler::span(crate::profiler::Metric::WorkspaceRender);
        self.metrics.apply_to_window(window);
        self.sync_traffic_lights(window);
        self.sync_floating_stack();
        {
            let _span = crate::profiler::span(crate::profiler::Metric::WorkspaceSyncWebviews);
            self.sync_webviews(window, cx);
        }
        self.ensure_settings_tab(window, cx);
        self.prepare_workspace(window, cx);
        self.sync_project_logos(window, cx);
        #[cfg(all(target_os = "macos", not(test)))]
        self.sync_background_effects(window, cx);
        self.sync_toast(cx);
        self.sync_window_accessibility(window, cx);
        let (actions, actions_width) = titlebar::actions(self, window, cx);
        #[cfg(target_os = "linux")]
        self.fit_sidebar_to_window(window, actions_width, cx);
        let theme = &self.theme;
        let tab_focused = self.appearance.layout == muxy_app_core::settings::AppLayout::TabFocused
            && !self.extension_sidebar_active();
        let sidebar_width = self.sidebar_width();
        let header = if tab_focused || self.state.current_project().groups().is_some() {
            super::tab_sidebar::titlebar(self, sidebar_width, window, actions, cx)
        } else {
            tab_strip::tab_strip(self, sidebar_width, window, (actions, actions_width), cx)
        };
        let content = super::splits::render(self, window, cx).unwrap_or_else(|| empty(self, cx));
        let content = self.webview_panel_content(content, window, cx);
        let content = super::agents_panel::right_panel_content(content, self, window, cx);
        let content = super::agent_sessions::panel_content(content, self, window, cx);
        let content = self.composer_content(content, window, cx);
        let workspace = action_handlers(cx)
            .capture_any_mouse_down(cx.listener(|model, _, _, _| {
                model.enter_surface(Surface::Main);
            }))
            .capture_key_down(cx.listener(|model, event, window, cx| {
                model.enter_surface(Surface::Main);
                model.cancel_drag_on_escape(event, window, cx);
            }))
            .track_focus(&self.focus)
            .relative()
            .flex()
            .size_full()
            .text_color(theme.fg)
            .font_family(".SystemUIFont")
            .line_height(relative(1.2))
            .when(sidebar_width > 0.0, |body| {
                body.child(self.cached_sidebar())
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .bg(theme.bg)
                            .map(|header| {
                                titlebar::top_corners(header, window, sidebar_width <= 0.0, true)
                            })
                            .child(header)
                            .child(div().h(px(1.0)).flex_none().bg(theme.border)),
                    )
                    .child(
                        div()
                            .debug_selector(|| "workspace-content".into())
                            .flex_1()
                            .min_h(px(0.0))
                            .overflow_hidden()
                            .child(content),
                    )
                    .when(self.appearance.status_bar_visible, |column| {
                        column.child(status_bar::status_bar(self, cx))
                    }),
            )
            .child(sidebar_divider(self, sidebar_width, window))
            .child(titlebar::navigation(self, sidebar_width, window, cx))
            .when(self.appearance.sidebar_expanded, |body| {
                body.child(sidebar::resize_handle(self, cx))
            })
            .when(self.sidebar_resize.is_some(), |body| {
                body.child(div().absolute().inset_0().cursor_ew_resize().occlude())
            })
            .child(super::splits::drag::track_pointer(cx))
            .child(super::splits::drag::overlay(self, window))
            .child(super::move_drag::overlay(self, window))
            .child(self.floating_composer(window, cx))
            .children(super::tips::toast(self, window, cx))
            .children(super::banners::render(self, window, cx))
            .child(super::floating::layer(self, window, cx))
            .when(
                self.settings_picker_open() && self.overlay.is_none(),
                |body| {
                    body.children(
                        self.settings_tab
                            .as_ref()
                            .map(|settings| titlebar::overlay(settings.overlay.clone(), window)),
                    )
                },
            )
            .when(self.tab_windows.overlay == Surface::Main, |body| {
                body.child(titlebar::overlay(overlays::layer(self, window, cx), window))
            })
            .child(self.apply_webview_regions(cx));
        #[cfg(target_os = "linux")]
        let workspace = titlebar::frame::render(
            workspace,
            theme,
            (sidebar_width > 0.0).then_some((px(sidebar_width), self.sidebar_background())),
            matches!(self.overlay, Some(overlays::Overlay::Onboarding))
                .then_some(super::onboarding::BACKDROP_ALPHA),
            window,
        );
        crate::profiler::workspace(workspace).into_any_element()
    }
}

/// The line between the sidebar and the content. It starts below the
/// navigation buttons where they reach past the sidebar.
fn sidebar_divider(model: &AppModel, sidebar_width: f32, window: &Window) -> gpui::Div {
    let top = if sidebar_width >= titlebar::navigation_width(model, window) {
        0.0
    } else {
        f32::from(model.metrics.title_bar_height()) + 1.0
    };
    div()
        .absolute()
        .top(px(top))
        .bottom_0()
        .left(px(sidebar_width - 1.0))
        .w(px(1.0))
        .bg(model.theme.border)
}

fn empty(model: &AppModel, cx: &mut Context<AppModel>) -> gpui::AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    let missing = model.state.current_project().status() == muxy_app_core::ProjectStatus::Missing;
    div()
        .bg(theme.bg)
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(m.spacing7())
        .child(IconGlyph::new(
            Icon::AppWindow,
            m.icon_xxl(),
            theme.fg_muted,
        ))
        .child(
            div()
                .text_size(m.font_headline())
                .font_weight(FontWeight::SEMIBOLD)
                .child(accessibility::label("empty-title", if missing { tr!("Project folder is missing") } else { tr!("No tabs in %@", &model.state.current_project().name) })),
        )
        .child(
            div()
                .max_w(m.scaled(360.0))
                .text_center()
                .text_size(m.font_body())
                .text_color(theme.fg_muted)
                .child(accessibility::label("empty-detail", if missing { tr!("This project’s folder could not be found. Remove the project from the sidebar to clear its tabs.") } else { tr!("Open a new terminal tab to start working in this project.") })),
        )
        .when(!missing, |view| view.child(
            div()
                .id("empty-new-tab")
                .role(Role::Button)
                .accessibility_id("empty-new-tab")
                .aria_label(tr!("New Tab"))
                .flex()
                .items_center()
                .gap(m.spacing4())
                .px(m.spacing6())
                .py(m.spacing3())
                .rounded(m.radius_md())
                .bg(gpui::rgb(0x0a_7c_ff))
                .text_color(gpui::white())
                .text_size(m.font_body())
                .cursor_pointer()
                .hover(|style| style.opacity(0.85))
                .on_click(cx.listener(|model, _, _, cx| model.new_tab(cx)))
                .child(tr!("New Tab"))
                .when_some(model.settings.keymap.chord(ShortcutId::NewTab), |element, chord| element.child(
                    div()
                        .text_size(m.font_footnote())
                        .font_weight(FontWeight::MEDIUM)
                        .opacity(0.72)
                        .child(chord.to_string()),
                )),
        ))
        .into_any_element()
}
