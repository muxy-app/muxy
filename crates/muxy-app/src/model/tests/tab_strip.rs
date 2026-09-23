use super::*;
use crate::views::{titlebar::BeginWindowMove, workspace::Zoom};
use gpui::{
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, Styled, div, point,
};

struct WindowZoomObserver {
    model: Entity<AppModel>,
    zoom_requests: usize,
    move_requests: usize,
}

impl Render for WindowZoomObserver {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .capture_action(cx.listener(|observer, _: &Zoom, _, cx| {
                observer.zoom_requests += 1;
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|observer, _: &BeginWindowMove, _, cx| {
                observer.move_requests += 1;
                cx.stop_propagation();
            }))
            .child(self.model.clone())
    }
}

fn observe_window_zoom(
    state: AppState,
    cx: &mut TestAppContext,
) -> (Entity<WindowZoomObserver>, &mut VisualTestContext) {
    let (boot, _requests) = stub_boot(state);
    let result = cx.add_window_view(|window, cx| WindowZoomObserver {
        model: cx.new(|cx| AppModel::new(boot, window, cx)),
        zoom_requests: 0,
        move_requests: 0,
    });
    result.1.simulate_resize(size(px(1000.0), px(600.0)));
    result.1.run_until_parked();
    result
}

fn click(
    cx: &mut VisualTestContext,
    position: gpui::Point<gpui::Pixels>,
    button: MouseButton,
    click_count: usize,
) {
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button,
        click_count,
        ..Default::default()
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button,
        click_count,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn strip_background(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
    let viewport = cx.debug_bounds("tabs-scroll").expect("tabs viewport");
    point(viewport.right() - px(16.0), viewport.center().y)
}

#[gpui::test]
fn configuration_diagnostics_stay_below_the_titlebar(cx: &mut TestAppContext) {
    use muxy_app_core::settings::AppLayout;

    let state = AppState::bootstrap().expect("state");
    let (observer, cx) = observe_window_zoom(state, cx);
    let model = observer.read_with(cx, |observer, _| observer.model.clone());
    for (layout, selector) in [
        (AppLayout::ProjectFocused, "tab-strip"),
        (AppLayout::TabFocused, "project-titlebar"),
    ] {
        for expanded in [true, false] {
            model.update(cx, |model, cx| {
                model.appearance.layout = layout;
                model.appearance.sidebar_expanded = expanded;
                model.configuration_error = None;
                model.error = None;
                cx.notify();
            });
            cx.run_until_parked();
            let titlebar = cx.debug_bounds(selector).expect("titlebar");
            let navigation = cx.debug_bounds("nav-back").expect("navigation");
            for message in ["Unknown setting".to_owned(), "Unknown setting\n".repeat(20)] {
                model.update(cx, |model, cx| {
                    model.configuration_error = Some(message);
                    model.error = None;
                    cx.notify();
                });
                cx.run_until_parked();
                assert_eq!(cx.debug_bounds(selector), Some(titlebar));
                assert_eq!(cx.debug_bounds("nav-back"), Some(navigation));
                cx.executor()
                    .advance_clock(crate::model::banners::TOAST_TRANSITION);
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
                let diagnostics = cx
                    .debug_bounds("workspace-toast-message")
                    .expect("diagnostics");
                assert!(diagnostics.top() >= titlebar.bottom());
                assert!(diagnostics.size.height <= px(100.0));
            }
        }
    }
}

#[gpui::test]
fn tab_strip_background_double_click_requests_native_zoom_each_time(cx: &mut TestAppContext) {
    for has_tabs in [false, true] {
        let mut state = AppState::bootstrap().expect("state");
        if has_tabs {
            state.open_terminal_tab(state.home().id).expect("tab");
        }
        let (observer, cx) = observe_window_zoom(state, cx);
        let model = observer.read_with(cx, |observer, _| observer.model.clone());
        for expanded in [true, false] {
            model.update(cx, |model, cx| {
                model.appearance.sidebar_expanded = expanded;
                cx.notify();
            });
            cx.run_until_parked();
            let background = strip_background(cx);
            let before = observer.read_with(cx, |observer, _| observer.zoom_requests);
            for (button, count) in [
                (MouseButton::Left, 1),
                (MouseButton::Right, 2),
                (MouseButton::Middle, 2),
                (MouseButton::Left, 3),
            ] {
                click(cx, background, button, count);
                assert_eq!(
                    observer.read_with(cx, |observer, _| observer.zoom_requests),
                    before
                );
            }
            for expected in [before + 1, before + 2] {
                click(cx, background, MouseButton::Left, 1);
                click(cx, background, MouseButton::Left, 2);
                assert_eq!(
                    observer.read_with(cx, |observer, _| observer.zoom_requests),
                    expected
                );
            }
        }
    }
}

#[gpui::test]
fn tab_strip_tabs_and_controls_do_not_zoom_the_window(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let first = state.open_terminal_tab(state.home().id).expect("first tab");
    let second = state
        .open_terminal_tab(state.home().id)
        .expect("second tab");
    let pane = state.home().tabs[1].panes[0].id;
    state.split_pane(pane, Direction::Right).expect("split");
    state.select_tab(state.home().id, first).expect("select");
    let (observer, cx) = observe_window_zoom(state, cx);
    let model = observer.read_with(cx, |observer, _| observer.model.clone());
    for count in [1, 2] {
        let tab = cx.debug_bounds("tab-terminal").expect("second tab icon");
        click(cx, tab.center(), MouseButton::Left, count);
        assert_eq!(
            model.read_with(cx, |model, _| model.active_tab()),
            Some(second)
        );
    }
    for (selector, count, zoomed) in [("maximize-pane", 1, true), ("restore-pane", 2, false)] {
        let control = cx.debug_bounds(selector).expect("pane zoom control");
        click(cx, control.center(), MouseButton::Left, count);
        assert_eq!(
            model.read_with(cx, |model, _| model.state.home().tabs[1].zoomed.is_some()),
            zoomed
        );
    }
    for (selector, expected_counts) in [("new-tab-button", [3, 4]), ("close-tab-button", [3, 2])] {
        for (count, expected) in [1, 2].into_iter().zip(expected_counts) {
            let button = cx.debug_bounds(selector).expect("tab control");
            click(cx, button.center(), MouseButton::Left, count);
            assert_eq!(
                model.read_with(cx, |model, _| model.state.home().tabs.len()),
                expected
            );
        }
    }
    assert_eq!(
        observer.read_with(cx, |observer, _| observer.zoom_requests),
        0
    );
}

#[gpui::test]
fn collapsed_titlebar_navigation_does_not_click_through_to_tab_strip(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let first = state.open_terminal_tab(state.home().id).expect("first tab");
    let second = state
        .open_terminal_tab(state.home().id)
        .expect("second tab");
    let third = state.open_terminal_tab(state.home().id).expect("third tab");
    let (observer, cx) = observe_window_zoom(state, cx);
    let model = observer.read_with(cx, |observer, _| observer.model.clone());
    model.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = false;
        assert!(!model.can_navigate(false) && !model.can_navigate(true));
        cx.notify();
    });
    cx.run_until_parked();
    for selector in ["nav-back", "nav-forward"] {
        let arrow = cx
            .debug_bounds(selector)
            .expect("disabled navigation arrow");
        for count in [1, 2] {
            click(cx, arrow.center(), MouseButton::Left, count);
        }
        assert_eq!(
            observer.read_with(cx, |observer, _| observer.zoom_requests),
            0
        );
        assert_eq!(
            model.read_with(cx, |model, _| model.active_tab()),
            Some(third)
        );
    }
    model.update(cx, |model, cx| {
        model.select_tab(second, cx);
        model.select_tab(first, cx);
    });
    cx.run_until_parked();
    for (selector, targets) in [
        ("nav-back", [second, third]),
        ("nav-forward", [second, first]),
    ] {
        for (count, target) in [1, 2].into_iter().zip(targets) {
            let arrow = cx.debug_bounds(selector).expect("enabled navigation arrow");
            click(cx, arrow.center(), MouseButton::Left, count);
            assert_eq!(
                model.read_with(cx, |model, _| model.active_tab()),
                Some(target)
            );
            assert_eq!(
                observer.read_with(cx, |observer, _| observer.zoom_requests),
                0
            );
        }
    }
}

