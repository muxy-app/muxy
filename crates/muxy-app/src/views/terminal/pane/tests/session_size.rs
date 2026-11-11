use super::*;

/// A frame from the server after the session took `size`, as when another
/// client resized it.
fn frame(size: Size, seq: u64) -> ScreenFrame {
    ScreenFrame {
        size,
        graphics: None,
        seq,
        reset: true,
        rows: Vec::new(),
        cursor: Cursor {
            shape: muxy_protocol::CursorShape::default(),
            row: 0,
            col: 0,
            visible: true,
        },
        modes: Modes::default(),
    }
}

fn moved(x: f32) -> gpui::MouseMoveEvent {
    gpui::MouseMoveEvent {
        position: point(px(x), px(5.0)),
        ..gpui::MouseMoveEvent::default()
    }
}

#[gpui::test]
fn working_in_a_pane_takes_back_a_size_another_client_changed(cx: &mut TestAppContext) {
    // Not drawn, so only taking the size back changes the viewport.
    let pane = cx.new(|cx| {
        TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        )
    });
    let cx = cx.add_empty_window();
    let requested = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let phone = Size { cols: 8, rows: 2 };
    let mut own = None;
    cx.update(|window, cx| {
        let sizes = requested.clone();
        cx.subscribe(&pane, move |_, event, _| {
            if let PaneEvent::Viewport(size) = event {
                sizes.borrow_mut().push(*size);
            }
        })
        .detach();
        pane.update(cx, |pane, cx| {
            prepare_mouse(pane);
            let viewport = pane.viewport.unwrap();
            own = Some(viewport);

            // The session already has the pane's size.
            pane.apply(&frame(viewport, 1), cx);
            pane.mouse_move(&moved(5.0), cx);
            pane.send_text("a", cx);

            // A phone resized it: the pointer outside the pane changes
            // nothing, and moving over the pane asks for its size once.
            pane.apply(&frame(phone, 2), cx);
            pane.mouse_move(&moved(250.0), cx);
            pane.mouse_move(&moved(5.0), cx);
            pane.mouse_move(&moved(15.0), cx);
            assert_eq!(pane.grid.as_ref().unwrap().size, viewport);

            pane.apply(&frame(phone, 3), cx);
            pane.send_text("a", cx);

            pane.apply(&frame(phone, 4), cx);
            pane.set_focused(false, cx);
            pane.set_focused(true, cx);

            pane.apply(&frame(phone, 5), cx);
            pane.mouse_down(
                &gpui::MouseDownEvent {
                    button: MouseButton::Left,
                    position: point(px(5.0), px(5.0)),
                    ..gpui::MouseDownEvent::default()
                },
                window,
                cx,
            );

            // A pane that is no longer live leaves the session alone.
            pane.apply(&frame(phone, 6), cx);
            pane.state = PaneState::Exited {
                reason: Some(ExitReason::Ended),
                unavailable: false,
            };
            pane.mouse_move(&moved(5.0), cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(*requested.borrow(), vec![own.unwrap(); 4]);
}
