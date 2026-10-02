use super::*;
use gpui::EntityInputHandler;
use muxy_protocol::{KeyAction, TerminalInput};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
enum Sent {
    Raw(Vec<u8>),
    Terminal(TerminalInput),
}

fn pane(window: &mut Window, cx: &mut Context<TerminalPane>) -> TerminalPane {
    let mut pane = TerminalPane::new(
        Palette::new(true),
        muxy_app_core::settings::TerminalSettings::default(),
        cx,
    );
    prepare_mouse(&mut pane);
    pane.server_input = true;
    pane.focus.focus(window);
    pane
}

fn record(
    pane: &gpui::Entity<TerminalPane>,
    cx: &mut gpui::VisualTestContext,
) -> Rc<RefCell<Vec<Sent>>> {
    let sent = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        let output = sent.clone();
        cx.subscribe(pane, move |_, event, _| match event {
            PaneEvent::Input(_, bytes) => output.borrow_mut().push(Sent::Raw(bytes.clone())),
            PaneEvent::TerminalInput(_, input) => {
                output.borrow_mut().push(Sent::Terminal(input.clone()));
            }
            _ => {}
        })
        .detach();
    });
    sent
}

#[gpui::test]
fn server_keys_join_text_input_and_report_repeat_and_paired_release(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(pane);
    let sent = record(&pane, cx);
    cx.simulate_keystrokes("a");
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke: gpui::Keystroke {
            key: "a".into(),
            key_char: Some("a".into()),
            modifiers: gpui::Modifiers::default(),
        },
        is_held: true,
    });
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("a").unwrap(),
    });
    cx.run_until_parked();
    let events = sent.borrow().clone();
    assert_eq!(events.len(), 3, "{events:?}");
    let mut terminal = muxy_terminal::Terminal::new(Size { cols: 20, rows: 3 }, 1024).unwrap();
    terminal.feed(b"\x1b[>31u");
    for (event, (action, expected)) in events.iter().zip([
        (KeyAction::Press, "\x1b[97;;97u"),
        (KeyAction::Repeat, "\x1b[97;1:2;97u"),
        (KeyAction::Release, "\x1b[97;1:3u"),
    ]) {
        let Sent::Terminal(TerminalInput::Key(key)) = event else {
            panic!("{event:?}")
        };
        assert_eq!(key.action, action);
        assert_eq!(
            terminal.encode_key(key.clone()).unwrap(),
            expected.as_bytes()
        );
    }
    sent.borrow_mut().clear();
    cx.simulate_keystrokes("cmd-backspace cmd-left cmd-right cmd-k");
    for key in ["cmd-backspace", "cmd-left", "cmd-right", "cmd-k"] {
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
    }
    cx.run_until_parked();
    assert_eq!(
        sent.borrow().as_slice(),
        [
            Sent::Raw(vec![0x15]),
            Sent::Raw(vec![0x01]),
            Sent::Raw(vec![0x05]),
            Sent::Terminal(TerminalInput::ClearScreen),
        ]
    );
    sent.borrow_mut().clear();
    cx.simulate_keystrokes("up");
    pane.update(cx, |pane, cx| pane.focus_changed(false, cx));
    let events = sent.borrow();
    assert!(
        matches!(&events[0], Sent::Terminal(TerminalInput::Key(key)) if key.action == KeyAction::Press)
    );
    assert!(
        matches!(&events[1], Sent::Terminal(TerminalInput::Key(key)) if key.action == KeyAction::Release)
    );
    assert_eq!(events[2], Sent::Terminal(TerminalInput::Focus(false)));
}

