use gpui::SharedString;
use muxy_app_core::{Project, ProjectId, ProjectStatus, ServerId};
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::tr;

use crate::model::AppModel;

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Project(ProjectId),
    Server(ServerId),
}

pub(super) fn registry(model: &AppModel) -> Registry<Target> {
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
