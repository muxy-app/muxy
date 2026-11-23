use std::time::Duration;

use gpui::{
    Bounds, Modifiers, MouseButton, MouseExitEvent, Pixels, Point, TestAppContext,
    VisualTestContext, point, px, size,
};
use muxy_app_core::settings::SidebarCollapsedStyle;

use super::*;
use crate::model::sidebar_peek::{DWELL, GRACE, SLIDE};
use crate::views::workspace::ToggleSidebar;

const EDGE: Point<Pixels> = point(px(15.0), px(400.0));
const AWAY: Point<Pixels> = point(px(800.0), px(400.0));
const INSIDE: Point<Pixels> = point(px(30.0), px(400.0));

#[derive(Clone, Copy)]
struct Sidebar {
    expanded: bool,
    collapsed: SidebarCollapsedStyle,
    auto_hide: bool,
}

const FLOATING: Sidebar = Sidebar {
    expanded: true,
    collapsed: SidebarCollapsedStyle::Icons,
    auto_hide: true,
};

fn open(sidebar: Sidebar, cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    open_boot(boot, sidebar, cx)
}

fn open_boot(
    mut boot: Boot,
    sidebar: Sidebar,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext) {
    boot.settings.appearance.sidebar_expanded = sidebar.expanded;
    boot.settings.appearance.sidebar_collapsed_style = sidebar.collapsed;
    boot.settings.appearance.sidebar_auto_hide = sidebar.auto_hide;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (view, cx)
}

fn hover(cx: &mut VisualTestContext, position: Point<Pixels>) {
    cx.simulate_mouse_move(position, None, Modifiers::none());
}

