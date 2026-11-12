use super::*;

#[gpui::test]
fn double_click_and_drag_use_current_word_characters(cx: &mut TestAppContext) {
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
            pane.grid.as_mut().unwrap().rows[0] = row(0, "a_b/c_d e-f").runs;
            let defaults = pane.terminal.options.selection_word_chars.clone();
            for (characters, clicked, dragged) in [
                (defaults.as_str(), "a_b/c_d", "a_b/c_d e-f"),
                ("_", "a_b", "a_b/c_d e-"),
                ("", "b", "b/c_d e-"),
            ] {
                pane.terminal.options.selection_word_chars = characters.into();
                for backwards in [false, true] {
                    let (start, end) = if backwards {
                        (95.0, 25.0)
                    } else {
                        (25.0, 95.0)
                    };
                    pane.mouse_down(
                        &gpui::MouseDownEvent {
                            position: point(px(start), px(5.0)),
                            click_count: 2,
                            ..gpui::MouseDownEvent::default()
                        },
                        window,
                        cx,
                    );
                    if !backwards {
                        assert_eq!(
                            pane.selection.unwrap().text(pane.displayed_grid().unwrap()),
                            clicked
                        );
                    }
                    pane.mouse_move(
                        &gpui::MouseMoveEvent {
                            position: point(px(end), px(5.0)),
                            pressed_button: Some(MouseButton::Left),
                            ..gpui::MouseMoveEvent::default()
                        },
                        cx,
                    );
                    pane.copy_selection(cx);
                    assert_eq!(
                        cx.read_from_clipboard().unwrap().text().as_deref(),
                        Some(dragged)
                    );
                    pane.mouse_up(
                        &gpui::MouseUpEvent {
                            position: point(px(end), px(5.0)),
                            ..gpui::MouseUpEvent::default()
                        },
                        window,
                        cx,
                    );
                }
            }
        });
    });
}
