use super::*;
use gpui::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, point};
use muxy_app_core::settings::{AppLayout, ProjectOrder, Settings, SidebarCollapsedStyle};

fn click(cx: &mut VisualTestContext, selector: &str) {
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
}

type Requests = std::sync::mpsc::Receiver<(u64, Work)>;

fn fixture() -> (Boot, Requests, [ProjectId; 2], [TabId; 3]) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let first = state.open_terminal_tab(home).expect("first");
    let second = state.open_terminal_tab(home).expect("second");
    let project = state.add_project(std::env::temp_dir()).expect("project");
    let third = state.open_terminal_tab(project).expect("third");
    state.select_tab(home, first).expect("select first");
    let (mut boot, requests) = stub_boot(state);
    boot.settings.appearance.layout = AppLayout::TabFocused;
    boot.settings.appearance.sidebar_expanded = true;
    boot.settings
        .appearance
        .tab_focused_expanded
        .extend([(home, true), (project, true)]);
    (boot, requests, [home, project], [first, second, third])
}

#[gpui::test]
fn sidebar_toggle_restores_expanded_and_collapsed_after_restart(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let path = boot.state_path.clone();
    let (view, window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    window.run_until_parked();
    for expanded in [true, false, true] {
        click(window, "sidebar-toggle");
        let saved = Settings::load(&path.with_file_name("settings.toml")).expect("saved settings");
        assert_eq!(saved.appearance.sidebar_expanded, expanded);
        view.read_with(window, |model, _| {
            assert_eq!(model.appearance.sidebar_expanded, expanded);
        });
        let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("restart state"));
        boot.state_path = path.clone();
        boot.settings = saved;
        let (restarted, restart_window) =
            window.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        restart_window.run_until_parked();
        restarted.read_with(restart_window, |model, _| {
            assert_eq!(model.appearance.sidebar_expanded, expanded);
            assert_eq!(
                px(model.sidebar_width()),
                px(if expanded { 270.0 } else { 44.0 })
            );
        });
    }
}

#[gpui::test]
fn layout_menu_switches_and_restores_without_changing_tabs(cx: &mut TestAppContext) {
    let (mut boot, _requests, _, _) = fixture();
    boot.settings.appearance.layout = AppLayout::ProjectFocused;
    boot.settings.appearance.sidebar_expanded = true;
    let path = boot.state_path.clone();
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    window.run_until_parked();
    let state = view.read_with(window, |model, _| model.state.clone());
    click(window, "layout-menu");
    window.simulate_keystrokes("down down enter");
    window.run_until_parked();
    assert!(window.debug_bounds("tab-sidebar").is_some());
    assert!(window.debug_bounds("project-titlebar").is_some());
    view.read_with(window, |model, _| {
        assert_eq!(model.state, state);
        assert_eq!(model.appearance.layout, AppLayout::TabFocused);
    });
    click(window, "sidebar-toggle");
    let saved = Settings::load(&path.with_file_name("settings.toml")).expect("settings");
    assert_eq!(saved.appearance.layout, AppLayout::TabFocused);
    assert!(!saved.appearance.sidebar_expanded);
    let (mut boot, _requests) = stub_boot(state);
    boot.state_path = path;
    boot.settings = saved;
    let (restarted, restart_window) =
        window.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    restart_window.run_until_parked();
    restarted.read_with(restart_window, |model, _| {
        assert_eq!(model.appearance.layout, AppLayout::TabFocused);
    });
    assert!(restart_window.debug_bounds("tab-sidebar").is_none());
    assert!(restart_window.debug_bounds("project-titlebar").is_some());
    click(restart_window, "sidebar-toggle");
    click(restart_window, "layout-menu");
    restart_window.simulate_keystrokes("down enter");
    restart_window.run_until_parked();
    assert!(restart_window.debug_bounds("tabs-scroll").is_some());
}

