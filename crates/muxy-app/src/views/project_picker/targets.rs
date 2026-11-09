use gpui::SharedString;
use muxy_app_core::{Project, ProjectId, ProjectStatus, ServerId};
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::tr;

use crate::model::AppModel;

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Project(ProjectId),
    /// Another computer's server, to browse its folders or open its Home.
    Server(ServerId),
}

/// Every server's projects, which New Tab in Project offers by name before
/// its folders, each marked with its server, and each other computer's
/// server to browse, before its projects.
pub(super) fn registry(model: &AppModel) -> Registry<Target> {
    let mut commands = Registry::default();
    for server in model.known_servers() {
        let badge = SharedString::from(model.server_label(server));
        let remote = !server.is_local();
        if remote && let Some(entry) = model.settings.server(server) {
            commands.register(
                Command::new(
                    format!("server-{server}"),
                    entry.name.clone(),
                    Target::Server(server),
                )
                .keywords(entry.destination().to_owned())
                .badge(tr!("Server")),
            );
        }
        for project in projects(model, server)
            .into_iter()
            .filter(|project| !(remote && project.home))
        {
            commands.register(project_command(
                project,
                model.project_title(project),
                &badge,
            ));
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
