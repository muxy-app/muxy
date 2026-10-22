//! The menus of remote servers, in the Remote popover and the Remote Servers
//! tab: one server, the servers selected in the tab, or a group.

use gpui::{Context, Pixels, Point, SharedString, Window};
use muxy_app_core::ServerId;
use muxy_ui::tr;

use super::menu::{Command, Item};
use super::overlays::Overlay;
use crate::model::{AppModel, RemoteServer, ServerStatus};

/// Which servers a menu row acts on.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Servers {
    One(ServerId),
    /// The servers selected in the Remote Servers tab.
    Selected,
    /// The group at an index of `Settings::server_groups`, or the ungrouped.
    Group(Option<usize>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    NewTab,
    AddProject,
    /// Connects the servers that are offline.
    Connect,
    Reconnect,
    Disconnect,
    Edit,
    /// Moves them to the group at an index of `Settings::server_groups`, or
    /// out of theirs.
    MoveTo(Option<usize>),
    NewGroup,
    RenameGroup,
    /// Ungroups a group's servers, after asking.
    DeleteGroup,
    Restart,
    Stop,
    Install,
    CopyUpdateCommand,
    Remove,
}

fn item(label: SharedString, servers: Servers, action: Action) -> Item {
    Item::action(label, Command::RemoteServers(servers, action))
}

/// It is connected or connecting.
pub(crate) fn live(server: &RemoteServer) -> bool {
    matches!(
        server.status,
        ServerStatus::Connected | ServerStatus::Connecting
    )
}

/// What connecting `members` means now: Connect for those offline, or
/// Reconnect when none are, so live connections stay up.
pub(crate) fn connect_choice(members: &[&RemoteServer]) -> (SharedString, Action) {
    let offline = members
        .iter()
        .filter(|server| server.status == ServerStatus::Disconnected)
        .count();
    let label = match (offline, members.len()) {
        (0, 1) => tr!("Reconnect"),
        (0, count) => tr!("Reconnect %lld Servers", count),
        (1, _) => tr!("Connect"),
        (count, _) => tr!("Connect %lld Servers", count),
    };
    let action = if offline == 0 {
        Action::Reconnect
    } else {
        Action::Connect
    };
    (label, action)
}

/// Connect or Reconnect, then Disconnect when any of `members` is live.
fn connection_items(servers: Servers, members: &[&RemoteServer]) -> Vec<Item> {
    let (label, action) = connect_choice(members);
    let mut items = vec![item(label, servers, action)];
    let live = members.iter().filter(|server| live(server)).count();
    if live > 0 {
        let label = if live == 1 {
            tr!("Disconnect")
        } else {
            tr!("Disconnect %lld Servers", live)
        };
        items.push(item(label, servers, Action::Disconnect));
    }
    items
}

fn remove_label(count: usize) -> SharedString {
    if count == 1 {
        tr!("Remove Server…")
    } else {
        tr!("Remove %lld Servers…", count)
    }
}

/// Each group, checked when it holds every one of `members`, then no group
/// and a new one.
fn move_items(groups: &[&str], servers: Servers, members: &[&RemoteServer]) -> Vec<Item> {
    let shared = |group: Option<&str>| {
        members
            .iter()
            .all(|server| server.entry.group.as_deref() == group)
    };
    let mut items: Vec<_> = groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            item(
                (*group).to_owned().into(),
                servers,
                Action::MoveTo(Some(index)),
            )
            .checked_if(shared(Some(group)))
        })
        .collect();
    let none = item(tr!("No Group"), servers, Action::MoveTo(None)).checked_if(shared(None));
    items.push(if items.is_empty() {
        none
    } else {
        none.separated()
    });
    items.push(item(tr!("New Group…"), servers, Action::NewGroup));
    items
}

