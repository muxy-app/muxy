use super::*;
use gpui::point;
use muxy_app_core::settings::{AppLayout, Settings, SidebarCollapsedStyle};

fn width(cx: &mut VisualTestContext) -> Pixels {
    cx.debug_bounds("workspace-sidebar")
        .expect("sidebar")
        .size
        .width
}

fn grab(cx: &mut VisualTestContext) -> Point<Pixels> {
    let handle = cx.debug_bounds("sidebar-resize").expect("resize handle");
    let position = point(handle.left() + px(2.0), handle.center().y);
    pointer(cx, position, false);
    cx.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
    position
}

#[gpui::test]
fn expanded_sidebar_resizes_live_and_remembers_width_across_collapse_and_restart(
    cx: &mut TestAppContext,
) {
    for layout in [AppLayout::ProjectFocused, AppLayout::TabFocused] {
        let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
        boot.settings.appearance.layout = layout;
        boot.settings.appearance.sidebar_expanded = true;
        let path = boot.state_path.clone();
        boot.settings
            .appearance
            .save(&path.with_file_name("settings.toml"))
            .expect("initial settings");
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.run_until_parked();
        let from = grab(cx);
        pointer(cx, from, true);
        assert_eq!(width(cx), px(270.0));
        let to = from + point(px(80.0), px(0.0));
        pointer(cx, to, true);
        assert_eq!(width(cx), px(350.0));
        let titlebar = if layout == AppLayout::ProjectFocused {
            "tab-strip"
        } else {
            "project-titlebar"
        };
        assert_eq!(
            cx.debug_bounds(titlebar).expect("titlebar").left(),
            px(350.0)
        );
        assert_eq!(
            cx.debug_bounds("titlebar-navigation")
                .expect("navigation")
                .right(),
            px(350.0)
        );
        let settings = || Settings::load(&path.with_file_name("settings.toml")).expect("settings");
        assert_eq!(settings().appearance.sidebar_expanded_width, None);
        release(cx, to + point(px(10.0), px(0.0)));
        assert_eq!(width(cx), px(360.0));
        assert_eq!(
            settings().appearance.sidebar_expanded_width.map(px),
            Some(px(360.0))
        );
        pointer(cx, from, false);
        assert_eq!(width(cx), px(360.0));
        for expanded in [false, true] {
            cx.update(|window, cx| view.update(cx, |model, cx| model.toggle_sidebar(window, cx)));
            cx.run_until_parked();
            view.read_with(cx, |model, _| {
                assert_eq!(model.appearance.sidebar_expanded, expanded);
                assert!(model.sidebar_resize.is_none());
            });
        }
        assert_eq!(width(cx), px(360.0));
        let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("restart state"));
        boot.state_path = path;
        boot.settings = view.read_with(cx, |model, _| {
            Settings::load(&model.path.with_file_name("settings.toml")).expect("saved settings")
        });
        let (_, restarted) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        restarted.run_until_parked();
        assert_eq!(width(restarted), px(360.0));
    }
}

#[gpui::test]
fn sidebar_resize_clamps_and_stops_on_collapse_or_a_missed_release(cx: &mut TestAppContext) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.appearance.sidebar_expanded = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let from = grab(cx);
    pointer(cx, from + point(px(-1000.0), px(0.0)), true);
    assert_eq!(width(cx), px(180.0));
    pointer(cx, from + point(px(1000.0), px(0.0)), true);
    assert_eq!(width(cx), px(480.0));
    pointer(cx, from, true);
    assert_eq!(width(cx), px(270.0));
    pointer(cx, from + point(px(50.0), px(0.0)), true);
    pointer(cx, from, false);
    pointer(cx, from + point(px(100.0), px(0.0)), true);
    assert_eq!(width(cx), px(320.0));
    view.read_with(cx, |model, _| {
        assert!(model.sidebar_resize.is_none());
        assert_eq!(
            model.settings.appearance.sidebar_expanded_width.map(px),
            Some(px(320.0))
        );
    });
    let from = grab(cx);
    pointer(cx, from + point(px(40.0), px(0.0)), true);
    cx.update(|window, cx| view.update(cx, |model, cx| model.toggle_sidebar(window, cx)));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(!model.appearance.sidebar_expanded);
        assert!(model.sidebar_resize.is_none());
    });
    pointer(cx, from, true);
    cx.update(|window, cx| view.update(cx, |model, cx| model.toggle_sidebar(window, cx)));
    cx.run_until_parked();
    assert_eq!(width(cx), px(360.0));
}

#[gpui::test]
fn sidebar_width_validates_saved_values_and_collapsed_styles_disable_resizing(
    cx: &mut TestAppContext,
) {
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.appearance.sidebar_expanded = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for (saved, expected) in [
        (None, 270.0),
        (Some(-20.0), 180.0),
        (Some(900.0), 480.0),
        (Some(f32::NAN), 270.0),
        (Some(f32::INFINITY), 270.0),
    ] {
        view.update(cx, |model, cx| {
            model.appearance.sidebar_expanded_width = saved;
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(width(cx), px(expected));
    }
    for layout in [AppLayout::ProjectFocused, AppLayout::TabFocused] {
        for style in [SidebarCollapsedStyle::Icons, SidebarCollapsedStyle::Hidden] {
            let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
            boot.settings.appearance.layout = layout;
            boot.settings.appearance.sidebar_collapsed_style = style;
            boot.settings.appearance.sidebar_expanded_width = Some(350.0);
            let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
            cx.run_until_parked();
            assert!(cx.debug_bounds("sidebar-resize").is_none());
            view.read_with(cx, |model, _| {
                let expected = if layout == AppLayout::ProjectFocused
                    && style == SidebarCollapsedStyle::Icons
                {
                    44.0
                } else {
                    0.0
                };
                assert_eq!(px(model.sidebar_width()), px(expected));
            });
        }
    }
}

#[gpui::test]
fn sidebar_resize_updates_terminal_geometry_without_changing_focus(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(42).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (mut boot, requests) = stub_boot(state);
    boot.settings.appearance.sidebar_expanded = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    pane,
                    session,
                    attachment: attachment(),
                    created: false,
                },
            ),
            cx,
        );
    });
    cx.run_until_parked();
    let terminal = view.read_with(cx, |model, _| {
        model.terminal(&pane).expect("terminal").view.clone()
    });
    let before = terminal.read_with(cx, |pane, _| pane.geometry.expect("geometry"));
    let _ = requests.try_iter().collect::<Vec<_>>();
    let from = grab(cx);
    let to = from + point(px(80.0), px(0.0));
    pointer(cx, to, true);
    release(cx, to);
    terminal.read_with(cx, |pane, _| {
        let after = pane.geometry.expect("geometry");
        assert_eq!(after.0.left() - before.0.left(), px(80.0));
        assert_eq!(before.0.size.width - after.0.size.width, px(80.0));
        assert!(pane.selection.is_none());
    });
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Resize(_, _)))
    );
    cx.update(|window, cx| assert!(terminal.read(cx).focus.is_focused(window)));
}
