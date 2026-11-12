use gpui::App;
use muxy_app_core::{
    Direction,
    settings::{KeyChord, TerminalAction},
};
use muxy_core::shortcuts::ShortcutId;

use super::Command;
use crate::model::AppModel;
use crate::views::project_editor::Field;
use crate::views::tab_menu::Action as TabAction;

/// The command that does to the active tab what a tab menu row does to its own.
fn tab_shortcut(action: TabAction) -> Option<ShortcutId> {
    use muxy_app_core::{TabCloseScope, TabSide};
    match action {
        TabAction::New(TabSide::Left) => Some(ShortcutId::NewTabToTheLeft),
        TabAction::New(TabSide::Right) => Some(ShortcutId::NewTabToTheRight),
        TabAction::Rename => Some(ShortcutId::RenameTab),
        TabAction::ResetTitle => Some(ShortcutId::ResetTabTitle),
        TabAction::TogglePin => Some(ShortcutId::ToggleTabPin),
        TabAction::CloseTabs(TabCloseScope::Other) => Some(ShortcutId::CloseOtherTabs),
        TabAction::CloseTabs(TabCloseScope::Left) => Some(ShortcutId::CloseTabsToTheLeft),
        TabAction::CloseTabs(TabCloseScope::Right) => Some(ShortcutId::CloseTabsToTheRight),
        TabAction::MoveToWindow => Some(ShortcutId::MoveTabToNewWindow),
        TabAction::ReturnToMainWindow => Some(ShortcutId::MoveTabToMainWindow),
        TabAction::Close
        | TabAction::SetColor(_)
        | TabAction::ResetColor
        | TabAction::MoveToProject(_) => None,
    }
}

impl Command {
    pub(crate) fn shortcut(self, model: &AppModel, cx: &App) -> Option<String> {
        let current = model.state.current_project().id;
        let action = match self {
            Self::AddProject(server) if server == model.state.current_project().server_id => {
                Some(ShortcutId::AddProject)
            }
            Self::AgentsPanel(_) => Some(ShortcutId::ToggleAgentsPanel),
            Self::TerminalCopy(_) => Some(ShortcutId::Copy),
            Self::TerminalPaste(_) => Some(ShortcutId::Paste),
            Self::TerminalSelectCommandOutput(_) => Some(ShortcutId::SelectCommandOutput),
            Self::SplitPane(_, Direction::Right) => Some(ShortcutId::SplitRight),
            Self::SplitPane(_, Direction::Down) => Some(ShortcutId::SplitDown),
            Self::ToggleZoomPane(_) => Some(ShortcutId::ToggleZoomPane),
            Self::EqualizeSplits(_) => Some(ShortcutId::EqualizeSplits),
            Self::ClosePane(_) => Some(ShortcutId::ClosePane),
            Self::DetachTerminal(_) => Some(ShortcutId::DetachTerminal),
            Self::ExistingSessions(_) => Some(ShortcutId::ExistingTerminals),
            Self::Tab(_, TabAction::Close) => Some(ShortcutId::CloseTab),
            Self::Tab(id, action) if model.active_tab() == Some(id) => tab_shortcut(action),
            Self::MovePaneToWindow(pane) if model.active_pane() == Some(pane) => {
                Some(ShortcutId::MovePaneToNewWindow)
            }
            Self::FloatPane(pane) if model.active_pane() == Some(pane) => {
                Some(ShortcutId::FloatPane)
            }
            Self::OpenInBrowser(pane, None) if model.active_pane() == Some(pane) => {
                Some(ShortcutId::BrowserOpenExternally)
            }
            Self::BrowserInspect(pane) if model.active_pane() == Some(pane) => {
                Some(ShortcutId::BrowserInspect)
            }
            Self::CopyPath(id) if current == id => Some(ShortcutId::CopyProjectPath),
            Self::RevealPath(id) if current == id => Some(ShortcutId::RevealProject),
            Self::EditProject(id, Field::Name) if current == id => Some(ShortcutId::RenameProject),
            Self::RemoveProject(id) if current == id => Some(ShortcutId::RemoveProject),
            Self::ProjectLayouts(id) if current == id => Some(ShortcutId::ApplyLayout),
            Self::NewProjectTab(id) if current == id => Some(ShortcutId::NewTab),
            Self::Worktrees(id) if model.current_parent_project() == id => {
                Some(ShortcutId::ToggleWorktrees)
            }
            Self::FocusProject(None) => Some(ShortcutId::ToggleProjectFocus),
            Self::FocusProject(Some(id)) if model.current_parent_project() == id => {
                Some(ShortcutId::ToggleProjectFocus)
            }
            Self::NewWorkspace(None) => Some(ShortcutId::NewWorkspace),
            Self::CreateExtension => Some(ShortcutId::CreateExtension),
            Self::ManageExtensions => Some(ShortcutId::ManageExtensions),
            _ => None,
        };
        let chord = action.and_then(|action| model.settings.keymap.chord(action));
        let terminal_action = match self {
            Self::TerminalCopy(_) => Some(TerminalAction::Copy),
            Self::TerminalPaste(_) => Some(TerminalAction::Paste),
            Self::TerminalSelectAll(_) => Some(TerminalAction::SelectAll),
            _ => None,
        };
        let terminal = match self {
            Self::TerminalCopy(id)
            | Self::TerminalPaste(id)
            | Self::TerminalSelectAll(id)
            | Self::TerminalSelectCommandOutput(id)
            | Self::SplitPane(id, _)
            | Self::ToggleZoomPane(id)
            | Self::EqualizeSplits(id)
            | Self::ClosePane(id)
            | Self::DetachTerminal(id) => model.terminal(&id).map(|pane| pane.view.read(cx)),
            _ => None,
        };
        let chord = chord
            .filter(|chord| {
                terminal.is_none_or(|pane| {
                    pane.terminal
                        .keybindings
                        .bindings
                        .get(chord)
                        .is_none_or(|bound| terminal_action.as_ref() == Some(bound))
                })
            })
            .or_else(|| {
                let wanted = terminal_action.as_ref()?;
                let bindings = &terminal?.terminal.keybindings;
                bindings.chords_for_action(wanted).find(|chord| {
                    bindings.bindings.contains_key(chord) || !application_handles(chord, cx)
                })
            })?;
        Some(crate::views::command_palette::chord_label(chord))
    }
}

/// Whether an app shortcut takes `chord` before a terminal's default key
/// for it can run.
pub(crate) fn application_handles(chord: &KeyChord, cx: &App) -> bool {
    let Ok(key) = gpui::Keystroke::parse(chord.as_str()) else {
        return true;
    };
    let mut contexts = [
        gpui::KeyContext::new_with_defaults(),
        gpui::KeyContext::default(),
        gpui::KeyContext::default(),
    ];
    contexts[1].add("WorkspaceTabs");
    contexts[2].add("TerminalPane");
    let (bindings, pending) = cx
        .key_bindings()
        .borrow()
        .bindings_for_input(&[key], &contexts);
    !bindings.is_empty() || pending
}
