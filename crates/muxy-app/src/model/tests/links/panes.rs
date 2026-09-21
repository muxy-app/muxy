use super::*;

fn split_state() -> (AppState, PaneId, PaneId) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.window().active_pane.expect("first");
    let second = state.split_pane(first, Direction::Right).expect("split");
    for (pane, session) in [(first, 71), (second, 72)] {
        state
            .set_pane_session(pane, SessionId::new(session))
            .expect("session");
    }
    (state, first, second)
}

fn open_pane_menu(view: &Entity<AppModel>, pane: PaneId, cx: &mut VisualTestContext) {
    cx.run_until_parked();
    let position = view.read_with(cx, |model, cx| {
        let pane = model.terminal(&pane).expect("terminal").view.read(cx);
        let (bounds, cell) = pane.geometry.expect("geometry");
        bounds.origin + point(cell.width, cell.height / 2.0)
    });
    cx.simulate_mouse_move(position, None, Modifiers::default());
    cx.simulate_mouse_down(position, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |model, _| model.active_pane()),
        Some(pane)
    );
}

fn choose(selector: &'static str, cx: &mut VisualTestContext) {
    let item = cx.debug_bounds(selector).expect("menu action");
    cx.simulate_click(item.center(), Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn terminal_menu_shortcuts_follow_bindings_and_fit_beside_labels(cx: &mut TestAppContext) {
    use crate::views::menu::Command;
    use muxy_app_core::settings::TerminalAction;

    let (state, first, _) = split_state();
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.keymap = boot
        .settings
        .keymap
        .with_binding(
            "split_right",
            Some("ctrl-alt-shift-f12".parse().expect("chord")),
        )
        .expect("split binding")
        .with_binding("detach_terminal", Some("cmd-alt-x".parse().expect("chord")))
        .expect("detach binding");
    boot.terminal
        .keybindings
        .bindings
        .insert("cmd-c".parse().expect("chord"), TerminalAction::Ignore);
    boot.terminal
        .keybindings
        .bindings
        .insert("alt-c".parse().expect("chord"), TerminalAction::Copy);
    boot.terminal
        .keybindings
        .bindings
        .insert("alt-a".parse().expect("chord"), TerminalAction::SelectAll);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    view.read_with(cx, |model, cx| {
        for (command, keys) in [
            (Command::TerminalCopy(first), "alt-c"),
            (Command::TerminalPaste(first), "cmd-v"),
            (Command::TerminalSelectAll(first), "alt-a"),
            (
                Command::SplitPane(first, Direction::Right),
                "ctrl-alt-shift-f12",
            ),
            (Command::SplitPane(first, Direction::Down), "cmd-shift-d"),
            (Command::ToggleZoomPane(first), "cmd-shift-enter"),
            (Command::ClosePane(first), "cmd-w"),
            (Command::DetachTerminal(first), "cmd-alt-x"),
        ] {
            assert_eq!(
                command.shortcut(model, cx),
                Some(Keystroke::parse(keys).expect("keys").to_string())
            );
        }
    });
    for scale in [1.0, 1.5] {
        view.update(cx, |model, cx| {
            model.metrics = Metrics::new(scale);
            model.terminal_menu(first, point(px(990.0), px(690.0)), cx);
        });
        cx.run_until_parked();
        let menu = cx.debug_bounds("context-menu").expect("menu");
        assert!(menu.right() <= px(1000.0));
        assert!(menu.bottom() <= px(700.0));
        for (label, shortcut) in [
            ("menu-label-Copy", "menu-shortcut-Copy"),
            ("menu-label-Select All", "menu-shortcut-Select All"),
            ("menu-label-Split Right", "menu-shortcut-Split Right"),
            (
                "menu-label-Detach Terminal",
                "menu-shortcut-Detach Terminal",
            ),
        ] {
            let label = cx.debug_bounds(label).expect("label");
            let shortcut = cx.debug_bounds(shortcut).expect("shortcut");
            assert!(label.right() < shortcut.left());
            assert!(shortcut.right() < menu.right());
        }
    }
}

#[gpui::test]
fn terminal_menu_omits_unassigned_and_overridden_shortcuts(cx: &mut TestAppContext) {
    use crate::views::menu::Command;
    use muxy_app_core::settings::TerminalAction;

    let (state, first, _) = split_state();
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.keymap = boot
        .settings
        .keymap
        .with_binding("split_down", Some("cmd-d".parse().expect("chord")))
        .expect("displaced default");
    boot.terminal
        .keybindings
        .bindings
        .insert("cmd-v".parse().expect("chord"), TerminalAction::Ignore);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    open_pane_menu(&view, first, cx);
    for (command, selector) in [
        (
            Command::SplitPane(first, Direction::Right),
            "menu-shortcut-Split Right",
        ),
        (
            Command::TerminalSelectAll(first),
            "menu-shortcut-Select All",
        ),
        (
            Command::DetachTerminal(first),
            "menu-shortcut-Detach Terminal",
        ),
    ] {
        assert!(view.read_with(cx, |model, cx| command.shortcut(model, cx).is_none()));
        assert!(cx.debug_bounds(selector).is_none());
    }
    view.update(cx, |model, cx| {
        assert_eq!(
            Command::TerminalPaste(first).shortcut(model, cx),
            Some(
                Keystroke::parse("cmd-shift-v")
                    .expect("default paste")
                    .to_string()
            )
        );
        model.settings.keymap = model
            .settings
            .keymap
            .with_binding(
                "toggle_composer",
                Some("cmd-shift-v".parse().expect("chord")),
            )
            .expect("composer binding");
        crate::views::workspace::bind_keys(&model.settings.keymap, cx);
        assert!(Command::TerminalPaste(first).shortcut(model, cx).is_none());
        model.settings.keymap = model
            .settings
            .keymap
            .with_binding("toggle_composer", None)
            .expect("reset composer binding");
        cx.clear_key_bindings();
        crate::views::workspace::bind_keys(&model.settings.keymap, cx);
        model
            .terminal(&first)
            .expect("terminal")
            .view
            .update(cx, |pane, _| {
                pane.terminal.keybindings.clear_defaults = true;
            });
        assert!(Command::TerminalPaste(first).shortcut(model, cx).is_none());
    });
}

#[gpui::test]
fn terminal_menu_splits_the_clicked_pane_and_maximizes_then_restores_it(cx: &mut TestAppContext) {
    let (state, first, second) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    for (index, (direction, selector)) in [
        (Direction::Right, "menu-label-Split Right"),
        (Direction::Down, "menu-label-Split Down"),
    ]
    .into_iter()
    .enumerate()
    {
        view.update(cx, |model, cx| model.focus_pane(second, cx));
        open_pane_menu(&view, first, cx);
        view.update(cx, |model, cx| model.focus_pane(second, cx));
        choose(selector, cx);
        view.read_with(cx, |model, _| {
            let new = model.active_pane().expect("new pane");
            assert_ne!(new, first);
            assert_ne!(new, second);
            assert_eq!(model.state.neighbor(first, direction), Some(new));
            assert_eq!(model.state.home().tabs[0].panes.len(), 3 + index);
        });
    }
    let layout = view.read_with(cx, |model, _| model.state.home().tabs[0].layout.clone());
    open_pane_menu(&view, first, cx);
    choose("menu-label-Maximize Pane", cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.visible_panes(), [first]);
        assert_eq!(model.state.home().tabs[0].zoomed, Some(first));
    });
    open_pane_menu(&view, first, cx);
    choose("menu-label-Restore Pane", cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.visible_panes(), layout.leaves());
        assert_eq!(model.state.home().tabs[0].layout, layout);
        assert_eq!(model.state.home().tabs[0].zoomed, None);
        assert_eq!(store::load(&model.path).expect("saved"), model.state);
    });
}

