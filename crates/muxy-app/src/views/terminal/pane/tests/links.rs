use super::*;

fn history_page(terminal: &mut muxy_terminal::Terminal, size: Size) -> HistoryPage {
    let muxy_terminal::TerminalHistory {
        rows,
        prompts,
        mut links,
    } = terminal.history_with_links(0..5).unwrap();
    links.extend(
        terminal
            .read_screen_links(muxy_protocol::MAX_LINK_SPANS - links.len())
            .unwrap()
            .into_iter()
            .map(|mut row| {
                row.row += 5;
                row
            }),
    );
    HistoryPage {
        links: Some(links),
        prompts,
        rows,
        next: None,
        total_rows: 5,
        screen: Some(SavedScreen {
            graphics: muxy_protocol::Graphics::default(),
            size,
            rows: terminal.screen().unwrap(),
            cursor: terminal.cursor().unwrap(),
            reason: None,
        }),
    }
}

#[gpui::test]
fn command_click_opens_osc8_links_after_scrolling_into_history(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(|_, cx| {
        TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        )
    });
    let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    cx.update(|window, cx| {
        let targets = opened.clone();
        cx.subscribe(&pane, move |_, event, _| {
            if let PaneEvent::OpenLink(target) = event {
                targets.borrow_mut().push(target.clone());
            }
        })
        .detach();
        pane.update(cx, |pane, cx| {
            prepare_mouse(pane);
            let size = pane.viewport.unwrap();
            let mut terminal = muxy_terminal::Terminal::new(size, 1_000_000).unwrap();
            terminal.feed(b"\x1b]8;;https://example.com/1\x07link 1\x1b]8;;\x07");
            let frame = |terminal: &mut muxy_terminal::Terminal, seq| ScreenFrame {
                size,
                graphics: None,
                seq,
                reset: false,
                rows: terminal.take_changed_rows().unwrap(),
                cursor: terminal.cursor().unwrap(),
                modes: terminal.modes().unwrap(),
            };
            pane.apply(&frame(&mut terminal, 1), cx);
            pane.metadata(
                MetadataEvent::Links {
                    seq: 1,
                    rows: terminal.screen_links(),
                },
                cx,
            );
            let mut click = gpui::MouseDownEvent {
                position: point(px(10.0), px(5.0)),
                click_count: 1,
                modifiers: gpui::Modifiers {
                    platform: cfg!(target_os = "macos"),
                    control: !cfg!(target_os = "macos"),
                    ..Default::default()
                },
                ..Default::default()
            };
            pane.mouse_down(&click, window, cx);
            assert_eq!(
                pane.link_hover.target,
                Some(muxy_app_core::opener::Target::Url(
                    "https://example.com/1".into()
                )),
                "the live-screen link resolves"
            );

            for index in 2..=8 {
                terminal.feed(
                    format!("\r\n\x1b]8;;https://example.com/{index}\x07link {index}\x1b]8;;\x07")
                        .as_bytes(),
                );
            }
            pane.apply(&frame(&mut terminal, 2), cx);
            pane.metadata(
                MetadataEvent::Links {
                    seq: 2,
                    rows: terminal.screen_links(),
                },
                cx,
            );
            let request = pane
                .scroll
                .move_rows(5.0, pane.grid.as_ref().unwrap(), 3)
                .unwrap();
            pane.receive_history(request, Ok(history_page(&mut terminal, size)), cx);
            assert!((pane.scroll.offset - 5.0).abs() < f64::EPSILON);
            pane.mouse_down(&click, window, cx);
            let live = pane.grid.as_ref().unwrap();
            pane.scroll.move_rows(-4.0, live, 3);
            click.position = point(px(10.0), px(45.0));
            pane.mouse_down(&click, window, cx);
            pane.scroll.bottom();
            pane.mouse_down(&click, window, cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(
        *opened.borrow(),
        [1, 1, 7, 8]
            .map(|index| muxy_app_core::opener::Target::Url(format!("https://example.com/{index}")))
            .to_vec(),
        "the same OSC 8 target opens from scrollback"
    );
}
