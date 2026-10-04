use super::*;
use muxy_protocol::{ActivitySnapshot, AgentActivity, AgentProvider, AgentState};

fn detect(state: &mut AppState, project: ProjectId, tab: TabId, session: u64) -> AgentActivity {
    let pane = state
        .project(project)
        .expect("project")
        .tabs
        .iter()
        .find(|item| item.id == tab)
        .expect("tab")
        .panes[0]
        .id;
    let session = SessionId::new(session).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    AgentActivity {
        session,
        project,
        provider: AgentProvider::Codex,
        state: AgentState::Idle,
    }
}

#[gpui::test]
fn agents_layout_menu_preserves_tabs_and_restores_with_the_top_tab_bar(cx: &mut TestAppContext) {
    let (boot, _requests, _, _) = fixture();
    let path = boot.state_path.clone();
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    window.run_until_parked();
    let state = view.read_with(window, |model, _| model.state.clone());
    click(window, "layout-menu");
    click(window, "menu-label-Agents Focused");
    assert!(window.debug_bounds("tab-sidebar").is_some());
    assert!(window.debug_bounds("tab-strip").is_some());
    view.read_with(window, |model, _| {
        assert_eq!(model.appearance.layout, AppLayout::AgentsFocused);
        assert_eq!(model.state, state);
        assert!(model.sidebar_tabs(model.state.home()).next().is_none());
    });
    click(window, "sidebar-toggle");
    let saved = Settings::load(&path.with_file_name("settings.toml")).expect("settings");
    assert_eq!(saved.appearance.layout, AppLayout::AgentsFocused);
    let (mut boot, _requests) = stub_boot(state.clone());
    boot.state_path = path;
    boot.settings = saved;
    let (restarted, window) = window.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    window.run_until_parked();
    restarted.read_with(window, |model, _| {
        assert_eq!(model.appearance.layout, AppLayout::AgentsFocused);
        assert_eq!(model.state, state);
        assert_eq!(px(model.sidebar_width()), px(0.0));
    });
    assert!(window.debug_bounds("tab-sidebar").is_none());
    assert!(window.debug_bounds("tab-strip").is_some());
    click(window, "sidebar-toggle");
    restarted.read_with(window, |model, _| {
        assert!(model.appearance.sidebar_expanded);
        assert_eq!(px(model.sidebar_width()), px(270.0));
    });
    click(window, "layout-menu");
    click(window, "menu-label-Tab Focused");
    assert!(window.debug_bounds("project-titlebar").is_some());
    restarted.read_with(window, |model, _| {
        assert_eq!(model.appearance.layout, AppLayout::TabFocused);
        assert_eq!(model.sidebar_tabs(model.state.home()).count(), 2);
    });
}

#[gpui::test]
fn agents_sidebar_tracks_detection_including_idle_without_changing_topbar_navigation(
    cx: &mut TestAppContext,
) {
    let (mut boot, _requests, [home, project], [first, second, third]) = fixture();
    boot.settings.appearance.layout = AppLayout::AgentsFocused;
    let first_agent = detect(&mut boot.state, home, first, 11);
    let third_agent = detect(&mut boot.state, project, third, 12);
    boot.state.select_tab(home, second).expect("shell");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    for state in [AgentState::Working, AgentState::Blocked, AgentState::Idle] {
        view.update(cx, |model, cx| {
            model.receive_activity(
                Ok(ActivitySnapshot {
                    revision: 1,
                    agents: vec![
                        AgentActivity {
                            state,
                            ..first_agent.clone()
                        },
                        third_agent.clone(),
                    ],
                    events: vec![],
                }),
                cx,
            );
            assert_eq!(
                model
                    .sidebar_tabs(model.state.home())
                    .map(|tab| tab.id)
                    .collect::<Vec<_>>(),
                [first]
            );
            assert_eq!(model.navigation_tabs(), [first, second]);
            assert!(
                model
                    .terminal(&model.state.home().tabs[0].panes[0].id)
                    .is_none()
            );
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds(format!("sidebar-tab-{first}").leak())
                .is_some()
        );
        assert!(
            cx.debug_bounds(format!("sidebar-tab-{second}").leak())
                .is_none()
        );
        assert!(
            cx.debug_bounds(format!("sidebar-tab-{third}").leak())
                .is_some()
        );
        assert!(cx.debug_bounds("tab-cell-0").is_some());
        assert!(cx.debug_bounds("tab-cell-1").is_some());
    }
    cx.simulate_keystrokes("cmd-1");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert_eq!(model.active_tab(), Some(first)));
    cx.simulate_keystrokes("cmd-2");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert_eq!(model.active_tab(), Some(second)));
    let project_row = format!("tab-project-{project}").leak();
    let before_row = cx.debug_bounds(project_row).expect("project row");
    view.update(cx, |model, cx| {
        let before = model.state.clone();
        let mut snapshot = model.activity.snapshot.clone();
        snapshot.agents.clear();
        model.receive_activity(Ok(snapshot), cx);
        assert_eq!(model.state, before);
        assert_eq!(model.sidebar_tabs(model.state.home()).count(), 0);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds(project_row).expect("project row").top() < before_row.top());
    assert!(
        cx.debug_bounds(format!("tab-project-{project}").leak())
            .is_some()
    );
    assert!(cx.debug_bounds("tab-cell-0").is_some());
}

