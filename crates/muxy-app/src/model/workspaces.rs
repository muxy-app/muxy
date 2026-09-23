//! User-defined workspaces that group projects and filter the sidebar.

use gpui::Context;
use muxy_app_core::{AppError, AppState, ProjectId, ProjectStatus, WorkspaceId};

use super::{AppModel, Quitting};

impl AppModel {
    /// Saves a workspace edit, then keeps the current project listed.
    pub(crate) fn edit_workspaces(
        &mut self,
        edit: impl FnOnce(&mut AppState) -> Result<(), AppError>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.quitting != Quitting::Idle {
            return false;
        }
        let previous = self.state.clone();
        if let Err(error) = edit(&mut self.state) {
            self.state = previous;
            self.fail(error.to_string(), cx);
            return false;
        }
        if !self.save(cx) {
            self.state = previous;
            return false;
        }
        self.select_listed_project(cx);
        cx.notify();
        true
    }

    pub(crate) fn create_workspace(
        &mut self,
        name: &str,
        project: Option<ProjectId>,
        cx: &mut Context<Self>,
    ) -> Option<WorkspaceId> {
        let mut created = None;
        let saved = self.edit_workspaces(
            |state| {
                let id = state.create_workspace(name)?;
                if let Some(project) = project {
                    state.set_workspace_member(id, project, true)?;
                }
                created = Some(id);
                Ok(())
            },
            cx,
        );
        created.filter(|_| saved)
    }

    pub(crate) fn select_workspace(
        &mut self,
        workspace: Option<WorkspaceId>,
        cx: &mut Context<Self>,
    ) {
        if self.appearance.sidebar_focus {
            self.appearance.sidebar_focus = false;
            self.save_appearance(cx);
        }
        self.edit_workspaces(|state| state.select_workspace(workspace), cx);
    }

    /// Opening a project while a workspace is active adds it there, like main.
    pub(crate) fn join_active_workspace(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if self.state.active_workspace().is_some() {
            self.edit_workspaces(
                |state| {
                    state.join_active_workspace(project);
                    Ok(())
                },
                cx,
            );
        }
    }

    fn select_listed_project(&mut self, cx: &mut Context<Self>) {
        if self.state.is_listed(self.state.current_project()) {
            return;
        }
        let target = self
            .listed_parents()
            .into_iter()
            .find(|project| !project.home && project.status() == ProjectStatus::Available)
            .map_or(self.state.home().id, |project| {
                self.preferred_worktree(project.id)
            });
        self.select_project(target, cx);
    }
}
