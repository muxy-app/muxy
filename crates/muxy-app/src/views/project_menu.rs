use gpui::{AsyncApp, Context};
use muxy_app_core::{AppState, Project, ProjectId, ProjectStatus, WorkspaceId};
use muxy_ui::dialog::{ConfirmationResponse, Parent, Question, ask};
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use super::menu::{Command, Item, color_items};
use super::project_editor::Field;
use crate::model::{AppModel, git::Repository};

/// The menu of a top-level project. `worktrees` is `None` when its folder is not
/// a Git repository, and otherwise whether its worktrees are shown.
pub(crate) fn items(
    state: &AppState,
    project: &Project,
    worktrees: Option<bool>,
    focus: Item,
) -> Vec<Item> {
    let id = project.id;
    if project.status() != ProjectStatus::Available {
        return removal(project);
    }
    let mut items = vec![
        Item::action(tr!("New Terminal Tab"), Command::NewProjectTab(id)),
        Item::action(tr!("Existing Terminals…"), Command::ExistingSessions(id)),
        Item::action(tr!("Apply Layout…"), Command::ProjectLayouts(id)),
        focus.separated(),
    ];
    let icon = Item::submenu(tr!("Icon"), icon_items(project));
    // Another computer's Home goes by its server's name.
    if project.is_remote_home() {
        items.push(icon.separated());
    } else {
        items.extend([
            Item::action(tr!("Rename…"), Command::EditProject(id, Field::Name)).separated(),
            icon,
        ]);
    }
    items.push(Item::submenu(
        tr!("Color"),
        color_items(Some(project.color.as_str()), |index| {
            Command::ProjectColor(id, index)
        }),
    ));
    if !project.home {
        items.push(Item::submenu(tr!("Workspaces"), workspace_items(state, id)));
        if let Some(visible) = worktrees {
            items.push(
                Item::submenu(tr!("Worktrees"), worktree_toggle_items(id, visible)).separated(),
            );
        }
    }
    items.extend(path_items(project));
    items.extend(removal(project).into_iter().map(Item::separated));
    items
}

/// A folder on another computer can't be revealed here, but its path can be copied.
fn path_items(project: &Project) -> Vec<Item> {
    let copy = Item::action(tr!("Copy Path"), Command::CopyPath(project.id));
    if project.server_id.is_local() {
        vec![
            Item::action(
                crate::opener::reveal_label(),
                Command::RevealPath(project.id),
            )
            .separated(),
            copy,
        ]
    } else {
        vec![copy.separated()]
    }
}

fn removal(project: &Project) -> Vec<Item> {
    if project.home {
        Vec::new()
    } else {
        vec![Item::action(
            tr!("Remove Project…"),
            Command::RemoveProject(project.id),
        )]
    }
}

fn icon_items(project: &Project) -> Vec<Item> {
    let id = project.id;
    let mut items = Vec::new();
    if muxy_ui::symbols::PROJECT_ICONS {
        items.push(Item::action(
            tr!("Choose Icon…"),
            Command::EditProject(id, Field::Icon),
        ));
    }
    items.push(Item::action(tr!("Set Logo…"), Command::ProjectLogo(id)));
    if project.logo.is_some() {
        items.push(Item::action(
            tr!("Remove Logo"),
            Command::RemoveProjectLogo(id),
        ));
    }
    items
}