#[gpui::test]
fn filter_and_sort_menus_persist_and_control_tab_navigation(cx: &mut TestAppContext) {
    let (mut boot, _requests, [home, project], [first, second, third]) = fixture();
    boot.state.rename_project(project, "Zulu").expect("rename");
    let directory = tempfile::tempdir().expect("project folder");
    let alpha = boot
        .state
        .add_project(directory.path().to_owned())
        .expect("project");
    boot.state.rename_project(alpha, "Alpha").expect("rename");
    let fourth = boot.state.open_terminal_tab(alpha).expect("tab");
    boot.state.select_tab(home, first).expect("select home");
    boot.settings
        .appearance
        .tab_focused_expanded
        .insert(alpha, true);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let path = boot.state_path.clone();
    let (view, window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    window.run_until_parked();
    click(window, "sidebar-project-sort");
    window.simulate_keystrokes("down down enter");
    window.run_until_parked();
    view.read_with(window, |model, _| {
        assert_eq!(model.navigation_tabs(), [first, second, fourth, third]);
    });
    click(window, "sidebar-project-filter");
    window.simulate_keystrokes("down down enter");
    window.run_until_parked();
    view.read_with(window, |model, _| {
        assert_eq!(model.navigation_tabs(), [first, second]);
    });
    let saved = Settings::load(&path.with_file_name("settings.toml")).expect("settings");
    assert!(saved.appearance.sidebar_focus);
    assert_eq!(saved.appearance.sidebar_project_order, ProjectOrder::Name);
    let state = view.read_with(window, |model, _| model.state.clone());
    let (mut boot, _requests) = stub_boot(state);
    boot.state_path = path;
    boot.settings = saved;
    let (restarted, restart_window) =
        window.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    restart_window.run_until_parked();
    restarted.read_with(restart_window, |model, _| {
        assert_eq!(model.navigation_tabs(), [first, second]);
    });
    click(restart_window, "sidebar-project-filter");
    restart_window.simulate_keystrokes("down enter");
    restart_window.run_until_parked();
    restarted.read_with(restart_window, |model, _| {
        assert_eq!(model.navigation_tabs(), [first, second, fourth, third]);
    });
    click(restart_window, "sidebar-project-sort");
    restart_window.simulate_keystrokes("down enter");
    restart_window.run_until_parked();
    restarted.read_with(restart_window, |model, _| {
        assert_eq!(model.navigation_tabs(), [first, second, third, fourth]);
    });
}

#[gpui::test]
fn switching_layouts_keeps_the_sidebar_frame_filter_and_order(cx: &mut TestAppContext) {
    let (mut boot, _requests, [home, project], _) = fixture();
    boot.state.rename_project(project, "Zulu").expect("rename");
    let alpha = boot.state.add_project(std::env::temp_dir()).expect("alpha");
    boot.state.rename_project(alpha, "Alpha").expect("rename");
    boot.state.select_project(home).expect("home");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    window.run_until_parked();
    let frame = window.debug_bounds("workspace-sidebar").expect("sidebar");
    let filter = window
        .debug_bounds("sidebar-project-filter")
        .expect("filter");
    let sort = window.debug_bounds("sidebar-project-sort").expect("sort");
    let layout = window.debug_bounds("layout-menu").expect("layout selector");
    let toggle = window
        .debug_bounds("sidebar-toggle")
        .expect("sidebar toggle");
    let titlebar = window
        .debug_bounds("titlebar-navigation")
        .expect("titlebar");
    assert!(titlebar.contains(&toggle.center()));
    assert_eq!(layout.center().y, sort.center().y);
    assert!(sort.right() <= layout.left());
    click(window, "sidebar-project-sort");
    window.simulate_keystrokes("down down enter");
    window.run_until_parked();
    click(window, "layout-menu");
    window.simulate_keystrokes("down enter");
    window.run_until_parked();
    assert_eq!(window.debug_bounds("workspace-sidebar"), Some(frame));
    assert_eq!(window.debug_bounds("sidebar-project-filter"), Some(filter));
    assert_eq!(window.debug_bounds("sidebar-project-sort"), Some(sort));
    view.read_with(window, |model, _| {
        assert_eq!(model.appearance.layout, AppLayout::ProjectFocused);
        assert_eq!(
            model
                .sidebar_projects()
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            [home, alpha, project]
        );
    });
    window.simulate_keystrokes("cmd-alt-]");
    window.run_until_parked();
    assert_eq!(
        view.read_with(window, |model, _| model.state.current_project().id),
        alpha
    );
    click(window, "sidebar-project-filter");
    window.simulate_keystrokes("down down enter");
    window.run_until_parked();
    click(window, "layout-menu");
    window.simulate_keystrokes("down down enter");
    window.run_until_parked();
    view.read_with(window, |model, _| {
        assert_eq!(model.appearance.layout, AppLayout::TabFocused);
        assert_eq!(
            model
                .sidebar_projects()
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
            [alpha]
        );
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).expect("settings");
        assert!(saved.appearance.sidebar_focus);
        assert_eq!(saved.appearance.sidebar_project_order, ProjectOrder::Name);
    });
    assert_eq!(window.debug_bounds("workspace-sidebar"), Some(frame));
}