#[gpui::test]
fn agents_sidebar_selects_a_detected_split_even_when_a_shell_is_zoomed(cx: &mut TestAppContext) {
    let (mut boot, _requests, [home, _], [first, second, _]) = fixture();
    boot.settings.appearance.layout = AppLayout::AgentsFocused;
    let shell = boot.state.home().tabs[0].panes[0].id;
    let agent_pane = boot
        .state
        .split_pane(shell, Direction::Right)
        .expect("split");
    let session = SessionId::new(21).expect("session");
    boot.state
        .set_pane_session(agent_pane, Some(session))
        .expect("session");
    boot.state
        .set_pane_title(agent_pane, "Agent task")
        .expect("title");
    boot.state.toggle_zoom(shell).expect("zoom shell");
    boot.state.select_tab(home, second).expect("other tab");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive_activity(
            Ok(ActivitySnapshot {
                revision: 1,
                agents: vec![AgentActivity {
                    session,
                    project: home,
                    provider: AgentProvider::Claude,
                    state: AgentState::Working,
                }],
                events: vec![],
            }),
            cx,
        );
        let tab = &model.state.home().tabs[0];
        assert_eq!(model.agent_tab_pane(tab).expect("agent").0.id, agent_pane);
        assert_eq!(model.sidebar_tabs(model.state.home()).count(), 1);
    });
    click(cx, &format!("sidebar-tab-{first}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_tab(), Some(first));
        assert_eq!(model.active_pane(), Some(agent_pane));
        assert_eq!(model.state.home().tabs[0].zoomed, Some(agent_pane));
    });
    view.update(cx, |model, cx| {
        let second_agent = detect(&mut model.state, home, first, 22);
        model.activity.snapshot.agents.push(second_agent);
        model.focus_pane(shell, cx);
        let tab = &model.state.home().tabs[0];
        assert_eq!(
            model.agent_tab_pane(tab).expect("focused agent").0.id,
            shell
        );
        assert_eq!(model.sidebar_tabs(model.state.home()).count(), 1);
    });
    click(cx, &format!("sidebar-tab-{first}"));
    view.read_with(cx, |model, _| assert_eq!(model.active_pane(), Some(shell)));
}

