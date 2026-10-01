use super::*;
use muxy_app_core::{Axis, Layout};

fn drag_to(
    view: &Entity<AppModel>,
    source: PaneId,
    target: Point<gpui::Pixels>,
    cx: &mut VisualTestContext,
) {
    let from = pane_bounds(view, source, cx).center();
    press(cx, from, true);
    pointer(cx, target, true);
    let preview = view.read_with(cx, |model, _| {
        model.layout_drag.preview.clone().expect("preview")
    });
    release(cx, target);
    assert_eq!(pane_bounds(view, source, cx), preview.bounds);
    view.read_with(cx, |model, _| {
        assert_eq!(store::load(&model.path).expect("saved"), model.state);
        assert!(!model.layout_drag_pending());
        assert!(model.error.is_none(), "{:?}", model.error);
    });
}

#[gpui::test]
fn outer_edge_gesture_makes_three_columns_instead_of_splitting_one_pane(cx: &mut TestAppContext) {
    let (state, [tab, _], [a, b, c, _]) = state();
    let (mut boot, _requests) = stub_boot(state);
    boot.terminal.options.padding_x = [0.0; 2];
    boot.terminal.options.padding_y = [0.0; 2];
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1001.0), px(701.0)));
    cx.run_until_parked();
    let bounds = view.read_with(cx, |model, _| model.layout_drag.geometry.get().0);
    drag_to(
        &view,
        c,
        point(bounds.right() - px(6.0), bounds.center().y),
        cx,
    );
    let rectangles = [a, b, c].map(|pane| pane_bounds(&view, pane, cx));
    for rectangle in rectangles {
        assert_eq!(rectangle.top(), bounds.top());
        assert_eq!(rectangle.bottom(), bounds.bottom());
        assert!((rectangle.size.width - bounds.size.width / 3.0).abs() <= px(2.0));
    }
    assert!(rectangles[0].right() < rectangles[1].left());
    assert!(rectangles[1].right() < rectangles[2].left());
    assert_eq!(
        view.read_with(cx, |model, _| model.tab(tab).expect("tab").layout.leaves()),
        [a, b, c]
    );
}

#[gpui::test]
fn pane_edge_gestures_build_an_aligned_two_by_two_grid(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let a = state.window().active_pane.expect("pane");
    let b = state.split_pane(a, Direction::Right).expect("split");
    let c = state.split_pane(b, Direction::Right).expect("split");
    let d = state.split_pane(c, Direction::Right).expect("split");
    for (index, pane) in [a, b, c, d].into_iter().enumerate() {
        state
            .set_pane_session(pane, SessionId::new(200 + index as u64))
            .expect("session");
    }
    let records = state.home().tabs[0].panes.clone();
    let (mut boot, _requests) = stub_boot(state);
    boot.terminal.options.padding_x = [0.0; 2];
    boot.terminal.options.padding_y = [0.0; 2];
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1401.0), px(901.0)));
    cx.run_until_parked();
    let target = pane_bounds(&view, b, cx);
    drag_to(
        &view,
        d,
        point(target.center().x, target.bottom() - px(40.0)),
        cx,
    );
    let target = pane_bounds(&view, a, cx);
    drag_to(
        &view,
        c,
        point(target.center().x, target.bottom() - px(40.0)),
        cx,
    );
    let [a, b, c, d] = [a, b, c, d].map(|pane| pane_bounds(&view, pane, cx));
    assert_eq!(a.top(), b.top());
    assert_eq!(a.bottom(), b.bottom());
    assert_eq!(c.top(), d.top());
    assert_eq!(c.bottom(), d.bottom());
    assert_eq!(a.left(), c.left());
    assert_eq!(a.right(), c.right());
    assert_eq!(b.left(), d.left());
    assert_eq!(b.right(), d.right());
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs[0].panes, records);
    });
}

#[gpui::test]
fn outer_top_gesture_places_a_full_width_pane_above_the_remaining_layout(cx: &mut TestAppContext) {
    let (mut state, _, [a, b, c, _]) = state();
    state.split_pane(b, Direction::Right).expect("split");
    state.split_pane(c, Direction::Right).expect("split");
    let (mut boot, _requests) = stub_boot(state);
    boot.terminal.options.padding_x = [0.0; 2];
    boot.terminal.options.padding_y = [0.0; 2];
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1001.0), px(701.0)));
    cx.run_until_parked();
    let bounds = view.read_with(cx, |model, _| model.layout_drag.geometry.get().0);
    drag_to(
        &view,
        a,
        point(bounds.center().x, bounds.top() + px(6.0)),
        cx,
    );
    let actual = pane_bounds(&view, a, cx);
    assert_eq!(actual.left(), bounds.left());
    assert_eq!(actual.right(), bounds.right());
    assert_eq!(actual.top(), bounds.top());
    assert!((actual.size.height - bounds.size.height / 3.0).abs() <= px(1.0));
    view.read_with(cx, |model, _| {
        let Layout::Split { axis, second, .. } = &model.state.home().tabs[0].layout else {
            panic!("split");
        };
        assert_eq!(*axis, Axis::Vertical);
        assert_eq!(second.leaves().len(), 4);
    });
}

