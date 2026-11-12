//! Whether each command can run now. The palette lists a command that
//! can't as disabled.

use muxy_app_core::{Direction, ProjectStatus, TabCloseScope};

use crate::model::AppModel;

pub(super) fn project_present(model: &AppModel) -> bool {
    model.state.current_project().status() != ProjectStatus::Missing
}

pub(super) fn has_tab(model: &AppModel) -> bool {
    model.active_tab().is_some()
}

pub(super) fn has_pane(model: &AppModel) -> bool {
    project_present(model) && model.active_pane().is_some()
}

/// Whether the active pane can move into a window of its own: any pane of
/// the main window, or a pane sharing its tab window with another.
pub(super) fn pane_can_leave_window(model: &AppModel) -> bool {
    project_present(model)
        && model
            .active_tab()
            .and_then(|tab| model.tab(tab))
            .is_some_and(|tab| !tab.detached() || tab.floating() || tab.panes.len() >= 2)
}

pub(super) fn can_float_pane(model: &AppModel) -> bool {
    project_present(model)
        && has_terminal(model)
        && model
            .active_pane()
            .is_some_and(|pane| !model.pane_floats(pane))
}

pub(super) fn has_floats(model: &AppModel) -> bool {
    model.state.floating_tabs().next().is_some()
}

pub(super) fn in_float(model: &AppModel) -> bool {
    model.key_float().is_some()
}

pub(super) fn float_in_project(model: &AppModel) -> bool {
    model
        .key_float()
        .and_then(|id| model.float_scope(id))
        .is_some_and(|scope| scope == muxy_app_core::FloatScope::Project)
}

pub(super) fn float_in_all_projects(model: &AppModel) -> bool {
    model
        .key_float()
        .and_then(|id| model.float_scope(id))
        .is_some_and(|scope| scope == muxy_app_core::FloatScope::AllProjects)
}

pub(super) fn has_split(model: &AppModel) -> bool {
    project_present(model)
        && model
            .active_tab()
            .and_then(|tab| model.tab(tab))
            .is_some_and(|tab| tab.panes.len() >= 2)
}

pub(super) fn has_terminal(model: &AppModel) -> bool {
    model
        .active_pane()
        .and_then(|pane| model.terminal(&pane))
        .is_some()
}

pub(super) fn has_other_project(model: &AppModel) -> bool {
    model.navigation_projects().len() >= 2
}

pub(super) fn has_other_tab(model: &AppModel) -> bool {
    model.navigation_tabs().len() >= 2
}

pub(super) fn can_add_worktree(model: &AppModel) -> bool {
    model.new_worktree_target().is_some()
}

pub(super) fn can_switch_worktree(model: &AppModel) -> bool {
    model.worktree_switcher_project().is_some()
}

pub(super) fn can_open_browser(model: &AppModel) -> bool {
    project_present(model) && model.browser_enabled()
}

pub(super) fn browser_enabled(model: &AppModel) -> bool {
    model.browser_enabled()
}

pub(super) fn has_browser(model: &AppModel) -> bool {
    model.active_browser().is_some()
}

pub(super) fn can_go_back(model: &AppModel) -> bool {
    model.can_navigate(false)
}

pub(super) fn can_go_forward(model: &AppModel) -> bool {
    model.can_navigate(true)
}

pub(super) fn has_workspaces(model: &AppModel) -> bool {
    !model.state.workspaces().is_empty()
}

pub(super) fn tips_shown(model: &AppModel) -> bool {
    model.appearance.tips_visible
}

fn active_tab(model: &AppModel) -> Option<&muxy_app_core::Tab> {
    model.active_tab().and_then(|tab| model.tab(tab))
}

/// Whether the active tab sits among its project's tabs in the main window.
pub(super) fn has_listed_tab(model: &AppModel) -> bool {
    project_present(model) && active_tab(model).is_some_and(|tab| !tab.detached())
}

pub(super) fn has_detached_tab(model: &AppModel) -> bool {
    active_tab(model).is_some_and(muxy_app_core::Tab::detached)
}

pub(super) fn has_titled_tab(model: &AppModel) -> bool {
    active_tab(model).is_some_and(|tab| tab.custom_title.is_some())
}

/// A tab in a window of its own has no tabs beside it to close.
fn can_close_tabs(model: &AppModel, scope: TabCloseScope) -> bool {
    has_listed_tab(model)
        && model
            .active_tab()
            .zip(model.active_tab().and_then(|tab| model.tab_project(tab)))
            .is_some_and(|(tab, project)| !project.closable_tabs(tab, scope).is_empty())
}

pub(super) fn can_close_other_tabs(model: &AppModel) -> bool {
    can_close_tabs(model, TabCloseScope::Other)
}

pub(super) fn can_close_tabs_to_the_left(model: &AppModel) -> bool {
    can_close_tabs(model, TabCloseScope::Left)
}

pub(super) fn can_close_tabs_to_the_right(model: &AppModel) -> bool {
    can_close_tabs(model, TabCloseScope::Right)
}

pub(super) fn can_move_tab_left(model: &AppModel) -> bool {
    model.tab_move_target(false).is_some()
}

pub(super) fn can_move_tab_right(model: &AppModel) -> bool {
    model.tab_move_target(true).is_some()
}

pub(super) fn project_available(model: &AppModel) -> bool {
    model.state.current_project().status() == ProjectStatus::Available
}

/// Another computer's Home goes by its server's name, so it can't be renamed.
pub(super) fn can_rename_project(model: &AppModel) -> bool {
    project_available(model) && !model.state.current_project().is_remote_home()
}

pub(super) fn can_remove_project(model: &AppModel) -> bool {
    !model.state.current_project().home
}

pub(super) fn can_reveal_project(model: &AppModel) -> bool {
    project_available(model) && model.state.current_project().server_id.is_local()
}

pub(super) fn can_toggle_worktrees(model: &AppModel) -> bool {
    let parent = model.current_parent_project();
    model
        .state
        .project(parent)
        .is_some_and(|project| !project.home && project.status() == ProjectStatus::Available)
        && model.tracks_worktrees(parent)
}

pub(super) fn can_open_project_in_app(model: &AppModel) -> bool {
    model.project_apps_directory().is_some() && model.default_project_app().is_some()
}

pub(super) fn can_resize_left(model: &AppModel) -> bool {
    project_present(model) && model.can_resize_active_pane(Direction::Left)
}

pub(super) fn can_resize_right(model: &AppModel) -> bool {
    project_present(model) && model.can_resize_active_pane(Direction::Right)
}

pub(super) fn can_resize_up(model: &AppModel) -> bool {
    project_present(model) && model.can_resize_active_pane(Direction::Up)
}

pub(super) fn can_resize_down(model: &AppModel) -> bool {
    project_present(model) && model.can_resize_active_pane(Direction::Down)
}
