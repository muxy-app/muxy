use gpui::{AnyWindowHandle, AsyncApp, Context};
use muxy_app_core::{Project, ProjectId, ProjectStatus};
use muxy_ui::dialog::ConfirmationResponse;

use super::menu::{Command, Item};
use super::project_editor::Field;
use crate::model::AppModel;

pub(crate) fn items(project: &Project, worktrees_visible: bool) -> Vec<Item> {
    let id = project.id;
    let mut items = Vec::new();
    if project.status() == ProjectStatus::Available {
        items.push(Item::action("Set Logo…", Command::ProjectLogo(id)));
        if project.logo.is_some() {
            items.push(Item::action("Remove Logo", Command::RemoveProjectLogo(id)));
        }
        items.extend([
            Item::action("Rename…", Command::EditProject(id, Field::Name)),
            Item::action("Change Icon…", Command::EditProject(id, Field::Icon)),
            Item::action("Change Color ▸", Command::ProjectColor(id)),
            Item::action("Reveal in Finder", Command::RevealPath(id)),
            Item::action("Copy Path", Command::CopyPath(id)),
            Item::action("Existing Terminals…", Command::ExistingSessions(id)),
        ]);
    }
    if !project.home && project.status() == ProjectStatus::Available {
        if project.parent_id.is_none() {
            items.push(
                Item::action("Worktrees", Command::Worktrees(id)).checked_if(worktrees_visible),
            );
            items.push(Item::action("New Worktree…", Command::NewWorktree(id)));
        } else {
            items.push(Item::action(
                "Remove Worktree and Files…",
                Command::RemoveWorktree(id),
            ));
        }
    }
    if !project.home {
        items.push(Item::action("Remove Project…", Command::RemoveProject(id)));
    }
    items
}

pub(crate) fn worktree_items(project: &Project, primary: bool) -> Vec<Item> {
    if project.status() == ProjectStatus::Missing {
        return items(project, false);
    }
    let id = project.id;
    let mut items = vec![
        Item::action("New Terminal Tab", Command::NewProjectTab(id)),
        Item::action("Reveal in Finder", Command::RevealPath(id)),
        Item::action("Copy Path", Command::CopyPath(id)),
    ];
    if !primary {
        items.push(Item::action(
            "Rename Worktree…",
            Command::EditProject(id, Field::Name),
        ));
        items.push(Item::action(
            "Remove Worktree and Files…",
            Command::RemoveWorktree(id),
        ));
        items.push(Item::action("Remove Project…", Command::RemoveProject(id)));
    }
    items
}

impl AppModel {
    pub(crate) fn confirm_remove_project(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        let Some(record) = self.state.project(project).filter(|project| !project.home) else {
            return;
        };
        let message = format!(
            "Remove “{}” from every client? All of its terminal sessions will end, including those displayed in other clients, and their saved output will be discarded. The folder and its files will stay on disk.",
            record.name
        );
        self.dismiss_overlay(cx);
        let window = self.window;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = prompt(window, message, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.close_prompt = None;
                model.focus_requested = true;
                match response {
                    Ok(ConfirmationResponse::Confirmed { .. }) => {
                        model.remove_project_confirmed(project, cx);
                    }
                    Ok(ConfirmationResponse::Cancelled) => {}
                    Err(error) => {
                        model.fail(format!("Could not show removal confirmation: {error}"), cx);
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
    message: String,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, _| {
            muxy_ui::dialog::confirm(
                window,
                "Remove Project?",
                &message,
                "Remove",
                None,
                move |response| {
                    let _ = sender.try_send(response);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
async fn prompt(
    window: AnyWindowHandle,
    message: String,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                "Remove Project?",
                Some(&message),
                &["Remove", "Cancel"],
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