/// One server's menu.
fn single_server_items(server: &RemoteServer, groups: &[&str], controls: bool) -> Vec<Item> {
    let servers = Servers::One(server.entry.id);
    let mut items = vec![
        item(tr!("New Tab"), servers, Action::NewTab),
        item(tr!("Add Project…"), servers, Action::AddProject),
    ];
    items.extend(
        connection_items(servers, &[server])
            .into_iter()
            .enumerate()
            .map(|(index, row)| if index == 0 { row.separated() } else { row }),
    );
    items.push(item(tr!("Edit…"), servers, Action::Edit));
    items.push(Item::submenu(
        tr!("Move to Group"),
        move_items(groups, servers, &[server]),
    ));
    if controls {
        items.push(item(tr!("Restart Server…"), servers, Action::Restart).separated());
        items.push(item(tr!("Stop Server…"), servers, Action::Stop));
    }
    if server.missing && crate::model::released_version().is_some() {
        items.push(item(tr!("Install Muxy…"), servers, Action::Install));
    }
    if server.incompatible && crate::model::released_version().is_some() {
        items.push(item(
            tr!("Copy Update Command"),
            servers,
            Action::CopyUpdateCommand,
        ));
    }
    items.push(item(tr!("Remove…"), servers, Action::Remove).separated());
    items
}

impl AppModel {
    /// Every remote server, in the order the popover and the tab list them.
    pub(crate) fn listed_servers(&self) -> Vec<ServerId> {
        crate::model::by_group(&self.settings.servers, |entry| entry)
            .into_iter()
            .flat_map(|(_, entries)| entries)
            .map(|entry| entry.id)
            .collect()
    }

    /// The servers a menu row acts on, in the listed order.
    pub(crate) fn menu_servers(&self, servers: Servers) -> Vec<ServerId> {
        let group = match servers {
            Servers::One(id) => return vec![id],
            Servers::Selected => {
                let mut listed = self.listed_servers();
                listed.retain(|id| self.remote_selection.contains(id));
                return listed;
            }
            Servers::Group(None) => None,
            Servers::Group(Some(index)) => match self.settings.server_groups().get(index) {
                Some(group) => Some((*group).to_owned()),
                None => return Vec::new(),
            },
        };
        let mut listed = self.listed_servers();
        listed.retain(|id| {
            self.settings
                .server(*id)
                .is_some_and(|entry| entry.group == group)
        });
        listed
    }

    /// The servers `servers` names, with their state, in the listed order.
    pub(crate) fn menu_members(&self, servers: Servers) -> Vec<RemoteServer> {
        let ids = self.menu_servers(servers);
        let mut members: Vec<_> = self
            .remote_servers()
            .into_iter()
            .filter(|server| ids.contains(&server.entry.id))
            .collect();
        members.sort_by_key(|server| ids.iter().position(|id| *id == server.entry.id));
        members
    }

    /// Opens `items`. Opened from the Remote popover, the popover stays shown
    /// under them and comes back when they close.
    fn open_server_items(
        &mut self,
        items: Vec<Item>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let over_popover = matches!(self.overlay, Some(Overlay::RemoteServers));
        self.open_menu(items, position, window, cx);
        if let Some(Overlay::Menu(menu)) = &mut self.overlay {
            menu.over_remote_servers = over_popover;
        }
    }

    /// A server's menu. With `selection`, a server among several selected in
    /// the tab opens the selection's menu. The selection never changes.
    pub(crate) fn open_server_menu(
        &mut self,
        id: ServerId,
        selection: bool,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let servers = if selection
            && self.remote_selection.len() > 1
            && self.remote_selection.contains(&id)
        {
            Servers::Selected
        } else {
            Servers::One(id)
        };
        let members = self.menu_members(servers);
        let members: Vec<_> = members.iter().collect();
        let groups = self.settings.server_groups();
        let items = match members.as_slice() {
            [] => return,
            [server] => single_server_items(server, &groups, self.control_enabled(server.entry.id)),
            several => {
                let mut items = connection_items(servers, several);
                items.push(Item::submenu(
                    tr!("Move to Group"),
                    move_items(&groups, servers, several),
                ));
                items.push(item(remove_label(several.len()), servers, Action::Remove).separated());
                items
            }
        };
        self.open_server_items(items, position, window, cx);
        cx.notify();
    }

    /// Move to Group's choices for `servers`, as a menu of its own.
    pub(crate) fn open_move_menu(
        &mut self,
        servers: Servers,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let members = self.menu_members(servers);
        if members.is_empty() {
            return;
        }
        let members: Vec<_> = members.iter().collect();
        let items = move_items(&self.settings.server_groups(), servers, &members);
        self.open_server_items(items, position, window, cx);
    }

