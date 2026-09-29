use muxy_app_core::{Color, Project, TabCloseScope, TabId, TabSide};

use super::menu::{Command, Item, color_items};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    New(TabSide),
    Rename,
    ResetTitle,
    /// Sets the tab's color to `PROJECT_COLORS[index]`.
    SetColor(usize),
    ResetColor,
    TogglePin,
    Close,
    CloseTabs(TabCloseScope),
}

pub(crate) fn items(project: &Project, id: TabId) -> Vec<Item> {
    let Some(tab) = project.tabs.iter().find(|tab| tab.id == id) else {
        return Vec::new();
    };
    let item = |label, action| Item::action(label, Command::Tab(id, action));
    let mut items = vec![
        item("New Tab to the Left", Action::New(TabSide::Left)),
        item("New Tab to the Right", Action::New(TabSide::Right)),
        item("Rename Tab", Action::Rename).separated(),
    ];
    if tab.custom_title.is_some() {
        items.push(item("Reset Title", Action::ResetTitle));
    }
    let mut palette = color_items(tab.color.as_ref().map(Color::as_str), |index| {
        Command::Tab(id, Action::SetColor(index))
    })
    .into_iter();
    let mut colors = vec![item("Default", Action::ResetColor).checked_if(tab.color.is_none())];
    colors.extend(palette.next().map(Item::separated));
    colors.extend(palette);
    items.push(Item::submenu("Color", colors));
    items.push(
        item(
            if tab.pinned { "Unpin Tab" } else { "Pin Tab" },
            Action::TogglePin,
        )
        .separated(),
    );
    if !tab.pinned {
        items.push(item("Close Tab", Action::Close).separated());
    }
    for (label, scope) in [
        ("Close Other Tabs", TabCloseScope::Other),
        ("Close Tabs to the Left", TabCloseScope::Left),
        ("Close Tabs to the Right", TabCloseScope::Right),
    ] {
        let action = item(label, Action::CloseTabs(scope));
        let action = if tab.pinned && scope == TabCloseScope::Other {
            action.separated()
        } else {
            action
        };
        items.push(if project.closable_tabs(id, scope).is_empty() {
            action.disabled()
        } else {
            action
        });
    }
    items
}
