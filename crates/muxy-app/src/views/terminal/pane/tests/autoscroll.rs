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
fn selection_autoscroll_repeats_without_motion_and_reverses(cx: &mut TestAppContext) {
    let pane = pane(cx);
    for clicks in [1, 2, 3] {
        pane.update(cx, |pane, cx| {
            pane.clear_selection();
            pane.scroll.bottom();
            start(pane, clicks, cx);
            drag(pane, 5.0, cx);
        });
        let revision = pane.read_with(cx, |pane, _| pane.scroll.revision);
        tick(cx);
        tick(cx);
        pane.update(cx, |pane, cx| {
            assert_ne!(pane.scroll.revision, revision);
            assert!((pane.scroll.offset - 2.0).abs() < 0.01);
            let selection = pane.selection.unwrap();
            assert_eq!(selection.head.row, -2);
            assert_eq!(selection.anchor.row, 1);
            assert_eq!(selection.head.column, if clicks < 3 { 3 } else { 0 });
            drag(pane, 55.0, cx);
        });
        tick(cx);
        pane.update(cx, |pane, cx| {
            assert!((pane.scroll.offset - 1.0).abs() < 0.01);
            if clicks == 1 {
                assert!(
                    pane.selection.is_none(),
                    "returning to the anchor collapses selection"
                );
            } else {
                assert_eq!(pane.selection.unwrap().head.row, 1);
            }
            drag(pane, 30.0, cx);
            assert!(pane.selection_scroll.is_none());
        });
        tick(cx);
        pane.read_with(cx, |pane, _| {
            assert!((pane.scroll.offset - 1.0).abs() < 0.01);
        });
    }
}

#[gpui::test]
fn selection_autoscroll_clamps_edges_and_stops_at_history_limits(cx: &mut TestAppContext) {
    let pane = pane(cx);
    pane.update(cx, |pane, cx| {
        start(pane, 1, cx);
        drag(pane, -1000.0, cx);
    });
    for _ in 0..3 {
        tick(cx);
    }
    pane.update(cx, |pane, cx| {
        assert!((pane.scroll.offset - 10.0).abs() < 0.01);
        assert_eq!(pane.selection.unwrap().head.row, -10);
        assert!(pane.scroll.elastic.abs() < 0.01);
        assert!(pane.selection_scroll.is_none());
        drag(pane, 1000.0, cx);
    });
    for _ in 0..3 {
        tick(cx);
    }
    pane.read_with(cx, |pane, _| {
        assert!(pane.scroll.offset.abs() < 0.01);
        assert_eq!(pane.selection.unwrap().head.row, 2);
        assert!(pane.scroll.elastic.abs() < 0.01);
        assert!(pane.selection_scroll.is_none());
    });
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

#[gpui::test]
fn selection_autoscroll_mouse_release_copies_once_and_cancels_timer(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(|_, cx| {
        TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        )
    });
    cx.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            prepare_mouse(pane);
            pane.copy_on_select = true;
            pane.input_modes.mouse_tracking = true;
            pane.mouse_down(
                &gpui::MouseDownEvent {
                    position: point(px(0.0), px(25.0)),
                    click_count: 1,
                    modifiers: gpui::Modifiers {
                        shift: true,
                        ..gpui::Modifiers::default()
                    },
                    ..gpui::MouseDownEvent::default()
                },
                window,
                cx,
            );
            drag(pane, 5.0, cx);
            assert!(pane.selection_scroll.is_some());
            assert!(pane.scroll_selection(cx));
            assert!(pane.held_buttons.is_empty());
            pane.mouse_up(
                &gpui::MouseUpEvent {
                    position: point(px(30.0), px(5.0)),
                    ..gpui::MouseUpEvent::default()
                },
                window,
                cx,
            );
            assert!(pane.selection_scroll.is_none());
            assert!(pane.selection_pointer.is_none());
            assert!(pane.selecting.is_none());
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some("tory row\nalpha beta")
            );
        });
    });
}

#[gpui::test]
fn selection_autoscroll_cancelled_target_does_not_move_on_delayed_history(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(|_, cx| {
        TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        )
    });
    for cancel in 0..3 {
        cx.update(|window, cx| {
            pane.update(cx, |pane, cx| {
                pane.clear_selection();
                pane.scroll.bottom();
                prepare_mouse(pane);
                let grid = pane.grid.as_mut().unwrap();
                grid.history = (0..10).map(|index| row(index, "history")).collect();
                grid.history_total = 20;
                grid.history_cursor = Some(muxy_protocol::HistoryCursor(42));
                start(pane, 1, cx);
                drag(pane, -1000.0, cx);
                let request = pane
                    .scroll
                    .move_selection(10.0, pane.grid.as_ref().unwrap(), 3)
                    .unwrap();
                pane.scroll_selection(cx);
                assert!((pane.scroll.requested_pixels(20.0) - 300.0).abs() < 0.01);
                match cancel {
                    0 => pane.mouse_up(
                        &gpui::MouseUpEvent {
                            position: point(px(30.0), px(-1000.0)),
                            ..gpui::MouseUpEvent::default()
                        },
                        window,
                        cx,
                    ),
                    1 => pane.focus_changed(false, cx),
                    _ => drag(pane, 30.0, cx),
                }
                let selected = pane.selection;
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
                assert!((pane.scroll.offset - 10.0).abs() < 0.01, "cancel {cancel}");
                assert_eq!(pane.selection, selected);
                assert!(pane.selection_scroll.is_none());
            });
        });
    }
}
