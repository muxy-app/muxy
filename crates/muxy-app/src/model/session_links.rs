use gpui::Context;
use muxy_app_core::session_link::SessionLink;
use muxy_app_core::{AppError, Direction, Pane, PaneId, ProjectId, ServerId, TabId};
use muxy_client::ClientError;
use muxy_protocol::{SessionId, SessionInfo};
use muxy_ui::tr;

use super::catalog::Opening;
use super::{AppModel, Work};

#[derive(Default)]
pub(crate) struct SessionLinks {
    waiting: Option<SessionLink>,
    locating: Option<(ServerId, SessionLink)>,
}

impl AppModel {
    pub(crate) fn open_session_link(&mut self, link: SessionLink, cx: &mut Context<Self>) {
        self.session_links = SessionLinks {
            waiting: Some(link),
            locating: None,
        };
        self.resume_session_link(cx);
    }

    pub(super) fn resume_session_link(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.session_links.waiting else {
            return;
        };
        let Some(server) = self.link_server(link) else {
            if !self.servers.local.catalog.identified {
                return;
            }
            self.session_links.waiting = None;
            self.fail(
                tr!("This link is for a server Muxy isn't connected to.").to_string(),
                cx,
            );
            return;
        };
        if !self.link_server_loaded(server) {
            self.reveal_server(server, cx);
            self.connect_on_demand(server, cx);
            return;
        }
        self.session_links.waiting = None;
        if self.reveal_session(server, link.session, cx) {
            return;
        }
        if self.send(server, Work::LocateSession(link.session), cx) {
            self.session_links.locating = Some((server, link));
        }
    }

    pub(super) fn forget_session_link(&mut self, server: ServerId) {
        if self
            .session_links
            .waiting
            .is_some_and(|link| !server.is_local() && self.link_server(link) == Some(server))
        {
            self.session_links.waiting = None;
        }
        if self
            .session_links
            .locating
            .is_some_and(|(owner, _)| owner == server)
        {
            self.session_links.locating = None;
        }
    }

    pub(super) fn receive_located_session(
        &mut self,
        server: ServerId,
        session: SessionId,
        result: Result<Option<SessionInfo>, ClientError>,
        cx: &mut Context<Self>,
    ) {
        let Some((_, link)) = self
            .session_links
            .locating
            .take_if(|(owner, link)| *owner == server && link.session == session)
        else {
            return;
        };
        match result {
            Ok(Some(info)) => self.show_linked_session(server, link, info.project, cx),
            Ok(None) => self.fail(
                tr!("Terminal %lld isn't running.", session.get()).to_string(),
                cx,
            ),
            Err(error) => self.fail(
                tr!("Could not open the terminal: %@", error.to_string()).to_string(),
                cx,
            ),
        }
    }

    pub(super) fn jump_to_pane(
        &mut self,
        project: ProjectId,
        tab: TabId,
        pane: PaneId,
        cx: &mut Context<Self>,
    ) {
        if self.raise_pane_window(pane, cx) {
            self.dismiss_overlay(cx);
            return;
        }
        self.select_project(project, cx);
        self.select_tab(tab, cx);
        self.focus_pane(pane, cx);
        self.dismiss_overlay(cx);
        self.focus_owner.request_active_pane();
        self.raise_window(cx);
    }

    fn reveal_session(
        &mut self,
        server: ServerId,
        session: SessionId,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some((project, tab, pane)) = self
            .session_pane(server, session)
            .map(|(project, tab, pane)| (project.id, tab, pane))
        {
            self.jump_to_pane(project, tab, pane, cx);
            return true;
        }
        if server.is_local() && self.state.quick_terminal().and_then(Pane::session) == Some(session)
        {
            if !self.quick.visible {
                self.toggle_quick_terminal(cx);
            }
            return true;
        }
        false
    }

    fn link_server(&self, link: SessionLink) -> Option<ServerId> {
        let Some(identity) = link.server else {
            return Some(ServerId::local());
        };
        self.servers
            .ids()
            .into_iter()
            .find(|server| self.state.server_identity(*server) == Some(identity))
    }

    fn link_server_loaded(&self, server: ServerId) -> bool {
        self.ready(server)
            && self.servers.get(server).is_some_and(|runtime| {
                runtime.catalog.identified && runtime.catalog.restore.is_none()
            })
    }

    fn show_linked_session(
        &mut self,
        server: ServerId,
        link: SessionLink,
        project: ProjectId,
        cx: &mut Context<Self>,
    ) {
        if self.reveal_session(server, link.session, cx) {
            return;
        }
        if self.state.project_server(project) != Some(server) {
            self.fail(
                tr!("The terminal's project isn't in Muxy yet. Try again in a moment.").to_string(),
                cx,
            );
            return;
        }
        let split = link
            .split
            .is_some_and(|edge| self.split_for_session(project, link.session, edge, cx));
        if split
            || self
                .open_session_tab(project, link.session, Opening::Selected, cx)
                .is_ok()
        {
            self.focus_owner.request_active_pane();
            self.raise_window(cx);
        }
    }

    fn split_for_session(
        &mut self,
        project: ProjectId,
        session: SessionId,
        edge: Direction,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(tab) = self.state.window().selected_tab.get(&project).copied() else {
            return false;
        };
        if self
            .state
            .project(project)
            .and_then(|project| project.tabs.iter().find(|entry| entry.id == tab))
            .is_none_or(|tab| tab.window.is_some())
        {
            return false;
        }
        self.reveal_server(
            self.state
                .project_server(project)
                .unwrap_or_else(ServerId::local),
            cx,
        );
        self.select_project(project, cx);
        self.select_tab(tab, cx);
        let Some(source) = self.active_pane().filter(|pane| {
            self.state.pane_window(*pane).is_none()
                && self
                    .tab(tab)
                    .is_some_and(|tab| tab.panes.iter().any(|entry| entry.id == *pane))
        }) else {
            return false;
        };
        let split = self.transact(
            |state| {
                let pane = state.split_pane(source, edge)?;
                state.set_pane_session(pane, Some(session))?;
                Ok::<_, AppError>(pane)
            },
            cx,
        );
        if matches!(split, Ok(Some(_))) {
            self.dismiss_overlay(cx);
            self.sync_visible(cx);
            true
        } else {
            false
        }
    }
}
