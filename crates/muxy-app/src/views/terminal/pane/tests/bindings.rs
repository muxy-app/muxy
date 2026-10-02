use super::*;
use muxy_app_core::settings::TerminalAction;
use std::cell::RefCell;
use std::rc::Rc;

#[gpui::test]
fn macos_editing_and_clear_defaults_use_distinct_input_and_control_paths(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(|window, cx| {
        let mut pane = TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        );
        prepare_mouse(&mut pane);
        pane.focus.focus(window);
        pane
    });
    let input = Rc::new(RefCell::new(Vec::new()));
    let clears = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        let input = input.clone();
        let clears = clears.clone();
        cx.subscribe(&pane, move |_, event, _| match event {
            PaneEvent::Input(_, bytes) => input.borrow_mut().extend_from_slice(bytes),
            PaneEvent::ClearScreen(channel) => clears.borrow_mut().push(*channel),
            _ => {}
        })
        .detach();
    });
    cx.simulate_keystrokes("cmd-backspace cmd-left cmd-right cmd-k");
    assert_eq!(&*input.borrow(), b"\x15\x01\x05");
    assert_eq!(clears.borrow().len(), 1);
    input.borrow_mut().clear();
    clears.borrow_mut().clear();
    pane.update(cx, |pane, _| {
        pane.terminal.keybindings.bindings.insert(
            "cmd-k".parse().unwrap(),
            TerminalAction::Text(b"custom".to_vec()),
        );
        pane.terminal
            .keybindings
            .bindings
            .insert("cmd-backspace".parse().unwrap(), TerminalAction::Ignore);
    });
    cx.simulate_keystrokes("cmd-k cmd-backspace");
    assert_eq!(&*input.borrow(), b"custom");
    assert!(clears.borrow().is_empty());
    input.borrow_mut().clear();
    pane.update(cx, |pane, _| {
        pane.terminal.keybindings.bindings.clear();
        pane.terminal.keybindings.clear_defaults = true;
    });
    cx.simulate_keystrokes("cmd-k cmd-backspace cmd-left cmd-right");
    assert!(input.borrow().is_empty());
    assert!(clears.borrow().is_empty());
    pane.update(cx, |pane, cx| {
        pane.terminal.keybindings.clear_defaults = false;
        pane.set_state(PaneState::Disconnected, cx);
    });
    cx.simulate_keystrokes("cmd-k cmd-backspace");
    assert!(input.borrow().is_empty());
    assert!(clears.borrow().is_empty());
}

#[gpui::test]
fn terminal_defaults_select_scroll_and_reset_font_without_sending_input(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(|window, cx| {
        let mut pane = TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        );
        prepare_mouse(&mut pane);
        pane.focus.focus(window);
        pane
    });
    cx.run_until_parked();
    pane.update(cx, |pane, _| {
        let grid = pane.grid.as_mut().unwrap();
        grid.history = (0..200).map(|index| row(index, "history")).collect();
        grid.history_total = 200;
        grid.history_fresh = true;
    });
    let input = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        let input = input.clone();
        cx.subscribe(&pane, move |_, event, _| {
            if let PaneEvent::Input(_, bytes) = event {
                input.borrow_mut().extend_from_slice(bytes);
            }
        })
        .detach();
    });
    cx.simulate_keystrokes("cmd-a");
    assert!(pane.read_with(cx, |pane, _| pane.selection.is_some()));
    cx.simulate_keystrokes("cmd-pageup");
    assert!(pane.read_with(cx, |pane, _| pane.scroll.offset > 0.0));
    cx.simulate_keystrokes("cmd-pagedown");
    assert!(pane.read_with(cx, |pane, _| pane.scroll.view.is_none()));
    cx.simulate_keystrokes("cmd-home");
    assert!(pane.read_with(cx, |pane, _| pane.scroll.offset > 0.0));
    cx.simulate_keystrokes("cmd-end");
    assert!(pane.read_with(cx, |pane, _| pane.scroll.view.is_none()));
    pane.update(cx, |pane, _| pane.terminal.zoom(5.0));
    cx.simulate_keystrokes("cmd-0");
    assert!(
        pane.read_with(cx, |pane, _| pane.terminal.font_size.to_bits()
            == pane.configured_font_size.to_bits())
    );
    assert!(input.borrow().is_empty());
}

