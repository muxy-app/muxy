//! New Tab in Project: every top-level project, this computer's then other
//! computers', or a local folder found by the project picker.

use std::rc::Rc;

use gpui::{Context, SharedString, Window};
use muxy_app_core::{Project, ProjectId, ProjectStatus, ServerId};
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::tr;

use super::command_palette::Handler;
use super::overlays::Overlay;
use crate::model::AppModel;

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Project(ProjectId),
    Server(ServerId),
}

pub(crate) fn command() -> Command<Handler> {
    let handler: Handler = Rc::new(AppModel::toggle_new_tab_list);
    Command::new("new_tab_in_project", tr!("New Tab in Project…"), handler)
        .keywords("New Tab in Project")
}

/// Local projects, then each remote server, by its name, and its projects.
fn targets(model: &AppModel) -> Registry<Target> {
    let mut commands = Registry::default();
    let local = tr!("Local");
    for project in projects(model, ServerId::local()) {
        commands.register(project_command(
            project,
            model.project_title(project),
            &local,
        ));
    }
    let remote = tr!("Remote");
    for entry in &model.settings.servers {
        let server = entry.id;
        commands.register(
            Command::new(
                format!("server-{server}"),
                entry.name.clone(),
                Target::Server(server),
            )
            .keywords(entry.destination().to_owned())
            .badge(remote.clone()),
        );
        for project in projects(model, server).into_iter().filter(|p| !p.home) {
            let title = format!("{} · {}", entry.name, project.name);
            commands.register(project_command(project, title, &remote));
        }
    }
    commands
}

/// `server`'s top-level projects in the sidebar's order, Home first.
fn projects(model: &AppModel, server: ServerId) -> Vec<&Project> {
    let mut projects: Vec<_> = model
        .state
        .projects()
        .iter()
        .filter(|project| {
            project.server_id == server
                && project.parent_id.is_none()
                && model.project_shown(project)
        })
        .collect();
    model.sort_projects(&mut projects);
    projects
}

fn project_command(project: &Project, title: String, badge: &SharedString) -> Command<Target> {
    Command::new(project.id.to_string(), title, Target::Project(project.id))
        .keywords(project.directory.to_string_lossy())
        .disabled(project.status() == ProjectStatus::Missing)
        .badge(badge.clone())
}

impl AppModel {
    /// Opens New Tab in Project, or closes it when it is open.
    pub(crate) fn toggle_new_tab_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if let Some(Overlay::Projects(picker)) = &self.overlay
            && picker.read(cx).opens_terminal()
        {
            self.dismiss_overlay(cx);
            return;
        }
        let targets = targets(self);
        self.open_project_picker(window, cx);
        if let Some(Overlay::Projects(picker)) = &self.overlay {
            picker.update(cx, |picker, cx| {
                picker.set_terminal_targets(targets, &self.settings.keymap, cx);
            });
        }
    }
}