fn wait(cx: &mut VisualTestContext, by: Duration) {
    let step = Duration::from_millis(10);
    let mut left = by;
    while !left.is_zero() {
        let tick = left.min(step);
        cx.executor().advance_clock(tick);
        cx.run_until_parked();
        left -= tick;
    }
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn peeking(view: &Entity<AppModel>, cx: &VisualTestContext) -> bool {
    view.read_with(cx, |model, _| model.sidebar_peeking())
}

fn island(cx: &mut VisualTestContext) -> Option<Bounds<Pixels>> {
    cx.debug_bounds("sidebar-island")
}

fn content_left(cx: &mut VisualTestContext) -> Pixels {
    cx.debug_bounds("workspace-content")
        .expect("workspace content")
        .origin
        .x
}

fn reveal(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    hover(cx, EDGE);
    wait(cx, DWELL + SLIDE);
    island(cx).expect("the sidebar slides in")
}

#[gpui::test]
fn an_auto_hiding_expanded_sidebar_floats_in_from_near_the_edge(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    let left = content_left(cx);
    assert_eq!(left, px(0.0), "nothing is docked while the sidebar hides");
    assert!(island(cx).is_none());
    assert!(cx.debug_bounds("workspace-sidebar").is_none());

    hover(cx, EDGE);
    wait(cx, DWELL / 2);
    assert!(!peeking(&view, cx), "the pointer must rest before it opens");

    wait(cx, DWELL + SLIDE);
    let card = island(cx).expect("island");
    let natural = view.read_with(cx, |model, _| px(model.sidebar_natural_width()));
    assert!(
        card.origin.x > px(0.0),
        "the island floats off the window edge"
    );
    assert!(
        card.size.width >= natural,
        "the island has the expanded width"
    );
    assert!(
        cx.debug_bounds("sidebar-project-filter").is_some(),
        "the island renders the expanded sidebar"
    );
    assert_eq!(content_left(cx), left, "the content does not move");
}

#[gpui::test]
fn webview_coverage_follows_the_sidebar_as_it_slides_in_and_out(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    view.read_with(cx, |model, _| {
        let edge = model.sidebar_peek.edge.get().expect("hover target");
        let near_edge = point(edge.left() + px(1.0), edge.center().y);
        let regions = model.webviews.occlusions.borrow();
        assert!(
            regions
                .iter()
                .any(|(_, cover)| cover.pointer_only && cover.bounds.contains(&near_edge)),
            "native pages pass edge input to the sidebar"
        );
        assert!(
            regions
                .iter()
                .all(|(_, cover)| cover.pointer_only || !cover.bounds.contains(&near_edge)),
            "the hover target leaves the underlying page visible"
        );
    });

    for _ in 0..2 {
        cx.dispatch_action(ToggleSidebar);
        wait(cx, SLIDE / 2);
        let card = island(cx).expect("sidebar in motion");
        let offset = card.size.width / 4.0;
        view.read_with(cx, |model, _| {
            let regions = model.webviews.occlusions.borrow();
            let covered = |x| {
                regions.iter().any(|(_, cover)| {
                    !cover.pointer_only && cover.bounds.contains(&point(x, card.center().y))
                })
            };
            assert!(
                covered(card.right() - offset),
                "the sidebar covers the page"
            );
            assert!(
                !covered(card.right() + offset),
                "the page stays visible beyond the moving sidebar"
            );
        });
        wait(cx, SLIDE / 2);
    }
}

#[gpui::test]
fn an_auto_hiding_icon_sidebar_floats_as_a_narrow_column(cx: &mut TestAppContext) {
    let (view, cx) = open(
        Sidebar {
            expanded: false,
            ..FLOATING
        },
        cx,
    );
    assert_eq!(content_left(cx), px(0.0), "the icon column is not docked");
    let card = reveal(cx);
    let (collapsed, expanded) = view.read_with(cx, |model, _| {
        (
            model.metrics.sidebar_collapsed_width(),
            model.metrics.sidebar_expanded_width(),
        )
    });
    assert!(card.size.width >= collapsed && card.size.width < expanded);
    assert!(cx.debug_bounds("sidebar-project-filter").is_none());
    assert_eq!(content_left(cx), px(0.0));
}

#[gpui::test]
fn sidebars_that_are_docked_or_hidden_never_float(cx: &mut TestAppContext) {
    let docked = Sidebar {
        auto_hide: false,
        ..FLOATING
    };
    let hidden = Sidebar {
        expanded: false,
        collapsed: SidebarCollapsedStyle::Hidden,
        auto_hide: true,
    };
    for sidebar in [docked, hidden] {
        let (view, cx) = open(sidebar, cx);
        hover(cx, EDGE);
        wait(cx, DWELL + SLIDE);
        assert!(!peeking(&view, cx));
        assert!(island(cx).is_none());
    }
}

#[gpui::test]
fn a_quick_pass_along_the_edge_does_not_open_the_sidebar(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    hover(cx, EDGE);
    wait(cx, DWELL / 2);
    hover(cx, AWAY);
    wait(cx, DWELL * 4);
    assert!(!peeking(&view, cx));
    assert!(island(cx).is_none());
}

#[gpui::test]
fn leaving_slides_the_island_away_after_a_grace_period(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    let shown = reveal(cx);

    hover(cx, AWAY);
    wait(cx, GRACE / 2);
    assert_eq!(
        island(cx),
        Some(shown),
        "a brief exit keeps the island in place"
    );
    hover(cx, INSIDE);
    wait(cx, GRACE * 2);
    assert_eq!(island(cx), Some(shown), "coming back cancels the hide");

    hover(cx, AWAY);
    wait(cx, GRACE + SLIDE / 2);
    let sliding = island(cx).expect("sliding island");
    assert!(
        sliding.origin.x < shown.origin.x,
        "the island slides out to the left"
    );
    wait(cx, SLIDE);
    assert!(!peeking(&view, cx));
}

#[gpui::test]
fn the_pointer_leaving_the_window_hides_the_island(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    reveal(cx);
    cx.simulate_event(MouseExitEvent {
        position: point(px(-4.0), px(400.0)),
        pressed_button: None,
        modifiers: Modifiers::none(),
    });
    wait(cx, GRACE + SLIDE);
    assert!(!peeking(&view, cx));
}

#[gpui::test]
fn an_open_menu_holds_the_island_until_it_closes(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    reveal(cx);
    let filter = cx
        .debug_bounds("sidebar-project-filter")
        .expect("workspace filter");
    cx.simulate_click(filter.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| model.overlay.is_some()));

    hover(cx, AWAY);
    wait(cx, GRACE * 4);
    assert!(peeking(&view, cx), "the island stays under its menu");

    view.update(cx, AppModel::dismiss_overlay);
    wait(cx, GRACE * 2 + SLIDE);
    assert!(!peeking(&view, cx), "it leaves once the menu closes");
}

#[gpui::test]
fn a_held_button_keeps_the_island_until_it_is_released(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    reveal(cx);
    cx.simulate_mouse_move(AWAY, MouseButton::Left, Modifiers::none());
    wait(cx, GRACE * 4);
    assert!(peeking(&view, cx), "a drag keeps the island open");

    cx.simulate_mouse_up(AWAY, MouseButton::Left, Modifiers::none());
    wait(cx, GRACE * 2 + SLIDE);
    assert!(
        !peeking(&view, cx),
        "releasing ends the hold without another move"
    );
}

#[gpui::test]
fn an_inactive_window_neither_keeps_nor_opens_the_island(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    reveal(cx);
    cx.deactivate_window();
    wait(cx, SLIDE);
    assert!(!peeking(&view, cx), "switching away hides the island");

    hover(cx, AWAY);
    hover(cx, EDGE);
    wait(cx, DWELL + SLIDE);
    assert!(
        !peeking(&view, cx),
        "the edge of a background window does not open it"
    );
}

