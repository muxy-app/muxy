use super::*;

/// A three-row pane showing live output, above ten rows of history.
fn pane(cx: &mut TestAppContext) -> (gpui::Entity<TerminalPane>, &mut gpui::VisualTestContext) {
    let pane = cx.new(|cx| {
        let mut pane = TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        );
        prepare_mouse(&mut pane);
        let grid = pane.grid.as_mut().unwrap();
        grid.history = (0..10)
            .map(|index| row(index, &index.to_string()))
            .collect();
        grid.history_total = 10;
        pane
    });
    (pane, cx.add_empty_window())
}

fn wait(cx: &mut gpui::VisualTestContext, time: Duration) {
    cx.run_until_parked();
    cx.executor().advance_clock(time);
    cx.run_until_parked();
}

/// Replaces the screen's first row, as the program's next output does.
fn output(pane: &mut TerminalPane, text: &str, cx: &mut Context<TerminalPane>) {
    let grid = pane.grid.as_ref().unwrap();
    let frame = ScreenFrame {
        size: grid.size,
        graphics: None,
        seq: 1,
        reset: false,
        rows: vec![row(0, text)],
        cursor: grid.cursor,
        modes: Modes::default(),
    };
    pane.apply(&frame, cx);
}

/// The text of the screen's first row as the pane shows it.
fn shown(pane: &TerminalPane) -> String {
    let grid = pane.displayed_grid().unwrap();
    grid.content_row(grid.history.len()).unwrap()[0]
        .text
        .clone()
}

#[gpui::test]
fn a_scroll_resting_under_a_row_above_the_bottom_returns_to_live_output(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    pane.update(cx, |pane, cx| {
        pane.scroll_rows(0.25, cx);
        assert!(pane.scroll.view.is_some());
        output(pane, "new output", cx);
        assert_eq!(shown(pane), "alpha beta");
    });
    wait(cx, SCROLL_SETTLE);
    pane.update(cx, |pane, cx| {
        assert!(pane.scroll.view.is_none());
        assert_eq!(pane.scroll.offset.to_bits(), 0.0_f64.to_bits());
        assert_eq!(shown(pane), "new output");
        output(pane, "newer output", cx);
        assert_eq!(shown(pane), "newer output");
    });
}

#[cfg(target_os = "macos")]
#[gpui::test]
fn a_stray_native_scroll_of_a_few_pixels_returns_to_live_output(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    pane.update(cx, |pane, cx| {
        pane.native_position(
            muxy_ui::native_scroll::ScrollPosition {
                revision: pane.scroll.revision,
                sequence: 1,
                from_bottom: 4.0,
            },
            cx,
        );
        assert!(pane.scroll.view.is_some());
    });
    wait(cx, SCROLL_SETTLE);
    pane.update(cx, |pane, cx| {
        assert!(pane.scroll.view.is_none());
        output(pane, "new output", cx);
        assert_eq!(shown(pane), "new output");
    });
}

#[gpui::test]
fn a_scroll_of_a_row_or_more_stays_where_it_is(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    pane.update(cx, |pane, cx| pane.scroll_rows(1.0, cx));
    wait(cx, SCROLL_SETTLE * 5);
    pane.update(cx, |pane, cx| {
        assert_eq!(pane.scroll.offset.to_bits(), 1.0_f64.to_bits());
        output(pane, "new output", cx);
        assert_eq!(shown(pane), "alpha beta");
    });
}

#[gpui::test]
fn the_view_stays_while_scrolling_goes_on(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    for _ in 0..3 {
        pane.update(cx, |pane, cx| pane.scroll_rows(0.25, cx));
        wait(cx, SCROLL_SETTLE / 2);
        pane.read_with(cx, |pane, _| assert!(pane.scroll.view.is_some()));
    }
    wait(cx, SCROLL_SETTLE / 2);
    pane.read_with(cx, |pane, _| assert!(pane.scroll.view.is_none()));

    pane.update(cx, |pane, cx| {
        pane.scroll_rows(0.25, cx);
        pane.scroll_rows(2.0, cx);
    });
    wait(cx, SCROLL_SETTLE * 5);
    pane.read_with(cx, |pane, _| {
        assert_eq!(pane.scroll.offset.to_bits(), 2.25_f64.to_bits());
    });
}

#[gpui::test]
fn a_drag_holds_the_view_until_it_ends(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    pane.update(cx, |pane, cx| {
        let point = Point { row: 1, column: 3 };
        let selection = Selection {
            anchor: point,
            head: point,
        };
        pane.selecting = Some((selection, 1));
        pane.scroll_rows(0.25, cx);
    });
    wait(cx, SCROLL_SETTLE * 5);
    pane.update_in(cx, |pane, window, cx| {
        assert!(pane.scroll.view.is_some());
        pane.mouse_up(
            &gpui::MouseUpEvent {
                position: point(px(30.0), px(30.0)),
                click_count: 1,
                ..gpui::MouseUpEvent::default()
            },
            window,
            cx,
        );
        assert!(pane.scroll.view.is_some());
    });
    wait(cx, SCROLL_SETTLE);
    pane.read_with(cx, |pane, _| assert!(pane.scroll.view.is_none()));
}

#[gpui::test]
fn a_selection_survives_the_return_or_holds_the_view(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    let first_row = |pane: &TerminalPane| {
        Selection::row(Point { row: 0, column: 0 }, pane.displayed_grid().unwrap())
    };
    pane.update(cx, |pane, cx| {
        pane.select(first_row(pane), cx);
        pane.scroll_rows(0.25, cx);
    });
    wait(cx, SCROLL_SETTLE);
    pane.update(cx, |pane, cx| {
        assert!(pane.scroll.view.is_none());
        assert_eq!(pane.selection, Some(first_row(pane)));

        pane.scroll_rows(0.25, cx);
        output(pane, "new output", cx);
    });
    wait(cx, SCROLL_SETTLE * 5);
    pane.read_with(cx, |pane, _| {
        assert!(pane.scroll.view.is_some());
        assert_eq!(
            pane.selection.unwrap().text(pane.displayed_grid().unwrap()),
            "alpha beta"
        );
    });
}
