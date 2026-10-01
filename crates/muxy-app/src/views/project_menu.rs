use gpui::{AnyWindowHandle, AsyncApp, Context};
use muxy_app_core::{AppState, Project, ProjectId, ProjectStatus, WorkspaceId};
use muxy_ui::dialog::ConfirmationResponse;

use super::menu::{Command, Item, color_items};
use super::project_editor::Field;
use crate::model::{AppModel, git::Repository};

/// The menu of a top-level project. `worktrees` is `None` when its folder is not
/// a Git repository, and otherwise whether its worktrees are shown.
pub(crate) fn items(state: &AppState, project: &Project, worktrees: Option<bool>) -> Vec<Item> {
    let id = project.id;
    if project.status() != ProjectStatus::Available {
        return removal(project);
    }
    let mut items = vec![
        Item::action("New Terminal Tab", Command::NewProjectTab(id)),
        Item::action("Existing Terminals…", Command::ExistingSessions(id)),
        Item::action("Apply Layout…", Command::ProjectLayouts(id)),
        Item::action("Rename…", Command::EditProject(id, Field::Name)).separated(),
        Item::submenu("Icon", icon_items(project)),
        Item::submenu(
            "Color",
            color_items(Some(project.color.as_str()), |index| {
                Command::ProjectColor(id, index)
            }),
        ),
    ];
    if !project.home {
        items.push(Item::submenu("Workspaces", workspace_items(state, id)));
        if let Some(visible) = worktrees {
            items.push(Item::submenu("Worktrees", worktree_toggle_items(id, visible)).separated());
        }
    }
    items.push(Item::action("Reveal in Finder", Command::RevealPath(id)).separated());
    items.push(Item::action("Copy Path", Command::CopyPath(id)));
    items.extend(removal(project).into_iter().map(Item::separated));
    items
}

fn removal(project: &Project) -> Vec<Item> {
    if project.home {
        Vec::new()
    } else {
        vec![Item::action(
            "Remove Project…",
            Command::RemoveProject(project.id),
        )]
    }
}

fn icon_items(project: &Project) -> Vec<Item> {
    let id = project.id;
    let mut items = vec![
        Item::action("Choose Icon…", Command::EditProject(id, Field::Icon)),
        Item::action("Set Logo…", Command::ProjectLogo(id)),
    ];
    if project.logo.is_some() {
        items.push(Item::action("Remove Logo", Command::RemoveProjectLogo(id)));
    }
    items
}

fn worktree_toggle_items(id: ProjectId, visible: bool) -> Vec<Item> {
    let mut items =
        vec![Item::action("Show Worktrees", Command::Worktrees(id)).checked_if(visible)];
    if visible {
        items.push(Item::action("New Worktree…", Command::NewWorktree(id)).separated());
    }
    items
}

pub(crate) fn workspace_items(state: &AppState, project: ProjectId) -> Vec<Item> {
    let mut items: Vec<_> = state
        .workspaces()
        .iter()
        .map(|workspace| {
            Item::action(
                workspace.name.clone(),
                Command::ToggleWorkspaceMember(workspace.id, project),
            )
            .checked_if(workspace.projects.contains(&project))
        })
        .collect();
    let create = Item::action("New Workspace…", Command::NewWorkspace(Some(project)));
    items.push(if items.is_empty() {
        create
    } else {
        create.separated()
    });
    items
}

/// The menu of a row in a project's worktree list. `primary` is the project's own row.
pub(crate) fn worktree_items(project: &Project, primary: bool) -> Vec<Item> {
    let id = project.id;
    if project.status() != ProjectStatus::Available {
        return removal(project);
    }
    let mut items = vec![
        Item::action("New Terminal Tab", Command::NewProjectTab(id)),
        Item::action("Existing Terminals…", Command::ExistingSessions(id)),
        Item::action("Apply Layout…", Command::ProjectLayouts(id)),
    ];
    if !primary {
        items.push(
            Item::action("Rename Worktree…", Command::EditProject(id, Field::Name)).separated(),
        );
    }
    items.push(Item::action("Reveal in Finder", Command::RevealPath(id)).separated());
    items.push(Item::action("Copy Path", Command::CopyPath(id)));
    if !primary {
        items.push(
            Item::action("Remove Worktree and Files…", Command::RemoveWorktree(id)).separated(),
        );
        items.push(Item::action("Remove Project…", Command::RemoveProject(id)));
    }
    items
}

impl AppModel {
    /// The sidebar menu of `project`. Worktree projects get the worktree menu.
    pub(crate) fn project_menu(&self, project: &Project) -> Vec<Item> {
        if project.parent_id.is_some() {
            return worktree_items(project, false);
        }
        let repository = self
            .state
            .projects()
            .iter()
            .any(|child| child.parent_id == Some(project.id))
            || !self
                .git
                .projects
                .get(&project.id)
                .is_some_and(Repository::not_a_repository);
        items(
            &self.state,
            project,
            repository.then(|| self.worktrees_visible(project.id)),
        )
    }

    pub(crate) fn confirm_remove_project(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(record) = self.state.project(project).filter(|project| !project.home) else {
            return;
        };
        let message = format!(
            "Remove “{}” from every client? All of its terminal sessions will end, including those displayed in other clients, and their saved output will be discarded. The folder and its files will stay on disk.",
            record.name
        );
        self.confirm(
            "Remove Project?",
            message,
            "Remove",
            cx,
            move |model, cx| {
                model.remove_project_confirmed(project, cx);
            },
        );
    }

    pub(crate) fn confirm_delete_workspace(
        &mut self,
        workspace: WorkspaceId,
        cx: &mut Context<Self>,
    ) {
        let Some(record) = self.state.workspace(workspace) else {
            return;
        };
        let message = format!(
            "Delete “{}”? Its projects will not be removed.",
            record.name
        );
        self.confirm(
            "Delete Workspace?",
            message,
            "Delete",
            cx,
            move |model, cx| {
                model.edit_workspaces(|state| state.delete_workspace(workspace), cx);
            },
        );
    }

    pub(crate) fn confirm(
        &mut self,
        title: &'static str,
        message: String,
        action: &'static str,
        cx: &mut Context<Self>,
        confirmed: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = prompt(window, title, message, action, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.close_prompt = None;
                model.focus_requested = true;
                match response {
                    Ok(ConfirmationResponse::Confirmed { .. }) => confirmed(model, cx),
                    Ok(ConfirmationResponse::Cancelled) => {}
                    Err(error) => {
                        model.fail(format!("Could not show confirmation: {error}"), cx);
                    }
                }
                cx.notify();
            });
        }));
    }
}

#[cfg(not(test))]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: String,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, _| {
            muxy_ui::dialog::confirm(window, title, &message, action, None, move |response| {
                let _ = sender.try_send(response);
            })
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: String,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                title,
                Some(&message),
                &[action, "Cancel"],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(if answer.await == Ok(0) {
        ConfirmationResponse::Confirmed {
            dont_ask_again: false,
        }
    } else {
        ConfirmationResponse::Cancelled
    })
}
