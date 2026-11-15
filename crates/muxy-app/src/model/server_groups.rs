use gpui::Context;
use muxy_app_core::ServerId;

use super::AppModel;

impl AppModel {
    pub(crate) fn known_servers(&self) -> Vec<ServerId> {
        std::iter::once(ServerId::local())
            .chain(self.listed_servers())
            .collect()
    }

    pub(crate) fn sidebar_servers(&self) -> Vec<ServerId> {
        if self.appearance.sidebar_remote_servers {
            self.known_servers()
        } else {
            vec![ServerId::local()]
        }
    }

    pub(crate) fn server_expanded(&self, server: ServerId) -> bool {
        self.appearance.server_expanded(server)
    }

    pub(crate) fn toggle_server_group(&mut self, server: ServerId, cx: &mut Context<Self>) {
        let expanded = !self.server_expanded(server);
        self.set_server_expanded(server, expanded, cx);
        if expanded && self.state.server_home(server).is_none() {
            self.connect_on_demand(server, cx);
        }
    }

    pub(crate) fn reveal_server(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !self.server_expanded(server) {
            self.set_server_expanded(server, true, cx);
        }
    }

    pub(super) fn reveal_restored_server(&mut self, cx: &mut Context<Self>) {
        let server = self.state.current_project().server_id;
        if !self
            .appearance
            .sidebar_servers_expanded
            .contains_key(&server)
        {
            self.reveal_server(server, cx);
        }
    }

    fn set_server_expanded(&mut self, server: ServerId, expanded: bool, cx: &mut Context<Self>) {
        let known = self.known_servers();
        let groups = &mut self.appearance.sidebar_servers_expanded;
        groups.retain(|listed, _| known.contains(listed));
        groups.insert(server, expanded);
        self.save_appearance(cx);
        cx.notify();
    }
}