#[gpui::test]
fn nonprinting_keys_infer_repeat_when_cocoa_loses_the_held_flag(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(pane);
    let sent = record(&pane, cx);
    let mut terminal = muxy_terminal::Terminal::new(Size { cols: 20, rows: 3 }, 0).unwrap();
    terminal.feed(b"\x1b[>11u");
    for (name, expected) in [
        ("up", ["\x1b[1;1:1A", "\x1b[1;1:2A", "\x1b[1;1:3A"]),
        ("backspace", ["\x1b[127u", "\x1b[127;1:2u", "\x1b[127;1:3u"]),
    ] {
        for _ in 0..2 {
            cx.simulate_event(gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse(name).unwrap(),
                is_held: false,
            });
        }
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(name).unwrap(),
        });
        cx.run_until_parked();
        let events = sent.borrow().clone();
        assert_eq!(events.len(), 3, "{events:?}");
        for ((event, action), expected) in events
            .iter()
            .zip([KeyAction::Press, KeyAction::Repeat, KeyAction::Release])
            .zip(expected)
        {
            let Sent::Terminal(TerminalInput::Key(key)) = event else {
                panic!("{event:?}")
            };
            assert_eq!(key.action, action);
            assert_eq!(
                terminal.encode_key(key.clone()).unwrap(),
                expected.as_bytes()
            );
        }
        sent.borrow_mut().clear();
        cx.simulate_keystrokes(name);
        let events = sent.borrow();
        assert!(
            matches!(&events[0], Sent::Terminal(TerminalInput::Key(key)) if key.action == KeyAction::Press)
        );
        drop(events);
        pane.update(cx, |pane, cx| pane.focus_changed(false, cx));
        sent.borrow_mut().clear();
    }
}

#[gpui::test]
fn composition_commits_once_without_a_fictitious_key_or_paste(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(pane);
    let sent = record(&pane, cx);
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke: gpui::Keystroke {
            key: "n".into(),
            key_char: Some("n".into()),
            modifiers: gpui::Modifiers::default(),
        },
        is_held: false,
    });
    cx.update(|window, cx| {
        pane.update(cx, |pane, cx| {
            pane.replace_and_mark_text_in_range(None, "に", None, window, cx);
            pane.replace_text_in_range(None, "日本", window, cx);
            pane.unmark_text(window, cx);
        });
    });
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("n").unwrap(),
    });
    cx.run_until_parked();
    assert_eq!(
        sent.borrow().as_slice(),
        [Sent::Raw("日本".as_bytes().to_vec())]
    );
    sent.borrow_mut().clear();
    pane.update(cx, |pane, cx| pane.set_state(PaneState::Disconnected, cx));
    cx.simulate_keystrokes("a up cmd-k");
    assert!(sent.borrow().is_empty());
}

#[gpui::test]
fn server_input_respects_overrides_app_actions_ime_and_unbinding(cx: &mut TestAppContext) {
    use muxy_app_core::settings::TerminalAction;
    let (pane, cx) = cx.add_window_view(pane);
    let sent = record(&pane, cx);
    pane.update(cx, |pane, _| pane.composition.text = "に".into());
    cx.simulate_keystrokes("cmd-backspace cmd-left cmd-right cmd-k");
    assert!(sent.borrow().is_empty());
    cx.update(|window, cx| {
        pane.update(cx, |pane, _| pane.composition.text.clear());
        cx.focus_handle().focus(window);
    });
    cx.simulate_keystrokes("cmd-backspace cmd-k");
    assert!(sent.borrow().is_empty());
    cx.update(|window, cx| {
        pane.update(cx, |pane, _| pane.focus.focus(window));
        cx.bind_keys([gpui::KeyBinding::new(
            "cmd-k",
            muxy_ui::text_input::Copy,
            Some("TerminalPane"),
        )]);
    });
    cx.simulate_keystrokes("cmd-k");
    assert!(sent.borrow().is_empty());
    pane.update(cx, |pane, _| {
        pane.terminal.keybindings.bindings.insert(
            "cmd-k".parse().unwrap(),
            TerminalAction::Text(b"custom".to_vec()),
        );
        pane.terminal
            .keybindings
            .bindings
            .insert("shift-enter".parse().unwrap(), TerminalAction::Unbind);
    });
    cx.simulate_keystrokes("cmd-k shift-enter");
    for key in ["cmd-k", "shift-enter"] {
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
    }
    cx.run_until_parked();
    let events = sent.borrow().clone();
    assert_eq!(events.len(), 3, "{events:?}");
    assert_eq!(events[0], Sent::Raw(b"custom".to_vec()));
    let mut terminal = muxy_terminal::Terminal::new(Size { cols: 20, rows: 3 }, 0).unwrap();
    terminal.feed(b"\x1b[>11u");
    for (event, expected) in events[1..]
        .iter()
        .zip([b"\x1b[13;2u".as_slice(), b"\x1b[13;2:3u"])
    {
        let Sent::Terminal(TerminalInput::Key(key)) = event else {
            panic!("{event:?}")
        };
        assert_eq!(terminal.encode_key(key.clone()).unwrap(), expected);
    }
    sent.borrow_mut().clear();
    pane.update(cx, |pane, _| {
        pane.terminal.keybindings.bindings.clear();
        pane.terminal.keybindings.clear_defaults = true;
    });
    cx.simulate_keystrokes("cmd-backspace cmd-left cmd-right");
    assert_eq!(sent.borrow().len(), 3);
    assert!(
        sent.borrow()
            .iter()
            .all(|event| matches!(event, Sent::Terminal(TerminalInput::Key(_))))
    );
}

