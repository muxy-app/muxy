use super::worktrees::pruning_fixture;
use super::*;
use muxy_app_core::settings::{AppLayout, Appearance};

/// Projects `with_tab`, `current`, and `idle`, where only `with_tab` has a tab.
fn three_projects() -> (AppState, [ProjectId; 3], TabId) {
    let mut state = AppState::bootstrap().expect("state");
    let ids = [(); 3].map(|()| {
        state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project")
    });
    let tab = state.open_terminal_tab(ids[0]).expect("tab");
    state.select_project(ids[1]).expect("current");
    (state, ids, tab)
}

fn open(
    state: AppState,
    layout: AppLayout,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext) {
    open_with_sidebar(state, layout, true, cx)
}

fn open_with_sidebar(
    state: AppState,
    layout: AppLayout,
    expanded: bool,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.layout = layout;
    boot.settings.appearance.sidebar_expanded = expanded;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();
    (view, cx)
}

fn listed(model: &AppModel) -> Vec<ProjectId> {
    model
        .tab_sidebar_projects()
        .0
        .iter()
        .map(|project| project.id)
        .collect()
}

fn row(cx: &mut VisualTestContext, selector: String) -> Option<gpui::Bounds<gpui::Pixels>> {
    cx.debug_bounds(selector.leak())
}

#[gpui::test]
fn projects_without_tabs_wait_behind_a_toggle_even_when_current(cx: &mut TestAppContext) {
    let (state, [with_tab, current, idle], _) = three_projects();
    let (view, cx) = open(state, AppLayout::TabFocused, cx);
    view.read_with(cx, |model, _| {
        let home = model.state.home().id;
        assert!(model.state.home().tabs.is_empty());
        assert_eq!(model.state.current_project().id, current);
        assert_eq!(listed(model), [with_tab]);
        assert_eq!(
            model.tab_sidebar_projects().1,
            3,
            "Home, current, and idle are hidden"
        );
        assert!(!listed(model).contains(&home));
    });
    assert!(row(cx, format!("tab-project-{idle}")).is_none());

    let toggle = cx
        .debug_bounds("tab-sidebar-projects-without-tabs")
        .expect("toggle");
    cx.simulate_click(toggle.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(row(cx, format!("tab-project-{idle}")).is_some());
    view.read_with(cx, |model, _| {
        assert_eq!(listed(model).len(), 4);
        assert_eq!(model.tab_sidebar_projects().1, 3);
    });

    let toggle = cx
        .debug_bounds("tab-sidebar-projects-without-tabs")
        .expect("toggle");
    cx.simulate_click(toggle.center(), Modifiers::none());
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert!(!listed(model).contains(&idle));
        model.set_layout(AppLayout::AgentsFocused, cx);
        assert!(
            listed(model).is_empty(),
            "a terminal tab without an agent does not count"
        );
        assert_eq!(model.tab_sidebar_projects().1, 4);
    });
}

#[gpui::test]
fn closing_the_last_tab_hides_the_current_project_at_once(cx: &mut TestAppContext) {
    let (state, [with_tab, _, _], tab) = three_projects();
    let (view, cx) = open(state, AppLayout::TabFocused, cx);
    view.update(cx, |model, cx| {
        model.select_project(with_tab, cx);
        assert_eq!(listed(model), [with_tab]);
        model.close_tab(tab, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, with_tab);
        assert!(listed(model).is_empty());
    });
}

