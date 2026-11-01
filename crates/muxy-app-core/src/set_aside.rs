//! This computer's server can start with a fresh catalog, under a new
//! identity, when its saved one was lost or couldn't be read. The app then
//! takes the fresh catalog and keeps the previous view aside, so restoring the
//! kept catalog brings that view back.

use std::collections::{BTreeMap, HashSet};

use muxy_protocol::CatalogPage;
use serde::{Deserialize, Serialize};

use crate::servers::ServerState;
use crate::{AppError, AppState, Project, ProjectId, ServerId, TabId, WorkspaceId};

/// This computer's projects, with their tabs and splits, and what the app kept
/// for the server that held them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SetAsideView {
    server: ServerState,
    projects: Vec<Project>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    selected_tabs: BTreeMap<ProjectId, TabId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    workspaces: Vec<(WorkspaceId, ProjectId)>,
}

/// What a catalog from this computer's server did to the app's view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalCatalog {
    /// The server the app knew, or its first catalog.
    Applied,
    /// A server with a fresh catalog. The previous view was set aside.
    Fresh,
    /// The server whose view was set aside. That view is back.
    Restored,
}

impl AppState {
    /// Applies this computer's catalog. A server with another identity
    /// replaced its catalog: its projects replace the view, and the previous
    /// view is kept aside only while `kept_catalog` says the server still has
    /// a catalog file that could bring it back.
    pub fn apply_local_catalog(
        &mut self,
        page: &CatalogPage,
        kept_catalog: bool,
    ) -> Result<LocalCatalog, AppError> {
        let local = ServerId::local();
        let mut next = self.clone();
        let outcome = match next.server_identity(local) {
            Some(known) if known != page.server => {
                let previous = next.take_local_view();
                let outcome = match next.set_aside.take() {
                    Some(view) if view.server.identity == Some(page.server) => {
                        next.put_back(view);
                        LocalCatalog::Restored
                    }
                    _ => LocalCatalog::Fresh,
                };
                next.set_aside = Some(previous);
                outcome
            }
            _ => LocalCatalog::Applied,
        };
        if !kept_catalog {
            next.set_aside = None;
        }
        next.apply_catalog(local, page)?;
        *self = next;
        Ok(outcome)
    }

    /// Removes this computer's projects and server state, leaving no Home
    /// until the next catalog brings one.
    fn take_local_view(&mut self) -> SetAsideView {
        let local = ServerId::local();
        let (projects, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.projects)
            .into_iter()
            .partition(|project| project.server_id == local);
        self.projects = kept;
        let ids: HashSet<_> = projects.iter().map(|project| project.id).collect();
        let selected_tabs = ids
            .iter()
            .filter_map(|id| Some((*id, self.window.selected_tab.remove(id)?)))
            .collect();
        let workspaces = self
            .workspaces
            .iter()
            .flat_map(|workspace| {
                workspace
                    .projects
                    .iter()
                    .filter(|project| ids.contains(project))
                    .map(|project| (workspace.id, *project))
            })
            .collect();
        SetAsideView {
            server: self.servers.remove(&local).unwrap_or_default(),
            projects,
            selected_tabs,
            workspaces,
        }
    }

    /// Puts a set-aside view back in place of this computer's projects, which
    /// [`Self::take_local_view`] removed.
    fn put_back(&mut self, view: SetAsideView) {
        self.projects.splice(0..0, view.projects);
        self.window.selected_tab.extend(view.selected_tabs);
        for (workspace, project) in view.workspaces {
            if let Some(workspace) = self
                .workspaces
                .iter_mut()
                .find(|candidate| candidate.id == workspace)
            {
                workspace.projects.insert(project);
            }
        }
        self.servers.insert(ServerId::local(), view.server);
    }
}
