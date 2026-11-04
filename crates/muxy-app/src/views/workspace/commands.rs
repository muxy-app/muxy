use std::rc::Rc;

use gpui::{Action, Context, Div, InteractiveElement, MenuItem, SharedString, Window, div};
use muxy_app_core::Direction;
use muxy_core::capability::Feature;
use muxy_core::shortcuts::{ALL, ShortcutId};
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::l10n::tr_key;

use super::{
    AddProject, BrowserBack, BrowserCopyUrl, BrowserFocusAddress, BrowserForward, BrowserInspect,
    BrowserReload, BrowserZoomIn, BrowserZoomOut, BrowserZoomReset, CheckForUpdates, ClosePane,
    CloseTab, DecreaseFontSize, DetachTerminal, EndAllSessionsAndQuit, EqualizeSplits,
    ExistingTerminals, Find, FindNext, FindPrevious, FocusPaneDown, FocusPaneLeft, FocusPaneRight,
    FocusPaneUp, HideApp, HideOthers, IncreaseFontSize, InstallCommandLineTool, Minimize,
    NavigateBack, NavigateForward, NewBrowserTab, NewHomeTab, NewTab, NewTabInProject, NewWorktree,
    NextProject, NextPrompt, NextTab, OpenAbout, OpenConfiguration, OpenSettings, PreviousProject,
    PreviousPrompt, PreviousTab, Quit, ScrollToBottom, SelectCommandOutput, ShowAll, SplitDown,
    SplitRight, SwitchWorktree, ToggleAgentSessions, ToggleAgentsPanel, ToggleCommandPalette,
    ToggleComposer, ToggleFullScreen, ToggleSidebar, ToggleThemePicker, ToggleZoomPane, Zoom,
};
use crate::model::AppModel;
use crate::model::browsers::BrowserAction;
use crate::views::command_palette::{Handler, with_shortcut};

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

    fn palette_command(&self, model: &AppModel) -> Command<Handler> {
        let boxed = self.boxed;
        let handler: Handler = Rc::new(move |_, window, cx| window.dispatch_action(boxed(), cx));
        let command = Command::new(self.palette_id, self.title(), handler)
            .keywords(self.search_words())
            .disabled(self.available.is_some_and(|available| !available(model)));
        match self.shortcut {
            Some(id) => with_shortcut(model, id, command),
            None => command,
        }
    }
}

fn project_present(model: &AppModel) -> bool {
    model.state.current_project().status() != muxy_app_core::ProjectStatus::Missing
}

fn has_tab(model: &AppModel) -> bool {
    model.active_tab().is_some()
}

fn has_pane(model: &AppModel) -> bool {
    project_present(model) && model.active_pane().is_some()
}

fn has_split(model: &AppModel) -> bool {
    project_present(model)
        && model
            .active_tab()
            .and_then(|tab| model.tab(tab))
            .is_some_and(|tab| tab.panes.len() >= 2)
}

fn has_terminal(model: &AppModel) -> bool {
    model
        .active_pane()
        .and_then(|pane| model.terminal(&pane))
        .is_some()
}

fn has_other_project(model: &AppModel) -> bool {
    model.navigation_projects().len() >= 2
}

fn has_other_tab(model: &AppModel) -> bool {
    model.navigation_tabs().len() >= 2
}

fn can_add_worktree(model: &AppModel) -> bool {
    model.new_worktree_target().is_some()
}

fn can_switch_worktree(model: &AppModel) -> bool {
    model.worktree_switcher_project().is_some()
}

fn can_open_browser(model: &AppModel) -> bool {
    project_present(model) && model.browser_enabled()
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
            model.select_project(model.state.home().id, cx);
            model.new_tab(cx);
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
    unbound::<CheckForUpdates>(
        "check_for_updates",
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
    shortcut_only::<NavigateBack>(ShortcutId::NavigateBack)
        .run(|model, _, cx| model.navigate(false, cx)),
    shortcut_only::<NavigateForward>(ShortcutId::NavigateForward)
        .run(|model, _, cx| model.navigate(true, cx)),
    shortcut_only::<OpenConfiguration>(ShortcutId::OpenConfiguration).run(AppModel::open_settings),
    shortcut_only::<BrowserFocusAddress>(ShortcutId::BrowserFocusAddress).run(
        |model, window, cx| {
            model.browser_action(BrowserAction::FocusAddress, window, cx);
        },
    ),
    shortcut_only::<BrowserReload>(ShortcutId::BrowserReload).run(|model, window, cx| {
        model.browser_action(BrowserAction::Reload, window, cx);
    }),
    shortcut_only::<BrowserBack>(ShortcutId::BrowserBack).run(|model, window, cx| {
        model.browser_action(BrowserAction::Back, window, cx);
    }),
    shortcut_only::<BrowserForward>(ShortcutId::BrowserForward).run(|model, window, cx| {
        model.browser_action(BrowserAction::Forward, window, cx);
    }),
    shortcut_only::<BrowserZoomIn>(ShortcutId::BrowserZoomIn).run(|model, window, cx| {
        model.browser_action(BrowserAction::ZoomIn, window, cx);
    }),
    shortcut_only::<BrowserZoomOut>(ShortcutId::BrowserZoomOut).run(|model, window, cx| {
        model.browser_action(BrowserAction::ZoomOut, window, cx);
    }),
    shortcut_only::<BrowserZoomReset>(ShortcutId::BrowserZoomReset).run(|model, window, cx| {
        model.browser_action(BrowserAction::ZoomReset, window, cx);
    }),
    shortcut_only::<BrowserCopyUrl>(ShortcutId::BrowserCopyUrl).run(|model, window, cx| {
        model.browser_action(BrowserAction::CopyUrl, window, cx);
    }),
    shortcut_only::<BrowserInspect>(ShortcutId::BrowserInspect).run(|model, window, cx| {
        model.browser_action(BrowserAction::Inspect, window, cx);
    }),
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

pub(crate) fn register_listed(registry: &mut Registry<Handler>, model: &AppModel) {
    for row in COMMANDS.iter().filter(|row| row.palette == Palette::Listed) {
        if row
            .feature
            .is_none_or(|feature| model.capabilities.has_backend(feature))
        {
            registry.register(row.palette_command(model));
        }
    }
}

pub(crate) fn register_placed(registry: &mut Registry<Handler>, model: &AppModel, id: ShortcutId) {
    if let Some(row) = with_id(id) {
        registry.register(row.palette_command(model));
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
