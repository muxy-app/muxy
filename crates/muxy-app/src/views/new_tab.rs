//! New Tab in Project: every top-level project, this computer's then other
//! computers', where a new terminal tab can open.

use std::rc::Rc;

use gpui::{Context, SharedString, Window};
use muxy_app_core::settings::ProjectOrder;
use muxy_app_core::{Project, ProjectStatus, ServerId};
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::tr;

use super::command_palette::Handler;
use super::overlays::Overlay;
use crate::model::AppModel;

pub(crate) const PAGE_ID: &str = "new_tab_in_project";

/// Local projects, then each remote server, by its name, and its projects.
pub(crate) fn command(cx: &Context<AppModel>) -> Command<Handler> {
    let model = cx.weak_entity();
    Command::list(PAGE_ID, tr!("New Tab in Project…"), move |cx| {
        let mut commands = Registry::default();
        let Some(model) = model.upgrade() else {
            return commands;
        };
        let model = model.read(cx);
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
            let home: Handler = Rc::new(move |model, _, cx| model.new_remote_home_tab(server, cx));
            commands.register(
                Command::new(format!("server-{server}"), entry.name.clone(), home)
                    .keywords(entry.ssh.clone())
                    .badge(remote.clone()),
            );
            for project in projects(model, server).into_iter().filter(|p| !p.home) {
                let title = format!("{} · {}", entry.name, project.name);
                commands.register(project_command(project, title, &remote));
            }
        }
        commands
    })
    .keywords("New Tab in Project")
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
    if model.appearance.sidebar_project_order == ProjectOrder::Name {
        projects.sort_by_cached_key(|project| (!project.home, project.name.to_lowercase()));
    }
    projects
}

fn project_command(project: &Project, title: String, badge: &SharedString) -> Command<Handler> {
    let id = project.id;
    let handler: Handler = Rc::new(move |model, _, cx| model.new_tab_in(id, cx));
    Command::new(id.to_string(), title, handler)
        .keywords(project.directory.to_string_lossy())
        .disabled(project.status() == ProjectStatus::Missing)
        .badge(badge.clone())
}

impl AppModel {
    /// Opens New Tab in Project, or closes it when it is open.
    pub(crate) fn toggle_new_tab_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(Overlay::Commands { palette, .. }) = &self.overlay
            && palette.read(cx).is_page(PAGE_ID)
        {
            self.dismiss_overlay(cx);
            return;
        }
        let mut registry = Registry::default();
        registry.register(command(cx));
        self.show_palette(registry, Some(PAGE_ID), window, cx);
    }
}