#[gpui::test]
fn collapsed_style_applies_only_to_projects_and_restores_with_the_layout(cx: &mut TestAppContext) {
    for style in [SidebarCollapsedStyle::Icons, SidebarCollapsedStyle::Hidden] {
        let (mut boot, _requests, _, _) = fixture();
        boot.settings.appearance.sidebar_expanded = false;
        boot.settings.appearance.sidebar_collapsed_style = style;
        let path = boot.state_path.clone();
        let (view, window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        window.run_until_parked();
        for layout in [
            AppLayout::ProjectFocused,
            AppLayout::TabFocused,
            AppLayout::ProjectFocused,
        ] {
            view.update(window, |model, cx| model.set_layout(layout, cx));
            window.run_until_parked();
            view.read_with(window, |model, _| {
                assert!(!model.appearance.sidebar_expanded);
                let width = if layout == AppLayout::ProjectFocused
                    && style == SidebarCollapsedStyle::Icons
                {
                    44.0
                } else {
                    0.0
                };
                assert_eq!(px(model.sidebar_width()), px(width));
            });
        }
        let saved = Settings::load(&path.with_file_name("settings.toml")).expect("settings");
        assert_eq!(saved.appearance.sidebar_collapsed_style, style);
        assert!(!saved.appearance.sidebar_expanded);
        let state = view.read_with(window, |model, _| model.state.clone());
        let (mut boot, _requests) = stub_boot(state);
        boot.state_path = path;
        boot.settings = saved;
        let (restored, restart) =
            window.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        restart.run_until_parked();
        click(restart, "sidebar-toggle");
        restored.read_with(restart, |model, _| {
            assert!(model.appearance.sidebar_expanded);
            assert_eq!(px(model.sidebar_width()), px(270.0));
        });
    }
}

#[gpui::test]
fn project_chevrons_expand_without_selecting_and_tab_shortcuts_follow_visible_rows(
    cx: &mut TestAppContext,
) {
    let (boot, _requests, [home, project], [first, second, third]) = fixture();
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    click(cx, &format!("tab-project-toggle-{project}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(model.active_tab(), Some(first));
        assert_eq!(model.navigation_tabs(), [first, second]);
        assert!(
            !Settings::load(&model.path.with_file_name("settings.toml"))
                .expect("settings")
                .appearance
                .tab_focused_expanded[&project]
        );
    });
    click(cx, &format!("tab-project-toggle-{project}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(model.active_tab(), Some(first));
        assert_eq!(model.navigation_tabs(), [first, second, third]);
    });
    cx.simulate_keystrokes("cmd-3");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.active_tab(), Some(third));
    });
    click(cx, &format!("sidebar-tab-{second}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(model.active_tab(), Some(second));
    });
    view.update(cx, |model, cx| model.cycle_tab(true, cx));
    assert_eq!(
        view.read_with(cx, |model, _| model.active_tab()),
        Some(third)
    );
}

