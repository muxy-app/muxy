use gpui::{KeyBinding, actions};
use muxy_core::shortcuts::{KEY_CONTEXT, ShortcutAction, ShortcutMap};

actions!(
    workspace_tabs,
    [
        NewTab,
        NewHomeTab,
        NewBrowserTab,
        CloseTab,
        RenameTab,
        PinUnpinTab,
        SplitRight,
        SplitDown,
        ClosePane,
        FocusPaneLeft,
        FocusPaneRight,
        FocusPaneUp,
        FocusPaneDown,
        MovePaneLeft,
        MovePaneRight,
        MovePaneUp,
        MovePaneDown,
        CycleNextTabAcrossPanes,
        CyclePreviousTabAcrossPanes,
        NextTab,
        PreviousTab,
        SelectTab1,
        SelectTab2,
        SelectTab3,
        SelectTab4,
        SelectTab5,
        SelectTab6,
        SelectTab7,
        SelectTab8,
        SelectTab9,
        ToggleMaximizePane,
        OpenProject,
        RecentlyRemovedProjects,
        RefreshWorktrees,
        CreateWorktree,
        NextProject,
        PreviousProject,
        SelectProject1,
        SelectProject2,
        SelectProject3,
        SelectProject4,
        SelectProject5,
        SelectProject6,
        SelectProject7,
        SelectProject8,
        SelectProject9,
        NavigateBack,
        NavigateForward,
        FindInTerminal,
        TerminalOmnibox,
        TerminalOmniboxProjects,
        TerminalOmniboxWorktrees,
        TerminalOmniboxWorkspaces,
        TerminalOmniboxCommands,
        ToggleSidebar,
        ToggleFullScreen,
        ToggleThemePicker,
        ReloadConfig
    ]
);

#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = command_shortcuts, no_json)]
pub struct RunCommandShortcut {
    pub id: String,
}

pub fn command_bindings(config: &muxy_core::store::CommandShortcuts) -> Vec<KeyBinding> {
    let Some(prefix) = config.prefix_combo.keystroke() else {
        return Vec::new();
    };
    config
        .shortcuts
        .iter()
        .filter(|shortcut| !shortcut.trimmed_command().is_empty())
        .filter_map(|shortcut| {
            let combo = shortcut.combo.keystroke()?;
            Some(KeyBinding::new(
                &format!("{prefix} {combo}"),
                RunCommandShortcut {
                    id: shortcut.id.clone(),
                },
                Some(KEY_CONTEXT),
            ))
        })
        .collect()
}

pub fn key_bindings(shortcuts: &ShortcutMap) -> Vec<KeyBinding> {
    shortcuts
        .bindings()
        .iter()
        .filter_map(|(action, combo)| binding(*action, combo.keystroke()?))
        .collect()
}

