//! Finding panes wherever they live: in a project's tabs, or in the Quick
//! Terminal, which belongs to no tab and runs on this computer.

use muxy_protocol::SessionId;

use crate::{AppState, Pane, PaneId, Project, ServerId, Tab, TabId};

/// A pane and where it lives. The Quick Terminal has no project or tab.
#[derive(Clone, Copy, Debug)]
pub struct PlacedPane<'a> {
    pub server: ServerId,
    pub project: Option<&'a Project>,
    pub tab: Option<&'a Tab>,
    pub pane: &'a Pane,
}

impl AppState {
    /// Every pane, with the Quick Terminal last.
    pub fn panes(&self) -> impl Iterator<Item = PlacedPane<'_>> {
        self.projects
            .iter()
            .flat_map(|project| {
                project.tabs.iter().flat_map(move |tab| {
                    tab.panes.iter().map(move |pane| PlacedPane {
                        server: project.server_id,
                        project: Some(project),
                        tab: Some(tab),
                        pane,
                    })
                })
            })
            .chain(self.quick_terminal.iter().map(|pane| PlacedPane {
                server: ServerId::local(),
                project: None,
                tab: None,
                pane,
            }))
    }

    pub fn placed_pane(&self, id: PaneId) -> Option<PlacedPane<'_>> {
        self.panes().find(|placed| placed.pane.id == id)
    }

    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.placed_pane(id).map(|placed| placed.pane)
    }

    /// The server and session a terminal pane shows.
    pub fn pane_session(&self, id: PaneId) -> Option<(ServerId, SessionId)> {
        let placed = self.placed_pane(id)?;
        Some((placed.server, placed.pane.session()?))
    }

    pub fn pane_tab(&self, id: PaneId) -> Option<TabId> {
        self.placed_pane(id)?.tab.map(|tab| tab.id)
    }

    /// The project a pane works in. The Quick Terminal works in Home.
    pub fn pane_project(&self, id: PaneId) -> Option<&Project> {
        let placed = self.placed_pane(id)?;
        Some(placed.project.unwrap_or_else(|| self.home()))
    }

    /// Panes that show `server`'s `session`.
    pub fn session_panes(
        &self,
        server: ServerId,
        session: SessionId,
    ) -> impl Iterator<Item = PaneId> + '_ {
        self.panes()
            .filter(move |placed| placed.server == server && placed.pane.session() == Some(session))
            .map(|placed| placed.pane.id)
    }
}