fn tabs(count: usize) -> (AppState, Vec<TabId>) {
    let mut state = AppState::bootstrap().expect("state");
    let ids = (0..count)
        .map(|_| state.open_terminal_tab(state.home().id).expect("tab"))
        .collect();
    (state, ids)
}

fn tab_bounds(cx: &mut VisualTestContext, index: usize) -> gpui::Bounds<gpui::Pixels> {
    let selector = match index {
        0 => "tab-cell-0",
        1 => "tab-cell-1",
        2 => "tab-cell-2",
        10 => "tab-cell-10",
        12 => "tab-cell-12",
        _ => panic!("unconfigured test tab index"),
    };
    cx.debug_bounds(selector).expect("tab cell")
}

fn tab_order(view: &Entity<AppModel>, cx: &VisualTestContext) -> Vec<TabId> {
    view.read_with(cx, |model, _| {
        model
            .state
            .current_project()
            .tabs
            .iter()
            .map(|tab| tab.id)
            .collect()
    })
}

fn pointer(cx: &mut VisualTestContext, position: gpui::Point<gpui::Pixels>, pressed: bool) {
    cx.simulate_event(gpui::MouseMoveEvent {
        position,
        pressed_button: pressed.then_some(MouseButton::Left),
        ..Default::default()
    });
    cx.run_until_parked();
}