fn binding(action: ShortcutAction, key: String) -> Option<KeyBinding> {
    let context = Some(KEY_CONTEXT);
    let key = key.as_str();
    Some(match action {
        ShortcutAction::NewTab => KeyBinding::new(key, NewTab, context),
        ShortcutAction::NewHomeTab => KeyBinding::new(key, NewHomeTab, context),
        ShortcutAction::NewBrowserTab => KeyBinding::new(key, NewBrowserTab, context),
        ShortcutAction::CloseTab => KeyBinding::new(key, CloseTab, context),
        ShortcutAction::RenameTab => KeyBinding::new(key, RenameTab, context),
        ShortcutAction::PinUnpinTab => KeyBinding::new(key, PinUnpinTab, context),
        ShortcutAction::SplitRight => KeyBinding::new(key, SplitRight, context),
        ShortcutAction::SplitDown => KeyBinding::new(key, SplitDown, context),
        ShortcutAction::ClosePane => KeyBinding::new(key, ClosePane, context),
        ShortcutAction::FocusPaneLeft => KeyBinding::new(key, FocusPaneLeft, context),
        ShortcutAction::FocusPaneRight => KeyBinding::new(key, FocusPaneRight, context),
        ShortcutAction::FocusPaneUp => KeyBinding::new(key, FocusPaneUp, context),
        ShortcutAction::FocusPaneDown => KeyBinding::new(key, FocusPaneDown, context),
        ShortcutAction::MovePaneLeft => KeyBinding::new(key, MovePaneLeft, context),
        ShortcutAction::MovePaneRight => KeyBinding::new(key, MovePaneRight, context),
        ShortcutAction::MovePaneUp => KeyBinding::new(key, MovePaneUp, context),
        ShortcutAction::MovePaneDown => KeyBinding::new(key, MovePaneDown, context),
        ShortcutAction::CycleNextTabAcrossPanes => {
            KeyBinding::new(key, CycleNextTabAcrossPanes, context)
        }
        ShortcutAction::CyclePreviousTabAcrossPanes => {
            KeyBinding::new(key, CyclePreviousTabAcrossPanes, context)
        }
        ShortcutAction::NextTab => KeyBinding::new(key, NextTab, context),
        ShortcutAction::PreviousTab => KeyBinding::new(key, PreviousTab, context),
        ShortcutAction::SelectTab1 => KeyBinding::new(key, SelectTab1, context),
        ShortcutAction::SelectTab2 => KeyBinding::new(key, SelectTab2, context),
        ShortcutAction::SelectTab3 => KeyBinding::new(key, SelectTab3, context),
        ShortcutAction::SelectTab4 => KeyBinding::new(key, SelectTab4, context),
        ShortcutAction::SelectTab5 => KeyBinding::new(key, SelectTab5, context),
        ShortcutAction::SelectTab6 => KeyBinding::new(key, SelectTab6, context),
        ShortcutAction::SelectTab7 => KeyBinding::new(key, SelectTab7, context),
        ShortcutAction::SelectTab8 => KeyBinding::new(key, SelectTab8, context),
        ShortcutAction::SelectTab9 => KeyBinding::new(key, SelectTab9, context),
        ShortcutAction::ToggleMaximizePane => KeyBinding::new(key, ToggleMaximizePane, context),
        ShortcutAction::OpenProject => KeyBinding::new(key, OpenProject, context),
        ShortcutAction::RecentlyRemovedProjects => {
            KeyBinding::new(key, RecentlyRemovedProjects, context)
        }
        ShortcutAction::RefreshWorktrees => KeyBinding::new(key, RefreshWorktrees, context),
        ShortcutAction::CreateWorktree => KeyBinding::new(key, CreateWorktree, context),
        ShortcutAction::NextProject => KeyBinding::new(key, NextProject, context),
        ShortcutAction::PreviousProject => KeyBinding::new(key, PreviousProject, context),
        ShortcutAction::SelectProject1 => KeyBinding::new(key, SelectProject1, context),
        ShortcutAction::SelectProject2 => KeyBinding::new(key, SelectProject2, context),
        ShortcutAction::SelectProject3 => KeyBinding::new(key, SelectProject3, context),
        ShortcutAction::SelectProject4 => KeyBinding::new(key, SelectProject4, context),
        ShortcutAction::SelectProject5 => KeyBinding::new(key, SelectProject5, context),
        ShortcutAction::SelectProject6 => KeyBinding::new(key, SelectProject6, context),
        ShortcutAction::SelectProject7 => KeyBinding::new(key, SelectProject7, context),
        ShortcutAction::SelectProject8 => KeyBinding::new(key, SelectProject8, context),
        ShortcutAction::SelectProject9 => KeyBinding::new(key, SelectProject9, context),
        ShortcutAction::NavigateBack => KeyBinding::new(key, NavigateBack, context),
        ShortcutAction::NavigateForward => KeyBinding::new(key, NavigateForward, context),
        ShortcutAction::FindInTerminal => KeyBinding::new(key, FindInTerminal, context),
        ShortcutAction::TerminalOmnibox => KeyBinding::new(key, TerminalOmnibox, context),
        ShortcutAction::TerminalOmniboxProjects => {
            KeyBinding::new(key, TerminalOmniboxProjects, context)
        }
        ShortcutAction::TerminalOmniboxWorktrees => {
            KeyBinding::new(key, TerminalOmniboxWorktrees, context)
        }
        ShortcutAction::TerminalOmniboxWorkspaces => {
            KeyBinding::new(key, TerminalOmniboxWorkspaces, context)
        }
        ShortcutAction::TerminalOmniboxCommands => {
            KeyBinding::new(key, TerminalOmniboxCommands, context)
        }
        ShortcutAction::ToggleSidebar => KeyBinding::new(key, ToggleSidebar, context),
        ShortcutAction::ToggleFullScreen => KeyBinding::new(key, ToggleFullScreen, context),
        ShortcutAction::ToggleThemePicker => KeyBinding::new(key, ToggleThemePicker, context),
        ShortcutAction::ReloadConfig => KeyBinding::new(key, ReloadConfig, context),
    })
}