#[gpui::test]
fn boundary_jitter_and_release_commit_the_displayed_plan(cx: &mut TestAppContext) {
    for divider in [false, true] {
        let (state, _, [a, _, c, _]) = state();
        let (mut boot, _requests) = stub_boot(state);
        boot.terminal.options.padding_x = [0.0; 2];
        boot.terminal.options.padding_y = [0.0; 2];
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.simulate_resize(size(px(1001.0), px(701.0)));
        cx.run_until_parked();
        let target = pane_bounds(&view, c, cx);
        let (source, start, jitter) = if divider {
            (
                c,
                point(target.left() - px(0.5), target.center().y),
                point(px(2.0), px(0.0)),
            )
        } else {
            (
                a,
                point(
                    target.center().x,
                    target.top() + target.size.height * 0.3 + px(1.0),
                ),
                point(px(0.0), px(-3.0)),
            )
        };
        let from = pane_bounds(&view, source, cx).center();
        press(cx, from, true);
        pointer(cx, start, true);
        let preview = view.read_with(cx, |model, _| {
            model.layout_drag.preview.clone().expect("preview")
        });
        pointer(cx, start + jitter, true);
        view.read_with(cx, |model, _| {
            assert_eq!(model.layout_drag.preview.as_ref(), Some(&preview));
        });
        release(cx, start + jitter * 2.0);
        assert_eq!(pane_bounds(&view, source, cx), preview.bounds);
    }
}

#[gpui::test]
fn stale_plans_cancel_if_the_tree_or_window_geometry_changes(cx: &mut TestAppContext) {
    for resize in [false, true] {
        let (state, [tab, _], [a, _, c, _]) = state();
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.run_until_parked();
        let from = pane_bounds(&view, a, cx).center();
        let target = pane_bounds(&view, c, cx).center();
        press(cx, from, true);
        pointer(cx, target, true);
        assert!(view.read_with(cx, |model, _| model.layout_drag.preview.is_some()));
        if resize {
            cx.simulate_resize(size(px(1301.0), px(901.0)));
        } else {
            cx.update(|_, cx| {
                view.update(cx, |model, cx| {
                    model.state.set_ratio(tab, &[], 0.61).expect("ratio");
                    cx.notify();
                });
            });
        }
        cx.run_until_parked();
        let before = view.read_with(cx, |model, _| model.state.clone());
        release(cx, target);
        view.read_with(cx, |model, _| {
            assert_eq!(model.state, before);
            assert!(!model.layout_drag_pending());
            assert!(model.layout_drag.preview.is_none());
        });
    }
}

#[gpui::test]
fn reordering_columns_keeps_their_widths_and_dropping_in_place_changes_nothing(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let a = state.window().active_pane.expect("pane");
    let b = state.split_pane(a, Direction::Right).expect("split");
    let c = state.split_pane(b, Direction::Right).expect("split");
    for (index, pane) in [a, b, c].into_iter().enumerate() {
        state
            .set_pane_session(pane, SessionId::new(200 + index as u64))
            .expect("session");
    }
    let (mut boot, _requests) = stub_boot(state);
    boot.terminal.options.padding_x = [0.0; 2];
    boot.terminal.options.padding_y = [0.0; 2];
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1201.0), px(701.0)));
    cx.run_until_parked();
    let widths = [a, b, c].map(|pane| pane_bounds(&view, pane, cx).size.width);
    let original = view.read_with(cx, |model, _| model.state.home().tabs[0].layout.clone());
    let beside = |pane, cx: &mut VisualTestContext| {
        let bounds = pane_bounds(&view, pane, cx);
        point(bounds.left() + bounds.size.width * 0.15, bounds.center().y)
    };
    let in_place = beside(b, cx);
    let from = pane_bounds(&view, a, cx).center();
    press(cx, from, true);
    pointer(cx, in_place, true);
    view.read_with(cx, |model, _| assert!(model.layout_drag.preview.is_none()));
    release(cx, in_place);
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs[0].layout, original);
    });
    let between = beside(c, cx);
    drag_to(&view, a, between, cx);
    let [a, b, c] = [a, b, c].map(|pane| pane_bounds(&view, pane, cx));
    assert!(b.right() < a.left() && a.right() < c.left());
    for (moved, width) in [(b, widths[1]), (a, widths[0]), (c, widths[2])] {
        assert!((moved.size.width - width).abs() <= px(2.0));
    }
}