#[gpui::test]
fn clear_frame_invalidates_frozen_history_selection_and_pending_pages(cx: &mut TestAppContext) {
    let pane = cx.new(|cx| {
        TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        )
    });
    pane.update(cx, |pane, cx| {
        prepare_mouse(pane);
        pane.grid.as_mut().unwrap().history_fresh = false;
        let request = pane
            .scroll
            .move_rows(1.0, pane.grid.as_ref().unwrap(), 3)
            .unwrap();
        pane.select_all(cx);
        assert!(pane.selection.is_some());
        assert!(pane.scroll.view.is_some());
        let frame = ScreenFrame {
            size: pane.grid.as_ref().unwrap().size,
            graphics: None,
            seq: 2,
            reset: true,
            rows: vec![row(0, "prompt")],
            cursor: pane.grid.as_ref().unwrap().cursor,
            modes: Modes::default(),
        };
        pane.apply(&frame, cx);
        assert!(pane.scroll.view.is_none());
        assert!(!pane.scroll.expects(request));
        assert!(pane.selection.is_none());
        assert!(pane.grid.as_ref().unwrap().history.is_empty());
    });
}

#[gpui::test]
fn terminal_defaults_respect_ime_focus_app_shortcuts_and_unbinding(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(|window, cx| {
        let mut pane = TerminalPane::new(
            Palette::new(true),
            muxy_app_core::settings::TerminalSettings::default(),
            cx,
        );
        prepare_mouse(&mut pane);
        pane.focus.focus(window);
        pane
    });
    let actions = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        let actions = actions.clone();
        cx.subscribe(&pane, move |_, event, _| {
            if matches!(event, PaneEvent::Input(..) | PaneEvent::ClearScreen(_)) {
                actions
                    .borrow_mut()
                    .push(matches!(event, PaneEvent::ClearScreen(_)));
            }
        })
        .detach();
    });
    pane.update(cx, |pane, _| pane.composition.text = "に".into());
    cx.simulate_keystrokes("cmd-backspace cmd-left cmd-right cmd-k");
    assert!(actions.borrow().is_empty());
    cx.update(|window, cx| {
        pane.update(cx, |pane, _| pane.composition.text.clear());
        let other = cx.focus_handle();
        other.focus(window);
    });
    cx.simulate_keystrokes("cmd-backspace cmd-left cmd-right cmd-k");
    assert!(actions.borrow().is_empty());
    cx.update(|window, cx| {
        pane.update(cx, |pane, _| pane.focus.focus(window));
        cx.bind_keys([gpui::KeyBinding::new(
            "cmd-k",
            muxy_ui::text_input::Copy,
            Some("TerminalPane"),
        )]);
    });
    cx.simulate_keystrokes("cmd-k");
    assert!(actions.borrow().is_empty());
    pane.update(cx, |pane, _| {
        pane.terminal
            .keybindings
            .bindings
            .insert("cmd-k".parse().unwrap(), TerminalAction::ClearScreen);
        pane.terminal
            .keybindings
            .bindings
            .insert("cmd-backspace".parse().unwrap(), TerminalAction::Unbind);
    });
    cx.simulate_keystrokes("cmd-k cmd-backspace");
    assert_eq!(actions.borrow().as_slice(), [true]);
}

#[gpui::test]
fn clear_frame_rejects_search_replies_from_before_the_clear(cx: &mut TestAppContext) {
    let (pane, cx) = cx.add_window_view(searchable_pane);
    cx.run_until_parked();
    pane.update(cx, |pane, cx| {
        let find = pane.find.as_mut().unwrap();
        find.results.query = "alpha".into();
        let request = find.results.restart().unwrap();
        pane.apply(
            &ScreenFrame {
                size: pane.grid.as_ref().unwrap().size,
                graphics: None,
                seq: 1,
                reset: true,
                rows: vec![row(0, "prompt")],
                cursor: pane.grid.as_ref().unwrap().cursor,
                modes: Modes::default(),
            },
            cx,
        );
        pane.receive_search(&request, Ok(search_page(200)), cx);
        assert!(pane.find.as_ref().unwrap().results.matches.is_empty());
    });
}
