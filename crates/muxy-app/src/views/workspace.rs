use muxy_core::shortcuts::ShortcutId;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Context, Focusable, FontWeight, InteractiveElement, IntoElement, KeyBinding,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, actions, div, px, relative,
};
use muxy_app_core::Direction;
use muxy_app_core::settings::Keymap;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use super::{menu, overlays, sidebar, status_bar, tab_strip, titlebar};
use crate::model::AppModel;
use crate::model::browsers::BrowserAction;

actions!(
    muxy,
    [
        OpenSettings,
        CheckForUpdates,
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
        ToggleVoiceRecording,
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
    registry.register(ShortcutId::OpenSettings, &OpenSettings);
    registry.register(ShortcutId::NewHomeTab, &NewHomeTab);
    registry.register(ShortcutId::NewTabInProject, &NewTabInProject);
    registry.register(ShortcutId::NewWorktree, &NewWorktree);
    registry.register(ShortcutId::SwitchWorktree, &SwitchWorktree);
    registry.register(ShortcutId::ToggleSidebar, &ToggleSidebar);
    registry.register(ShortcutId::ToggleAgentsPanel, &ToggleAgentsPanel);
    registry.register(ShortcutId::ToggleAgentSessions, &ToggleAgentSessions);
    registry.register(ShortcutId::ToggleFullScreen, &ToggleFullScreen);
    registry.register(ShortcutId::ToggleThemePicker, &ToggleThemePicker);
    registry.register(ShortcutId::ToggleCommandPalette, &ToggleCommandPalette);
    registry.register(ShortcutId::ToggleComposer, &ToggleComposer);
    registry.register(ShortcutId::ToggleVoiceRecording, &ToggleVoiceRecording);
    registry.register(ShortcutId::VoiceFinish, &super::voice::Finish);
    registry.register(ShortcutId::VoiceCancel, &super::voice::Cancel);
    registry.register(ShortcutId::VoicePause, &super::voice::Pause);
    registry.register(ShortcutId::ComposerSubmit, &super::composer::Submit);
    registry.register(ShortcutId::ComposerInsert, &super::composer::Insert);
    registry.register(ShortcutId::ComposerClose, &super::composer::Close);
    registry.register(ShortcutId::ComposerVoice, &super::composer::ToggleVoice);
    registry.register(ShortcutId::NavigateBack, &NavigateBack);
    registry.register(ShortcutId::NavigateForward, &NavigateForward);
    registry.register(ShortcutId::Quit, &Quit);
    registry.register(ShortcutId::HideApp, &HideApp);
    registry.register(ShortcutId::HideOthers, &HideOthers);
    registry.register(ShortcutId::Minimize, &Minimize);
    registry.register(ShortcutId::NewTab, &NewTab);
    registry.register(ShortcutId::ExistingTerminals, &ExistingTerminals);
    registry.register(ShortcutId::DetachTerminal, &DetachTerminal);
    registry.register(ShortcutId::CloseTab, &CloseTab);
    registry.register(ShortcutId::SplitRight, &SplitRight);
    registry.register(ShortcutId::SplitDown, &SplitDown);
    registry.register(ShortcutId::FocusPaneLeft, &FocusPaneLeft);
    registry.register(ShortcutId::FocusPaneRight, &FocusPaneRight);
    registry.register(ShortcutId::FocusPaneUp, &FocusPaneUp);
    registry.register(ShortcutId::FocusPaneDown, &FocusPaneDown);
    registry.register(ShortcutId::ToggleZoomPane, &ToggleZoomPane);
    registry.register(ShortcutId::EqualizeSplits, &EqualizeSplits);
    registry.register(ShortcutId::ClosePane, &ClosePane);
    registry.register(ShortcutId::NextTab, &NextTab);
    registry.register(ShortcutId::PreviousTab, &PreviousTab);
    registry.register(ShortcutId::PreviousProject, &PreviousProject);
    registry.register(ShortcutId::NextProject, &NextProject);
    registry.register(ShortcutId::AddProject, &AddProject);
    for (index, id) in ShortcutId::TABS.into_iter().enumerate() {
        registry.register(id, &SelectTab { index });
    }
    for (index, id) in ShortcutId::PROJECTS.into_iter().enumerate() {
        registry.register(id, &SelectProject { index });
    }
    registry.register(ShortcutId::Copy, &muxy_ui::text_input::Copy);
    registry.register(ShortcutId::Paste, &muxy_ui::text_input::Paste);
    registry.register(ShortcutId::Find, &Find);
    registry.register(ShortcutId::FindNext, &FindNext);
    registry.register(ShortcutId::FindPrevious, &FindPrevious);
    registry.register(ShortcutId::PreviousPrompt, &PreviousPrompt);
    registry.register(ShortcutId::NextPrompt, &NextPrompt);
    registry.register(ShortcutId::SelectCommandOutput, &SelectCommandOutput);
    registry.register(ShortcutId::ScrollToBottom, &ScrollToBottom);
    registry.register(ShortcutId::IncreaseFontSize, &IncreaseFontSize);
    registry.register(ShortcutId::DecreaseFontSize, &DecreaseFontSize);
    registry.register(ShortcutId::NewBrowserTab, &NewBrowserTab);
    registry.register(ShortcutId::BrowserFocusAddress, &BrowserFocusAddress);
    registry.register(ShortcutId::BrowserReload, &BrowserReload);
    registry.register(ShortcutId::BrowserBack, &BrowserBack);
    registry.register(ShortcutId::BrowserForward, &BrowserForward);
    registry.register(ShortcutId::BrowserZoomIn, &BrowserZoomIn);
    registry.register(ShortcutId::BrowserZoomOut, &BrowserZoomOut);
    registry.register(ShortcutId::BrowserZoomReset, &BrowserZoomReset);
    registry.register(ShortcutId::BrowserCopyUrl, &BrowserCopyUrl);
    registry.register(ShortcutId::BrowserInspect, &BrowserInspect);
    muxy_ui::text_input::register_shortcuts(&mut registry);
    muxy_ui::picker::register_shortcuts(&mut registry);
    muxy_ui::components::register_shortcuts(&mut registry);
    menu::register_shortcuts(&mut registry);
    super::terminal::pane::register_shortcuts(&mut registry);
    super::settings::register_shortcuts(&mut registry);
    registry.register(ShortcutId::EndAllSessionsAndQuit, &EndAllSessionsAndQuit);
    registry.register(ShortcutId::ShowAll, &ShowAll);
    registry.register(ShortcutId::Zoom, &Zoom);
    registry.register(ShortcutId::OpenConfiguration, &OpenConfiguration);
    registry.register(ShortcutId::InstallCommandLineTool, &InstallCommandLineTool);
    registry.into_bindings()
}

impl AppModel {
    fn detach_active_terminal(
        &mut self,
        _: &DetachTerminal,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.overlay.is_none()
            && let Some(pane) = self.active_pane()
        {
            self.detach_terminal(pane, cx);
        }
    }

    fn existing_terminals(
        &mut self,
        _: &ExistingTerminals,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_none() {
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
        if self.overlay.is_some() || self.close_prompt.is_some() {
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
            .is_some_and(|pane| self.state.is_servers_pane(pane))
        {
            if !self.servers_tab.focus.contains_focused(window, cx) {
                self.servers_tab.focus.focus(window);
            }
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

#[allow(
    clippy::too_many_lines,
    reason = "One workspace action registration chain"
)]
fn action_handlers(cx: &mut Context<AppModel>) -> gpui::Div {
    div()
        .on_action(
            cx.listener(|model, _: &OpenSettings, window, cx| model.open_settings(window, cx)),
        )
        .on_action(cx.listener(|model, _: &Find, window, cx| model.find_terminal(window, cx)))
        .on_action(cx.listener(|model, _: &FindNext, _, cx| model.step_find(false, cx)))
        .on_action(cx.listener(|model, _: &FindPrevious, _, cx| model.step_find(true, cx)))
        .on_action(
            cx.listener(|model, _: &PreviousPrompt, _, cx| model.prompt_action(Some(true), cx)),
        )
        .on_action(cx.listener(|model, _: &NextPrompt, _, cx| model.prompt_action(Some(false), cx)))
        .on_action(
            cx.listener(|model, _: &SelectCommandOutput, _, cx| model.prompt_action(None, cx)),
        )
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
        .on_action(cx.listener(|model, _: &NewTab, _, cx| model.new_tab(cx)))
        .on_action(cx.listener(|model, _: &NewBrowserTab, _, cx| model.new_browser_tab(cx)))
        .on_action(cx.listener(|model, _: &BrowserFocusAddress, window, cx| {
            model.browser_action(BrowserAction::FocusAddress, window, cx);
        }))
        .on_action(cx.listener(|model, _: &BrowserReload, window, cx| {
            model.browser_action(BrowserAction::Reload, window, cx);
        }))
        .on_action(cx.listener(|model, _: &BrowserBack, window, cx| {
            model.browser_action(BrowserAction::Back, window, cx);
        }))
        .on_action(cx.listener(|model, _: &BrowserForward, window, cx| {
            model.browser_action(BrowserAction::Forward, window, cx);
        }))
        .on_action(cx.listener(|model, _: &BrowserZoomIn, window, cx| {
            model.browser_action(BrowserAction::ZoomIn, window, cx);
        }))
        .on_action(cx.listener(|model, _: &BrowserZoomOut, window, cx| {
            model.browser_action(BrowserAction::ZoomOut, window, cx);
        }))
        .on_action(cx.listener(|model, _: &BrowserZoomReset, window, cx| {
            model.browser_action(BrowserAction::ZoomReset, window, cx);
        }))
        .on_action(cx.listener(|model, _: &BrowserCopyUrl, window, cx| {
            model.browser_action(BrowserAction::CopyUrl, window, cx);
        }))
        .on_action(cx.listener(|model, _: &BrowserInspect, window, cx| {
            model.browser_action(BrowserAction::Inspect, window, cx);
        }))
        .on_action(cx.listener(AppModel::detach_active_terminal))
        .on_action(cx.listener(AppModel::existing_terminals))
        .on_action(
            cx.listener(|model, _: &SplitRight, _, cx| model.split_pane(Direction::Right, cx)),
        )
        .on_action(cx.listener(|model, _: &SplitDown, _, cx| model.split_pane(Direction::Down, cx)))
        .on_action(cx.listener(|model, _: &ToggleZoomPane, _, cx| model.toggle_zoom_pane(cx)))
        .on_action(cx.listener(|model, _: &EqualizeSplits, _, cx| {
            if let Some(pane) = model.active_pane() {
                model.equalize_splits(pane, cx);
            }
        }))
        .on_action(cx.listener(|model, _: &ClosePane, _, cx| {
            if let Some(pane) = model.active_pane() {
                model.close_pane(pane, cx);
            }
        }))
        .on_action(cx.listener(|model, _: &FocusPaneLeft, _, cx| {
            model.focus_direction(Direction::Left, cx);
        }))
        .on_action(cx.listener(|model, _: &FocusPaneRight, _, cx| {
            model.focus_direction(Direction::Right, cx);
        }))
        .on_action(
            cx.listener(|model, _: &FocusPaneUp, _, cx| model.focus_direction(Direction::Up, cx)),
        )
        .on_action(cx.listener(|model, _: &FocusPaneDown, _, cx| {
            model.focus_direction(Direction::Down, cx);
        }))
        .on_action(cx.listener(|model, _: &InstallCommandLineTool, _, cx| {
            model.install_command_line(cx);
        }))
        .on_action(cx.listener(|model, _: &CheckForUpdates, _, cx| {
            model.check_for_updates(true, cx);
        }))
        .on_action(cx.listener(|model, _: &Quit, _, cx| {
            model.quit(cx);
            cx.stop_propagation();
        }))
        .on_action(cx.listener(|model, _: &EndAllSessionsAndQuit, _, cx| {
            model.end_all_and_quit(cx);
            cx.stop_propagation();
        }))
        .on_action(cx.listener(|model, _: &NewHomeTab, _, cx| {
            model.select_project(model.state.home().id, cx);
            model.new_tab(cx);
        }))
        .on_action(cx.listener(|model, _: &NewTabInProject, window, cx| {
            model.toggle_new_tab_list(window, cx);
        }))
        .on_action(cx.listener(|model, _: &NewWorktree, window, cx| {
            if model.close_prompt.is_none()
                && let Some(project) = model.new_worktree_target()
            {
                model.open_git_form(project, true, cx);
                model.focus_git_form(window, cx);
            }
        }))
        .on_action(cx.listener(|model, _: &SwitchWorktree, window, cx| {
            model.toggle_worktree_switcher(window, cx);
        }))
        .on_action(cx.listener(|model, _: &PreviousProject, _, cx| model.cycle_project(false, cx)))
        .on_action(cx.listener(|model, _: &NextProject, _, cx| model.cycle_project(true, cx)))
        .on_action(cx.listener(|model, action: &SelectProject, _, cx| {
            if let Some(project) = model.navigation_projects().get(action.index) {
                model.select_project(*project, cx);
            }
        }))
        .on_action(
            cx.listener(|model, _: &AddProject, window, cx| model.open_project_picker(window, cx)),
        )
        .on_action(cx.listener(|model, _: &CloseTab, _, cx| {
            if let Some(tab) = model.active_tab() {
                model.close_tab(tab, cx);
            }
        }))
        .on_action(cx.listener(|model, _: &NextTab, _, cx| model.cycle_tab(true, cx)))
        .on_action(cx.listener(|model, _: &PreviousTab, _, cx| model.cycle_tab(false, cx)))
        .on_action(cx.listener(|model, action: &SelectTab, _, cx| {
            if let Some(tab) = model.navigation_tabs().get(action.index) {
                model.select_tab(*tab, cx);
            }
        }))
        .on_action(
            cx.listener(|model, _: &ToggleSidebar, window, cx| model.toggle_sidebar(window, cx)),
        )
        .on_action(cx.listener(|model, _: &ToggleAgentsPanel, window, cx| {
            model.toggle_agents_panel(window, cx);
        }))
        .on_action(cx.listener(|model, _: &ToggleAgentSessions, window, cx| {
            model.toggle_agent_sessions(window, cx);
        }))
        .on_action(cx.listener(|_, _: &ToggleFullScreen, window, _| window.toggle_fullscreen()))
        .on_action(cx.listener(|model, _: &ToggleThemePicker, window, cx| {
            model.open_theme_picker(window, cx);
        }))
        .on_action(cx.listener(|model, _: &NavigateBack, _, cx| model.navigate(false, cx)))
        .on_action(cx.listener(|model, _: &NavigateForward, _, cx| model.navigate(true, cx)))
        .on_action(cx.listener(|_, _: &Minimize, window, _| window.minimize_window()))
        .on_action(cx.listener(|model, _: &HideApp, _, cx| {
            model.forward(muxy_core::capability::Feature::AppHiding, cx);
        }))
        .on_action(cx.listener(|model, _: &HideOthers, _, cx| {
            model.forward(muxy_core::capability::Feature::AppHiding, cx);
        }))
        .on_action(cx.listener(|model, _: &ShowAll, _, cx| {
            model.forward(muxy_core::capability::Feature::AppHiding, cx);
        }))
        .on_action(cx.listener(|_, _: &Zoom, window, _| window.zoom_window()))
        .on_action(cx.listener(titlebar::begin_window_move))
        .on_action(
            cx.listener(|model, _: &ReloadConfiguration, _, cx| model.reload_configuration(cx)),
        )
        .on_action(
            cx.listener(|model, _: &OpenConfiguration, window, cx| model.open_settings(window, cx)),
        )
        .on_action(cx.listener(|model, _: &IncreaseFontSize, _, cx| model.zoom_terminal(1.0, cx)))
        .on_action(cx.listener(|model, _: &DecreaseFontSize, _, cx| model.zoom_terminal(-1.0, cx)))
}

fn register_composer_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
) {
    use super::command_palette::action;

    registry.register(action(
        model,
        ShortcutId::ToggleComposer,
        tr_key!("Toggle Composer"),
        ToggleComposer,
    ));
    if model
        .capabilities
        .has_backend(muxy_core::capability::Feature::Voice)
    {
        registry.register(action(
            model,
            ShortcutId::ToggleVoiceRecording,
            tr_key!("Toggle Voice Recording"),
            ToggleVoiceRecording,
        ));
    }
}

/// Commands that act on the focused terminal.
fn register_terminal_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
) {
    use super::command_palette::action;

    let no_terminal = model
        .active_pane()
        .and_then(|pane| model.terminal(&pane))
        .is_none();
    for command in [
        action(model, ShortcutId::Find, tr_key!("Find…"), Find),
        action(model, ShortcutId::FindNext, tr_key!("Find Next"), FindNext),
        action(
            model,
            ShortcutId::FindPrevious,
            tr_key!("Find Previous"),
            FindPrevious,
        ),
        action(
            model,
            ShortcutId::PreviousPrompt,
            tr_key!("Previous Prompt"),
            PreviousPrompt,
        ),
        action(
            model,
            ShortcutId::NextPrompt,
            tr_key!("Next Prompt"),
            NextPrompt,
        ),
        action(
            model,
            ShortcutId::SelectCommandOutput,
            tr_key!("Select Command Output"),
            SelectCommandOutput,
        ),
        action(
            model,
            ShortcutId::ScrollToBottom,
            tr_key!("Scroll to Bottom"),
            ScrollToBottom,
        ),
        action(
            model,
            ShortcutId::IncreaseFontSize,
            tr_key!("Increase Font Size"),
            IncreaseFontSize,
        ),
        action(
            model,
            ShortcutId::DecreaseFontSize,
            tr_key!("Decrease Font Size"),
            DecreaseFontSize,
        ),
        action(
            model,
            ShortcutId::DetachTerminal,
            tr_key!("Detach Terminal"),
            DetachTerminal,
        ),
    ] {
        registry.register(command.disabled(no_terminal));
    }
}

fn register_pane_focus_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
    unsplit: bool,
) {
    use super::command_palette::action;

    for command in [
        action(
            model,
            ShortcutId::FocusPaneLeft,
            tr_key!("Focus Pane Left"),
            FocusPaneLeft,
        ),
        action(
            model,
            ShortcutId::FocusPaneRight,
            tr_key!("Focus Pane Right"),
            FocusPaneRight,
        ),
        action(
            model,
            ShortcutId::FocusPaneUp,
            tr_key!("Focus Pane Up"),
            FocusPaneUp,
        ),
        action(
            model,
            ShortcutId::FocusPaneDown,
            tr_key!("Focus Pane Down"),
            FocusPaneDown,
        ),
    ] {
        registry.register(command.disabled(unsplit));
    }
}

/// Commands for the window and the app as a whole.
fn register_app_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
) {
    use super::command_palette::{action, dispatch, titled};
    use muxy_core::capability::Feature;
    use muxy_ui::command_palette::Command;

    for command in [
        action(
            model,
            ShortcutId::ExistingTerminals,
            tr_key!("Existing Terminals…"),
            ExistingTerminals,
        ),
        action(
            model,
            ShortcutId::ToggleFullScreen,
            tr_key!("Toggle Full Screen"),
            ToggleFullScreen,
        ),
        action(
            model,
            ShortcutId::Minimize,
            tr_key!("Minimize Window"),
            Minimize,
        ),
        action(model, ShortcutId::Zoom, tr_key!("Zoom Window"), Zoom)
            .keywords("Zoom Window Maximize Restore"),
        titled(
            model,
            ShortcutId::Quit,
            tr!("Quit %@", crate::app_name()),
            Quit,
        )
        .keywords("Quit"),
        action(
            model,
            ShortcutId::EndAllSessionsAndQuit,
            tr_key!("End All Sessions and Quit"),
            EndAllSessionsAndQuit,
        ),
    ] {
        registry.register(command);
    }
    if model.capabilities.has_backend(Feature::Updates) {
        registry.register(
            Command::new(
                "check_for_updates",
                tr!("Check for Updates…"),
                dispatch(CheckForUpdates),
            )
            .keywords("Check for Updates"),
        );
    }
    if model.capabilities.has_backend(Feature::CommandLineTool) {
        registry.register(action(
            model,
            ShortcutId::InstallCommandLineTool,
            tr_key!("Install Command Line Tool…"),
            InstallCommandLineTool,
        ));
    }
}

