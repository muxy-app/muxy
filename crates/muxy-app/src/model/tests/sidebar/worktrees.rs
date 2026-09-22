use super::*;
use muxy_app_core::settings::{AppLayout, Settings};

fn fixture() -> (AppState, tempfile::TempDir, [ProjectId; 3], TabId) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let parent = state.add_project(std::env::temp_dir()).expect("parent");
    let directory = tempfile::tempdir().expect("worktree");
    std::fs::write(directory.path().join(".git"), "gitdir: /tmp/unused").expect("marker");
    let child = state
        .add_project(directory.path().to_owned())
        .expect("child");
    let tab = state.open_terminal_tab(child).expect("tab");
    state.select_project(parent).expect("select");
    let mut stored = serde_json::to_value(&state).expect("state");
    let record = stored["projects"]
        .as_array_mut()
        .expect("projects")
        .iter_mut()
        .find(|record| record["id"] == serde_json::to_value(child).expect("id"))
        .expect("child");
    record["parent_id"] = serde_json::to_value(parent).expect("parent");
    record["kind"] = serde_json::to_value(muxy_app_core::ProjectKind::Worktree).expect("kind");
    (
        serde_json::from_value(stored).expect("worktree state"),
        directory,
        [home, parent, child],
        tab,
    )
}

fn click(cx: &mut VisualTestContext, selector: &str) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let position = cx
        .debug_bounds(selector.to_owned().leak())
        .expect("control")
        .center();
    cx.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn toggle_from_menu(cx: &mut VisualTestContext) {
    let position = cx.debug_bounds("project-row-1").expect("project").center();
    cx.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Right,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
    click(cx, "menu-label-Worktrees");
}

#[gpui::test]
fn worktree_checkbox_hides_details_preserves_tabs_and_survives_restart(cx: &mut TestAppContext) {
    let (mut state, _directory, [home, parent, child], tab) = fixture();
    state.select_project(child).expect("child");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    boot.settings.appearance.auto_expand_worktrees = true;
    let path = boot.state_path.clone();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(format!("worktree-{child}").leak())
            .is_some()
    );
    toggle_from_menu(cx);
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.current_project().id, parent);
        assert_eq!(model.state.project(child).expect("child").tabs[0].id, tab);
        assert!(!model.worktrees_visible(parent));
        assert!(!model.has_worktrees(model.state.project(parent).expect("parent")));
        assert!(!model.expanded_worktrees.contains(&parent));
    });
    click(cx, "project-row-0");
    click(cx, "project-row-1");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, parent);
    });
    let mut state = view.read_with(cx, |model, _| model.state.clone());
    state.select_project(home).expect("home");
    let (mut boot, _requests) = stub_boot(state);
    boot.state_path = path.clone();
    boot.settings = Settings::load(&path.with_file_name("settings.toml")).expect("saved");
    let (restarted, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    click(cx, "project-row-1");
    restarted.read_with(cx, |model, _| {
        assert!(!model.worktrees_visible(parent));
        assert_eq!(model.state.current_project().id, parent);
    });
    for selector in [
        format!("project-worktree-label-{parent}"),
        format!("project-worktrees-toggle-{parent}"),
        format!("worktree-{child}"),
    ] {
        assert!(
            cx.debug_bounds(selector.clone().leak()).is_none(),
            "{selector}"
        );
    }
    toggle_from_menu(cx);
    assert!(
        cx.debug_bounds(format!("project-worktree-label-{parent}").leak())
            .is_some()
    );
    click(cx, &format!("project-worktrees-toggle-{parent}"));
    click(cx, &format!("worktree-{child}"));
    restarted.read_with(cx, |model, _| {
        assert!(model.worktrees_visible(parent));
        assert_eq!(model.state.current_project().id, child);
        assert_eq!(model.active_tab(), Some(tab));
        assert!(model.overlay.is_none());
    });
}

#[gpui::test]
fn collapsed_worktree_icon_opens_inline_list_without_a_picker(cx: &mut TestAppContext) {
    let (state, _directory, [_, parent, child], _) = fixture();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    click(cx, "project-row-1");
    view.read_with(cx, |model, _| {
        assert!(model.appearance.sidebar_expanded);
        assert!(model.expanded_worktrees.contains(&parent));
        assert!(model.overlay.is_none());
    });
    assert!(
        cx.debug_bounds(format!("worktree-{child}").leak())
            .is_some()
    );
}

