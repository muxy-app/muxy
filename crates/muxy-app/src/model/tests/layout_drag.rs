use super::*;
use gpui::{Bounds, MouseButton, Point, point};
use muxy_app_core::Direction;

mod docking;

fn state() -> (AppState, [TabId; 2], [PaneId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let a = state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.window().active_pane.expect("pane");
    let second = state.split_pane(first, Direction::Right).expect("split");
    let third = state.split_pane(second, Direction::Down).expect("split");
    let b = state.open_terminal_tab(state.home().id).expect("tab");
    let fourth = state.window().active_pane.expect("pane");
    for (index, pane) in [first, second, third, fourth].iter().enumerate() {
        state
            .set_pane_session(*pane, SessionId::new(200 + index as u64))
            .expect("session");
    }
    state.select_tab(state.home().id, a).expect("select");
    (state, [a, b], [first, second, third, fourth])
}

fn press(cx: &mut VisualTestContext, position: Point<gpui::Pixels>, command: bool) {
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        modifiers: Modifiers {
            platform: command,
            ..Default::default()
        },
        ..Default::default()
    });
    cx.run_until_parked();
}

fn pointer(cx: &mut VisualTestContext, position: Point<gpui::Pixels>, pressed: bool) {
    cx.simulate_event(gpui::MouseMoveEvent {
        position,
        pressed_button: pressed.then_some(MouseButton::Left),
        ..Default::default()
    });
    cx.run_until_parked();
}