#[gpui::test]
fn project_rows_select_and_expand_while_chevrons_can_hide_the_active_projects_tabs(
    cx: &mut TestAppContext,
) {
    let (mut boot, _requests, [home, project], [first, second, third]) = fixture();
    boot.settings
        .appearance
        .tab_focused_expanded
        .insert(project, false);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    for _ in 0..2 {
        click(cx, &format!("tab-project-{project}"));
        view.read_with(cx, |model, _| {
            assert_eq!(model.state.current_project().id, project);
            assert_eq!(model.active_tab(), Some(third));
            assert_eq!(model.navigation_tabs(), [first, second, third]);
        });
    }
    let active_state = view.read_with(cx, |model, _| model.state.clone());
    click(cx, &format!("tab-project-toggle-{project}"));
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state, active_state);
        assert_eq!(model.active_tab(), Some(third));
        assert_eq!(model.navigation_tabs(), [first, second]);
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).expect("settings");
        assert!(!saved.appearance.tab_focused_expanded[&project]);
        assert_eq!(store::load(&model.path).expect("saved state"), active_state);
    });
    click(cx, &format!("tab-project-{project}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state, active_state);
        assert_eq!(model.navigation_tabs(), [first, second, third]);
    });
    click(cx, &format!("tab-project-{home}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(model.active_tab(), Some(first));
    });
}

#[gpui::test]
fn selecting_an_empty_project_does_not_create_tabs(cx: &mut TestAppContext) {
    let (mut boot, _requests, [home, _], _) = fixture();
    let empty = boot
        .state
        .add_project(std::env::temp_dir())
        .expect("empty project");
    boot.state.select_project(home).expect("select home");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    click(cx, &format!("tab-project-{empty}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, empty);
        assert!(model.active_tab().is_none());
        assert!(model.state.current_project().tabs.is_empty());
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).expect("settings");
        assert!(saved.appearance.tab_focused_expanded[&empty]);
    });
}

#[gpui::test]
fn closing_an_inactive_project_tab_does_not_steal_focus_and_new_tabs_use_their_project(
    cx: &mut TestAppContext,
) {
    let (boot, _requests, [home, project], [first, _, third]) = fixture();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    click(cx, &format!("sidebar-close-{third}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_tab(), Some(first));
        assert_eq!(model.state.current_project().id, home);
        assert!(
            model
                .state
                .project(project)
                .expect("project")
                .tabs
                .is_empty()
        );
    });
    let header = cx
        .debug_bounds(format!("tab-project-{project}").leak())
        .expect("project header");
    cx.simulate_event(MouseMoveEvent {
        position: header.center(),
        ..Default::default()
    });
    cx.run_until_parked();
    click(cx, &format!("project-new-tab-{project}"));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_ne!(model.active_tab(), Some(third));
    });
}

#[gpui::test]
fn vertical_drag_reorders_only_within_its_project_and_persists(cx: &mut TestAppContext) {
    let (boot, _requests, [home, _], [first, second, third]) = fixture();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let from = cx
        .debug_bounds(format!("sidebar-tab-{first}").leak())
        .expect("first")
        .center();
    let to = cx
        .debug_bounds(format!("sidebar-tab-{second}").leak())
        .expect("second")
        .center();
    let other = cx
        .debug_bounds(format!("sidebar-tab-{third}").leak())
        .expect("third")
        .center();
    cx.simulate_event(MouseDownEvent {
        position: from,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    for position in [from + point(px(0.0), px(6.0)), other, to] {
        cx.simulate_event(MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        });
        cx.run_until_parked();
    }
    cx.simulate_event(MouseUpEvent {
        position: to,
        button: MouseButton::Left,
        ..Default::default()
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model
                .state
                .project(home)
                .expect("home")
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect::<Vec<_>>(),
            [second, first]
        );
        assert_eq!(model.active_tab(), Some(first));
        assert_eq!(store::load(&model.path).expect("saved layout"), model.state);
    });
}

