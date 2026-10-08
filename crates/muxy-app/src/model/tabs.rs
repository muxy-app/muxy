use gpui::{Context, Pixels, Point, Window};
use muxy_app_core::{AppError, AppState, ProjectStatus, Tab, TabCloseScope, TabId, TabSide};

use super::{AppModel, ConnectionState, Quitting};
use crate::views::tab_strip::TabDrop;

impl AppModel {
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
            self.focus_requested = true;
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
            self.focus_requested = true;
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
        if self.edit_tab(
            |state| {
                state
                    .open_terminal_tab_adjacent(project, anchor, side)
                    .map(|_| ())
            },
            cx,
        ) {
            self.changed(cx);
            self.focus_requested = true;
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
        let items = crate::views::tab_menu::items(project, id);
        self.open_menu(items, position, window, cx);
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
        self.begin_close_tabs(tabs, None, cx);
    }
}