fn release(cx: &mut VisualTestContext, position: Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn pane_bounds(
    view: &Entity<AppModel>,
    pane: PaneId,
    cx: &mut VisualTestContext,
) -> Bounds<gpui::Pixels> {
    view.read_with(cx, |model, cx| {
        model.grids[&pane]
            .view
            .read(cx)
            .geometry
            .expect("geometry")
            .0
    })
}

#[gpui::test]
fn dragging_tabs_into_the_workspace_never_splits_or_shows_a_placeholder(cx: &mut TestAppContext) {
    for command in [false, true] {
        let (state, [a, b], panes) = state();
        let tabs = state.home().tabs.clone();
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.run_until_parked();
        for (x, y) in [(0.1, 0.5), (0.9, 0.5), (0.5, 0.1), (0.5, 0.9), (0.5, 0.5)] {
            let from = cx.debug_bounds("tab-cell-1").expect("tab").center();
            press(cx, from, command);
            let bounds = cx.debug_bounds("workspace-content").expect("content");
            let target = point(
                bounds.left() + bounds.size.width * x,
                bounds.top() + bounds.size.height * y,
            );
            pointer(cx, target, true);
            view.read_with(cx, |model, _| {
                assert!(model.tab_drag.is_active());
                assert!(model.layout_drag.preview.is_none());
            });
            release(cx, target);
            view.read_with(cx, |model, _| {
                assert_eq!(model.active_tab(), Some(b));
                assert_eq!(model.visible_panes(), [panes[3]]);
                assert_eq!(model.state.home().tabs, tabs);
                assert!(!model.layout_drag_pending());
            });
            cx.update(|_, cx| view.update(cx, |model, cx| model.select_tab(a, cx)));
            cx.run_until_parked();
            assert_eq!(
                view.read_with(cx, |model, _| model.visible_panes()),
                panes[..3]
            );
        }
    }
}

#[gpui::test]
fn cmd_drag_previews_match_final_pane_bounds_for_every_edge_and_center(cx: &mut TestAppContext) {
    for (x, y) in [(0.1, 0.5), (0.9, 0.5), (0.5, 0.1), (0.5, 0.9), (0.5, 0.5)] {
        let (state, [a, _], panes) = state();
        let (mut boot, _requests) = stub_boot(state);
        boot.terminal.options.padding_x = [0.0; 2];
        boot.terminal.options.padding_y = [0.0; 2];
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.simulate_resize(size(px(1001.0), px(701.0)));
        cx.run_until_parked();
        let original = view.read_with(cx, |model, _| model.tab(a).expect("tab").panes.clone());
        let entities = view.read_with(cx, |model, _| {
            panes[..3]
                .iter()
                .map(|pane| model.grids[pane].view.entity_id())
                .collect::<Vec<_>>()
        });
        let from = pane_bounds(&view, panes[0], cx).center();
        let bounds = pane_bounds(&view, panes[2], cx);
        let target = point(
            bounds.left() + bounds.size.width * x,
            bounds.top() + bounds.size.height * y,
        );
        press(cx, from, true);
        pointer(cx, from + point(px(3.0), px(0.0)), true);
        assert!(!view.read_with(cx, |model, _| model.layout_drag.active()));
        pointer(cx, target, true);
        let preview = view.read_with(cx, |model, _| {
            model.layout_drag.preview.clone().unwrap_or_else(|| {
                panic!(
                    "missing preview: geometry={:?}, tab_drag={}, pane_drag={}",
                    model.layout_drag.geometry.get(),
                    model.tab_drag.is_active(),
                    model.layout_drag.active()
                )
            })
        });
        release(cx, target);
        assert_eq!(pane_bounds(&view, panes[0], cx), preview.bounds);
        view.read_with(cx, |model, _| {
            assert_eq!(model.tab(a).expect("tab").panes, original);
            assert_eq!(model.active_pane(), Some(panes[0]));
            assert!(!model.layout_drag.active());
            assert!(model.layout_drag.preview.is_none());
            assert_eq!(
                panes[..3]
                    .iter()
                    .map(|pane| model.grids[pane].view.entity_id())
                    .collect::<Vec<_>>(),
                entities
            );
            assert!(model.error.is_none(), "{:?}", model.error);
        });
    }
}

#[gpui::test]
fn pane_drags_cancel_without_mutation_and_require_command_at_mouse_down(cx: &mut TestAppContext) {
    for interruption in [
        "escape",
        "deactivate",
        "outside",
        "lost-release",
        "no-command",
        "self",
        "overlay",
        "switch-tab",
        "zoom",
    ] {
        let (state, [a, _], panes) = state();
        let original = state.home().tabs[0].layout.clone();
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.run_until_parked();
        cx.update(|window, _| window.activate_window());
        cx.run_until_parked();
        let from = pane_bounds(&view, panes[0], cx).center();
        let target = pane_bounds(&view, panes[2], cx).center();
        press(cx, from, interruption != "no-command");
        pointer(cx, target, true);
        match interruption {
            "escape" => cx.simulate_keystrokes("escape"),
            "deactivate" => cx.deactivate_window(),
            "outside" => release(cx, point(px(-10.0), px(-10.0))),
            "lost-release" => pointer(cx, target, false),
            "self" => release(cx, from),
            "overlay" => cx.update(|window, cx| {
                view.update(cx, |model, cx| model.open_theme_picker(window, cx));
            }),
            "switch-tab" => cx.update(|_, cx| {
                view.update(cx, |model, cx| {
                    let other = model.state.home().tabs[1].id;
                    model.select_tab(other, cx);
                });
            }),
            "zoom" => cx.update(|_, cx| {
                view.update(cx, AppModel::toggle_zoom_pane);
            }),
            "no-command" => release(cx, target),
            _ => unreachable!(),
        }
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert_eq!(
                model.tab(a).expect("tab").layout,
                original,
                "{interruption}"
            );
            assert!(!model.layout_drag.active(), "{interruption}");
            assert!(model.layout_drag.preview.is_none(), "{interruption}");
        });
    }
}