    /// A group's menu; the ungrouped servers can't be renamed or deleted.
    pub(crate) fn open_group_menu(
        &mut self,
        group: Option<&str>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(servers) = self.group_servers(group) else {
            return;
        };
        let named = matches!(servers, Servers::Group(Some(_)));
        let members = self.menu_members(servers);
        let members: Vec<_> = members.iter().collect();
        let mut items = connection_items(servers, &members);
        if named {
            items.push(item(tr!("Rename Group…"), servers, Action::RenameGroup).separated());
            items.push(item(tr!("Delete Group…"), servers, Action::DeleteGroup));
        }
        items.push(item(remove_label(members.len()), servers, Action::Remove).separated());
        self.open_server_items(items, position, window, cx);
    }

    /// The servers of `group`, or of the ungrouped; `None` when no server
    /// is in that group.
    pub(crate) fn group_servers(&self, group: Option<&str>) -> Option<Servers> {
        match group {
            Some(group) => self
                .settings
                .server_groups()
                .iter()
                .position(|listed| *listed == group)
                .map(|index| Servers::Group(Some(index))),
            None => Some(Servers::Group(None)),
        }
    }

    /// Does what a row of those menus, or a button, says. Rows that manage
    /// the servers leave the popover open to show the result; the others
    /// close it.
    pub(crate) fn perform_server_menu(
        &mut self,
        servers: Servers,
        action: Action,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids = self.menu_servers(servers);
        let Some(&first) = ids.first() else {
            return;
        };
        let leaves = matches!(
            action,
            Action::NewTab
                | Action::AddProject
                | Action::Restart
                | Action::Stop
                | Action::Install
                | Action::CopyUpdateCommand
        );
        if leaves && matches!(self.overlay, Some(Overlay::RemoteServers)) {
            self.dismiss_overlay(cx);
        }
        let group = match servers {
            Servers::Group(Some(index)) => self
                .settings
                .server_groups()
                .get(index)
                .map(|group| (*group).to_owned()),
            _ => None,
        };
        match action {
            Action::NewTab => self.new_remote_home_tab(first, cx),
            Action::AddProject => self.open_remote_project_picker(first, cx),
            Action::Connect => {
                let offline: Vec<_> = ids
                    .into_iter()
                    .filter(|id| self.status_of(*id) == ServerStatus::Disconnected)
                    .collect();
                self.reconnect_remote_servers(&offline, cx);
            }
            Action::Reconnect => self.reconnect_remote_servers(&ids, cx),
            Action::Disconnect => self.disconnect_remote_servers(&ids, cx),
            Action::Edit => self.open_server_form(Some(first), cx),
            Action::MoveTo(index) => {
                let group = match index {
                    Some(index) => match self.settings.server_groups().get(index) {
                        Some(group) => Some((*group).to_owned()),
                        None => return,
                    },
                    None => None,
                };
                if let Err(error) = self.move_remote_servers(&ids, group.as_deref(), cx) {
                    self.fail(error, cx);
                }
            }
            Action::NewGroup => self.open_server_group_editor(servers, position, window, cx),
            Action::RenameGroup => {
                if let Servers::Group(Some(index)) = servers {
                    self.open_group_rename_editor(index, position, window, cx);
                }
            }
            Action::DeleteGroup => {
                if let Some(group) = group {
                    self.confirm_delete_server_group(ids, &group, cx);
                }
            }
            Action::Restart | Action::Stop => {
                let restart = matches!(action, Action::Restart);
                self.confirm_server_control(first, restart, window.window_handle(), cx);
            }
            Action::Install => self.confirm_install_server(first, window.window_handle(), cx),
            Action::CopyUpdateCommand => self.copy_update_command(cx),
            Action::Remove => self.confirm_forget_servers(ids, window.window_handle(), cx),
        }
    }

    /// Names a group, puts `servers` in it, and says whether that worked. A
    /// renamed group keeps its servers where they are.
    pub(crate) fn name_server_group(
        &mut self,
        servers: Servers,
        name: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if name.is_empty() {
            self.fail(tr!("Enter a group name").to_string(), cx);
            return false;
        }
        let ids = self.menu_servers(servers);
        let result = if matches!(servers, Servers::Group(Some(_))) {
            self.regroup_remote_servers(&ids, Some(name), cx)
        } else {
            self.move_remote_servers(&ids, Some(name), cx)
        };
        match result {
            Ok(()) => true,
            Err(error) => {
                self.fail(error, cx);
                false
            }
        }
    }
}
