use muxy_app_core::{Project, ProjectId, ProjectStatus, ServerId};
use muxy_ui::command_palette::{Command, Registry};

use crate::model::AppModel;

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Project(ProjectId),
}

/// The default server's projects, which New Tab in Project offers by name
/// before its folders.
pub(super) fn registry(model: &AppModel, server: ServerId) -> Registry<Target> {
    let mut commands = Registry::default();
    for project in projects(model, server) {
        commands.register(project_command(project, model.project_title(project)));
    }
    commands
}

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

fn project_command(project: &Project, title: String) -> Command<Target> {
    Command::new(project.id.to_string(), title, Target::Project(project.id))
        .keywords(project.directory.to_string_lossy())
        .disabled(project.status() == ProjectStatus::Missing)
}
