use super::*;
use gpui::{MouseButton, MouseDownEvent, MouseUpEvent};
use muxy_app_core::WorkspaceId;
use muxy_app_core::settings::AppLayout;

fn click(cx: &mut VisualTestContext, selector: &str, button: MouseButton) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let position = cx
        .debug_bounds(selector.to_owned().leak())
        .unwrap_or_else(|| panic!("missing {selector}"))
        .center();
    cx.simulate_event(MouseDownEvent {
        position,
        button,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent {
        position,
        button,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn menu(cx: &mut VisualTestContext, label: &str) {
    click(cx, &format!("menu-label-{label}"), MouseButton::Left);
}

fn project_menu(cx: &mut VisualTestContext, row: usize) {
    click(cx, &format!("project-row-{row}"), MouseButton::Right);
    menu(cx, "Workspaces ▸");
}

fn projects() -> (AppState, [ProjectId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let mut add = |name: &str| {
        let id = state.add_project(std::env::temp_dir()).expect("project");
        state.rename_project(id, name).expect("name");
        id
    };
    let alpha = add("Alpha");
    let beta = add("Beta");
    let gamma = add("Gamma");
    state.select_project(beta).expect("select");
    (state, [home, alpha, beta, gamma])
}

fn open(
    state: AppState,
    layout: AppLayout,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.layout = layout;
    boot.settings.appearance.sidebar_expanded = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    cx.run_until_parked();
    (view, cx)
}

fn listed(view: &Entity<AppModel>, cx: &VisualTestContext) -> Vec<ProjectId> {
    view.read_with(cx, |model, _| {
        model
            .sidebar_projects()
            .iter()
            .map(|project| project.id)
            .collect()
    })
}

fn workspace(view: &Entity<AppModel>, cx: &VisualTestContext, name: &str) -> WorkspaceId {
    view.read_with(cx, |model, _| {
        model
            .state
            .workspaces()
            .iter()
            .find(|workspace| workspace.name == name)
            .unwrap_or_else(|| panic!("missing workspace {name}"))
            .id
    })
}

fn members(
    view: &Entity<AppModel>,
    cx: &VisualTestContext,
    workspace: WorkspaceId,
) -> Vec<ProjectId> {
    view.read_with(cx, |model, _| {
        let mut projects: Vec<_> = model
            .state
            .workspace(workspace)
            .expect("workspace")
            .projects
            .iter()
            .copied()
            .collect();
        projects.sort();
        projects
    })
}

fn sorted<const N: usize>(mut ids: [ProjectId; N]) -> Vec<ProjectId> {
    ids.sort();
    ids.to_vec()
}

#[gpui::test]
fn project_menus_group_projects_into_overlapping_workspaces(cx: &mut TestAppContext) {
    let (state, [home, alpha, beta, gamma]) = projects();
    let (view, cx) = open(state, AppLayout::ProjectFocused, cx);
    project_menu(cx, 1);
    menu(cx, "New Workspace…");
    cx.simulate_input("Work");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let work = workspace(&view, cx, "Work");
    assert_eq!(members(&view, cx, work), [alpha]);
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.state.active_workspace().is_none());
        assert_eq!(model.state.current_project().id, beta);
    });

    project_menu(cx, 2);
    menu(cx, "Work");
    project_menu(cx, 2);
    menu(cx, "New Workspace…");
    cx.simulate_input("Personal");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let personal = workspace(&view, cx, "Personal");
    assert_eq!(members(&view, cx, work), sorted([alpha, beta]));
    assert_eq!(members(&view, cx, personal), [beta]);
    project_menu(cx, 2);
    menu(cx, "Work");
    assert_eq!(members(&view, cx, work), [alpha]);
    assert_eq!(listed(&view, cx), [home, alpha, beta, gamma]);
    view.read_with(cx, |model, _| {
        assert_eq!(store::load(&model.path).expect("saved"), model.state);
    });
}

#[gpui::test]
fn the_sidebar_filter_switches_renames_and_deletes_workspaces(cx: &mut TestAppContext) {
    let (mut state, [home, alpha, beta, gamma]) = projects();
    let work = state.create_workspace("Work").expect("work");
    state
        .set_workspace_member(work, alpha, true)
        .expect("alpha");
    state
        .set_workspace_member(work, gamma, true)
        .expect("gamma");
    let (view, cx) = open(state, AppLayout::ProjectFocused, cx);

    click(cx, "sidebar-project-filter", MouseButton::Left);
    menu(cx, "Work");
    assert_eq!(listed(&view, cx), [home, alpha, gamma]);
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.current_project().id,
            alpha,
            "the hidden selection moves to the first member"
        );
        assert_eq!(model.sidebar_filter_label(), "Work");
    });

    click(cx, "sidebar-project-filter", MouseButton::Left);
    menu(cx, "Focus Current Project");
    assert_eq!(listed(&view, cx), [alpha]);
    click(cx, "sidebar-project-filter", MouseButton::Left);
    menu(cx, "Rename Workspace…");
    cx.simulate_input("Client");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.workspace(work).expect("work").name, "Client");
        assert_eq!(model.sidebar_filter_label(), "Focused Project");
    });

    click(cx, "sidebar-project-filter", MouseButton::Left);
    menu(cx, "New Workspace…");
    cx.simulate_input("Empty");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let empty = workspace(&view, cx, "Empty");
    assert_eq!(listed(&view, cx), [home]);
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.sidebar_focus);
        assert_eq!(model.state.active_workspace().map(|w| w.id), Some(empty));
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(model.sidebar_filter_label(), "Empty");
    });

    click(cx, "sidebar-project-filter", MouseButton::Left);
    menu(cx, "Delete Workspace…");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| model.state.workspace(empty).is_some()));
    click(cx, "sidebar-project-filter", MouseButton::Left);
    menu(cx, "Delete Workspace…");
    cx.simulate_prompt_answer("Delete");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.state.workspace(empty).is_none());
        assert!(model.state.active_workspace().is_none());
        assert_eq!(model.state.projects().len(), 4);
        assert_eq!(model.sidebar_filter_label(), "All Projects");
    });
    assert_eq!(listed(&view, cx), [home, alpha, beta, gamma]);

    click(cx, "sidebar-project-filter", MouseButton::Left);
    menu(cx, "Client");
    click(cx, "project-row-1", MouseButton::Left);
    click(cx, "sidebar-project-filter", MouseButton::Left);
    menu(cx, "Focus Current Project");
    assert_eq!(listed(&view, cx), [alpha]);
    project_menu(cx, 0);
    menu(cx, "Client");
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.current_project().id,
            gamma,
            "leaving the filtered workspace moves the selection to the next listed project"
        );
        assert!(model.appearance.sidebar_focus);
        assert_eq!(store::load(&model.path).expect("saved"), model.state);
    });
}

