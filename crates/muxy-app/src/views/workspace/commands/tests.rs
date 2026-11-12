use std::collections::HashSet;

use muxy_core::shortcuts::{ALL, WORKSPACE_CLIPBOARD_CONTEXT};

use super::{COMMANDS, Label, Palette};

/// Shortcuts the workspace binds outside the table: one action with an
/// index, or an action a text field shares.
fn bound_outside_the_table(id: &str) -> bool {
    id.starts_with("select_tab")
        || id.starts_with("select_project")
        || ["copy", "paste"].contains(&id)
}

#[test]
fn every_command_has_one_id_and_a_shortcut_slot() {
    let mut ids = HashSet::new();
    let mut shortcuts = HashSet::new();
    for row in COMMANDS {
        assert!(
            ids.insert(row.palette_id),
            "{} is listed twice",
            row.palette_id
        );
        let Some(shortcut) = row.shortcut else {
            assert_eq!(
                row.palette_id, "open_about",
                "{} can't be bound",
                row.palette_id
            );
            continue;
        };
        assert_eq!(row.palette_id, shortcut.name());
        assert!(
            shortcuts.insert(shortcut.name()),
            "{} is bound twice",
            row.palette_id
        );
    }
}

#[test]
fn every_command_the_palette_or_a_menu_shows_has_a_title() {
    for row in COMMANDS {
        let untitled = matches!(row.label, Label::None);
        assert!(
            !untitled || row.palette == Palette::Hidden,
            "{} has no title",
            row.palette_id
        );
    }
}

#[test]
fn every_workspace_shortcut_starts_as_a_command() {
    let scopes = [
        None,
        Some("WorkspaceTabs"),
        Some("Browser"),
        Some(WORKSPACE_CLIPBOARD_CONTEXT),
    ];
    for shortcut in ALL {
        if !shortcut
            .contexts
            .iter()
            .any(|context| scopes.contains(context))
            || bound_outside_the_table(shortcut.id)
        {
            continue;
        }
        assert!(
            COMMANDS
                .iter()
                .any(|row| row.shortcut.is_some_and(|id| id.name() == shortcut.id)),
            "{} has a shortcut but no command",
            shortcut.id
        );
    }
}
