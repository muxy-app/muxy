use std::collections::{BTreeMap, BTreeSet, HashSet};

use muxy_protocol::{OperationId, ProjectIntent, ServerIdentity, SessionId};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::{AppError, AppState, PaneContent, PaneId, Project, ProjectId, ServerId};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ServerState {
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
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) given_names: BTreeMap<SessionId, String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) dismissed: BTreeSet<SessionId>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) recent_paths: Vec<String>,
}

const RECENT_PATHS: usize = 100;

impl AppState {
    pub fn project_server(&self, project: ProjectId) -> Option<ServerId> {
        self.project(project).map(|project| project.server_id)
    }

    pub fn pane_server(&self, pane: PaneId) -> Option<ServerId> {
        self.placed_pane(pane).map(|placed| placed.server)
    }

    pub fn dismissed_sessions(&self, server: ServerId) -> BTreeSet<SessionId> {
        self.servers
            .get(&server)
            .map(|state| state.dismissed.clone())
            .unwrap_or_default()
    }

    pub fn dismiss_sessions(
        &mut self,
        server: ServerId,
        sessions: impl IntoIterator<Item = SessionId>,
    ) -> bool {
        let mut sessions = sessions.into_iter().peekable();
        if sessions.peek().is_none() {
            return false;
        }
        let dismissed = &mut self.servers.entry(server).or_default().dismissed;
        let before = dismissed.len();
        dismissed.extend(sessions);
        dismissed.len() != before
    }

    pub fn retain_dismissed(&mut self, server: ServerId, live: &HashSet<SessionId>) -> bool {
        let shown: HashSet<_> = self.session_references(server).into_iter().collect();
        let Some(state) = self.servers.get_mut(&server) else {
            return false;
        };
        let before = state.dismissed.len();
        state
            .dismissed
            .retain(|session| live.contains(session) && !shown.contains(session));
        state.dismissed.len() != before
    }

    pub fn recent_paths(&self, server: ServerId) -> &[String] {
        self.servers
            .get(&server)
            .map_or(&[], |state| state.recent_paths.as_slice())
    }

    pub fn remember_path(&mut self, server: ServerId, path: &str) -> bool {
        let recent = &mut self.servers.entry(server).or_default().recent_paths;
        if recent.first().is_some_and(|first| first == path) {
            return false;
        }
        recent.retain(|other| other != path);
        recent.insert(0, path.to_owned());
        recent.truncate(RECENT_PATHS);
        true
    }

    pub fn server_home(&self, server: ServerId) -> Option<&Project> {
        self.projects
            .iter()
            .find(|project| project.home && project.server_id == server)
    }

    pub fn session_references(&self, server: ServerId) -> Vec<SessionId> {
        self.panes()
            .filter(|placed| placed.server == server)
            .filter_map(|placed| placed.pane.session())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn session_names(&self, server: ServerId) -> BTreeMap<SessionId, String> {
        let mut names = BTreeMap::new();
        for placed in self.panes().filter(|placed| placed.server == server) {
            let Some(session) = placed.pane.session() else {
                continue;
            };
            let Some(title) = placed.tab.and_then(|tab| tab.custom_title.as_deref()) else {
                continue;
            };
            if let Some(name) = shared_name(title) {
                names.entry(session).or_insert(name);
            }
        }
        names
    }

    pub fn given_session_names(&self, server: ServerId) -> BTreeMap<SessionId, String> {
        self.servers
            .get(&server)
            .map(|state| state.given_names.clone())
            .unwrap_or_default()
    }

    pub fn record_session_name(
        &mut self,
        server: ServerId,
        session: SessionId,
        name: Option<String>,
    ) {
        let names = &mut self.servers.entry(server).or_default().given_names;
        match name {
            Some(name) => names.insert(session, name),
            None => names.remove(&session),
        };
    }

    pub fn session_references_by_server(&self) -> BTreeMap<ServerId, Vec<SessionId>> {
        let mut references: BTreeMap<_, BTreeSet<_>> = self
            .projects
            .iter()
            .map(|project| (project.server_id, BTreeSet::new()))
            .collect();
        for placed in self.panes() {
            if let Some(session) = placed.pane.session() {
                references.entry(placed.server).or_default().insert(session);
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

    pub(crate) fn remove_saved_devices_panes(&mut self) -> Result<(), AppError> {
        let panes: Vec<_> = self
            .panes()
            .filter(|placed| placed.pane.content == PaneContent::Servers)
            .map(|placed| placed.pane.id)
            .collect();
        for pane in panes {
            self.remove_pane(pane)?;
        }
        Ok(())
    }

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
        self.starting_proxies
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
}

fn shared_name(title: &str) -> Option<String> {
    let mut length = 0;
    for grapheme in title.graphemes(true) {
        if length + grapheme.len() > muxy_protocol::MAX_SESSION_NAME {
            break;
        }
        length += grapheme.len();
    }
    let name = title[..length].trim_end();
    muxy_protocol::validate_session_name(name)
        .is_ok()
        .then(|| name.to_owned())
}
