use std::collections::{BTreeMap, HashSet};

use muxy_protocol::CatalogPage;
use serde::{Deserialize, Serialize};

use crate::servers::ServerState;
use crate::{AppError, AppState, Project, ProjectId, ServerId, TabId, WorkspaceId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SetAsideView {
    server: ServerState,
    projects: Vec<Project>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    selected_tabs: BTreeMap<ProjectId, TabId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    workspaces: Vec<(WorkspaceId, ProjectId)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalCatalog {
    Applied,
    Fresh,
    Restored,
}

impl AppState {
    pub fn apply_local_catalog(
        &mut self,
        page: &CatalogPage,
        kept_catalog: bool,
    ) -> Result<LocalCatalog, AppError> {
        let local = ServerId::local();
        if self
            .server_identity(local)
            .is_none_or(|known| known == page.server)
        {
            self.apply_catalog(local, page)?;
            if !kept_catalog {
                self.set_aside = None;
            }
            return Ok(LocalCatalog::Applied);
        }
        let mut next = self.clone();
        let previous = next.take_local_view();
        let outcome = match next.set_aside.take() {
            Some(view) if view.server.identity == Some(page.server) => {
                next.put_back(view);
                next.remove_saved_devices_panes()?;
                LocalCatalog::Restored
            }
            _ => LocalCatalog::Fresh,
        };
        next.set_aside = kept_catalog.then_some(previous);
        next.apply_catalog(local, page)?;
        *self = next;
        Ok(outcome)
    }

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
