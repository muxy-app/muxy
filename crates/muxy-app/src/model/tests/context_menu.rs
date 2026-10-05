use super::*;
use gpui::{Bounds, MouseButton, Pixels, Point, point};

fn open(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext, ProjectId) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    cx.run_until_parked();
    (view, cx, project)
}

fn bounds(cx: &mut VisualTestContext, selector: &str) -> Bounds<Pixels> {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds(selector.to_owned().leak())
        .unwrap_or_else(|| panic!("missing {selector}"))
}

fn right_click_project(cx: &mut VisualTestContext) {
    let row = bounds(cx, "project-row-1").center();
    cx.simulate_mouse_down(row, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(row, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
}

fn move_to(cx: &mut VisualTestContext, position: Point<Pixels>) {
    cx.simulate_mouse_move(position, None, Modifiers::none());
    cx.run_until_parked();
}

fn hover(cx: &mut VisualTestContext, label: &str) -> Point<Pixels> {
    let position = bounds(cx, &format!("menu-label-{label}")).center();
    move_to(cx, position);
    position
}

fn outline(view: &Entity<AppModel>, cx: &VisualTestContext) -> Vec<Vec<String>> {
    view.read_with(cx, |model, _| model.menu_outline())
}

/// Each open level's highlighted row, or `·` when none is.
fn path(view: &Entity<AppModel>, cx: &VisualTestContext) -> Vec<String> {
    outline(view, cx)
        .iter()
        .map(|rows| {
            rows.iter()
                .find_map(|row| row.strip_prefix('>'))
                .map_or_else(
                    || "·".to_owned(),
                    |row| row.trim_start_matches('✓').to_owned(),
                )
        })
        .collect()
}

#[gpui::test]
fn hovering_a_row_opens_its_submenu_beside_the_menu_and_keeps_the_menu_open(
    cx: &mut TestAppContext,
) {
    let (view, cx, project) = open(cx);
    right_click_project(cx);
    let color = hover(cx, "Color");
    assert_eq!(path(&view, cx), ["Color", "·"]);
    assert!(outline(&view, cx)[1][0].ends_with("Red"));
    let menu = bounds(cx, "context-menu");
    let submenu = bounds(cx, "context-submenu-1");
    let row = bounds(cx, "menu-item-5");
    let first = bounds(cx, "menu-item-1-0");
    assert!(submenu.left() >= menu.right() - px(8.0));
    assert!(submenu.left() < menu.right());
    assert_eq!(first.top(), row.top());
    let chevron = bounds(cx, "menu-chevron-Color");
    assert!(chevron.right() <= row.right() && chevron.left() > row.center().x);
    let swatch = bounds(cx, "menu-swatch-Red");
    assert!(swatch.right() < bounds(cx, "menu-label-Red").left());

    move_to(cx, point(color.x, color.y + px(25.0)));
    assert_eq!(
        path(&view, cx),
        ["Workspaces", "·"],
        "moving down switches at once"
    );
    assert_eq!(outline(&view, cx)[1], ["New Workspace…"]);

    hover(cx, "Color");
    let workspaces = bounds(cx, "menu-item-6");
    move_to(cx, point(color.x + px(60.0), workspaces.center().y));
    assert_eq!(
        path(&view, cx),
        ["Color", "·"],
        "heading into the submenu keeps it open"
    );
    assert!(outline(&view, cx)[1][0].ends_with("Red"));
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(path(&view, cx), ["Workspaces", "·"], "resting switches");

    hover(cx, "Color");
    let orange = hover(cx, "Orange");
    assert_eq!(path(&view, cx), ["Color", "Orange"]);
    cx.simulate_click(orange, Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(
            model
                .state
                .project(project)
                .expect("project")
                .color
                .as_str(),
            muxy_app_core::PROJECT_COLORS[1].1
        );
    });
}

#[gpui::test]
fn arrow_keys_enter_and_leave_submenus_and_escape_backs_out_one_level(cx: &mut TestAppContext) {
    let (view, cx, project) = open(cx);
    right_click_project(cx);
    hover(cx, "Workspaces");
    assert_eq!(outline(&view, cx)[1], ["New Workspace…"]);
    cx.simulate_keystrokes("right");
    assert_eq!(outline(&view, cx)[1], [">New Workspace…"]);
    cx.simulate_keystrokes("left");
    assert_eq!(path(&view, cx), ["Workspaces"]);
    cx.simulate_keystrokes("enter");
    assert_eq!(path(&view, cx), ["Workspaces", "New Workspace…"]);
    cx.simulate_keystrokes("left");
    cx.simulate_keystrokes("down right");
    assert_eq!(path(&view, cx), ["Worktrees", "Show Worktrees"]);
    cx.simulate_keystrokes("escape");
    assert_eq!(outline(&view, cx).len(), 1);
    cx.simulate_keystrokes("escape");
    assert!(view.read_with(cx, |model, _| model.overlay.is_none()));

    right_click_project(cx);
    hover(cx, "Workspaces");
    cx.simulate_keystrokes("right enter");
    cx.simulate_input("Work");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let workspace = &model.state.workspaces()[0];
        assert_eq!(workspace.name, "Work");
        assert!(workspace.projects.contains(&project));
    });
}

#[gpui::test]
fn worktree_items_only_appear_for_folders_that_may_be_git_repositories(cx: &mut TestAppContext) {
    let (view, cx, project) = open(cx);
    let top_level = |view: &Entity<AppModel>, cx: &VisualTestContext| outline(view, cx)[0].clone();
    right_click_project(cx);
    assert!(top_level(&view, cx).contains(&"Worktrees".to_owned()));
    cx.simulate_keystrokes("escape");
    view.update(cx, |model, _| {
        model.git.projects.entry(project).or_default().loaded = true;
    });
    right_click_project(cx);
    let rows = top_level(&view, cx);
    assert!(rows.contains(&"Workspaces".to_owned()));
    assert!(!rows.contains(&"Worktrees".to_owned()));
}