fn register_panel_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
) {
    use super::command_palette::action;

    for command in [
        action(
            model,
            ShortcutId::ToggleSidebar,
            tr_key!("Toggle Sidebar"),
            ToggleSidebar,
        ),
        action(
            model,
            ShortcutId::ToggleAgentsPanel,
            tr_key!("Toggle Agents Panel"),
            ToggleAgentsPanel,
        ),
        action(
            model,
            ShortcutId::ToggleAgentSessions,
            tr_key!("Toggle Agent Sessions"),
            ToggleAgentSessions,
        )
        .keywords("harness history resume providers"),
    ] {
        registry.register(command);
    }
}

fn register_worktree_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
) {
    use super::command_palette::action;

    for command in [
        action(
            model,
            ShortcutId::NewWorktree,
            tr_key!("New Worktree…"),
            NewWorktree,
        )
        .disabled(model.new_worktree_target().is_none()),
        action(
            model,
            ShortcutId::SwitchWorktree,
            tr_key!("Switch Worktree…"),
            SwitchWorktree,
        )
        .disabled(model.worktree_switcher_project().is_none()),
    ] {
        registry.register(command);
    }
}

pub(crate) fn register_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
) {
    use super::command_palette::action;

    let missing = model.state.current_project().status() == muxy_app_core::ProjectStatus::Missing;
    let no_pane = missing || model.active_pane().is_none();
    let unsplit = missing
        || model
            .active_tab()
            .and_then(|tab| model.tab(tab))
            .is_none_or(|tab| tab.panes.len() < 2);
    register_composer_commands(registry, model);
    for command in [
        action(model, ShortcutId::NewTab, tr_key!("New Tab"), NewTab).disabled(missing),
        action(
            model,
            ShortcutId::NewHomeTab,
            tr_key!("New Home Tab"),
            NewHomeTab,
        ),
        action(model, ShortcutId::CloseTab, tr_key!("Close Tab"), CloseTab)
            .disabled(model.active_tab().is_none()),
        action(
            model,
            ShortcutId::SplitRight,
            tr_key!("Split Right"),
            SplitRight,
        )
        .disabled(no_pane),
        action(
            model,
            ShortcutId::SplitDown,
            tr_key!("Split Down"),
            SplitDown,
        )
        .disabled(no_pane),
        action(
            model,
            ShortcutId::ClosePane,
            tr_key!("Close Pane"),
            ClosePane,
        )
        .disabled(no_pane),
        action(
            model,
            ShortcutId::ToggleZoomPane,
            tr_key!("Toggle Pane Zoom"),
            ToggleZoomPane,
        )
        .disabled(no_pane),
        action(
            model,
            ShortcutId::EqualizeSplits,
            tr_key!("Equalize Splits"),
            EqualizeSplits,
        )
        .disabled(unsplit),
        action(
            model,
            ShortcutId::NextProject,
            tr_key!("Next Project"),
            NextProject,
        )
        .disabled(model.navigation_projects().len() < 2),
        action(
            model,
            ShortcutId::PreviousProject,
            tr_key!("Previous Project"),
            PreviousProject,
        )
        .disabled(model.navigation_projects().len() < 2),
        action(model, ShortcutId::NextTab, tr_key!("Next Tab"), NextTab)
            .disabled(model.navigation_tabs().len() < 2),
        action(
            model,
            ShortcutId::PreviousTab,
            tr_key!("Previous Tab"),
            PreviousTab,
        )
        .disabled(model.navigation_tabs().len() < 2),
    ] {
        registry.register(command);
    }
    register_panel_commands(registry, model);
    register_worktree_commands(registry, model);
    register_terminal_commands(registry, model);
    register_pane_focus_commands(registry, model, unsplit);
    register_app_commands(registry, model);
}