fn press(cx: &mut VisualTestContext, position: gpui::Point<gpui::Pixels>) {
    pointer(cx, position, false);
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn release(cx: &mut VisualTestContext, position: gpui::Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

#[gpui::test]
fn tabs_select_on_press_and_reorder_live_without_a_drag_preview(cx: &mut TestAppContext) {
    for wide in [true, false] {
        let (state, ids) = tabs(3);
        let (mut boot, _requests) = stub_boot(state);
        boot.settings.appearance.sidebar_expanded = wide;
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.simulate_resize(size(px(1000.0), px(600.0)));
        cx.run_until_parked();
        let from = tab_bounds(cx, 0).center();
        let to = tab_bounds(cx, 2).center();
        press(cx, from);
        assert_eq!(
            view.read_with(cx, |model, _| model.active_tab()),
            Some(ids[0])
        );
        let pane = view.read_with(cx, |model, _| model.active_pane());
        pointer(cx, from + point(px(3.0), px(0.0)), true);
        assert!(!view.read_with(cx, |model, _| model.tab_drag.is_active()));
        pointer(cx, from + point(px(4.0), px(0.0)), true);
        assert!(view.read_with(cx, |model, _| model.tab_drag.is_active()));
        pointer(cx, to, true);
        assert_eq!(tab_order(&view, cx), [ids[1], ids[2], ids[0]]);
        assert!(
            !cx.update(|_, cx| cx.has_active_drag()),
            "no floating preview"
        );
        pointer(cx, to, true);
        assert_eq!(tab_order(&view, cx), [ids[1], ids[2], ids[0]]);
        pointer(cx, from, true);
        assert_eq!(tab_order(&view, cx), ids);
        pointer(cx, to, true);
        release(cx, to);
        assert!(!view.read_with(cx, |model, _| model.tab_drag.is_active()));
        assert_eq!(tab_order(&view, cx), [ids[1], ids[2], ids[0]]);
        view.read_with(cx, |model, _| {
            assert_eq!(model.active_pane(), pane);
            assert!(model.error.is_none());
            let saved = store::load(&model.path).expect("saved order");
            assert_eq!(
                saved
                    .home()
                    .tabs
                    .iter()
                    .map(|tab| tab.id)
                    .collect::<Vec<_>>(),
                [ids[1], ids[2], ids[0]]
            );
        });
    }
}

#[gpui::test]
fn tab_reordering_handles_fast_release_and_does_not_close_a_drop_target(cx: &mut TestAppContext) {
    let (state, ids) = tabs(3);
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    cx.run_until_parked();
    let from = tab_bounds(cx, 0).center();
    let close = cx
        .debug_bounds("close-tab-button")
        .expect("last tab close")
        .center();
    press(cx, from);
    release(cx, close);
    assert_eq!(tab_order(&view, cx), [ids[1], ids[2], ids[0]]);
    assert_eq!(
        view.read_with(cx, |model, _| model.active_tab()),
        Some(ids[0])
    );
}

#[gpui::test]
fn compact_and_scrolled_tabs_can_be_dragged_without_closing_them(cx: &mut TestAppContext) {
    for scrolled in [false, true] {
        let (state, ids) = tabs(if scrolled { 24 } else { 7 });
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.simulate_resize(size(px(700.0), px(440.0)));
        cx.run_until_parked();
        let viewport = cx.debug_bounds("tabs-scroll").expect("viewport");
        if scrolled {
            cx.simulate_event(gpui::ScrollWheelEvent {
                position: viewport.center(),
                delta: gpui::ScrollDelta::Pixels(point(px(-300.0), px(0.0))),
                ..Default::default()
            });
            cx.run_until_parked();
        }
        let (source, target) = if scrolled { (10, 12) } else { (0, 2) };
        let source_bounds = tab_bounds(cx, source);
        assert!(source_bounds.size.width < px(80.0));
        let from = point(source_bounds.left() + px(5.0), source_bounds.center().y);
        let to = tab_bounds(cx, target).center();
        assert!(viewport.contains(&from) && viewport.contains(&to));
        press(cx, from);
        pointer(cx, point(viewport.left() - px(5.0), from.y), true);
        assert_eq!(tab_order(&view, cx), ids, "clipped tabs are not targets");
        pointer(cx, to, true);
        let mut expected = ids;
        let dragged = expected.remove(source);
        expected.insert(target, dragged);
        assert_eq!(tab_order(&view, cx), expected);
        release(cx, to);
        assert_eq!(tab_order(&view, cx), expected);
        assert_eq!(
            view.read_with(cx, |model, _| model.active_tab()),
            Some(dragged)
        );
    }
}

#[gpui::test]
fn tab_close_controls_do_not_select_or_start_a_drag(cx: &mut TestAppContext) {
    let (mut state, ids) = tabs(3);
    state
        .select_tab(state.home().id, ids[0])
        .expect("select first");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let close = cx
        .debug_bounds("close-tab-button")
        .expect("last tab close")
        .center();
    press(cx, close);
    assert_eq!(
        view.read_with(cx, |model, _| model.active_tab()),
        Some(ids[0])
    );
    let target = tab_bounds(cx, 0).center();
    pointer(cx, target, true);
    assert!(!view.read_with(cx, |model, _| model.tab_drag.is_active()));
    release(cx, target);
    assert_eq!(tab_order(&view, cx), ids);
    click(cx, close, MouseButton::Left, 1);
    assert_eq!(tab_order(&view, cx), ids[..2]);
    assert_eq!(
        view.read_with(cx, |model, _| model.active_tab()),
        Some(ids[0])
    );
    let middle = tab_bounds(cx, 1).center();
    click(cx, middle, MouseButton::Middle, 1);
    assert_eq!(tab_order(&view, cx), ids[..1]);
}

#[gpui::test]
fn interrupted_tab_drags_cannot_resume_on_later_pointer_motion(cx: &mut TestAppContext) {
    for interruption in [
        "outside",
        "lost-release",
        "deactivate",
        "project",
        "overlay",
    ] {
        let (mut state, ids) = tabs(3);
        let home = state.home().id;
        let other = state.add_project(std::env::temp_dir()).expect("project");
        state.select_project(home).expect("home");
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.run_until_parked();
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let from = tab_bounds(cx, 0).center();
        press(cx, from);
        pointer(cx, from + point(px(5.0), px(0.0)), true);
        assert!(view.read_with(cx, |model, _| model.tab_drag.is_active()));
        match interruption {
            "outside" => release(cx, point(px(500.0), px(250.0))),
            "lost-release" => pointer(cx, from, false),
            "deactivate" => cx.deactivate_window(),
            "project" => {
                view.update(cx, |model, cx| model.select_project(other, cx));
                cx.run_until_parked();
                view.update(cx, |model, cx| model.select_project(home, cx));
            }
            "overlay" => cx.update(|window, cx| {
                view.update(cx, |model, cx| model.open_theme_picker(window, cx));
            }),
            _ => unreachable!(),
        }
        cx.run_until_parked();
        assert!(
            !view.read_with(cx, |model, _| model.tab_drag.is_active()),
            "{interruption}"
        );
        let target = tab_bounds(cx, 2).center();
        pointer(cx, target, true);
        release(cx, target);
        assert_eq!(tab_order(&view, cx), ids, "{interruption}");
    }
}

#[gpui::test]
fn only_empty_titlebar_space_requests_window_movement(cx: &mut TestAppContext) {
    for has_tabs in [false, true] {
        let (state, ids) = tabs(if has_tabs { 3 } else { 0 });
        let (observer, cx) = observe_window_zoom(state, cx);
        let model = observer.read_with(cx, |observer, _| observer.model.clone());
        for expanded in [true, false] {
            model.update(cx, |model, cx| {
                model.appearance.sidebar_expanded = expanded;
                cx.notify();
            });
            cx.run_until_parked();
            let strip = cx.debug_bounds("tab-strip").expect("tab strip");
            let back = cx.debug_bounds("nav-back").expect("back button");
            for from in [
                strip_background(cx),
                point(back.left() - px(4.0), back.center().y),
            ] {
                let before = observer.read_with(cx, |observer, _| observer.move_requests);
                for button in [MouseButton::Right, MouseButton::Middle] {
                    click(cx, from, button, 1);
                }
                click(cx, from, MouseButton::Left, 2);
                assert_eq!(
                    observer.read_with(cx, |observer, _| observer.move_requests),
                    before
                );
                press(cx, from);
                let to = point(strip.left() + px(200.0), strip.center().y);
                pointer(cx, to, true);
                release(cx, to);
                assert_eq!(
                    observer.read_with(cx, |observer, _| observer.move_requests),
                    before + 1
                );
                assert_eq!(tab_order(&model, cx), ids);
                assert!(!model.read_with(cx, |model, _| model.tab_drag.is_active()));
            }
        }
    }
}

#[gpui::test]
fn tabs_and_titlebar_controls_never_request_window_movement(cx: &mut TestAppContext) {
    for expanded in [true, false] {
        let (mut state, ids) = tabs(3);
        let pane = state.home().tabs[2].panes[0].id;
        state.split_pane(pane, Direction::Right).expect("split");
        let (observer, cx) = observe_window_zoom(state, cx);
        let model = observer.read_with(cx, |observer, _| observer.model.clone());
        model.update(cx, |model, cx| {
            model.appearance.sidebar_expanded = expanded;
            cx.notify();
        });
        cx.run_until_parked();
        let background = strip_background(cx);
        for selector in [
            "settings-button",
            "new-tab-button",
            "close-tab-button",
            "maximize-pane",
            "nav-back",
            "nav-forward",
            "sidebar-toggle",
        ] {
            let from = cx.debug_bounds(selector).expect("control").center();
            press(cx, from);
            pointer(cx, background, true);
            release(cx, background);
            assert_eq!(tab_order(&model, cx), ids, "{selector}");
            assert!(!model.read_with(cx, |model, _| model.tab_drag.is_active()));
        }
        for id in &ids[..2] {
            model.update(cx, |model, cx| model.select_tab(*id, cx));
        }
        cx.run_until_parked();
        assert!(model.read_with(cx, |model, _| model.can_navigate(false)));
        let back = cx.debug_bounds("nav-back").expect("back button").center();
        press(cx, back);
        pointer(cx, background, true);
        release(cx, background);
        let from = tab_bounds(cx, 0).center();
        let to = tab_bounds(cx, 2).center();
        press(cx, from);
        pointer(cx, to, true);
        release(cx, to);
        assert_eq!(tab_order(&model, cx), [ids[1], ids[2], ids[0]]);
        assert_eq!(
            observer.read_with(cx, |observer, _| observer.move_requests),
            0
        );
    }
}

#[gpui::test]
fn settings_button_stays_reachable_and_reuses_the_settings_window(cx: &mut TestAppContext) {
    for expanded in [false, true] {
        for count in [0, 3, 24] {
            let (mut state, ids) = tabs(count);
            if count == 3 {
                let pane = state.home().tabs[2].panes[0].id;
                state.split_pane(pane, Direction::Right).expect("split");
            }
            let (observer, cx) = observe_window_zoom(state, cx);
            let model = observer.read_with(cx, |observer, _| observer.model.clone());
            model.update(cx, |model, cx| {
                model.appearance.sidebar_expanded = expanded;
                model.connection = ConnectionState::Disconnected;
                cx.notify();
            });
            for width in [640.0, 1000.0] {
                cx.simulate_resize(size(px(width), px(600.0)));
                cx.run_until_parked();
                let gear = cx.debug_bounds("settings-button").expect("settings button");
                let viewport = cx.debug_bounds("tabs-scroll").expect("tabs viewport");
                assert!(gear.size.width > px(0.0));
                assert!(viewport.right() <= gear.left());
                for selector in ["new-tab-button", "maximize-pane"] {
                    if let Some(control) = cx.debug_bounds(selector) {
                        assert!(control.right() <= gear.left());
                    }
                }
                if count == 24 {
                    cx.simulate_event(gpui::ScrollWheelEvent {
                        position: viewport.center(),
                        delta: gpui::ScrollDelta::Pixels(point(px(-300.0), px(0.0))),
                        ..Default::default()
                    });
                    cx.run_until_parked();
                    assert_eq!(cx.debug_bounds("settings-button"), Some(gear));
                }
            }
            let gear = cx.debug_bounds("settings-button").expect("settings button");
            click(cx, gear.center(), MouseButton::Left, 1);
            let settings = model.read_with(cx, |model, _| {
                assert_eq!(model.state.home().tabs.len(), count);
                model
                    .settings_window
                    .as_ref()
                    .expect("settings window")
                    .window
            });
            if let Some(first) = ids.first() {
                model.update(cx, |model, cx| model.select_tab(*first, cx));
                cx.run_until_parked();
            }
            click(cx, gear.center(), MouseButton::Left, 2);
            model.read_with(cx, |model, _| {
                assert_eq!(
                    model
                        .settings_window
                        .as_ref()
                        .expect("settings window")
                        .window,
                    settings
                );
                assert_eq!(model.state.home().tabs.len(), count);
                assert!(!model.tab_drag.is_active());
            });
            observer.read_with(cx, |observer, _| {
                assert_eq!(observer.zoom_requests, 0);
                assert_eq!(observer.move_requests, 0);
            });
        }
    }
}

#[gpui::test]
fn settings_button_remains_available_when_the_project_directory_is_missing(
    cx: &mut TestAppContext,
) {
    let directory = std::env::temp_dir().join(format!("muxy-settings-button-{}", ProjectId::new()));
    std::fs::create_dir(&directory).expect("mkdir");
    let mut state = AppState::bootstrap().expect("state");
    state.add_project(directory.clone()).expect("project");
    std::fs::remove_dir(&directory).expect("remove project directory");
    state.refresh_project_statuses();
    let (observer, cx) = observe_window_zoom(state, cx);
    let gear = cx.debug_bounds("settings-button").expect("settings button");
    click(cx, gear.center(), MouseButton::Left, 1);
    observer.read_with(cx, |observer, cx| {
        let model = observer.model.read(cx);
        assert!(model.settings_window.is_some());
        assert!(model.active_pane().is_none());
        assert_eq!(observer.zoom_requests, 0);
        assert_eq!(observer.move_requests, 0);
    });
}
