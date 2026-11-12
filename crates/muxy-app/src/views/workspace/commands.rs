use std::rc::Rc;

use gpui::{Action, App, Context, Div, InteractiveElement, MenuItem, SharedString, Window, div};
use muxy_app_core::settings::TerminalAction;
use muxy_app_core::{Direction, TabCloseScope, TabSide};
use muxy_core::capability::Feature;
use muxy_core::shortcuts::{ALL, ShortcutId};
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::l10n::tr_key;

use super::{
    AddProject, ApplyLayout, BrowserBack, BrowserCopyUrl, BrowserFocusAddress, BrowserForward,
    BrowserInspect, BrowserOpenExternally, BrowserReload, BrowserZoomIn, BrowserZoomOut,
    BrowserZoomReset, CheckForUpdates, ClearScreen, CloseOtherTabs, ClosePane, CloseTab,
    CloseTabsToTheLeft, CloseTabsToTheRight, CopyProjectPath, CreateExtension, DecreaseFontSize,
    DetachTerminal, DockFloatingTerminal, EndAllSessionsAndQuit, EqualizeSplits, ExistingTerminals,
    Find, FindNext, FindPrevious, FloatPane, FocusNextPane, FocusPaneDown, FocusPaneLeft,
    FocusPaneRight, FocusPaneUp, FocusPreviousPane, HideApp, HideOthers, IncreaseFontSize,
    InstallCommandLineTool, ManageExtensions, Minimize, MovePaneToNewTab, MovePaneToNewWindow,
    MoveTabLeft, MoveTabRight, MoveTabToMainWindow, MoveTabToNewWindow, NavigateBack,
    NavigateForward, NewBrowserTab, NewHomeTab, NewTab, NewTabInProject, NewTabToTheLeft,
    NewTabToTheRight, NewWorkspace, NewWorktree, NextProject, NextPrompt, NextTab, OpenAbout,
    OpenConfiguration, OpenKeyboardShortcuts, OpenProjectInDefaultApp, OpenSettings,
    PreviousProject, PreviousPrompt, PreviousTab, Quit, ReloadConfiguration, RemoveProject,
    RenameProject, RenameTab, ResetFontSize, ResetTabTitle, ResizePaneDown, ResizePaneLeft,
    ResizePaneRight, ResizePaneUp, RevealProject, ScrollToBottom, ScrollToTop, SelectAllInTerminal,
    SelectCommandOutput, ShowAll, ShowFloatingTerminalInAllProjects,
    ShowFloatingTerminalInItsProject, ShowServers, ShowTip, ShowWelcomeTour, SplitDown, SplitRight,
    SwitchProject, SwitchWorkspace, SwitchWorktree, ToggleAgentSessions, ToggleAgentsPanel,
    ToggleCommandPalette, ToggleComposer, ToggleFloatingTerminals, ToggleFullScreen,
    ToggleProjectFocus, ToggleSidebar, ToggleStatusBar, ToggleTabPin, ToggleThemePicker,
    ToggleWorktrees, ToggleZoomPane, Zoom,
};
use crate::model::AppModel;
use crate::model::browsers::BrowserAction;
use crate::views::command_palette::{Handler, chord_label};
use crate::views::menu::application_handles;

mod available;
#[cfg(test)]
mod tests;

use available::{
    browser_enabled, can_add_worktree, can_close_other_tabs, can_close_tabs_to_the_left,
    can_close_tabs_to_the_right, can_float_pane, can_go_back, can_go_forward, can_move_tab_left,
    can_move_tab_right, can_open_browser, can_open_project_in_app, can_remove_project,
    can_rename_project, can_resize_down, can_resize_left, can_resize_right, can_resize_up,
    can_reveal_project, can_switch_worktree, can_toggle_worktrees, float_in_all_projects,
    float_in_project, has_browser, has_detached_tab, has_floats, has_listed_tab, has_other_project,
    has_other_tab, has_pane, has_split, has_tab, has_terminal, has_titled_tab, has_workspaces,
    in_float, pane_can_leave_window, project_available, project_present, tips_shown,
};

/// The palette lists the project, workspace, and server switchers under
/// these ids, each a list of its own.
pub(crate) const SWITCH_PROJECT_PAGE: &str = ALL[ShortcutId::SwitchProject as usize].id;
pub(crate) const SWITCH_WORKSPACE_PAGE: &str = ALL[ShortcutId::SwitchWorkspace as usize].id;
pub(crate) const SERVERS_PAGE: &str = ALL[ShortcutId::ShowServers as usize].id;