pub(crate) fn register_browser_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
) {
    let missing = model.state.current_project().status() == muxy_app_core::ProjectStatus::Missing;
    registry.register(
        super::command_palette::action(
            model,
            ShortcutId::NewBrowserTab,
            tr_key!("New Browser Tab"),
            NewBrowserTab,
        )
        .disabled(missing || !model.browser_enabled()),
    );
}

impl AppModel {
    pub(crate) fn sync_pane_focus(&mut self, cx: &mut Context<Self>) {
        self.acknowledge_focused_activity(cx);
        let active = self.active_pane();
        if let Some(active) = active {
            self.completions.remove(&active);
        }
        let zoomed: Vec<_> = self
            .visible_tabs()
            .into_iter()
            .filter_map(|tab| self.tab(tab)?.zoomed)
            .collect();
        let split = self.visible_panes().len() > 1;
        if self.overlay.is_some() || self.close_prompt.is_some() {
            self.split_resize.end();
        }
        for (id, pane) in &self.grids {
            if self.is_quick_terminal(*id) {
                continue;
            }
            pane.view.update(cx, |pane, cx| {
                pane.set_focused(Some(*id) == active, cx);
                let border = (pane.focused && split).then_some(self.theme.accent);
                let radius = if zoomed.contains(id) {
                    (self.metrics.radius_lg() - px(1.0)).max(px(0.0))
                } else {
                    px(0.0)
                };
                if pane.focus_border != border || pane.corner_radius != radius {
                    pane.focus_border = border;
                    pane.corner_radius = radius;
                    cx.notify();
                }
                let native_visible = self.overlay.is_none()
                    && self.voice.view.is_none()
                    && self.voice.closing.is_none()
                    && self.close_prompt.is_none()
                    && self.composer.closing.is_none()
                    && (self.composer.view.is_none()
                        || (self.settings.composer.pinned
                            && self.settings.composer.presentation
                                == muxy_app_core::settings::ComposerPresentation::Panel));
                if pane.native_visible != native_visible {
                    pane.native_visible = native_visible;
                    cx.notify();
                }
                #[cfg(target_os = "macos")]
                if let Some(scroll) = &pane.native_scroll {
                    scroll.set_visible(pane.native_visible && pane.grid.is_some());
                }
            });
        }
    }
}

