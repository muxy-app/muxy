use super::*;
use muxy_ui::scrollbar::REVEAL_DURATION;

/// A three-row pane, sixty pixels tall, above twenty-seven rows of history.
/// Its thumb is twenty-four pixels long and travels thirty-six. The pane is
/// never drawn, so its size stays as set here.
fn pane(cx: &mut TestAppContext) -> (gpui::Entity<TerminalPane>, &mut gpui::VisualTestContext) {
    let pane = cx.new(|cx| {
        let mut pane = TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        );
        prepare_mouse(&mut pane);
        let grid = pane.grid.as_mut().unwrap();
        grid.history = (0..27)
            .map(|index| row(index, &index.to_string()))
            .collect();
        grid.history_total = 27;
        pane.scrollbar.place(Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(200.0), px(60.0)),
        ));
        pane
    });
    (pane, cx.add_empty_window())
}

fn thumb_top(pane: &TerminalPane, cx: &gpui::App) -> Option<f32> {
    pane.scrollbar_thumb(cx.background_executor().now())
        .map(|thumb| f32::from(thumb.top()))
}

fn pointer(y: f32, pressed: bool) -> gpui::MouseMoveEvent {
    gpui::MouseMoveEvent {
        position: point(px(195.0), px(y)),
        pressed_button: pressed.then_some(MouseButton::Left),
        ..gpui::MouseMoveEvent::default()
    }
}

fn press(pane: &mut TerminalPane, y: f32, window: &mut Window, cx: &mut Context<TerminalPane>) {
    pane.mouse_down(
        &gpui::MouseDownEvent {
            position: point(px(195.0), px(y)),
            click_count: 1,
            ..gpui::MouseDownEvent::default()
        },
        window,
        cx,
    );
}

#[gpui::test]
fn dragging_the_thumb_scrolls_history_without_selecting_text(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    pane.update_in(cx, |pane, window, cx| {
        press(pane, 40.0, window, cx);
        assert!(!pane.scrollbar_dragging());
        assert!(pane.selecting.take().is_some());

        pane.mouse_move(&pointer(40.0, false), cx);
        press(pane, 40.0, window, cx);
        assert!(pane.scrollbar_dragging());
        assert!(pane.scroll.view.is_none());

        pane.mouse_move(&pointer(22.0, true), cx);
        assert_eq!(pane.scroll.offset.to_bits(), 13.5_f64.to_bits());
        assert_eq!(thumb_top(pane, cx), Some(18.0));
        let mut frame = ScreenFrame {
            size: pane.grid.as_ref().unwrap().size,
            graphics: None,
            seq: 1,
            reset: false,
            rows: vec![row(0, "new output")],
            cursor: pane.grid.as_ref().unwrap().cursor,
            modes: Modes::default(),
        };
        frame.cursor.col = 0;
        pane.apply(&frame, cx);
        assert_eq!(pane.scroll.offset.to_bits(), 13.5_f64.to_bits());

        pane.mouse_move(&pointer(-500.0, true), cx);
        assert_eq!(pane.scroll.offset.to_bits(), 27.0_f64.to_bits());
        pane.mouse_move(&pointer(500.0, true), cx);
        assert!(pane.scroll.view.is_none());
        assert!(pane.selecting.is_none() && pane.selection.is_none());

        pane.mouse_up(
            &gpui::MouseUpEvent {
                position: point(px(195.0), px(500.0)),
                ..gpui::MouseUpEvent::default()
            },
            window,
            cx,
        );
        assert!(!pane.scrollbar_dragging());
        assert_eq!(thumb_top(pane, cx), Some(36.0));
    });
    cx.executor().advance_clock(REVEAL_DURATION);
    cx.run_until_parked();
    pane.update(cx, |pane, cx| assert_eq!(thumb_top(pane, cx), None));
}

#[gpui::test]
fn dragging_past_the_loaded_history_asks_for_older_rows(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    let requests = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        let requests = requests.clone();
        cx.subscribe(&pane, move |_, event, _| {
            if let PaneEvent::History(request) = event {
                requests.borrow_mut().push(*request);
            }
        })
        .detach();
    });
    pane.update_in(cx, |pane, window, cx| {
        let grid = pane.grid.as_mut().unwrap();
        grid.history_total = 100;
        grid.history_cursor = Some(muxy_protocol::HistoryCursor(42));
        pane.mouse_move(&pointer(40.0, false), cx);
        press(pane, 40.0, window, cx);
        pane.mouse_move(&pointer(-500.0, true), cx);
        assert_eq!(pane.scroll.offset.to_bits(), 27.0_f64.to_bits());
        assert_eq!(thumb_top(pane, cx), Some(0.0));
    });
    assert_eq!(
        requests
            .borrow()
            .iter()
            .map(|request| request.before)
            .collect::<Vec<_>>(),
        [muxy_protocol::HistoryCursor(42)]
    );
}

#[gpui::test]
fn a_pane_without_history_or_with_a_program_reading_the_wheel_has_no_scrollbar(
    cx: &mut TestAppContext,
) {
    let (pane, cx) = pane(cx);
    pane.update_in(cx, |pane, window, cx| {
        pane.input_modes.mouse_tracking = true;
        pane.scroll_rows(9.0, cx);
        pane.mouse_move(&pointer(40.0, false), cx);
        assert_eq!(thumb_top(pane, cx), None);
        pane.input_modes.mouse_tracking = false;

        let grid = pane.grid.as_mut().unwrap();
        grid.history.clear();
        grid.history_total = 0;
        pane.scroll.reset();
        pane.mouse_move(&pointer(40.0, false), cx);
        assert_eq!(thumb_top(pane, cx), None);
        press(pane, 40.0, window, cx);
        assert!(!pane.scrollbar_dragging());
    });
}