#[derive(Clone, Copy)]
enum Label {
    Key(&'static str),
    WithAppName(&'static str),
    None,
}

#[derive(Clone, Copy, PartialEq)]
enum Palette {
    Listed,
    Placed,
    Hidden,
}

type Run = fn(&mut AppModel, &mut Window, &mut Context<AppModel>);

struct CommandDef {
    shortcut: Option<ShortcutId>,
    palette_id: &'static str,
    label: Label,
    keywords: Option<&'static str>,
    feature: Option<Feature>,
    available: Option<fn(&AppModel) -> bool>,
    visible: Option<fn(&AppModel) -> bool>,
    terminal: Option<fn() -> TerminalAction>,
    palette: Palette,
    bind: fn(&mut muxy_ui::shortcuts::Registry<'_>, ShortcutId),
    boxed: fn() -> Box<dyn Action>,
    run: Option<Run>,
    listen: fn(Div, Run, &mut Context<AppModel>) -> Div,
}

fn bind<A: Action + Clone + Default>(
    registry: &mut muxy_ui::shortcuts::Registry<'_>,
    id: ShortcutId,
) {
    registry.register(id, &A::default());
}

fn listen<A: Action>(div: Div, run: Run, cx: &mut Context<AppModel>) -> Div {
    div.on_action(cx.listener(move |model, _: &A, window, cx| run(model, window, cx)))
}

fn boxed<A: Action + Default>() -> Box<dyn Action> {
    Box::new(A::default())
}

const fn unbound<A: Action + Clone + Default>(
    palette_id: &'static str,
    label: Label,
) -> CommandDef {
    CommandDef {
        shortcut: None,
        palette_id,
        label,
        keywords: None,
        feature: None,
        available: None,
        visible: None,
        terminal: None,
        palette: Palette::Listed,
        bind: bind::<A>,
        boxed: boxed::<A>,
        run: None,
        listen: listen::<A>,
    }
}

const fn row<A: Action + Clone + Default>(shortcut: ShortcutId, label: Label) -> CommandDef {
    let mut row = unbound::<A>(ALL[shortcut as usize].id, label);
    row.shortcut = Some(shortcut);
    row
}

const fn shortcut_only<A: Action + Clone + Default>(shortcut: ShortcutId) -> CommandDef {
    row::<A>(shortcut, Label::None).hidden()
}

impl CommandDef {
    const fn run(mut self, run: Run) -> Self {
        self.run = Some(run);
        self
    }

    const fn keywords(mut self, keywords: &'static str) -> Self {
        self.keywords = Some(keywords);
        self
    }

    const fn feature(mut self, feature: Feature) -> Self {
        self.feature = Some(feature);
        self
    }

    const fn available(mut self, available: fn(&AppModel) -> bool) -> Self {
        self.available = Some(available);
        self
    }

    /// Lists the command only while `visible` holds.
    const fn visible(mut self, visible: fn(&AppModel) -> bool) -> Self {
        self.visible = Some(visible);
        self
    }

    /// Runs `action` in the active terminal. The palette shows the
    /// terminal's own key for it when the command has none.
    const fn terminal(mut self, action: fn() -> TerminalAction) -> Self {
        self.terminal = Some(action);
        self
    }

    const fn placed(mut self) -> Self {
        self.palette = Palette::Placed;
        self
    }

    const fn hidden(mut self) -> Self {
        self.palette = Palette::Hidden;
        self
    }

    fn title(&self) -> SharedString {
        match self.label {
            Label::Key(key) => muxy_ui::l10n::translate(key),
            Label::WithAppName(key) => {
                muxy_core::l10n::format(key, &[muxy_core::l10n::Arg::from(crate::app_name())])
                    .into()
            }
            Label::None => SharedString::default(),
        }
    }

    fn search_words(&self) -> &'static str {
        self.keywords.unwrap_or(match self.label {
            Label::Key(key) | Label::WithAppName(key) => key,
            Label::None => "",
        })
    }

    fn palette_command(&self, model: &AppModel, cx: &App) -> Command<Handler> {
        let boxed = self.boxed;
        let handler: Handler = Rc::new(move |_, window, cx| window.dispatch_action(boxed(), cx));
        let command = Command::new(self.palette_id, self.title(), handler)
            .keywords(self.search_words())
            .disabled(self.available.is_some_and(|available| !available(model)));
        match self.shortcut_label(model, cx) {
            Some(shortcut) => command.shortcut(shortcut),
            None => command,
        }
    }

    /// The key that runs the command from where the keyboard is. A key the
    /// focused terminal binds to something else never reaches the command,
    /// so the terminal's own key for the same action shows instead.
    fn shortcut_label(&self, model: &AppModel, cx: &App) -> Option<String> {
        let action = self.terminal.map(|action| action());
        let bindings = model
            .active_pane()
            .and_then(|pane| model.terminal(&pane))
            .map(|pane| &pane.view.read(cx).terminal.keybindings);
        let chord = self
            .shortcut
            .and_then(|id| model.settings.keymap.chord(id))
            .filter(|chord| {
                bindings.is_none_or(|bindings| {
                    bindings
                        .bindings
                        .get(chord)
                        .is_none_or(|bound| action.as_ref() == Some(bound))
                })
            });
        chord.map(chord_label).or_else(|| {
            let bindings = bindings?;
            bindings
                .chords_for_action(action.as_ref()?)
                .find(|chord| {
                    bindings.bindings.contains_key(chord) || !application_handles(chord, cx)
                })
                .map(chord_label)
        })
    }

    fn listed(&self, model: &AppModel) -> bool {
        self.feature
            .is_none_or(|feature| model.capabilities.has_backend(feature))
            && self.visible.is_none_or(|visible| visible(model))
    }
}

static COMMANDS: &[CommandDef] = &[
    row::<ToggleComposer>(
        ShortcutId::ToggleComposer,
        Label::Key(tr_key!("Toggle Composer")),
    )
    .run(|model, window, cx| {
        model.toggle_composer(window, cx);
    }),
    row::<NewTab>(ShortcutId::NewTab, Label::Key(tr_key!("New Tab")))
        .available(project_present)
        .run(|model, _, cx| model.new_tab(cx)),
    row::<NewHomeTab>(ShortcutId::NewHomeTab, Label::Key(tr_key!("New Home Tab"))).run(
        |model, _, cx| {
            model.show_main_window(cx);
            let home = model.state.home().id;
            model.select_project(home, cx);
            model.new_tab_in(home, cx);
        },
    ),
    row::<CloseTab>(ShortcutId::CloseTab, Label::Key(tr_key!("Close Tab")))
        .available(has_tab)
        .run(|model, _, cx| {
            if let Some(tab) = model.active_tab() {
                model.close_tab(tab, cx);
            }
        }),
    row::<SplitRight>(ShortcutId::SplitRight, Label::Key(tr_key!("Split Right")))
        .available(has_pane)
        .run(|model, _, cx| model.split_pane(Direction::Right, cx)),
    row::<SplitDown>(ShortcutId::SplitDown, Label::Key(tr_key!("Split Down")))
        .available(has_pane)
        .run(|model, _, cx| model.split_pane(Direction::Down, cx)),
    row::<ClosePane>(ShortcutId::ClosePane, Label::Key(tr_key!("Close Pane")))
        .available(has_pane)
        .run(|model, _, cx| {
            if let Some(pane) = model.active_pane() {
                model.close_pane(pane, cx);
            }
        }),
    row::<ToggleZoomPane>(
        ShortcutId::ToggleZoomPane,
        Label::Key(tr_key!("Toggle Pane Zoom")),
    )
    .available(has_pane)
    .run(|model, _, cx| model.toggle_zoom_pane(cx)),
    row::<EqualizeSplits>(
        ShortcutId::EqualizeSplits,
        Label::Key(tr_key!("Equalize Splits")),
    )
    .available(has_split)
    .run(|model, _, cx| {
        if let Some(pane) = model.active_pane() {
            model.equalize_splits(pane, cx);
        }
    }),
    row::<NextProject>(ShortcutId::NextProject, Label::Key(tr_key!("Next Project")))
        .available(has_other_project)
        .run(|model, _, cx| model.cycle_project(true, cx)),
    row::<PreviousProject>(
        ShortcutId::PreviousProject,
        Label::Key(tr_key!("Previous Project")),
    )
    .available(has_other_project)
    .run(|model, _, cx| model.cycle_project(false, cx)),
    row::<NextTab>(ShortcutId::NextTab, Label::Key(tr_key!("Next Tab")))
        .available(has_other_tab)
        .run(|model, _, cx| model.cycle_tab(true, cx)),
    row::<PreviousTab>(ShortcutId::PreviousTab, Label::Key(tr_key!("Previous Tab")))
        .available(has_other_tab)
        .run(|model, _, cx| model.cycle_tab(false, cx)),
    row::<ToggleSidebar>(
        ShortcutId::ToggleSidebar,
        Label::Key(tr_key!("Toggle Sidebar")),
    )
    .run(AppModel::toggle_sidebar),
    row::<ToggleAgentsPanel>(
        ShortcutId::ToggleAgentsPanel,
        Label::Key(tr_key!("Toggle Agents Panel")),
    )
    .run(|model, window, cx| {
        model.toggle_agents_panel(window, cx);
    }),
    row::<ToggleAgentSessions>(
        ShortcutId::ToggleAgentSessions,
        Label::Key(tr_key!("Toggle Agent Sessions")),
    )
    .keywords("harness history resume providers")
    .run(|model, window, cx| {
        model.toggle_agent_sessions(window, cx);
    }),
    row::<NewWorktree>(
        ShortcutId::NewWorktree,
        Label::Key(tr_key!("New Worktree…")),
    )
    .available(can_add_worktree)
    .run(|model, window, cx| {
        if !model.dialogs.is_open()
            && let Some(project) = model.new_worktree_target()
        {
            model.open_git_form(project, true, cx);
            model.focus_git_form(window, cx);
        }
    }),
    row::<SwitchWorktree>(
        ShortcutId::SwitchWorktree,
        Label::Key(tr_key!("Switch Worktree…")),
    )
    .available(can_switch_worktree)
    .run(|model, window, cx| {
        model.toggle_worktree_switcher(window, cx);
    }),
    row::<Find>(ShortcutId::Find, Label::Key(tr_key!("Find…")))
        .available(has_terminal)
        .run(AppModel::find_terminal),
    row::<FindNext>(ShortcutId::FindNext, Label::Key(tr_key!("Find Next")))
        .available(has_terminal)
        .run(|model, _, cx| model.step_find(false, cx)),
    row::<FindPrevious>(
        ShortcutId::FindPrevious,
        Label::Key(tr_key!("Find Previous")),
    )
    .available(has_terminal)
    .run(|model, _, cx| model.step_find(true, cx)),
    row::<PreviousPrompt>(
        ShortcutId::PreviousPrompt,
        Label::Key(tr_key!("Previous Prompt")),
    )
    .available(has_terminal)
    .run(|model, _, cx| model.prompt_action(Some(true), cx)),
    row::<NextPrompt>(ShortcutId::NextPrompt, Label::Key(tr_key!("Next Prompt")))
        .available(has_terminal)
        .run(|model, _, cx| model.prompt_action(Some(false), cx)),
    row::<SelectCommandOutput>(
        ShortcutId::SelectCommandOutput,
        Label::Key(tr_key!("Select Command Output")),
    )
    .available(has_terminal)
    .run(|model, _, cx| model.prompt_action(None, cx)),
    row::<ScrollToBottom>(
        ShortcutId::ScrollToBottom,
        Label::Key(tr_key!("Scroll to Bottom")),
    )
    .available(has_terminal),
    row::<IncreaseFontSize>(
        ShortcutId::IncreaseFontSize,
        Label::Key(tr_key!("Increase Font Size")),
    )
    .available(has_terminal)
    .run(|model, _, cx| model.zoom_terminal(1.0, cx)),
    row::<DecreaseFontSize>(
        ShortcutId::DecreaseFontSize,
        Label::Key(tr_key!("Decrease Font Size")),
    )
    .available(has_terminal)
    .run(|model, _, cx| model.zoom_terminal(-1.0, cx)),
    row::<DetachTerminal>(
        ShortcutId::DetachTerminal,
        Label::Key(tr_key!("Detach Terminal")),
    )
    .available(has_terminal)
    .run(AppModel::detach_active_terminal),
    row::<FocusPaneLeft>(
        ShortcutId::FocusPaneLeft,
        Label::Key(tr_key!("Focus Pane Left")),
    )
    .available(has_split)
    .run(|model, _, cx| {
        model.focus_direction(Direction::Left, cx);
    }),
    row::<FocusPaneRight>(
        ShortcutId::FocusPaneRight,
        Label::Key(tr_key!("Focus Pane Right")),
    )
    .available(has_split)
    .run(|model, _, cx| {
        model.focus_direction(Direction::Right, cx);
    }),
    row::<FocusPaneUp>(
        ShortcutId::FocusPaneUp,
        Label::Key(tr_key!("Focus Pane Up")),
    )
    .available(has_split)
    .run(|model, _, cx| model.focus_direction(Direction::Up, cx)),
    row::<FocusPaneDown>(
        ShortcutId::FocusPaneDown,
        Label::Key(tr_key!("Focus Pane Down")),
    )
    .available(has_split)
    .run(|model, _, cx| {
        model.focus_direction(Direction::Down, cx);
    }),
    row::<ExistingTerminals>(
        ShortcutId::ExistingTerminals,
        Label::Key(tr_key!("Existing Terminals…")),
    )
    .run(AppModel::existing_terminals),
    row::<ToggleFullScreen>(
        ShortcutId::ToggleFullScreen,
        Label::Key(tr_key!("Toggle Full Screen")),
    )
    .run(|_, window, _| window.toggle_fullscreen()),
    row::<Minimize>(ShortcutId::Minimize, Label::Key(tr_key!("Minimize")))
        .keywords("Minimize Window")
        .run(|_, window, _| window.minimize_window()),
    row::<Zoom>(ShortcutId::Zoom, Label::Key(tr_key!("Zoom")))
        .keywords("Zoom Window Maximize Restore")
        .run(|_, window, _| window.zoom_window()),
    row::<Quit>(ShortcutId::Quit, Label::WithAppName(tr_key!("Quit %@")))
        .keywords("Quit")
        .run(|model, _, cx| {
            model.quit(cx);
            cx.stop_propagation();
        }),
    row::<EndAllSessionsAndQuit>(
        ShortcutId::EndAllSessionsAndQuit,
        Label::Key(tr_key!("End All Sessions and Quit")),
    )
    .run(|model, _, cx| {
        model.end_all_and_quit(cx);
        cx.stop_propagation();
    }),
    row::<MoveTabToNewWindow>(
        ShortcutId::MoveTabToNewWindow,
        Label::Key(tr_key!("Move Tab to New Window")),
    )
    .keywords("Detach Tab Window Separate")
    .available(project_present)
    .run(|model, _, cx| {
        if let Some(tab) = model.active_tab() {
            model.move_tab_to_window(tab, None, None, cx);
        }
    }),
    row::<MovePaneToNewTab>(
        ShortcutId::MovePaneToNewTab,
        Label::Key(tr_key!("Move Pane to New Tab")),
    )
    .keywords("Split Pane Tab Break Out")
    .available(has_split)
    .run(|model, _, cx| {
        if let Some(pane) = model.active_pane()
            && let Some(project) = model.state.pane_project(pane).map(|project| project.id)
        {
            model.move_pane_to_new_tab(pane, muxy_app_core::TabPlace::end(project), cx);
        }
    }),
    row::<MovePaneToNewWindow>(
        ShortcutId::MovePaneToNewWindow,
        Label::Key(tr_key!("Move Pane to New Window")),
    )
    .keywords("Detach Pane Window Separate")
    .available(pane_can_leave_window)
    .run(|model, _, cx| {
        if let Some(pane) = model.active_pane() {
            model.move_pane_to_window(pane, None, None, cx);
        }
    }),
    row::<FloatPane>(ShortcutId::FloatPane, Label::Key(tr_key!("Float Pane")))
        .keywords("Floating Terminal Panel Overlay Pop Out")
        .available(can_float_pane)
        .run(|model, _, cx| model.float_active_pane(cx)),
    row::<DockFloatingTerminal>(
        ShortcutId::DockFloatingTerminal,
        Label::Key(tr_key!("Dock Floating Terminal")),
    )
    .keywords("Floating Terminal Dock Tab Return")
    .available(in_float)
    .run(|model, _, cx| {
        if let Some(id) = model.key_float() {
            model.return_tab_window(id, None, cx);
        }
    }),
    row::<ShowFloatingTerminalInAllProjects>(
        ShortcutId::ShowFloatingTerminalInAllProjects,
        Label::Key(tr_key!("Show Floating Terminal in All Projects")),
    )
    .keywords("Floating Terminal Global Everywhere Scope")
    .available(float_in_project)
    .run(|model, _, cx| {
        if let Some(id) = model.key_float() {
            model.set_float_scope(id, muxy_app_core::FloatScope::AllProjects, cx);
        }
    }),
    row::<ShowFloatingTerminalInItsProject>(
        ShortcutId::ShowFloatingTerminalInItsProject,
        Label::Key(tr_key!("Show Floating Terminal Only in Its Project")),
    )
    .keywords("Floating Terminal Project Scope")
    .available(float_in_all_projects)
    .run(|model, _, cx| {
        if let Some(id) = model.key_float() {
            model.set_float_scope(id, muxy_app_core::FloatScope::Project, cx);
        }
    }),
    row::<ToggleFloatingTerminals>(
        ShortcutId::ToggleFloatingTerminals,
        Label::Key(tr_key!("Toggle Floating Terminals")),
    )
    .keywords("Floating Terminals Show Hide")
    .available(has_floats)
    .run(|model, _, cx| model.toggle_floats(cx)),
    row::<CheckForUpdates>(
        ShortcutId::CheckForUpdates,
        Label::Key(tr_key!("Check for Updates…")),
    )
    .keywords("Check for Updates")
    .feature(Feature::Updates)
    .run(|model, _, cx| {
        model.check_for_updates(true, cx);
    }),
    row::<InstallCommandLineTool>(
        ShortcutId::InstallCommandLineTool,
        Label::Key(tr_key!("Install Command Line Tool…")),
    )
    .feature(Feature::CommandLineTool)
    .run(|model, _, cx| {
        model.install_command_line(cx);
    }),
    row::<NewBrowserTab>(
        ShortcutId::NewBrowserTab,
        Label::Key(tr_key!("New Browser Tab")),
    )
    .available(can_open_browser)
    .run(|model, _, cx| model.new_browser_tab(cx)),
    row::<AddProject>(ShortcutId::AddProject, Label::Key(tr_key!("Open Project…")))
        .placed()
        .run(AppModel::open_project_picker),
    row::<NewTabInProject>(
        ShortcutId::NewTabInProject,
        Label::Key(tr_key!("New Tab in Project…")),
    )
    .placed()
    .run(|model, window, cx| {
        model.open_new_tab_picker(window, cx);
    }),
    row::<OpenSettings>(ShortcutId::OpenSettings, Label::Key(tr_key!("Settings…")))
        .keywords("Settings Open Settings")
        .placed()
        .run(AppModel::open_settings),
    row::<ShowWelcomeTour>(
        ShortcutId::ShowWelcomeTour,
        Label::Key(tr_key!("Show Welcome Tour")),
    )
    .keywords("Onboarding Welcome Tour Getting Started Intro Guide")
    .run(|model, _, cx| model.open_onboarding(cx)),
    unbound::<OpenAbout>("open_about", Label::WithAppName(tr_key!("About %@")))
        .hidden()
        .run(|model, window, cx| {
            model.show_about_settings(window, cx);
        }),
    row::<ToggleThemePicker>(
        ShortcutId::ToggleThemePicker,
        Label::Key(tr_key!("Change Theme…")),
    )
    .keywords("Change Theme Picker")
    .placed()
    .run(|model, window, cx| {
        model.open_theme_picker(window, cx);
    }),
    row::<ToggleCommandPalette>(
        ShortcutId::ToggleCommandPalette,
        Label::Key(tr_key!("Command Palette…")),
    )
    .hidden()
    .run(|model, window, cx| {
        model.toggle_command_palette(window, cx);
    }),
    row::<HideApp>(ShortcutId::HideApp, Label::WithAppName(tr_key!("Hide %@")))
        .hidden()
        .run(|model, _, cx| {
            model.forward(Feature::AppHiding, cx);
        }),
    row::<HideOthers>(ShortcutId::HideOthers, Label::Key(tr_key!("Hide Others")))
        .hidden()
        .run(|model, _, cx| {
            model.forward(Feature::AppHiding, cx);
        }),
    row::<ShowAll>(ShortcutId::ShowAll, Label::Key(tr_key!("Show All")))
        .hidden()
        .run(|model, _, cx| {
            model.forward(Feature::AppHiding, cx);
        }),
    row::<NavigateBack>(ShortcutId::NavigateBack, Label::Key(tr_key!("Back")))
        .keywords("Back Navigate History Previous Location")
        .available(can_go_back)
        .run(|model, _, cx| model.navigate(false, cx)),
    row::<NavigateForward>(ShortcutId::NavigateForward, Label::Key(tr_key!("Forward")))
        .keywords("Forward Navigate History Next Location")
        .available(can_go_forward)
        .run(|model, _, cx| model.navigate(true, cx)),
    shortcut_only::<OpenConfiguration>(ShortcutId::OpenConfiguration).run(AppModel::open_settings),
    row::<OpenKeyboardShortcuts>(
        ShortcutId::OpenKeyboardShortcuts,
        Label::Key(tr_key!("Keyboard Shortcuts…")),
    )
    .keywords("Keyboard Shortcuts Keybindings Keymap Settings")
    .run(AppModel::show_keyboard_settings),
    row::<ReloadConfiguration>(
        ShortcutId::ReloadConfiguration,
        Label::Key(tr_key!("Reload Configuration")),
    )
    .keywords("Reload Configuration Terminal Settings Config")
    .run(|model, _, cx| model.reload_configuration(cx)),
    row::<ToggleStatusBar>(
        ShortcutId::ToggleStatusBar,
        Label::Key(tr_key!("Toggle Status Bar")),
    )
    .keywords("Toggle Status Bar Show Hide")
    .run(|model, _, cx| model.toggle_status_bar(cx)),
    row::<SwitchProject>(
        ShortcutId::SwitchProject,
        Label::Key(tr_key!("Switch Project…")),
    )
    .keywords("Switch Project")
    .hidden()
    .run(|model, window, cx| {
        model.toggle_palette_page(SWITCH_PROJECT_PAGE, window, cx);
    }),
    row::<SwitchWorkspace>(
        ShortcutId::SwitchWorkspace,
        Label::Key(tr_key!("Switch Workspace…")),
    )
    .keywords("Switch Workspace")
    .hidden()
    .run(|model, window, cx| {
        if has_workspaces(model) {
            model.toggle_palette_page(SWITCH_WORKSPACE_PAGE, window, cx);
        }
    }),
    row::<ShowServers>(ShortcutId::ShowServers, Label::Key(tr_key!("Servers…")))
        .keywords("Servers Remote Clients Pairing")
        .hidden()
        .run(|model, window, cx| {
            model.toggle_palette_page(SERVERS_PAGE, window, cx);
        }),
    row::<RenameTab>(ShortcutId::RenameTab, Label::Key(tr_key!("Rename Tab…")))
        .keywords("Rename Tab Title")
        .available(has_tab)
        .run(AppModel::rename_active_tab),
    row::<ResetTabTitle>(
        ShortcutId::ResetTabTitle,
        Label::Key(tr_key!("Reset Tab Title")),
    )
    .available(has_titled_tab)
    .run(|model, _, cx| model.reset_active_tab_title(cx)),
    row::<ToggleTabPin>(
        ShortcutId::ToggleTabPin,
        Label::Key(tr_key!("Toggle Tab Pin")),
    )
    .keywords("Toggle Tab Pin Unpin")
    .available(has_listed_tab)
    .run(|model, _, cx| {
        if has_listed_tab(model) {
            model.toggle_active_tab_pin(cx);
        }
    }),
    row::<CloseOtherTabs>(
        ShortcutId::CloseOtherTabs,
        Label::Key(tr_key!("Close Other Tabs")),
    )
    .available(can_close_other_tabs)
    .run(|model, _, cx| model.close_tabs_around_active(TabCloseScope::Other, cx)),
    row::<CloseTabsToTheLeft>(
        ShortcutId::CloseTabsToTheLeft,
        Label::Key(tr_key!("Close Tabs to the Left")),
    )
    .available(can_close_tabs_to_the_left)
    .run(|model, _, cx| model.close_tabs_around_active(TabCloseScope::Left, cx)),
    row::<CloseTabsToTheRight>(
        ShortcutId::CloseTabsToTheRight,
        Label::Key(tr_key!("Close Tabs to the Right")),
    )
    .available(can_close_tabs_to_the_right)
    .run(|model, _, cx| model.close_tabs_around_active(TabCloseScope::Right, cx)),
    row::<NewTabToTheLeft>(
        ShortcutId::NewTabToTheLeft,
        Label::Key(tr_key!("New Tab to the Left")),
    )
    .available(has_listed_tab)
    .run(|model, _, cx| {
        if has_listed_tab(model) {
            model.new_tab_beside_active(TabSide::Left, cx);
        }
    }),
    row::<NewTabToTheRight>(
        ShortcutId::NewTabToTheRight,
        Label::Key(tr_key!("New Tab to the Right")),
    )
    .available(has_listed_tab)
    .run(|model, _, cx| {
        if has_listed_tab(model) {
            model.new_tab_beside_active(TabSide::Right, cx);
        }
    }),
    row::<MoveTabLeft>(
        ShortcutId::MoveTabLeft,
        Label::Key(tr_key!("Move Tab Left")),
    )
    .keywords("Move Tab Left Reorder Previous")
    .available(can_move_tab_left)
    .run(|model, _, cx| model.move_active_tab(false, cx)),
    row::<MoveTabRight>(
        ShortcutId::MoveTabRight,
        Label::Key(tr_key!("Move Tab Right")),
    )
    .keywords("Move Tab Right Reorder Next")
    .available(can_move_tab_right)
    .run(|model, _, cx| model.move_active_tab(true, cx)),
    row::<MoveTabToMainWindow>(
        ShortcutId::MoveTabToMainWindow,
        Label::Key(tr_key!("Move Tab to Main Window")),
    )
    .keywords("Move Tab to Main Window Return Attach")
    .available(has_detached_tab)
    .run(|model, _, cx| model.return_active_tab_to_main_window(cx)),
    row::<RenameProject>(
        ShortcutId::RenameProject,
        Label::Key(tr_key!("Rename Project…")),
    )
    .keywords("Rename Project Worktree")
    .available(can_rename_project)
    .run(|model, window, cx| {
        if can_rename_project(model) {
            model.rename_current_project(window, cx);
        }
    }),
    row::<RemoveProject>(
        ShortcutId::RemoveProject,
        Label::Key(tr_key!("Remove Project…")),
    )
    .keywords("Remove Project Worktree Delete")
    .available(can_remove_project)
    .run(|model, _, cx| model.remove_current_project(cx)),
    row::<CopyProjectPath>(
        ShortcutId::CopyProjectPath,
        Label::Key(tr_key!("Copy Project Path")),
    )
    .keywords("Copy Project Path Folder Directory")
    .available(project_available)
    .run(|model, _, cx| model.copy_project_path(model.state.current_project().id, cx)),
    row::<RevealProject>(
        ShortcutId::RevealProject,
        Label::Key(if cfg!(target_os = "macos") {
            tr_key!("Reveal Project in Finder")
        } else {
            tr_key!("Show Project in File Manager")
        }),
    )
    .keywords("Reveal Project in Finder Show File Manager Folder Directory")
    .available(can_reveal_project)
    .run(|model, _, cx| model.reveal_project(model.state.current_project().id, cx)),
    row::<ApplyLayout>(
        ShortcutId::ApplyLayout,
        Label::Key(tr_key!("Apply Layout…")),
    )
    .keywords("Apply Layout Project Tabs Panes")
    .available(project_available)
    .run(|model, window, cx| {
        model.open_layout_picker(model.state.current_project().id, window, cx);
    }),
    row::<ToggleProjectFocus>(
        ShortcutId::ToggleProjectFocus,
        Label::Key(tr_key!("Toggle Project Focus")),
    )
    .keywords("Toggle Project Focus Show All Projects Sidebar")
    .run(|model, _, cx| model.toggle_project_focus(cx)),
    row::<ToggleWorktrees>(
        ShortcutId::ToggleWorktrees,
        Label::Key(tr_key!("Toggle Worktrees")),
    )
    .keywords("Toggle Worktrees Show Hide Sidebar")
    .available(can_toggle_worktrees)
    .run(|model, _, cx| {
        if can_toggle_worktrees(model) {
            model.toggle_worktree_visibility(model.current_parent_project(), cx);
        }
    }),
    row::<OpenProjectInDefaultApp>(
        ShortcutId::OpenProjectInDefaultApp,
        Label::Key(tr_key!("Open Project in Default App")),
    )
    .keywords("Open Project in Default App Editor IDE")
    .available(can_open_project_in_app)
    .run(|model, _, cx| model.open_project_in_default(cx)),
    row::<NewWorkspace>(
        ShortcutId::NewWorkspace,
        Label::Key(tr_key!("New Workspace…")),
    )
    .run(AppModel::new_workspace),
    row::<ResizePaneLeft>(
        ShortcutId::ResizePaneLeft,
        Label::Key(tr_key!("Resize Pane Left")),
    )
    .keywords("Resize Pane Left Split Divider")
    .available(can_resize_left)
    .run(|model, _, cx| model.resize_active_pane(Direction::Left, cx)),
    row::<ResizePaneRight>(
        ShortcutId::ResizePaneRight,
        Label::Key(tr_key!("Resize Pane Right")),
    )
    .keywords("Resize Pane Right Split Divider")
    .available(can_resize_right)
    .run(|model, _, cx| model.resize_active_pane(Direction::Right, cx)),
    row::<ResizePaneUp>(
        ShortcutId::ResizePaneUp,
        Label::Key(tr_key!("Resize Pane Up")),
    )
    .keywords("Resize Pane Up Split Divider")
    .available(can_resize_up)
    .run(|model, _, cx| model.resize_active_pane(Direction::Up, cx)),
    row::<ResizePaneDown>(
        ShortcutId::ResizePaneDown,
        Label::Key(tr_key!("Resize Pane Down")),
    )
    .keywords("Resize Pane Down Split Divider")
    .available(can_resize_down)
    .run(|model, _, cx| model.resize_active_pane(Direction::Down, cx)),
    row::<FocusNextPane>(
        ShortcutId::FocusNextPane,
        Label::Key(tr_key!("Focus Next Pane")),
    )
    .available(has_split)
    .run(|model, _, cx| model.cycle_pane(true, cx)),
    row::<FocusPreviousPane>(
        ShortcutId::FocusPreviousPane,
        Label::Key(tr_key!("Focus Previous Pane")),
    )
    .available(has_split)
    .run(|model, _, cx| model.cycle_pane(false, cx)),
    row::<ClearScreen>(ShortcutId::ClearScreen, Label::Key(tr_key!("Clear Screen")))
        .keywords("Clear Screen Terminal Scrollback")
        .available(has_terminal)
        .terminal(|| TerminalAction::ClearScreen)
        .run(|model, _, cx| model.terminal_action(&TerminalAction::ClearScreen, cx)),
    row::<SelectAllInTerminal>(ShortcutId::SelectAll, Label::Key(tr_key!("Select All")))
        .keywords("Select All Terminal Text")
        .available(has_terminal)
        .terminal(|| TerminalAction::SelectAll)
        .run(|model, _, cx| model.terminal_action(&TerminalAction::SelectAll, cx)),
    row::<ScrollToTop>(
        ShortcutId::ScrollToTop,
        Label::Key(tr_key!("Scroll to Top")),
    )
    .available(has_terminal)
    .terminal(|| TerminalAction::ScrollTop)
    .run(|model, _, cx| model.terminal_action(&TerminalAction::ScrollTop, cx)),
    row::<ResetFontSize>(
        ShortcutId::ResetFontSize,
        Label::Key(tr_key!("Reset Font Size")),
    )
    .available(has_terminal)
    .terminal(|| TerminalAction::ResetFontSize)
    .run(|model, _, cx| model.terminal_action(&TerminalAction::ResetFontSize, cx)),
    row::<CreateExtension>(
        ShortcutId::CreateExtension,
        Label::Key(tr_key!("Create Extension…")),
    )
    .keywords("Create Extension new make build agent")
    .run(|model, _, cx| {
        model.open_extension_agent(crate::extensions::authoring::AgentTask::Create, cx);
    }),
    row::<ManageExtensions>(
        ShortcutId::ManageExtensions,
        Label::Key(tr_key!("Manage Extensions")),
    )
    .keywords("Manage Extensions settings installed marketplace")
    .run(AppModel::show_extensions_settings),
    row::<ShowTip>(ShortcutId::ShowTip, Label::Key(tr_key!("Show Muxy Tip")))
        .keywords("Show Muxy Tip Tips")
        .visible(tips_shown)
        .run(|model, _, cx| {
            if tips_shown(model) {
                model.show_tip(cx);
            }
        }),
    row::<BrowserFocusAddress>(
        ShortcutId::BrowserFocusAddress,
        Label::Key(tr_key!("Focus Address Bar")),
    )
    .keywords("Focus Address Bar Browser URL Location")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, window, cx| {
        model.browser_action(BrowserAction::FocusAddress, window, cx);
    }),
    row::<BrowserReload>(
        ShortcutId::BrowserReload,
        Label::Key(tr_key!("Reload Page")),
    )
    .keywords("Reload Page Browser Refresh")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, window, cx| {
        model.browser_action(BrowserAction::Reload, window, cx);
    }),
    row::<BrowserBack>(ShortcutId::BrowserBack, Label::Key(tr_key!("Page Back")))
        .keywords("Page Back Browser History Previous")
        .available(has_browser)
        .visible(browser_enabled)
        .run(|model, window, cx| {
            model.browser_action(BrowserAction::Back, window, cx);
        }),
    row::<BrowserForward>(
        ShortcutId::BrowserForward,
        Label::Key(tr_key!("Page Forward")),
    )
    .keywords("Page Forward Browser History Next")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, window, cx| {
        model.browser_action(BrowserAction::Forward, window, cx);
    }),
    row::<BrowserZoomIn>(
        ShortcutId::BrowserZoomIn,
        Label::Key(tr_key!("Zoom In Page")),
    )
    .keywords("Zoom In Page Browser Bigger")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, window, cx| {
        model.browser_action(BrowserAction::ZoomIn, window, cx);
    }),
    row::<BrowserZoomOut>(
        ShortcutId::BrowserZoomOut,
        Label::Key(tr_key!("Zoom Out Page")),
    )
    .keywords("Zoom Out Page Browser Smaller")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, window, cx| {
        model.browser_action(BrowserAction::ZoomOut, window, cx);
    }),
    row::<BrowserZoomReset>(
        ShortcutId::BrowserZoomReset,
        Label::Key(tr_key!("Reset Page Zoom")),
    )
    .keywords("Reset Page Zoom Browser Actual Size")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, window, cx| {
        model.browser_action(BrowserAction::ZoomReset, window, cx);
    }),
    row::<BrowserCopyUrl>(
        ShortcutId::BrowserCopyUrl,
        Label::Key(tr_key!("Copy Page Address")),
    )
    .keywords("Copy Page Address Browser URL Link")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, window, cx| {
        model.browser_action(BrowserAction::CopyUrl, window, cx);
    }),
    row::<BrowserInspect>(
        ShortcutId::BrowserInspect,
        Label::Key(tr_key!("Open Web Inspector")),
    )
    .keywords("Open Web Inspector Browser Developer Tools DevTools")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, window, cx| {
        model.browser_action(BrowserAction::Inspect, window, cx);
    }),
    row::<BrowserOpenExternally>(
        ShortcutId::BrowserOpenExternally,
        Label::Key(tr_key!("Open Page in Default Browser")),
    )
    .keywords("Open Page in Default Browser External")
    .available(has_browser)
    .visible(browser_enabled)
    .run(|model, _, cx| model.open_active_page_externally(cx)),
];