impl AppModel {
    fn prepare_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _span = crate::profiler::span(crate::profiler::Metric::WorkspacePrepare);
        if self.overlay.is_none()
            && (self.focus_requested
                || (self.active_pane().is_none()
                    && self.composer.view.is_none()
                    && self.webviews.panels.is_empty()
                    && !self.agents_panel.focus.contains_focused(window, cx)
                    && !(self.appearance.agent_sessions_visible
                        && self.agent_sessions.input.as_ref().is_some_and(|input| {
                            input.focus_handle(cx).contains_focused(window, cx)
                        }))))
        {
            match self.focus_return.take().and_then(|focus| focus.upgrade()) {
                Some(focus) => focus.focus(window),
                None => self.focus_active(window, cx),
            }
            self.focus_requested = false;
        }
        if self.overlay.is_some() || self.close_prompt.is_some() {
            self.cancel_titlebar_drag(cx);
        }
        if !self.appearance.sidebar_expanded
            || self.overlay.is_some()
            || self.close_prompt.is_some()
        {
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
        self.layout_drag.geometry.clear();
        self.tab_drag.clear_bounds();
    }
}

impl Render for AppModel {
    #[allow(clippy::too_many_lines, reason = "One workspace layout")]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _span = crate::profiler::span(crate::profiler::Metric::WorkspaceRender);
        {
            let _span = crate::profiler::span(crate::profiler::Metric::WorkspaceSyncWebviews);
            self.sync_webviews(window, cx);
        }
        self.prepare_workspace(window, cx);
        self.sync_project_logos(window, cx);
        #[cfg(all(target_os = "macos", not(test)))]
        self.sync_background_effects(window, cx);
        self.sync_toast(cx);
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
            .on_action(cx.listener(|model, _: &ToggleVoiceRecording, window, cx| {
                model.toggle_voice(window, cx);
            }))
            .on_action(cx.listener(|model, _: &ToggleComposer, window, cx| {
                model.toggle_composer(window, cx);
            }))
            .on_action(cx.listener(|model, _: &ToggleCommandPalette, window, cx| {
                model.toggle_command_palette(window, cx);
            }))
            .capture_key_down(cx.listener(AppModel::cancel_drag_on_escape))
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
            .child(super::splits::drag::overlay(self))
            .child(self.floating_composer(window, cx))
            .child(self.voice_panel())
            .children(super::banners::render(self, window, cx))
            .children(super::agents_panel::floating(self, window, cx))
            .child(titlebar::overlay(overlays::layer(self, window, cx), window))
            .child(self.apply_webview_regions(cx));
        #[cfg(target_os = "linux")]
        let workspace = titlebar::frame::render(
            workspace,
            theme,
            (sidebar_width > 0.0).then_some((px(sidebar_width), self.sidebar_background())),
            window,
        );
        crate::profiler::workspace(workspace)
    }
}

/// The line between the sidebar and the content. It starts below the
/// navigation buttons where they reach past the sidebar.
fn sidebar_divider(model: &AppModel, sidebar_width: f32, window: &Window) -> gpui::Div {
    let top = if sidebar_width >= titlebar::navigation_width(model, window) {
        0.0
    } else {
        33.0
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
                .child(if missing { tr!("Project folder is missing") } else { tr!("No tabs in %@", &model.state.current_project().name) }),
        )
        .child(
            div()
                .max_w(m.scaled(360.0))
                .text_center()
                .text_size(m.font_body())
                .text_color(theme.fg_muted)
                .child(if missing { tr!("This project’s folder could not be found. Remove the project from the sidebar to clear its tabs.") } else { tr!("Open a new terminal tab to start working in this project.") }),
        )
        .when(!missing, |view| view.child(
            div()
                .id("empty-new-tab")
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