#[gpui::test]
fn terminal_menu_disables_maximize_and_close_for_a_single_pinned_pane(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let tab = state.open_terminal_tab(state.home().id).expect("tab");
    state.toggle_tab_pin(tab).expect("pin");
    let pane = state.window().active_pane.expect("pane");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    open_pane_menu(&view, pane, cx);
    for selector in ["menu-label-Maximize Pane", "menu-label-Close Pane"] {
        choose(selector, cx);
        view.read_with(cx, |model, _| {
            assert!(matches!(model.overlay, Some(Overlay::Menu(_))));
            assert_eq!(model.state.home().tabs[0].zoomed, None);
            assert_eq!(model.state.home().tabs[0].layout.leaves(), [pane]);
        });
    }
    choose("menu-label-Split Right", cx);
    assert_eq!(
        view.read_with(cx, |model, _| model.visible_panes().len()),
        2
    );
}

#[gpui::test]
fn terminal_menu_close_checks_only_the_clicked_pane_and_can_be_cancelled(cx: &mut TestAppContext) {
    let (state, first, second) = split_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        for (pane, channel) in [(first, 1), (second, 2)] {
            let mut attached = attachment();
            attached.channel = ChannelId(channel);
            model.receive_attached(
                pane,
                model.pane_session(pane).expect("session"),
                attached,
                false,
                cx,
            );
        }
    });
    open_pane_menu(&view, first, cx);
    requests.try_iter().for_each(drop);
    choose("menu-label-Close Pane", cx);
    let checks: Vec<_> = requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::CheckClose { session, .. } => Some(session),
            _ => None,
        })
        .collect();
    assert_eq!(checks, [SessionId::new(71).expect("session")]);
    view.update(cx, |model, cx| {
        let tab = model.active_tab().expect("tab");
        model.receive_close_checked(
            tab,
            checks[0],
            Ok(Some(muxy_protocol::ForegroundProcess {
                name: "vim".into(),
                is_shell: false,
            })),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs[0].layout.leaves(), [first, second]);
        assert!(model.state.pending_discards().is_empty());
    });
}
