use super::*;
use crate::views::move_drag::Moving;

fn listed(model: &AppModel) -> Vec<TabId> {
    model
        .state
        .current_project()
        .listed_tabs()
        .map(|tab| tab.id)
        .collect()
}

#[gpui::test]
fn a_dragged_pane_shows_its_chip_and_drop_place_from_its_first_move(cx: &mut TestAppContext) {
    let (state, [tab, _], [_, _, c, _]) = state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let handle = cx
        .debug_bounds(format!("pane-handle-{c}").leak())
        .expect("handle");
    press(cx, handle.center(), false);
    let nudged = handle.center() + point(px(12.0), px(0.0));
    pointer(cx, nudged, true);
    let main = view.read_with(cx, |model, _| model.window.window_id());
    let home = pane_bounds(&view, c, cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.move_drag.lifted(), Some(Moving::Pane(c)));
        assert_eq!(model.move_drag.pointer(), Some((main, nudged)));
        assert_eq!(model.move_drag.marked(), Some(home));
    });
    let bounds = view.read_with(cx, |model, _| {
        model.layout_drag.geometry.get(tab).expect("geometry").0
    });
    let edge = point(bounds.right() - px(6.0), bounds.center().y);
    pointer(cx, edge, true);
    view.read_with(cx, |model, _| {
        let preview = model.layout_drag.preview.as_ref().expect("preview");
        assert_eq!(model.move_drag.marked(), Some(preview.bounds));
        assert_eq!(model.move_drag.pointer(), Some((main, edge)));
    });
    release(cx, edge);
    view.read_with(cx, |model, _| {
        assert_eq!(model.move_drag.lifted(), None);
        assert!(!model.layout_drag_pending());
    });
}

#[gpui::test]
fn a_drag_chip_goes_away_when_its_drag_is_cancelled(cx: &mut TestAppContext) {
    let (state, [_, _], [a, _, _, _]) = state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let from = pane_bounds(&view, a, cx).center();
    press(cx, from, true);
    pointer(cx, point(px(-40.0), px(-40.0)), true);
    assert!(view.read_with(cx, |model, _| model.move_drag.lifted().is_some()));
    cx.simulate_resize(size(px(1301.0), px(901.0)));
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(!model.layout_drag_pending());
        assert_eq!(model.move_drag.lifted(), None);
    });
}

#[gpui::test]
fn right_clicking_a_pane_header_opens_that_panes_menu(cx: &mut TestAppContext) {
    let (state, [_, _], [_, b, _, _]) = state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert_ne!(view.read_with(cx, |model, _| model.active_pane()), Some(b));
    let header = cx
        .debug_bounds(format!("pane-header-{b}").leak())
        .expect("header");
    let position = point(header.right() - px(4.0), header.center().y);
    cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(b));
        assert!(matches!(model.overlay, Some(Overlay::Menu(_))));
        assert!(!model.layout_drag_pending());
    });
}

#[gpui::test]
fn a_dragged_tab_trades_places_only_past_its_neighbors_middle(cx: &mut TestAppContext) {
    let (state, [a, b], _) = state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let first = cx.debug_bounds("tab-cell-0").expect("first cell");
    let second = cx.debug_bounds("tab-cell-1").expect("second cell");
    press(cx, first.center(), false);
    pointer(cx, point(second.left() + px(4.0), second.center().y), true);
    view.read_with(cx, |model, _| {
        assert_eq!(listed(model), [a, b]);
        assert_eq!(model.tab_drag.lifted(), Some(a));
        assert_eq!(model.move_drag.lifted(), Some(Moving::Tab(a)));
    });
    pointer(
        cx,
        point(second.center().x + px(4.0), second.center().y),
        true,
    );
    view.read_with(cx, |model, _| {
        assert_eq!(listed(model), [b, a]);
        assert_eq!(model.move_drag.marked(), Some(second));
    });
    for x in [
        second.left() - px(4.0),
        second.left() + px(4.0),
        second.left() - px(8.0),
    ] {
        pointer(cx, point(x, second.center().y), true);
        assert_eq!(view.read_with(cx, |model, _| listed(model)), [b, a]);
    }
    pointer(
        cx,
        point(first.center().x - px(4.0), first.center().y),
        true,
    );
    assert_eq!(view.read_with(cx, |model, _| listed(model)), [a, b]);
    release(cx, point(first.center().x - px(4.0), first.center().y));
    view.read_with(cx, |model, _| {
        assert_eq!(listed(model), [a, b]);
        assert!(!model.layout_drag_pending());
        assert_eq!(model.move_drag.lifted(), None);
        assert_eq!(store::load(&model.path).expect("saved"), model.state);
    });
}

#[gpui::test]
fn every_spot_inside_the_tab_marks_where_a_dragged_pane_would_land(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let tab = state.open_terminal_tab(state.home().id).expect("tab");
    let left = state.window().active_pane.expect("pane");
    let right = state.split_pane(left, Direction::Right).expect("split");
    for (index, pane) in [left, right].iter().enumerate() {
        state
            .set_pane_session(*pane, SessionId::new(300 + index as u64))
            .expect("session");
    }
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1001.0), px(701.0)));
    cx.run_until_parked();
    let bounds = view.read_with(cx, |model, _| {
        model.layout_drag.geometry.get(tab).expect("geometry").0
    });
    let home = pane_bounds(&view, right, cx);
    let handle = cx
        .debug_bounds(format!("pane-handle-{right}").leak())
        .expect("handle");
    press(cx, handle.center(), false);
    pointer(cx, handle.center() + point(px(12.0), px(0.0)), true);
    let y = bounds.center().y;
    let mut x = bounds.left() + px(2.0);
    while x < bounds.right() - px(2.0) {
        pointer(cx, point(x, y), true);
        view.read_with(cx, |model, _| {
            let marked = model.move_drag.marked();
            assert!(marked.is_some(), "nothing marked at {x:?}");
            if model.layout_drag.preview.is_none() {
                assert_eq!(marked, Some(home), "the pane stays put at {x:?}");
            }
        });
        x += px(25.0);
    }
    let before = view.read_with(cx, |model, _| model.state.clone());
    release(cx, point(home.center().x, y));
    view.read_with(cx, |model, _| {
        assert_eq!(model.state, before);
        assert!(!model.layout_drag_pending());
    });
}

#[gpui::test]
fn a_tab_dropped_where_nothing_changes_marks_its_own_place_and_stays(cx: &mut TestAppContext) {
    let (state, [a, b], _) = state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let home = cx.debug_bounds("tab-cell-1").expect("cell");
    let content = view.read_with(cx, |model, _| {
        model.layout_drag.geometry.get(a).expect("geometry").0
    });
    press(cx, home.center(), false);
    pointer(cx, home.center() + point(px(0.0), px(12.0)), true);
    pointer(cx, content.center(), true);
    let before = view.read_with(cx, |model, _| {
        assert_eq!(model.tab_drag.preview(), None);
        assert_eq!(model.move_drag.marked(), Some(home));
        model.state.clone()
    });
    release(cx, content.center());
    view.read_with(cx, |model, _| {
        assert_eq!(listed(model), [a, b]);
        assert_eq!(
            model.state.home().groups().is_some(),
            before.home().groups().is_some()
        );
        assert!(model.state.home().groups().is_none());
        assert!(!model.layout_drag_pending());
    });
}