#[gpui::test]
fn later_appearance_edits_preserve_another_instances_saved_sidebar_choice(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _requests) = stub_boot(state.clone());
    let path = boot.state_path.clone();
    boot.settings
        .appearance
        .save(&path.with_file_name("settings.toml"))
        .expect("initial appearance");
    let (mut other_boot, _requests) = stub_boot(state);
    other_boot.state_path = path.clone();
    other_boot.settings =
        Settings::load(&path.with_file_name("settings.toml")).expect("initial settings");
    let (first, first_window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let (second, second_window) =
        first_window.add_window_view(|window, cx| AppModel::new(other_boot, window, cx));
    first.update(second_window, |model, cx| {
        model.appearance.sidebar_expanded = !model.appearance.sidebar_expanded;
        model.save_appearance(cx);
    });
    second.update(second_window, |model, cx| {
        model.change_preference(crate::views::settings::Change::StatusBar(false), cx);
    });
    let saved = Settings::load(&path.with_file_name("settings.toml")).expect("saved settings");
    assert!(saved.appearance.sidebar_expanded);
    assert!(!saved.appearance.status_bar_visible);
    first.update(second_window, |model, cx| {
        model.appearance.sidebar_expanded = !model.appearance.sidebar_expanded;
        model.save_appearance(cx);
    });
    second.update(second_window, |model, cx| {
        model.appearance.dark_theme = "Dracula".into();
        model.save_appearance(cx);
    });
    let saved = Settings::load(&path.with_file_name("settings.toml")).expect("restart settings");
    assert!(!saved.appearance.sidebar_expanded);
    assert_eq!(saved.appearance.dark_theme, "Dracula");
    assert!(!saved.appearance.status_bar_visible);
}

#[gpui::test]
fn selecting_a_worktree_reveals_its_saved_parent_and_tabs(cx: &mut TestAppContext) {
    let (mut boot, _requests, [home, project], _) = fixture();
    let directory = tempfile::tempdir().expect("worktree folder");
    std::fs::write(directory.path().join(".git"), "gitdir: /tmp/unused").expect("worktree marker");
    let child = boot
        .state
        .add_project(directory.path().to_owned())
        .expect("child");
    let tab = boot.state.open_terminal_tab(child).expect("child tab");
    boot.state.select_project(home).expect("home");
    let mut stored = serde_json::to_value(&boot.state).expect("state");
    let child_record = stored["projects"]
        .as_array_mut()
        .expect("projects")
        .iter_mut()
        .find(|entry| entry["id"] == serde_json::to_value(child).expect("id"))
        .expect("child record");
    child_record["parent_id"] = serde_json::to_value(project).expect("parent");
    child_record["kind"] =
        serde_json::to_value(muxy_app_core::ProjectKind::Worktree).expect("kind");
    boot.state = serde_json::from_value(stored).expect("worktree state");
    boot.settings
        .appearance
        .tab_focused_expanded
        .extend([(project, false), (child, false)]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(!model.navigation_tabs().contains(&tab));
    });
    view.update(cx, |model, cx| model.select_tab(tab, cx));
    cx.run_until_parked();
    let parent_bounds = cx
        .debug_bounds(format!("tab-project-{project}").leak())
        .expect("parent row");
    let child_bounds = cx
        .debug_bounds(format!("tab-project-{child}").leak())
        .expect("child row");
    assert!(child_bounds.origin.x > parent_bounds.origin.x);
    assert!(child_bounds.origin.y > parent_bounds.origin.y);
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, child);
        assert!(model.navigation_tabs().contains(&tab));
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).expect("saved");
        assert!(saved.appearance.tab_focused_expanded[&project]);
        assert!(saved.appearance.tab_focused_expanded[&child]);
    });
}
