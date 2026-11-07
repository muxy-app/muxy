use std::path::Path;

use gpui::{Context, Pixels, Point, Window};
use muxy_app_core::{
    AppError, AppState, PaneId, ProjectId, ProjectStatus, Tab, TabCloseScope, TabId, TabSide,
};

use super::{AppModel, ConnectionState, Quitting};
use crate::views::tab_strip::TabDrop;

/// A terminal tab to open, and what its terminal starts with.
pub(crate) struct OpenTerminal<'a> {
    pub(crate) project: ProjectId,
    pub(crate) title: Option<&'a str>,
    pub(crate) command: Option<&'a str>,
    pub(crate) directory: Option<&'a Path>,
}

impl AppModel {
    /// Opens and selects a terminal tab, leaving the state as it was if any
    /// of it fails. `Ok(None)` means saving failed, and that has been reported.
    pub(crate) fn open_terminal(
        &mut self,
        OpenTerminal {
            project,
            title,
            command,
            directory,
        }: OpenTerminal<'_>,
        cx: &mut Context<Self>,
    ) -> Result<Option<(TabId, PaneId)>, String> {
        let opened = self.transact(
            |state| {
                let tab = state
                    .open_terminal_tab(project)
                    .map_err(|error| error.to_string())?;
                let pane = state
                    .window()
                    .active_pane
                    .ok_or_else(|| "could not create tab".to_owned())?;
                if let Some(title) = title {
                    state
                        .set_tab_title(tab, Some(title.into()))
                        .map_err(|error| error.to_string())?;
                }
                state.plan_creation(pane, directory, None);
                if let Some(command) = command {
                    state
                        .set_startup_command(pane, command)
                        .map_err(|error| error.to_string())?;
                }
                Ok::<_, String>((tab, pane))
            },
            cx,
        )?;
        if opened.is_some() {
            self.changed(cx);
            self.focus_owner.request_active_pane();
            let server = self.state.current_project().server_id;
            if self.connection(server) == ConnectionState::Disconnected {
                self.connect_server(server, cx);
            }
        }
        Ok(opened)
    }

    pub(crate) fn apply_layout_drop(
        &mut self,
        intent: crate::views::splits::drag::DropIntent,
        cx: &mut Context<Self>,
    ) {
        if self.close_request.is_some() {
            return;
        }
        if self.edit_tab(
            |state| match intent.target {
                crate::views::splits::drag::DropTarget::Swap(target) => {
                    state.move_pane(intent.source, target, None)
                }
                crate::views::splits::drag::DropTarget::Dock { pane, edge, level } => {
                    state.dock_pane(intent.source, pane, edge, level)
                }
            },
            cx,
        ) {
            self.changed(cx);
            self.focus_owner.request_active_pane();
        }
    }

    /// Applies a tab drop. `previous` was shown before pressing `tab` selected
    /// it, so the group `tab` leaves goes back to showing it.
    pub(crate) fn drop_tab(
        &mut self,
        tab: TabId,
        drop: TabDrop,
        previous: Option<TabId>,
        cx: &mut Context<Self>,
    ) {
        if self.close_request.is_some() {
            return;
        }
        let Some(project) = self.tab_project(tab).map(|project| project.id) else {
            return;
        };
        if self.edit_tab(
            |state| {
                match drop {
                    TabDrop::Split { beside, edge } => {
                        state.split_tab(tab, beside, edge)?;
                    }
                    TabDrop::Into { group, before } => {
                        state.move_tab_to_group(tab, group, before)?;
                    }
                }
                if let Some(previous) = previous.filter(|previous| {
                    state
                        .project(project)
                        .is_some_and(|project| !project.group_tabs(tab).contains(previous))
                }) {
                    state.select_tab(project, previous)?;
                    state.select_tab(project, tab)?;
                }
                Ok(())
            },
            cx,
        ) {
            self.changed(cx);
            self.focus_owner.request_active_pane();
        }
    }

    pub(crate) fn tab(&self, id: TabId) -> Option<&Tab> {
        self.tab_project(id)?.tabs.iter().find(|tab| tab.id == id)
    }

    pub(crate) fn edit_tab(
        &mut self,
        edit: impl FnOnce(&mut AppState) -> Result<(), AppError>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.quitting != Quitting::Idle {
            return false;
        }
        match self.transact(edit, cx) {
            Ok(Some(())) => {}
            Ok(None) => return false,
            Err(error) => {
                self.fail(error.to_string(), cx);
                return false;
            }
        }
        cx.notify();
        true
    }

    pub(crate) fn new_tab_adjacent(
        &mut self,
        anchor: TabId,
        side: TabSide,
        cx: &mut Context<Self>,
    ) {
        let Some((project, project_server)) = self
            .tab_project(anchor)
            .map(|project| (project.id, project.server_id))
        else {
            return;
        };
        let directory = self.new_tab_directory(project, cx);
        if self.edit_tab(
            |state| {
                state.open_terminal_tab_adjacent(project, anchor, side)?;
                if let Some(pane) = state.window().active_pane {
                    state.plan_creation(pane, directory.as_deref(), None);
                }
                Ok(())
            },
            cx,
        ) {
            self.changed(cx);
            self.focus_owner.request_active_pane();
            if self.connection(project_server) == ConnectionState::Disconnected {
                self.connect_server(project_server, cx);
            }
        }
    }

    pub(crate) fn open_tab_menu(
        &mut self,
        id: TabId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self
            .tab_project(id)
            .filter(|project| project.status() == ProjectStatus::Available)
        else {
            return;
        };
        let destinations = self.move_destinations(project.id);
        let items = crate::views::tab_menu::items(project, id, &destinations);
        self.open_menu(items, position, window, cx);
    }

    /// Projects a tab or pane of `project` can move to, by name: available
    /// ones on the same computer, other than `project`.
    pub(crate) fn move_destinations(
        &self,
        project: ProjectId,
    ) -> Vec<(ProjectId, gpui::SharedString)> {
        self.sidebar_projects()
            .into_iter()
            .chain(self.state.projects().iter())
            .filter(|candidate| {
                candidate.id != project
                    && candidate.status() == ProjectStatus::Available
                    && self.state.can_move_into(project, candidate.id)
            })
            .fold(Vec::new(), |mut found, candidate| {
                if !found.iter().any(|(id, _)| *id == candidate.id) {
                    found.push((candidate.id, self.project_title(candidate).into()));
                }
                found
            })
    }

    pub(crate) fn close_tabs(
        &mut self,
        anchor: TabId,
        scope: TabCloseScope,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self.tab_project(anchor) else {
            return;
        };
        let tabs = project.closable_tabs(anchor, scope);
        self.begin_close_tabs(tabs, None, None, cx);
    }
}