#[gpui::test]
fn project_rows_line_up_with_the_header_and_focus_from_their_menu(cx: &mut TestAppContext) {
    let (mut state, [with_tab, current, _], tab) = three_projects();
    state.open_terminal_tab(current).expect("tab");
    let (view, cx) = open(state, AppLayout::TabFocused, cx);
    view.update(cx, |model, cx| {
        model.appearance.tab_focused_expanded.insert(with_tab, true);
        cx.notify();
    });
    cx.run_until_parked();

    let header = row(cx, format!("tab-project-{with_tab}")).expect("header");
    let chevron = row(cx, format!("tab-project-toggle-{with_tab}")).expect("chevron");
    let title = row(cx, format!("tab-project-title-{with_tab}")).expect("project title");
    let tab_icon = row(cx, format!("sidebar-tab-icon-{tab}")).expect("tab icon");
    let accessory = row(cx, format!("sidebar-tab-accessory-{tab}")).expect("accessory");
    let filter = cx.debug_bounds("sidebar-project-filter").expect("filter");
    let layout = cx.debug_bounds("layout-menu").expect("layout menu");
    assert_eq!(
        title.left(),
        tab_icon.left(),
        "project names start where tab icons do"
    );
    assert!(title.left() > filter.left() && title.left() < filter.center().x);
    assert_eq!(
        chevron.right(),
        layout.right(),
        "the chevron lines up with the layout button"
    );
    assert_eq!(accessory.right(), layout.right());
    let tab_row = row(cx, format!("sidebar-tab-{tab}")).expect("tab row");
    let metrics = view.read_with(cx, |model, _| model.metrics);
    assert_eq!(
        tab_row.size.height,
        metrics.control_small() + metrics.spacing2() * 2.0,
        "tabs are sized like agents panel rows"
    );
    for id in [with_tab, current] {
        assert!(
            row(cx, format!("project-new-tab-{id}")).is_some(),
            "+ shows on every project"
        );
    }
    assert!(chevron.left() > header.center().x);

    cx.simulate_click(title.center(), Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.current_project().id,
            current,
            "a project name does not select the project"
        );
        assert!(
            !model.appearance.tab_focused_expanded[&with_tab],
            "a project name toggles its tabs"
        );
    });
    let title = row(cx, format!("tab-project-title-{with_tab}")).expect("project title");
    cx.simulate_click(title.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |model, _| model.appearance.tab_focused_expanded
            [&with_tab])
    );
    cx.simulate_click(chevron.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        !view.read_with(cx, |model, _| model.appearance.tab_focused_expanded
            [&with_tab])
    );
    let collapsed = row(cx, format!("tab-project-toggle-{with_tab}")).expect("chevron");
    cx.simulate_click(collapsed.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |model, _| model.appearance.tab_focused_expanded
            [&with_tab])
    );

    cx.simulate_mouse_down(header.center(), gpui::MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let focus = cx
        .debug_bounds("menu-label-Focus Project")
        .expect("focus item");
    cx.simulate_click(focus.center(), Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.appearance.sidebar_focus);
        assert_eq!(model.state.current_project().id, with_tab);
        assert_eq!(listed(model), [with_tab]);
    });
    cx.simulate_mouse_down(header.center(), gpui::MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let show = cx
        .debug_bounds("menu-label-Show All Projects")
        .expect("show all item");
    cx.simulate_click(show.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(!view.read_with(cx, |model, _| model.appearance.sidebar_focus));
}

#[gpui::test]
fn a_project_without_tabs_hides_even_when_its_worktree_has_tabs(cx: &mut TestAppContext) {
    let (mut state, root, [first, second, with_tab], _directory) = pruning_fixture();
    for tab in state.project(root).expect("root").tabs.clone() {
        state.close_tab(root, tab.id).expect("close");
    }
    state.open_terminal_tab(with_tab).expect("tab");
    let home = state.home().id;
    state.select_project(home).expect("home");
    let (view, cx) = open(state, AppLayout::TabFocused, cx);
    view.read_with(cx, |model, _| {
        let projects = listed(model);
        assert!(projects.contains(&with_tab));
        for hidden in [root, first, second] {
            assert!(!projects.contains(&hidden));
        }
    });
}

#[gpui::test]
fn the_shown_choice_lasts_the_session_unless_remembered(cx: &mut TestAppContext) {
    let (state, _, _) = three_projects();
    let (view, window) = open(state, AppLayout::TabFocused, cx);
    let saved = |model: &AppModel| {
        Appearance::load(&model.path.with_file_name("settings.toml"))
            .expect("settings")
            .projects_without_tabs_shown
    };
    view.update(window, |model, cx| {
        model.toggle_projects_without_tabs(cx);
        assert!(model.projects_without_tabs_shown);
        assert!(!saved(model), "off by default, the choice is not saved");

        model.appearance.remember_projects_without_tabs = true;
        model.save_appearance(cx);
        model.toggle_projects_without_tabs(cx);
        model.toggle_projects_without_tabs(cx);
        assert!(saved(model), "remembered, the choice is saved");
    });

    for remember in [true, false] {
        let (state, _, _) = three_projects();
        let (mut boot, _requests) = stub_boot(state);
        boot.settings.appearance.remember_projects_without_tabs = remember;
        boot.settings.appearance.projects_without_tabs_shown = true;
        let (view, window) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        assert_eq!(
            view.read_with(window, |model, _| model.projects_without_tabs_shown),
            remember
        );
    }
}

#[gpui::test]
fn project_rows_are_tab_sized_with_one_line_and_worktrees_line_up(cx: &mut TestAppContext) {
    let (state, root, [worktree, ..], _directory) = pruning_fixture();
    let (view, cx) = open(state, AppLayout::ProjectFocused, cx);
    view.update(cx, |model, cx| {
        model.expanded_worktrees.insert(root);
        cx.notify();
    });
    cx.run_until_parked();
    let metrics = view.read_with(cx, |model, _| model.metrics);
    let row_height = metrics.control_small() + metrics.spacing2() * 2.0;
    let index = view.read_with(cx, |model, _| {
        model
            .sidebar_projects()
            .iter()
            .position(|project| project.id == root)
            .expect("listed")
    });
    let project = row(cx, format!("project-row-{index}")).expect("project row");
    let worktree = row(cx, format!("worktree-{worktree}")).expect("worktree row");
    assert_eq!(project.size.height, row_height);
    assert_eq!(worktree.size.height, row_height);
    assert!(row(cx, format!("project-worktree-label-{root}")).is_none());
}

#[gpui::test]
fn collapsed_project_tiles_fill_their_squares(cx: &mut TestAppContext) {
    let (state, [with_tab, ..], _) = three_projects();
    let (view, cx) = open(state, AppLayout::ProjectFocused, cx);
    view.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = false;
        model.appearance.sidebar_collapsed_style =
            muxy_app_core::settings::SidebarCollapsedStyle::Icons;
        cx.notify();
    });
    cx.run_until_parked();
    let metrics = view.read_with(cx, |model, _| model.metrics);
    let sidebar = cx.debug_bounds("workspace-sidebar").expect("sidebar");
    assert_eq!(sidebar.size.width, metrics.sidebar_collapsed_width());
    let square = metrics.scaled(34.0);
    for selector in [
        "project-row-0",
        "remote-servers-section",
        "sidebar-add-project",
    ] {
        let bounds = cx.debug_bounds(selector).expect(selector);
        assert_eq!(bounds.size, size(square, square), "{selector}");
    }
    let tile = cx
        .debug_bounds(format!("project-tile-{with_tab}").leak())
        .expect("tile");
    assert_eq!(tile.size, size(metrics.icon_xxl(), metrics.icon_xxl()));
}

#[gpui::test]
fn the_titlebar_opens_tabs_unless_the_sidebar_shows_them(cx: &mut TestAppContext) {
    for (layout, expanded, titlebar) in [
        (AppLayout::ProjectFocused, true, true),
        (AppLayout::ProjectFocused, false, true),
        (AppLayout::TabFocused, true, false),
        (AppLayout::TabFocused, false, true),
        (AppLayout::AgentsFocused, true, false),
        (AppLayout::AgentsFocused, false, true),
    ] {
        let (state, [with_tab, _, _], _) = three_projects();
        let (view, cx) = open_with_sidebar(state, layout, expanded, cx);
        view.update(cx, |model, cx| model.select_project(with_tab, cx));
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("new-tab-button").is_some(),
            titlebar,
            "{layout:?}, sidebar expanded: {expanded}"
        );
        if layout == AppLayout::TabFocused && expanded {
            assert!(
                row(cx, format!("project-new-tab-{with_tab}")).is_some(),
                "the sidebar opens tabs instead"
            );
        }
    }
}