fn worktree_toggle_items(id: ProjectId, visible: bool) -> Vec<Item> {
    let mut items =
        vec![Item::action(tr!("Show Worktrees"), Command::Worktrees(id)).checked_if(visible)];
    if visible {
        items.push(Item::action(tr!("New Worktree…"), Command::NewWorktree(id)).separated());
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
    let create = Item::action(tr!("New Workspace…"), Command::NewWorkspace(Some(project)));
    items.push(if items.is_empty() {
        create
    } else {
        create.separated()
    });
    items
}

/// The menu of a row in a project's worktree list. `primary` is the project's own row.
/// A worktree whose folder is gone can only be removed.
pub(crate) fn worktree_items(project: &Project, primary: bool) -> Vec<Item> {
    let id = project.id;
    if project.status() != ProjectStatus::Available {
        return if primary {
            Vec::new()
        } else {
            vec![Item::action(
                tr!("Remove Worktree…"),
                Command::RemoveProject(id),
            )]
        };
    }
    let mut items = vec![
        Item::action(tr!("New Terminal Tab"), Command::NewProjectTab(id)),
        Item::action(tr!("Existing Terminals…"), Command::ExistingSessions(id)),
        Item::action(tr!("Apply Layout…"), Command::ProjectLayouts(id)),
    ];
    if !primary {
        items.push(
            Item::action(
                tr!("Rename Worktree…"),
                Command::EditProject(id, Field::Name),
            )
            .separated(),
        );
    }
    items.extend(path_items(project));
    if !primary {
        items.push(
            Item::action(
                tr!("Remove Worktree and Files…"),
                Command::RemoveWorktree(id),
            )
            .separated(),
        );
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
        let focus = if self.appearance.sidebar_focus {
            Item::action(
                self.state.active_workspace().map_or_else(
                    || tr!("Show All Projects"),
                    |workspace| tr!("Show “%@”", &workspace.name),
                ),
                Command::FocusProject(None),
            )
        } else {
            Item::action(
                tr!("Focus Project"),
                Command::FocusProject(Some(project.id)),
            )
        };
        items(
            &self.state,
            project,
            repository.then(|| self.worktrees_visible(project.id)),
            focus,
        )
    }

    pub(crate) fn confirm_remove_project(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(record) = self.state.project(project).filter(|project| !project.home) else {
            return;
        };
        let gone = record.parent_id.is_some() && record.status() != ProjectStatus::Available;
        let message = if gone {
            tr!(
                "Remove “%@” from every client? Its folder is already gone. All of its terminal sessions will end, including those displayed in other clients, and their saved output will be discarded.",
                &record.name
            )
        } else {
            tr!(
                "Remove “%@” from every client? All of its terminal sessions will end, including those displayed in other clients, and their saved output will be discarded. The folder and its files will stay on disk.",
                &record.name
            )
        }
        .to_string();
        self.confirm(
            if gone {
                tr_key!("Remove Worktree?")
            } else {
                tr_key!("Remove Project?")
            },
            message,
            tr_key!("Remove"),
            cx,
            move |model, cx| {
                if gone {
                    model.leave_worktree(project, cx);
                }
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
        let message = tr!(
            "Delete “%@”? Its projects will not be removed.",
            &record.name
        )
        .to_string();
        self.confirm(
            tr_key!("Delete Workspace?"),
            message,
            tr_key!("Delete"),
            cx,
            move |model, cx| {
                model.edit_workspaces(|state| state.delete_workspace(workspace), cx);
            },
        );
    }

    /// `title` and `action` are English keys, shown translated; `message` is
    /// shown as is.
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
        let parent = self.dialog_parent(self.window);
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = prompt(&parent, title, message, action, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                model.focus_requested = true;
                match response {
                    Ok(ConfirmationResponse::Confirmed { .. }) => confirmed(model, cx),
                    Ok(ConfirmationResponse::Cancelled) => {}
                    Err(error) => {
                        model.fail(
                            tr!("Could not show confirmation: %@", error).to_string(),
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        }));
    }
}

async fn prompt(
    parent: &Parent,
    title: &'static str,
    message: String,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let question = Question::Confirm {
        title: &muxy_ui::l10n::translate(title),
        message: &message,
        confirm: &muxy_ui::l10n::translate(action),
        checkbox: None,
    };
    let answer = ask(parent, question, cx)
        .await
        .map_err(|error| error.to_string())?;
    Ok(answer.map_or(ConfirmationResponse::Cancelled, |_| {
        ConfirmationResponse::Confirmed {
            dont_ask_again: false,
        }
    }))
}
