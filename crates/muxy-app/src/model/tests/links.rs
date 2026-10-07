use super::*;
use gpui::{MouseButton, point};
use muxy_protocol::{ChannelId, InputModes, MetadataEvent};

#[gpui::test]
fn terminal_menu_focuses_the_clicked_split_and_routes_clipboard_actions(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    for (id, session) in [(first, 71), (second, 72)] {
        state
            .set_pane_session(id, SessionId::new(session))
            .expect("session");
    }
    let (boot, requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        for (id, channel) in [(first, 1), (second, 2)] {
            let mut attachment = attachment();
            attachment.channel = ChannelId(channel);
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        sandbox: None,
                        pane: id,
                        session: model.pane_session(id).expect("session"),
                        attachment,
                        created: false,
                    },
                ),
                cx,
            );
        }
    });
    cx.run_until_parked();
    let terminal = view.read_with(cx, |model, _| {
        model.terminal(&first).expect("terminal").view.clone()
    });
    terminal.update(cx, |pane, cx| {
        pane.apply(
            &muxy_protocol::ScreenFrame {
                size: saved_screen().size,
                graphics: None,
                seq: 1,
                reset: true,
                rows: saved_screen().rows,
                cursor: saved_screen().cursor,
                modes: muxy_protocol::Modes::default(),
            },
            cx,
        );
    });
    cx.run_until_parked();
    let position = terminal.read_with(cx, |pane, _| {
        let (bounds, cell) = pane.geometry.expect("geometry");
        bounds.origin + point(cell.width, cell.height / 2.0)
    });
    cx.simulate_mouse_move(position, None, Modifiers::default());
    cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.active_pane(), Some(first));
        assert!(matches!(model.overlay, Some(Overlay::Menu(_))));
    });
    let all = cx.debug_bounds("menu-item-2").expect("Select All");
    cx.simulate_click(all.center(), Modifiers::default());
    cx.run_until_parked();
    assert!(terminal.read_with(cx, |pane, _| pane.selection.is_some()));
    assert!(view.read_with(cx, |model, cx| {
        model
            .terminal(&second)
            .expect("terminal")
            .view
            .read(cx)
            .selection
            .is_none()
    }));
    cx.simulate_keystrokes("cmd-c");
    assert!(cx.read(|cx| {
        cx.read_from_clipboard()
            .and_then(|item| item.text())
            .is_some_and(|text| text.contains("final marker"))
    }));
    requests.try_iter().for_each(drop);
    cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    let paste = cx.debug_bounds("menu-item-1").expect("Paste");
    cx.simulate_click(paste.center(), Modifiers::default());
    cx.run_until_parked();
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Input(ChannelId(1), _)))
    );
    verify_reporting_menu(&view, &terminal, position, cx);
}

fn verify_reporting_menu(
    view: &Entity<AppModel>,
    terminal: &Entity<TerminalPane>,
    position: gpui::Point<gpui::Pixels>,
    cx: &mut VisualTestContext,
) {
    terminal.update(cx, |pane, cx| {
        pane.metadata(
            MetadataEvent::InputModes(InputModes {
                mouse_tracking: true,
                ..InputModes::default()
            }),
            cx,
        );
    });
    cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| model.overlay.is_none()));
    let shift = Modifiers {
        shift: true,
        ..Modifiers::default()
    };
    cx.simulate_mouse_down(position, MouseButton::Right, shift);
    cx.simulate_mouse_up(position, MouseButton::Right, shift);
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| matches!(
        model.overlay,
        Some(Overlay::Menu(_))
    )));
}

#[gpui::test]
#[allow(clippy::float_cmp)]
fn prompt_shortcuts_and_command_output_menu_act_on_the_focused_terminal(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let terminal = view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.new_tab(cx);
        model
            .terminal(&model.active_pane().expect("pane"))
            .expect("terminal")
            .view
            .clone()
    });
    cx.run_until_parked();
    terminal.update(cx, |pane, cx| {
        let mut attached = attachment();
        let size = pane.viewport().unwrap_or(attached.grid.size);
        attached.grid.resize(size);
        attached.grid.history_fresh = true;
        attached.grid.rows = vec![vec![]; usize::from(size.rows)];
        let row = |index, text: &str| muxy_protocol::Row {
            index,
            runs: vec![muxy_protocol::Run {
                text: text.into(),
                width: u16::try_from(text.len()).expect("width"),
                style: muxy_protocol::Style::default(),
            }],
        };
        attached.grid.history = [
            row(0, "$ echo first"),
            row(1, "first"),
            row(2, "$ echo second"),
            row(3, "second"),
        ]
        .into();
        attached.grid.history_total = 4;
        attached.grid.prompts = [0, 2, 4].into();
        attached.grid.rows[0] = row(0, "$ ").runs;
        attached.grid.cursor = muxy_protocol::Cursor {
            shape: muxy_protocol::CursorShape::default(),
            row: 0,
            col: 2,
            visible: true,
        };
        pane.attach(ServerId::local(), attached, cx);
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-up");
    assert_eq!(terminal.read_with(cx, |pane, _| pane.scroll.offset), 2.0);
    cx.simulate_keystrokes("cmd-shift-up");
    assert_eq!(terminal.read_with(cx, |pane, _| pane.scroll.offset), 4.0);
    cx.simulate_keystrokes("cmd-shift-down");
    assert_eq!(terminal.read_with(cx, |pane, _| pane.scroll.offset), 2.0);
    cx.simulate_keystrokes("cmd-down");
    assert!(terminal.read_with(cx, |pane, _| pane.scroll.view.is_none()));
    cx.dispatch_action(crate::views::workspace::SelectCommandOutput);
    cx.simulate_keystrokes("cmd-c");
    assert_eq!(
        cx.read(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("second".into())
    );
    cx.simulate_keystrokes("cmd-up");
    cx.simulate_keystrokes("cmd-up");
    cx.run_until_parked();
    let position = terminal.read_with(cx, |pane, _| {
        let (bounds, cell) = pane.geometry.expect("geometry");
        bounds.origin + point(cell.width, cell.height * 1.5)
    });
    cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    let item = cx
        .debug_bounds("menu-item-3")
        .expect("Select Command Output");
    cx.simulate_click(item.center(), Modifiers::default());
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-c");
    assert_eq!(
        cx.read(|cx| cx.read_from_clipboard().and_then(|item| item.text())),
        Some("first".into())
    );
}