#[gpui::test]
fn large_server_pastes_are_bounded_complete_and_preserve_text(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(pane);
    let sent = record(&pane, cx);
    let text = "a".repeat(muxy_protocol::MAX_INPUT - 1) + "界\r\nend";
    pane.update(cx, |pane, cx| pane.send_clipboard(text.as_bytes(), cx));
    let mut terminal = muxy_terminal::Terminal::new(Size { cols: 20, rows: 3 }, 0).unwrap();
    terminal.feed(b"\x1b[?2004h");
    let mut contents = Vec::new();
    assert_eq!(sent.borrow().len(), 2);
    for event in sent.borrow().iter() {
        let Sent::Terminal(TerminalInput::Paste(bytes)) = event else {
            panic!("{event:?}")
        };
        assert!(bytes.len() <= muxy_protocol::MAX_INPUT);
        assert!(std::str::from_utf8(bytes).is_ok());
        let encoded = terminal.encode_paste(bytes.clone()).unwrap();
        assert!(encoded.starts_with(b"\x1b[200~"));
        assert!(encoded.ends_with(b"\x1b[201~"));
        contents.extend_from_slice(&encoded[6..encoded.len() - 6]);
    }
    assert_eq!(contents, text.replace("\r\n", "\n").as_bytes());
}

#[gpui::test]
fn clipboard_paths_and_focus_do_not_use_stale_client_modes(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(pane);
    let sent = record(&pane, cx);
    for bracketed_paste in [false, true] {
        pane.update(cx, |pane, _| {
            pane.grid.as_mut().unwrap().modes.bracketed_paste = bracketed_paste;
        });
        cx.update(|_, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                "one\r\ntwo\x1b[201~".into(),
            ));
        });
        pane.update(cx, TerminalPane::paste_clipboard);
        pane.update(cx, |pane, cx| {
            pane.drop_paths(&[PathBuf::from("/tmp/a b")], cx);
        });
        pane.update(cx, |pane, cx| pane.focus_changed(true, cx));
        assert_eq!(
            sent.borrow().as_slice(),
            [
                Sent::Terminal(TerminalInput::Paste(b"one\r\ntwo\x1b[201~".to_vec())),
                Sent::Terminal(TerminalInput::Paste(b"'/tmp/a b'".to_vec())),
                Sent::Terminal(TerminalInput::Focus(true)),
            ]
        );
        sent.borrow_mut().clear();
    }
    cx.update(|_, cx| cx.write_to_clipboard(gpui::ClipboardItem::new_image(&gpui::Image::empty())));
    pane.update(cx, TerminalPane::paste_clipboard);
    assert_eq!(sent.borrow().as_slice(), [Sent::Raw(vec![0x16])]);
}
