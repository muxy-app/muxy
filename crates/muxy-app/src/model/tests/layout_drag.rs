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