#[gpui::test]
fn navigation_and_opened_projects_follow_the_workspace_filter(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    std::fs::create_dir(root.path().join("Delta")).expect("delta");
    std::fs::create_dir(root.path().join("Epsilon")).expect("epsilon");
    let (mut state, [home, alpha, beta, gamma]) = projects();
    let existing = state
        .add_project(root.path().join("Epsilon"))
        .expect("existing");
    let work = state.create_workspace("Work").expect("work");
    let other = state.create_workspace("Other").expect("other");
    state
        .set_workspace_member(work, alpha, true)
        .expect("alpha");
    state
        .set_workspace_member(other, gamma, true)
        .expect("gamma");
    state.select_project(alpha).expect("alpha");
    state.select_workspace(Some(work)).expect("filter");
    let (view, cx) = open(state, AppLayout::TabFocused, cx);
    assert_eq!(listed(&view, cx), [home, alpha]);

    view.update(cx, |model, cx| model.select_project(gamma, cx));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.active_workspace().map(|w| w.id), Some(other));
    });
    view.update(cx, |model, cx| model.select_project(beta, cx));
    view.read_with(cx, |model, _| {
        assert!(model.state.active_workspace().is_none());
    });

    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("switch workspace");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.simulate_input("Work");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.active_workspace().map(|w| w.id), Some(work));
        assert_eq!(model.state.current_project().id, alpha);
    });

    for (name, expected) in [("Delta", None), ("Epsilon", Some(existing))] {
        cx.simulate_keystrokes("cmd-o");
        cx.run_until_parked();
        cx.simulate_input(&root.path().join(name).to_string_lossy());
        cx.executor().advance_clock(Duration::from_millis(125));
        cx.run_until_parked();
        cx.simulate_keystrokes("cmd-enter");
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            let current = model.state.current_project();
            assert_eq!(current.directory, root.path().join(name));
            if let Some(expected) = expected {
                assert_eq!(current.id, expected);
            }
            assert!(
                model
                    .state
                    .workspace(work)
                    .expect("work")
                    .projects
                    .contains(&current.id)
            );
            assert_eq!(model.state.active_workspace().map(|w| w.id), Some(work));
        });
    }
    assert_eq!(listed(&view, cx).len(), 4);
}