#[gpui::test]
fn hidden_worktrees_are_excluded_from_tab_sidebar_navigation(cx: &mut TestAppContext) {
    let (state, _directory, [home, parent, child], tab) = fixture();
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.layout = AppLayout::TabFocused;
    boot.settings.appearance.hidden_worktrees.insert(parent);
    boot.settings
        .appearance
        .tab_focused_expanded
        .insert(child, true);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .sidebar_projects()
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            [home, parent]
        );
        assert!(!model.navigation_tabs().contains(&tab));
    });
    view.update(cx, |model, cx| model.toggle_worktree_visibility(parent, cx));
    view.read_with(cx, |model, _| {
        assert!(model.sidebar_projects().iter().any(|p| p.id == child));
        assert!(model.navigation_tabs().contains(&tab));
    });
}

#[gpui::test]
fn project_worktrees_expand_select_and_restore_the_last_selected_child(cx: &mut TestAppContext) {
    let (state, _directory, [home, parent, child], _) = fixture();
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    let path = boot.state_path.clone();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(format!("worktree-{child}").leak())
            .is_none()
    );
    click(cx, &format!("project-worktrees-toggle-{parent}"));
    assert!(
        cx.debug_bounds(format!("worktree-{parent}").leak())
            .is_some()
    );
    click(cx, &format!("worktree-{child}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, child);
    });
    click(cx, "project-row-0");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, home);
    });
    click(cx, "project-row-1");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, child);
    });
    click(cx, "project-row-1");
    view.read_with(cx, |model, _| {
        assert!(!model.expanded_worktrees.contains(&parent));
    });
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, child);
    });
    let mut state = view.read_with(cx, |model, _| model.state.clone());
    state.select_project(home).expect("home");
    let (mut boot, _requests) = stub_boot(state);
    boot.state_path = path.clone();
    boot.settings = Settings::load(&path.with_file_name("settings.toml")).expect("saved");
    let (restarted, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    click(cx, "project-row-1");
    restarted.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, child);
    });
}

#[gpui::test]
fn collapsed_project_sidebar_keeps_one_icon_per_family_and_cycles_from_child(
    cx: &mut TestAppContext,
) {
    let (mut state, _directory, [home, parent, child], _) = fixture();
    state.select_project(child).expect("child");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .sidebar_projects()
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            [home, parent]
        );
    });
    assert!(cx.debug_bounds("project-row-2").is_none());
    view.update(cx, |model, cx| model.cycle_project(true, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, home);
    });
    view.update(cx, |model, cx| model.cycle_project(true, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, child);
    });
}

#[gpui::test]
fn tab_worktrees_stay_flat_visible_and_independently_collapsible(cx: &mut TestAppContext) {
    let (state, _directory, [_, parent, child], tab) = fixture();
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    boot.settings.appearance.layout = AppLayout::TabFocused;
    boot.settings
        .appearance
        .tab_focused_expanded
        .extend([(parent, false), (child, true)]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let parent_bounds = cx
        .debug_bounds(format!("tab-project-{parent}").leak())
        .expect("parent");
    let child_bounds = cx
        .debug_bounds(format!("tab-project-{child}").leak())
        .expect("child");
    assert_eq!(parent_bounds.left(), child_bounds.left());
    view.read_with(cx, |model, _| {
        assert!(model.navigation_tabs().contains(&tab));
    });
    click(cx, &format!("tab-project-toggle-{child}"));
    view.read_with(cx, |model, _| {
        assert!(!model.navigation_tabs().contains(&tab));
    });
    click(cx, &format!("tab-project-toggle-{parent}"));
    view.read_with(cx, |model, _| {
        assert!(!model.navigation_tabs().contains(&tab));
    });
    view.update(cx, |model, cx| model.select_tab(tab, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.navigation_tabs().contains(&tab));
    });
}

#[gpui::test]
fn project_worktree_activity_moves_between_parent_and_worktree_rows(cx: &mut TestAppContext) {
    let (mut state, _directory, [_, parent, child], _) = fixture();
    let pane = state.project(child).expect("child").tabs[0].panes[0].id;
    let session = SessionId::new(555).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.activity.snapshot.agents = vec![muxy_protocol::AgentActivity {
            session,
            project: child,
            provider: muxy_protocol::AgentProvider::Codex,
            state: muxy_protocol::AgentState::Blocked,
        }];
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(format!("project-activity-{parent}-blocked").leak())
            .is_some()
    );
    click(cx, &format!("project-worktrees-toggle-{parent}"));
    view.read_with(cx, |model, _| {
        assert!(model.expanded_worktrees.contains(&parent));
    });
    assert!(
        cx.debug_bounds(format!("worktree-activity-{child}-blocked").leak())
            .is_some()
    );
    click(cx, &format!("new-worktree-{parent}"));
    assert!(cx.debug_bounds("git-form").is_some());
}
