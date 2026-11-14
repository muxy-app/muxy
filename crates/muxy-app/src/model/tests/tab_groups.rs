use super::*;
use gpui::{Bounds, MouseButton, Point, point};

fn state() -> (AppState, [TabId; 3]) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tabs = [0, 1, 2].map(|index| {
        let tab = state.open_terminal_tab(home).expect("tab");
        let pane = state.window().active_pane.expect("pane");
        state
            .set_pane_session(pane, SessionId::new(300 + index))
            .expect("session");
        tab
    });
    state.select_tab(home, tabs[0]).expect("select");
    (state, tabs)
}

fn open(state: AppState, cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (mut boot, _requests) = stub_boot(state);
    boot.terminal.options.padding_x = [0.0; 2];
    boot.terminal.options.padding_y = [0.0; 2];
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1201.0), px(801.0)));
    cx.run_until_parked();
    (view, cx)
}

fn drag(cx: &mut VisualTestContext, from: Point<gpui::Pixels>, to: Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseDownEvent {
        position: from,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
    for position in [from + point(px(0.0), px(6.0)), to] {
        cx.simulate_event(gpui::MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        });
        cx.run_until_parked();
    }
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

fn content(
    view: &Entity<AppModel>,
    tab: TabId,
    cx: &mut VisualTestContext,
) -> Bounds<gpui::Pixels> {
    view.read_with(cx, |model, _| {
        model.layout_drag.geometry.get(tab).expect("tab geometry").0
    })
}

fn cell(cx: &mut VisualTestContext, selector: &str) -> Point<gpui::Pixels> {
    cx.debug_bounds(selector.to_owned().leak())
        .unwrap_or_else(|| panic!("{selector}"))
        .center()
}

fn groups(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> Vec<Vec<TabId>> {
    view.read_with(cx, |model, _| {
        model.state.home().groups().map_or_else(Vec::new, |groups| {
            groups
                .layout()
                .leaves()
                .into_iter()
                .filter_map(|id| groups.group(id))
                .map(|group| group.tabs().to_vec())
                .collect()
        })
    })
}

fn first_pane(model: &AppModel, tab: TabId) -> PaneId {
    model.tab(tab).expect("tab").layout.leaves()[0]
}

#[gpui::test]
fn dragging_a_tab_to_an_edge_opens_it_beside_the_previous_tab(cx: &mut TestAppContext) {
    let (state, [a, b, c]) = state();
    let (view, cx) = open(state, cx);
    let bounds = content(&view, a, cx);
    let target = point(bounds.right() - px(20.0), bounds.center().y);
    let from = cell(cx, "tab-cell-2");
    drag(cx, from, target);
    let preview = view.read_with(cx, |model, _| model.tab_drag.preview().expect("preview"));
    assert_eq!(preview.left(), bounds.center().x);
    release(cx, target);
    assert_eq!(groups(&view, cx), [vec![a, b], vec![c]]);
    view.read_with(cx, |model, _| {
        assert_eq!(model.visible_tabs(), [a, c]);
        assert_eq!(model.active_tab(), Some(c));
        assert!(model.tab_drag.preview().is_none());
        for tab in [a, c] {
            assert!(model.grids.contains_key(&first_pane(model, tab)));
        }
        assert!(!model.grids.contains_key(&first_pane(model, b)));
        assert!(model.error.is_none(), "{:?}", model.error);
    });
    let left = content(&view, a, cx);
    let right = content(&view, c, cx);
    assert!(left.right() <= right.left());
    assert!(cx.debug_bounds("group-divider-[]").is_some());
}