#[gpui::test]
fn agents_sidebar_reuses_project_expansion_focus_sort_and_new_terminal_controls(
    cx: &mut TestAppContext,
) {
    let (mut boot, _requests, [home, project], [first, _, third]) = fixture();
    boot.settings.appearance.layout = AppLayout::AgentsFocused;
    let agent = detect(&mut boot.state, project, third, 31);
    let empty = boot
        .state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("empty");
    boot.state.rename_project(empty, "Alpha").expect("name");
    boot.state.rename_project(project, "Zulu").expect("name");
    boot.state.select_tab(home, first).expect("home");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive_activity(
            Ok(ActivitySnapshot {
                revision: 1,
                agents: vec![agent],
                events: vec![],
            }),
            cx,
        );
    });
    cx.run_until_parked();
    let next_row = format!("tab-project-{empty}").leak();
    let expanded = cx.debug_bounds(next_row).expect("next project");
    click(cx, &format!("tab-project-toggle-{project}"));
    assert!(cx.debug_bounds(next_row).expect("next project").top() < expanded.top());
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, home);
        assert!(!model.appearance.tab_focused_expanded[&project]);
    });
    click(cx, &format!("tab-project-{project}"));
    assert!(
        cx.debug_bounds(format!("sidebar-tab-{third}").leak())
            .is_some()
    );
    click(cx, "sidebar-project-sort");
    click(cx, "menu-label-Name");
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .sidebar_projects()
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            [home, empty, project]
        );
    });
    click(cx, "sidebar-project-filter");
    click(cx, "menu-label-Focus Current Project");
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .sidebar_projects()
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            [project]
        );
    });
    click(cx, "sidebar-project-filter");
    click(cx, "menu-label-All Projects");
    click(cx, &format!("tab-project-{empty}"));
    view.read_with(cx, |model, _| {
        assert!(model.state.current_project().tabs.is_empty());
    });
    let header = cx
        .debug_bounds(format!("tab-project-{empty}").leak())
        .expect("header");
    cx.simulate_event(MouseMoveEvent {
        position: header.center(),
        ..Default::default()
    });
    click(cx, &format!("project-new-tab-{empty}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, empty);
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_eq!(model.sidebar_tabs(model.state.current_project()).count(), 0);
        assert_eq!(model.navigation_tabs().len(), 1);
    });
}

fn drag(cx: &mut VisualTestContext, from: &str, over: &[&str]) {
    let start = cx
        .debug_bounds(from.to_owned().leak())
        .expect("drag source")
        .center();
    cx.simulate_event(MouseDownEvent {
        position: start,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    let mut position = start;
    for selector in over {
        position = cx
            .debug_bounds((*selector).to_owned().leak())
            .expect("drag target")
            .center();
        cx.simulate_event(MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        });
        cx.run_until_parked();
    }
    cx.simulate_event(MouseUpEvent {
        position,
        button: MouseButton::Left,
        ..Default::default()
    });
    cx.run_until_parked();
}

#[gpui::test]
fn agents_sidebar_drag_keeps_shell_slots_and_is_independent_of_titlebar_drag(
    cx: &mut TestAppContext,
) {
    let (mut boot, _requests, [home, project], [first, second, third]) = fixture();
    boot.settings.appearance.layout = AppLayout::AgentsFocused;
    let fourth = boot.state.open_terminal_tab(home).expect("fourth");
    let agents = vec![
        detect(&mut boot.state, home, first, 41),
        detect(&mut boot.state, project, third, 42),
        detect(&mut boot.state, home, fourth, 43),
    ];
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.receive_activity(
            Ok(ActivitySnapshot {
                revision: 1,
                agents,
                events: vec![],
            }),
            cx,
        );
    });
    cx.run_until_parked();
    drag(
        cx,
        &format!("sidebar-tab-{first}"),
        &[&format!("sidebar-tab-{third}")],
    );
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .state
                .home()
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect::<Vec<_>>(),
            [first, second, fourth]
        );
    });
    drag(
        cx,
        &format!("sidebar-tab-{first}"),
        &[&format!("sidebar-tab-{fourth}")],
    );
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .state
                .home()
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect::<Vec<_>>(),
            [fourth, second, first]
        );
        assert_eq!(model.active_tab(), Some(first));
        assert_eq!(store::load(&model.path).expect("saved state"), model.state);
    });
    drag(cx, "tab-cell-2", &["tab-cell-1"]);
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .state
                .home()
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect::<Vec<_>>(),
            [fourth, first, second]
        );
        assert_eq!(store::load(&model.path).expect("saved state"), model.state);
    });
}