#[gpui::test]
fn native_escape_cancels_pending_and_active_drags_before_release(cx: &mut TestAppContext) {
    for tab_drag in [false, true] {
        for active in [false, true] {
            let (state, [a, _], panes) = state();
            let original = state.home().tabs[0].layout.clone();
            let (boot, _requests) = stub_boot(state);
            let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
            cx.run_until_parked();
            let (source, target) = if tab_drag {
                let bounds = cx.debug_bounds("workspace-content").expect("content");
                (
                    cx.debug_bounds("tab-cell-1").expect("tab").center(),
                    point(bounds.right() - px(10.0), bounds.center().y),
                )
            } else {
                (
                    pane_bounds(&view, panes[0], cx).center(),
                    pane_bounds(&view, panes[2], cx).center(),
                )
            };
            press(cx, source, !tab_drag);
            if active {
                pointer(cx, target, true);
            }
            view.read_with(cx, |model, _| {
                assert!(model.layout_drag_pending());
                assert_eq!(model.layout_drag.preview.is_some(), active && !tab_drag);
            });
            cx.update(|_, cx| {
                view.update(cx, |model, cx| {
                    model.webview_escape(crate::views::webview::SurfaceKind::Tab, cx);
                });
            });
            cx.run_until_parked();
            release(cx, target);
            view.read_with(cx, |model, _| {
                assert!(!model.layout_drag_pending());
                assert!(model.layout_drag.preview.is_none());
                assert_eq!(model.tab(a).expect("tab").layout, original);
            });
        }
    }
}

#[gpui::test]
fn pane_dividers_resize_persist_and_keep_drag_previews_precise(cx: &mut TestAppContext) {
    let (state, [a, _], panes) = state();
    let (mut boot, _requests) = stub_boot(state);
    boot.terminal.options.padding_x = [0.0; 2];
    boot.terminal.options.padding_y = [0.0; 2];
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1301.0), px(701.0)));
    cx.run_until_parked();
    let divider = cx
        .debug_bounds("split-divider-[]")
        .expect("divider")
        .center();
    press(cx, divider, false);
    let destination = divider + point(px(120.0), px(0.0));
    pointer(cx, destination, true);
    release(cx, destination);
    view.read_with(cx, |model, _| {
        let layout = &model.tab(a).expect("tab").layout;
        let muxy_app_core::Layout::Split { ratio, .. } = layout else {
            panic!("split")
        };
        assert!(*ratio > 0.5);
        let saved = store::load(&model.path).expect("saved");
        assert_eq!(&saved.home().tabs[0].layout, layout);
    });
    let source = pane_bounds(&view, panes[0], cx).center();
    let target = pane_bounds(&view, panes[2], cx).center();
    press(cx, source, true);
    pointer(cx, target, true);
    let preview = view.read_with(cx, |model, _| {
        model.layout_drag.preview.clone().expect("preview")
    });
    release(cx, target);
    assert_eq!(pane_bounds(&view, panes[0], cx), preview.bounds);
}

#[gpui::test]
fn stationary_hover_does_not_repaint_or_save_and_fast_release_still_drops(cx: &mut TestAppContext) {
    let (state, [a, _], panes) = state();
    let original = state.home().tabs[0].layout.clone();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let source = pane_bounds(&view, panes[0], cx).center();
    let target = pane_bounds(&view, panes[2], cx).center();
    press(cx, source, true);
    pointer(cx, target, true);
    let renders = view.read_with(cx, |model, cx| {
        model.grids[&panes[0]].view.read(cx).render_count
    });
    for _ in 0..10 {
        pointer(cx, target, true);
    }
    view.read_with(cx, |model, cx| {
        assert_eq!(model.grids[&panes[0]].view.read(cx).render_count, renders);
        assert_eq!(model.tab(a).expect("tab").layout, original);
        assert_eq!(
            store::load(&model.path).expect("saved").home().tabs[0].layout,
            original
        );
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    press(cx, source, true);
    release(cx, target);
    assert_ne!(
        view.read_with(cx, |model, _| model.tab(a).expect("tab").layout.clone()),
        original
    );
}
