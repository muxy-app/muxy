use gpui::SharedString;
use muxy_app_core::{Color, Project, ProjectId, TabCloseScope, TabId, TabSide};

use muxy_ui::tr;

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
    MoveToWindow,
    MoveToProject(ProjectId),
    ReturnToMainWindow,
}

/// The menu that moves a tab or pane into one of `destinations`, or nothing
/// when there is nowhere to go.
pub(crate) fn move_to_project(
    destinations: &[(ProjectId, SharedString)],
    command: impl Fn(ProjectId) -> Command,
) -> Option<Item> {
    if destinations.is_empty() {
        return None;
    }
    Some(Item::submenu(
        tr!("Move to Project"),
        destinations
            .iter()
            .map(|(project, name)| Item::action(name.clone(), command(*project)))
            .collect(),
    ))
}

pub(crate) fn items(
    project: &Project,
    id: TabId,
    destinations: &[(ProjectId, SharedString)],
) -> Vec<Item> {
    let Some(tab) = project.tabs.iter().find(|tab| tab.id == id) else {
        return Vec::new();
    };
    let item = |label: SharedString, action| Item::action(label, Command::Tab(id, action));
    if tab.detached() {
        let mut items = vec![item(tr!("Move to Main Window"), Action::ReturnToMainWindow)];
        items.extend(move_to_project(destinations, |project| {
            Command::Tab(id, Action::MoveToProject(project))
        }));
        items.push(item(tr!("Rename Tab"), Action::Rename).separated());
        items.push(item(tr!("Close Tab"), Action::Close).separated());
        return items;
    }
    let mut items = vec![
        item(tr!("New Tab to the Left"), Action::New(TabSide::Left)),
        item(tr!("New Tab to the Right"), Action::New(TabSide::Right)),
        item(tr!("Move to New Window"), Action::MoveToWindow).separated(),
    ];
    items.extend(move_to_project(destinations, |project| {
        Command::Tab(id, Action::MoveToProject(project))
    }));
    items.push(item(tr!("Rename Tab"), Action::Rename).separated());
    if tab.custom_title.is_some() {
        items.push(item(tr!("Reset Title"), Action::ResetTitle));
    }
    let mut palette = color_items(tab.color.as_ref().map(Color::as_str), |index| {
        Command::Tab(id, Action::SetColor(index))
    })
    .into_iter();
    let mut colors = vec![item(tr!("Default"), Action::ResetColor).checked_if(tab.color.is_none())];
    colors.extend(palette.next().map(Item::separated));
    colors.extend(palette);
    items.push(Item::submenu(tr!("Color"), colors));
    items.push(
        item(
            if tab.pinned {
                tr!("Unpin Tab")
            } else {
                tr!("Pin Tab")
            },
            Action::TogglePin,
        )
        .separated(),
    );
    if !tab.pinned {
        items.push(item(tr!("Close Tab"), Action::Close).separated());
    }
    for (label, scope) in [
        (tr!("Close Other Tabs"), TabCloseScope::Other),
        (tr!("Close Tabs to the Left"), TabCloseScope::Left),
        (tr!("Close Tabs to the Right"), TabCloseScope::Right),
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
