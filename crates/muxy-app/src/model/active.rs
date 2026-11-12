//! Commands that act on the tab, pane, or project the user is working in.

use gpui::{Context, Window};
use muxy_app_core::settings::TerminalAction;
use muxy_app_core::{Direction, ProjectId, ProjectStatus, TabCloseScope, TabId, TabSide};

use super::preferences::Change;
use super::tab_windows::Surface;
use super::{AppModel, Quitting};
use crate::views::project_editor::Field;

/// How far a resize command moves a divider, as a share of its split.
const RESIZE_STEP: f32 = 0.05;

impl AppModel {
    pub(crate) fn rename_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialogs.is_open() {
            return;
        }
        if let Some(tab) = self.active_tab() {
            let position = self.editor_position(window);
            self.open_tab_editor(tab, position, window, cx);
        }
    }

    pub(crate) fn reset_active_tab_title(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            self.edit_tab(|state| state.set_tab_title(tab, None), cx);
        }
    }

    pub(crate) fn toggle_active_tab_pin(&mut self, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            self.edit_tab(|state| state.toggle_tab_pin(tab), cx);
        }
    }

    pub(crate) fn close_tabs_around_active(
        &mut self,
        scope: TabCloseScope,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self
            .active_tab()
            .filter(|tab| self.tab(*tab).is_some_and(|tab| !tab.detached()))
        {
            self.close_tabs(tab, scope, cx);
        }
    }

    pub(crate) fn new_tab_beside_active(&mut self, side: TabSide, cx: &mut Context<Self>) {
        if let Some(tab) = self.active_tab() {
            self.new_tab_adjacent(tab, side, cx);
        }
    }

    /// The active tab and the tab it trades places with when it moves one
    /// step: its neighbor in the same project, pinned like it is.
    pub(crate) fn tab_move_target(&self, forward: bool) -> Option<(TabId, TabId)> {
        if self.surface() != Surface::Main {
            return None;
        }
        let active = self.active_tab()?;
        let tabs = self.navigation_tabs();
        let index = tabs.iter().position(|tab| *tab == active)?;
        let neighbor = if forward {
            index.checked_add(1)?
        } else {
            index.checked_sub(1)?
        };
        let target = *tabs.get(neighbor)?;
        let project = self.tab_project(active)?;
        let pinned = |id| {
            project
                .tabs
                .iter()
                .find(|tab| tab.id == id)
                .map(|tab| tab.pinned)
        };
        (pinned(target).is_some() && pinned(target) == pinned(active)).then_some((active, target))
    }

    pub(crate) fn move_active_tab(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some((tab, target)) = self.tab_move_target(forward) else {
            return;
        };
        let Some(project) = self.tab_project(tab).map(|project| project.id) else {
            return;
        };
        let visible = self.navigation_tabs();
        if self.edit_tab(
            |state| state.move_visible_tab(project, tab, target, &visible),
            cx,
        ) {
            self.changed(cx);
        }
    }

    pub(crate) fn return_active_tab_to_main_window(&mut self, cx: &mut Context<Self>) {
        if let Some(window) = self
            .active_tab()
            .and_then(|tab| self.tab(tab))
            .and_then(|tab| tab.window.as_ref())
            .map(|window| window.id)
        {
            self.return_tab_window(window, None, cx);
        }
    }

    /// The top-level project the current one belongs to: itself, or the
    /// project a worktree was made from.
    pub(crate) fn current_parent_project(&self) -> ProjectId {
        let project = self.state.current_project();
        project.parent_id.unwrap_or(project.id)
    }

    pub(crate) fn rename_current_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialogs.is_open() {
            return;
        }
        let project = self.state.current_project().id;
        let position = self.editor_position(window);
        self.open_project_editor(project, Field::Name, position, window, cx);
    }

    /// Removes the current project after asking. A worktree whose folder is
    /// still there is inspected first, as its files go with it.
    pub(crate) fn remove_current_project(&mut self, cx: &mut Context<Self>) {
        let project = self.state.current_project();
        let id = project.id;
        if project.parent_id.is_some() && project.status() == ProjectStatus::Available {
            self.git_request(id, muxy_protocol::GitAction::InspectRemoval, cx);
        } else {
            self.confirm_remove_project(id, cx);
        }
    }

    pub(crate) fn copy_project_path(&self, id: ProjectId, cx: &mut Context<Self>) {
        if let Some(project) = self.state.project(id)
            && project.status() == ProjectStatus::Available
        {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                project.directory.to_string_lossy().into_owned(),
            ));
        }
    }

    pub(crate) fn reveal_project(&self, id: ProjectId, cx: &mut Context<Self>) {
        if let Some(project) = self.state.project(id)
            && project.status() == ProjectStatus::Available
            && project.server_id.is_local()
        {
            cx.reveal_path(&project.directory);
        }
    }

    pub(crate) fn toggle_project_focus(&mut self, cx: &mut Context<Self>) {
        let focused = (!self.appearance.sidebar_focus).then(|| self.current_parent_project());
        self.set_project_focus(focused, cx);
    }

    pub(crate) fn new_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialogs.is_open() {
            return;
        }
        let position = self.editor_position(window);
        self.open_new_workspace_editor(None, position, window, cx);
    }

    pub(crate) fn can_resize_active_pane(&self, direction: Direction) -> bool {
        self.active_pane()
            .is_some_and(|pane| self.state.can_resize_pane(pane, direction, RESIZE_STEP))
    }

    pub(crate) fn resize_active_pane(&mut self, direction: Direction, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle {
            return;
        }
        let Some(pane) = self.active_pane() else {
            return;
        };
        if matches!(
            self.state.resize_pane(pane, direction, RESIZE_STEP),
            Ok(true)
        ) {
            self.changed(cx);
        }
    }

    /// Moves focus to the next pane of the active tab, or the previous one,
    /// wrapping around.
    pub(crate) fn cycle_pane(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle {
            return;
        }
        let Some(pane) = self.active_pane() else {
            return;
        };
        let Some(panes) = self
            .active_tab()
            .and_then(|tab| self.tab(tab))
            .map(|tab| tab.layout.leaves())
        else {
            return;
        };
        let Some(index) = panes.iter().position(|id| *id == pane) else {
            return;
        };
        let next = if forward {
            (index + 1) % panes.len()
        } else {
            (index + panes.len() - 1) % panes.len()
        };
        if panes[next] != pane {
            self.focus_pane(panes[next], cx);
        }
    }

    pub(crate) fn terminal_action(&mut self, action: &TerminalAction, cx: &mut Context<Self>) {
        if self.overlay.is_some() || self.dialogs.is_open() {
            return;
        }
        if let Some(pane) = self.active_pane().and_then(|id| self.terminal(&id)) {
            pane.view.update(cx, |pane, cx| pane.perform(action, cx));
        }
    }

    pub(crate) fn toggle_status_bar(&mut self, cx: &mut Context<Self>) {
        let visible = !self.appearance.status_bar_visible;
        self.change_preference(Change::StatusBar(visible), cx);
    }

    pub(crate) fn open_active_page_externally(&mut self, cx: &mut Context<Self>) {
        if let Some(pane) = self.active_pane() {
            self.open_browser_page_in(pane, None, cx);
        }
    }
}
