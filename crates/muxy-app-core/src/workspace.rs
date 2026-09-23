use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::{AppError, AppState, Project, ProjectId, WorkspaceId};

/// A client-owned group of top-level projects; groups may overlap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    #[serde(default)]
    pub projects: BTreeSet<ProjectId>,
}

impl AppState {
    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn workspace(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|workspace| workspace.id == id)
    }

    /// The workspace filtering the sidebar; `None` lists every project.
    pub fn active_workspace(&self) -> Option<&Workspace> {
        self.window.workspace.and_then(|id| self.workspace(id))
    }

    /// Whether the active workspace lists `project`. Home is always listed and
    /// worktrees follow their parent.
    pub fn is_listed(&self, project: &Project) -> bool {
        let root = project.parent_id.unwrap_or(project.id);
        self.active_workspace()
            .is_none_or(|workspace| root == self.home().id || workspace.projects.contains(&root))
    }

    pub fn create_workspace(&mut self, name: &str) -> Result<WorkspaceId, AppError> {
        let workspace = Workspace {
            id: WorkspaceId::new(),
            name: workspace_name(name)?,
            projects: BTreeSet::new(),
        };
        let id = workspace.id;
        self.workspaces.push(workspace);
        Ok(id)
    }

    pub fn rename_workspace(&mut self, id: WorkspaceId, name: &str) -> Result<(), AppError> {
        let name = workspace_name(name)?;
        self.workspace_mut(id)?.name = name;
        Ok(())
    }

    /// Removes the group only; its projects stay.
    pub fn delete_workspace(&mut self, id: WorkspaceId) -> Result<(), AppError> {
        self.workspace_mut(id)?;
        self.workspaces.retain(|workspace| workspace.id != id);
        if self.window.workspace == Some(id) {
            self.window.workspace = None;
        }
        Ok(())
    }

    pub fn select_workspace(&mut self, id: Option<WorkspaceId>) -> Result<(), AppError> {
        if let Some(id) = id {
            self.workspace_mut(id)?;
        }
        self.window.workspace = id;
        Ok(())
    }

    pub fn set_workspace_member(
        &mut self,
        id: WorkspaceId,
        project: ProjectId,
        member: bool,
    ) -> Result<(), AppError> {
        let candidate = self
            .project(project)
            .ok_or(AppError::UnknownProject(project))?;
        if candidate.home || candidate.parent_id.is_some() {
            return Err(AppError::InvalidState(
                "only top-level projects other than Home can join a workspace".into(),
            ));
        }
        let projects = &mut self.workspace_mut(id)?.projects;
        if member {
            projects.insert(project);
        } else {
            projects.remove(&project);
        }
        Ok(())
    }

    /// Adds an opened project, or a worktree's parent, to the active workspace;
    /// Home is skipped.
    pub fn join_active_workspace(&mut self, project: ProjectId) {
        let root = self
            .project(project)
            .map_or(project, |project| project.parent_id.unwrap_or(project.id));
        if let Some(id) = self.window.workspace {
            let _ = self.set_workspace_member(id, root, true);
        }
    }

    /// Keeps the current project listed by switching to a workspace that
    /// contains it, or to all projects.
    pub fn reveal_current_project(&mut self) {
        let current = self.current_project();
        if self.is_listed(current) {
            return;
        }
        let root = current.parent_id.unwrap_or(current.id);
        self.window.workspace = self
            .workspaces
            .iter()
            .find(|workspace| workspace.projects.contains(&root))
            .map(|workspace| workspace.id);
    }

    pub(crate) fn retain_workspace_members(&mut self) {
        let members: HashSet<_> = self
            .projects
            .iter()
            .filter(|project| !project.home && project.parent_id.is_none())
            .map(|project| project.id)
            .collect();
        for workspace in &mut self.workspaces {
            workspace
                .projects
                .retain(|project| members.contains(project));
        }
        if self.active_workspace().is_none() {
            self.window.workspace = None;
        }
    }

    fn workspace_mut(&mut self, id: WorkspaceId) -> Result<&mut Workspace, AppError> {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
            .ok_or_else(|| AppError::InvalidState("unknown workspace".into()))
    }
}

fn workspace_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidState(
            "workspace name cannot be empty".into(),
        ));
    }
    Ok(name.to_owned())
}
