use gpui::{Context, Pixels, Point, SharedString, Window};
use muxy_app_core::ServerId;
use muxy_ui::tr;

use super::menu::{Command, Item};
use super::overlays::Overlay;
use crate::model::{AppModel, RemoteServer, ServerStatus};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Servers {
    One(ServerId),
    Selected,
    Group(Option<usize>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    NewTab,
    AddProject,
    Connect,
    Reconnect,
    Disconnect,
    Edit,
    MoveTo(Option<usize>),
    NewGroup,
    RenameGroup,
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

pub(crate) fn live(server: &RemoteServer) -> bool {
    matches!(
        server.status,
        ServerStatus::Connected | ServerStatus::Connecting
    )
}

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
    if controls && server.entry.paired.is_none() {
        items.push(item(tr!("Restart Server…"), servers, Action::Restart).separated());
        items.push(item(tr!("Stop Server…"), servers, Action::Stop));
    } else if controls {
        items.push(item(tr!("Stop Server…"), servers, Action::Stop).separated());
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
    pub(crate) fn listed_servers(&self) -> Vec<ServerId> {
        crate::model::by_group(&self.settings.servers, |entry| entry)
            .into_iter()
            .flat_map(|(_, entries)| entries)
            .map(|entry| entry.id)
            .collect()
    }

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

    pub(crate) fn open_sidebar_server_menu(
        &mut self,
        server: ServerId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !server.is_local() {
            self.open_server_menu(server, false, position, window, cx);
            return;
        }
        let items = self.local_server_items();
        self.open_menu(items, position, window, cx);
        cx.notify();
    }

    fn local_server_items(&self) -> Vec<Item> {
        let server = ServerId::local();
        let servers = Servers::One(server);
        let mut items = Vec::new();
        if let Some(home) = self.state.server_home(server) {
            items.push(Item::action(
                tr!("New Tab"),
                Command::NewProjectTab(home.id),
            ));
        }
        items.push(Item::action(
            tr!("Add Project…"),
            Command::AddProject(server),
        ));
        let enabled = |item: Item, enabled: bool| if enabled { item } else { item.disabled() };
        if matches!(
            self.status_of(server),
            ServerStatus::Disconnected | ServerStatus::Connecting
        ) {
            items.push(
                enabled(
                    item(tr!("Connect"), servers, Action::Connect),
                    self.connect_enabled(server),
                )
                .separated(),
            );
        } else {
            let control = self.control_enabled(server);
            items.push(
                enabled(
                    item(tr!("Restart Server…"), servers, Action::Restart),
                    control,
                )
                .separated(),
            );
            items.push(enabled(
                item(tr!("Stop Server…"), servers, Action::Stop),
                control,
            ));
        }
        items
    }

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
            Action::Connect if first.is_local() => {
                if self.connect_enabled(first) {
                    self.connect_server(first, cx);
                }
            }
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
