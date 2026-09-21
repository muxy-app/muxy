use gpui::App;
use muxy_app_core::{
    Direction,
    settings::{KeyChord, TerminalAction},
};
use muxy_core::shortcuts::ShortcutId;

use super::Command;
use crate::model::AppModel;

impl Command {
    pub(crate) fn shortcut(self, model: &AppModel, cx: &App) -> Option<String> {
        let action = match self {
            Self::TerminalCopy(_) => Some(ShortcutId::Copy),
            Self::TerminalPaste(_) => Some(ShortcutId::Paste),
            Self::TerminalSelectCommandOutput(_) => Some(ShortcutId::SelectCommandOutput),
            Self::SplitPane(_, Direction::Right) => Some(ShortcutId::SplitRight),
            Self::SplitPane(_, Direction::Down) => Some(ShortcutId::SplitDown),
            Self::ToggleZoomPane(_) => Some(ShortcutId::ToggleZoomPane),
            Self::ClosePane(_) => Some(ShortcutId::ClosePane),
            Self::DetachTerminal(_) => Some(ShortcutId::DetachTerminal),
            Self::ExistingSessions(_) => Some(ShortcutId::ExistingTerminals),
            Self::Tab(_, super::super::tab_menu::Action::Close) => Some(ShortcutId::CloseTab),
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
        Some(
            gpui::Keystroke::parse(chord.as_str())
                .map_or_else(|_| chord.to_string(), |key| key.to_string()),
        )
    }
}

fn application_handles(chord: &KeyChord, cx: &App) -> bool {
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
