//! Which server each project and pane belongs to, and what the app keeps per
//! server. Session IDs are counters per server, so session-keyed state lives
//! with its server.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use muxy_protocol::{OperationId, ProjectIntent, ServerIdentity, SessionId};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState, Pane, PaneContent, PaneId, Project, ProjectId, ServerId};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ServerState {
    /// The server that answered last time; another one is refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) identity: Option<ServerIdentity>,
    pub(crate) catalog_revision: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) project_intents: Vec<ProjectIntent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) pending_cancellations: Vec<OperationId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) pending_discards: Vec<SessionId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) close_operations: BTreeMap<SessionId, OperationId>,
}

impl AppState {
    pub fn project_server(&self, project: ProjectId) -> Option<ServerId> {
        self.project(project).map(|project| project.server_id)
    }

    /// The Quick Terminal always runs on this computer.
    pub fn pane_server(&self, pane: PaneId) -> Option<ServerId> {
        self.server_panes()
            .find(|(_, candidate)| candidate.id == pane)
            .map(|(server, _)| server)
    }

    /// A remote server has a Home once its catalog has been applied.
    pub fn server_home(&self, server: ServerId) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| project.home && project.server_id == server)
    }

    /// Sessions shown by `server`'s panes, sorted and without repeats.
    pub fn session_references(&self, server: ServerId) -> Vec<SessionId> {
        self.server_panes()
            .filter(|(owner, _)| *owner == server)
            .filter_map(|(_, pane)| terminal_session(pane))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Sessions shown by panes, for every server with projects here. A server
    /// whose panes show none has an empty list.
    pub fn session_references_by_server(&self) -> BTreeMap<ServerId, Vec<SessionId>> {
        let mut references: BTreeMap<_, BTreeSet<_>> = self
            .projects
            .iter()
            .map(|project| (project.server_id, BTreeSet::new()))
            .collect();
        for (server, pane) in self.server_panes() {
            if let Some(session) = terminal_session(pane) {
                references.entry(server).or_default().insert(session);
            }
        }
        references
            .into_iter()
            .map(|(server, sessions)| (server, sessions.into_iter().collect()))
            .collect()
    }

    pub fn pending_discards(&self, server: ServerId) -> &[SessionId] {
        self.servers
            .get(&server)
            .map_or(&[], |state| &state.pending_discards)
    }

    pub fn prepare_closes(&mut self, server: ServerId) {
        let Some(state) = self.servers.get_mut(&server) else {
            return;
        };
        state
            .close_operations
            .retain(|session, _| state.pending_discards.contains(session));
        for session in &state.pending_discards {
            state.close_operations.entry(*session).or_default();
        }
    }

    pub fn close_operation(&self, server: ServerId, session: SessionId) -> Option<OperationId> {
        self.servers
            .get(&server)?
            .close_operations
            .get(&session)
            .copied()
    }

    pub fn queue_discard(&mut self, server: ServerId, session: SessionId) {
        let state = self.servers.entry(server).or_default();
        state.close_operations.entry(session).or_default();
        if !state.pending_discards.contains(&session) {
            state.pending_discards.push(session);
        }
    }

    pub fn complete_discard(&mut self, server: ServerId, session: SessionId) {
        if let Some(state) = self.servers.get_mut(&server) {
            state.close_operations.remove(&session);
            state.pending_discards.retain(|pending| *pending != session);
        }
    }

    /// Removes `server`'s projects with their tabs and workspace memberships,
    /// and what the app kept for it. Nothing changes on the server.
    pub fn forget_server(&mut self, server: ServerId) -> Result<(), AppError> {
        if server.is_local() {
            return Err(AppError::InvalidState(
                "this computer's server cannot be forgotten".into(),
            ));
        }
        self.remove_remote_projects(server);
        self.servers.remove(&server);
        Ok(())
    }

    /// Removes a remote server's projects with their tabs, workspace
    /// memberships, and window references. The window moves to the local Home
    /// if it showed one of them.
    pub(crate) fn remove_remote_projects(&mut self, server: ServerId) {
        if server.is_local() {
            return;
        }
        let (removed, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.projects)
            .into_iter()
            .partition(|project| project.server_id == server);
        self.projects = kept;
        let projects: HashSet<_> = removed.iter().map(|project| project.id).collect();
        let panes: HashSet<_> = removed
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .map(|pane| pane.id)
            .collect();
        self.window
            .selected_tab
            .retain(|project, _| !projects.contains(project));
        self.window
            .focus_history
            .retain(|pane| !panes.contains(pane));
        self.startup_commands
            .retain(|pane, _| !panes.contains(pane));
        self.starting_directories
            .retain(|pane, _| !panes.contains(pane));
        self.retain_workspace_members();
        if projects.contains(&self.window.current_project) {
            self.window.current_project = self.home().id;
            self.window.active_pane = None;
            self.focus_selected_tab();
        }
    }

    pub(crate) fn discard_unreferenced(&mut self, server: ServerId, session: SessionId) {
        if !self.session_references(server).contains(&session) {
            self.queue_discard(server, session);
        }
    }

    fn server_panes(&self) -> impl Iterator<Item = (ServerId, &Pane)> {
        self.projects
            .iter()
            .flat_map(|project| {
                project
                    .tabs
                    .iter()
                    .flat_map(|tab| &tab.panes)
                    .map(|pane| (project.server_id, pane))
            })
            .chain(
                self.quick_terminal
                    .iter()
                    .map(|pane| (ServerId::local(), pane)),
            )
    }
}

fn terminal_session(pane: &Pane) -> Option<SessionId> {
    match pane.content {
        PaneContent::Terminal { session } => session,
        PaneContent::Settings | PaneContent::Webview(_) => None,
    }
}