pub(super) fn action_handlers(cx: &mut Context<AppModel>) -> Div {
    let mut div = div();
    for row in COMMANDS {
        if let Some(run) = row.run {
            div = (row.listen)(div, run, cx);
        }
    }
    div
}

fn with_id(id: ShortcutId) -> Option<&'static CommandDef> {
    COMMANDS.iter().find(|row| row.shortcut == Some(id))
}

pub(super) fn bind_shortcuts(registry: &mut muxy_ui::shortcuts::Registry<'_>) {
    for row in COMMANDS {
        if let Some(id) = row.shortcut {
            (row.bind)(registry, id);
        }
    }
}

pub(crate) fn register_listed(registry: &mut Registry<Handler>, model: &AppModel, cx: &App) {
    for row in COMMANDS
        .iter()
        .filter(|row| row.palette == Palette::Listed && row.listed(model))
    {
        registry.register(row.palette_command(model, cx));
    }
}

pub(crate) fn register_placed(
    registry: &mut Registry<Handler>,
    model: &AppModel,
    id: ShortcutId,
    cx: &App,
) {
    if let Some(row) = with_id(id) {
        registry.register(row.palette_command(model, cx));
    }
}

pub(crate) fn title(id: ShortcutId) -> SharedString {
    with_id(id).map_or_else(SharedString::default, CommandDef::title)
}

pub(crate) fn search_words(id: ShortcutId) -> &'static str {
    with_id(id).map_or("", CommandDef::search_words)
}

/// A menu item for `action`, under its table label.
pub(crate) fn menu_item(action: impl Action) -> MenuItem {
    let title = COMMANDS
        .iter()
        .find(|row| (row.boxed)().partial_eq(&action))
        .map_or_else(|| action.name().into(), CommandDef::title);
    MenuItem::action(title, action)
}
