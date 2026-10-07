use super::*;

fn pane(cx: &mut TestAppContext) -> gpui::Entity<TerminalPane> {
    cx.new(|cx| {
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
    })
}

fn start(pane: &mut TerminalPane, clicks: usize, cx: &mut Context<TerminalPane>) {
    let point = Point { row: 1, column: 3 };
    let grid = pane.displayed_grid().unwrap();
    let anchor = match clicks {
        1 => Selection {
            anchor: point,
            head: point,
        },
        2 => Selection::word(point, grid),
        _ => Selection::row(point, grid),
    };
    pane.selecting = Some((anchor, clicks));
    pane.select(anchor, cx);
}

fn drag(pane: &mut TerminalPane, y: f32, cx: &mut Context<TerminalPane>) {
    pane.mouse_move(
        &gpui::MouseMoveEvent {
            position: point(px(30.0), px(y)),
            pressed_button: Some(MouseButton::Left),
            ..gpui::MouseMoveEvent::default()
        },
        cx,
    );
}

fn tick(cx: &mut TestAppContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
}

#[gpui::test]
fn selection_autoscroll_pages_history_and_extends_after_reply(cx: &mut TestAppContext) {
    let pane = pane(cx);
    let requests = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    cx.update(|cx| {
        let requests = requests.clone();
        cx.subscribe(&pane, move |_, event, _| {
            if let PaneEvent::History(request) = event {
                requests.borrow_mut().push(*request);
            }
        })
        .detach();
    });
    pane.update(cx, |pane, cx| {
        let grid = pane.grid.as_mut().unwrap();
        grid.history_total = 20;
        grid.history_cursor = Some(muxy_protocol::HistoryCursor(42));
        start(pane, 1, cx);
        drag(pane, -1000.0, cx);
    });
    for _ in 0..5 {
        tick(cx);
    }
    assert_eq!(requests.borrow().len(), 1);
    let request = requests.borrow()[0];
    pane.update(cx, |pane, cx| {
        assert!(pane.selection_scroll.is_none());
        pane.receive_history(
            request,
            Ok(HistoryPage {
                rows: (0..10).map(|index| row(index, "older")).collect(),
                next: None,
                total_rows: 20,
                prompts: Vec::new(),
                screen: None,
            }),
            cx,
        );
        assert_eq!(pane.selection.unwrap().head.row, -15);
        assert_eq!(pane.selection.unwrap().anchor, Point { row: 1, column: 3 });
        assert!(pane.selection_scroll.is_some());
    });
    tick(cx);
    pane.update(cx, |pane, cx| {
        assert_eq!(pane.selection.unwrap().head.row, -20);
        assert!(
            pane.selection
                .unwrap()
                .text(pane.displayed_grid().unwrap())
                .starts_with("er\nolder")
        );
        let point = Point {
            row: -20,
            column: 0,
        };
        let anchor = Selection {
            anchor: point,
            head: point,
        };
        pane.selecting = Some((anchor, 1));
        pane.select(anchor, cx);
        drag(pane, 1000.0, cx);
    });
    for _ in 0..5 {
        tick(cx);
    }
    pane.read_with(cx, |pane, _| {
        assert!(pane.scroll.offset.abs() < 0.01);
        let selection = pane.selection.unwrap();
        assert_eq!(selection.anchor.row, -20);
        assert_eq!(selection.head.row, 2);
        assert!(
            selection
                .text(pane.displayed_grid().unwrap())
                .starts_with("older\nolder")
        );
        assert!(pane.selection_scroll.is_none());
    });
}

#[gpui::test]
fn selection_autoscroll_cancels_with_drag_lifecycle(cx: &mut TestAppContext) {
    let pane = pane(cx);
    for cancel in 0..6 {
        pane.update(cx, |pane, cx| {
            prepare_mouse(pane);
            pane.native_visible = true;
            start(pane, 1, cx);
            drag(pane, 5.0, cx);
            assert!(pane.selection_scroll.is_some());
            match cancel {
                0 => pane.clear_selection(),
                1 => pane.focus_changed(false, cx),
                2 => pane.set_state(PaneState::Disconnected, cx),
                3 => pane.native_visible = false,
                4 => pane.mouse_move(&gpui::MouseMoveEvent::default(), cx),
                _ => {
                    pane.state = PaneState::Exited {
                        reason: None,
                        unavailable: false,
                    };
                    pane.focus_changed(false, cx);
                }
            }
        });
        tick(cx);
        pane.read_with(cx, |pane, _| {
            assert!(pane.scroll.offset.abs() < 0.01);
            assert!(pane.selecting.is_none());
            assert!(pane.selection_scroll.is_none());
        });
    }
}