#[gpui::test]
fn toggle_sidebar_shows_and_hides_the_island_without_ending_auto_hide(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    hover(cx, AWAY);

    cx.dispatch_action(ToggleSidebar);
    wait(cx, GRACE * 4);
    assert!(
        peeking(&view, cx),
        "the island stays while the pointer is elsewhere"
    );
    assert!(island(cx).is_some());
    view.read_with(cx, |model, _| {
        assert!(model.appearance.sidebar_auto_hide);
        assert!(model.appearance.sidebar_expanded);
    });
    assert_eq!(content_left(cx), px(0.0), "showing it does not dock it");

    cx.dispatch_action(ToggleSidebar);
    wait(cx, SLIDE * 2);
    assert!(!peeking(&view, cx));
    view.read_with(cx, |model, _| {
        assert!(model.appearance.sidebar_auto_hide);
        assert!(model.appearance.sidebar_expanded);
    });
}

#[gpui::test]
fn clicking_the_content_hides_an_island_opened_with_toggle_sidebar(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    hover(cx, AWAY);
    cx.dispatch_action(ToggleSidebar);
    wait(cx, SLIDE);
    assert!(peeking(&view, cx));

    cx.simulate_click(AWAY, Modifiers::none());
    wait(cx, SLIDE * 2);
    assert!(!peeking(&view, cx));
}

#[gpui::test]
fn toggle_sidebar_on_a_hidden_collapsed_sidebar_expands_it_into_the_island(
    cx: &mut TestAppContext,
) {
    let (view, cx) = open(
        Sidebar {
            expanded: false,
            collapsed: SidebarCollapsedStyle::Hidden,
            auto_hide: true,
        },
        cx,
    );
    hover(cx, AWAY);
    cx.dispatch_action(ToggleSidebar);
    wait(cx, SLIDE);
    assert!(view.read_with(cx, |model, _| model.appearance.sidebar_expanded));
    assert!(peeking(&view, cx));
    assert!(cx.debug_bounds("sidebar-project-filter").is_some());
}

#[gpui::test]
fn an_extension_sidebar_is_shown_only_while_the_island_is(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("directory");
    let directory = root.path().join("extensions/workspace-nav");
    std::fs::create_dir_all(&directory).expect("extension");
    std::fs::write(directory.join("index.html"), "<!doctype html>").expect("page");
    std::fs::write(
        directory.join("package.json"),
        serde_json::json!({
            "name": "workspace-nav",
            "version": "0.1.0",
            "muxy": { "sidebar": { "id": "main", "entry": "index.html" } }
        })
        .to_string(),
    )
    .expect("manifest");
    muxy_app_core::extensions::Registry::load(root.path())
        .set_enabled("workspace-nav", true)
        .expect("enabled");
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.state_path = root.path().join("state.json");
    boot.settings.appearance.extension_sidebar = "workspace-nav".into();
    let (view, cx) = open_boot(boot, FLOATING, cx);
    let shown = |cx: &VisualTestContext| {
        view.read_with(cx, |model, _| {
            model.extension_sidebar_active() && model.extension_sidebar_shown()
        })
    };
    assert!(view.read_with(cx, |model, _| model.extension_sidebar_active()));
    assert!(!shown(cx));

    reveal(cx);
    assert!(shown(cx), "the island presents the extension's webview");

    hover(cx, AWAY);
    wait(cx, GRACE + SLIDE * 2);
    assert!(!shown(cx));
}

#[gpui::test]
fn the_island_stays_clear_of_a_visible_status_bar(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    assert!(view.read_with(cx, |model, _| model.appearance.status_bar_visible));
    let status = cx.debug_bounds("project-status-bar").expect("status bar");

    hover(cx, point(EDGE.x, status.center().y));
    wait(cx, DWELL + SLIDE);
    assert!(
        !peeking(&view, cx),
        "the status bar's edge does not open it"
    );

    let card = reveal(cx);
    assert!(
        card.bottom() < status.top(),
        "the island ends above the status bar"
    );
}

#[gpui::test]
fn the_window_losing_hover_hides_the_island(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    reveal(cx);
    view.update(cx, |model, cx| {
        model.note_window_hover(true, cx);
        model.note_window_hover(false, cx);
    });
    wait(cx, GRACE + SLIDE);
    assert!(!peeking(&view, cx));
}

#[gpui::test]
fn hiding_with_toggle_sidebar_is_not_undone_by_a_small_move(cx: &mut TestAppContext) {
    let (view, cx) = open(FLOATING, cx);
    reveal(cx);
    hover(cx, INSIDE);
    cx.dispatch_action(ToggleSidebar);
    wait(cx, SLIDE / 4);
    hover(cx, point(INSIDE.x + px(40.0), INSIDE.y));
    wait(cx, SLIDE * 2);
    assert!(!peeking(&view, cx));
}
