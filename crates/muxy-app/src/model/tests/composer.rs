use super::*;

fn composer_text(view: &Entity<AppModel>, cx: &VisualTestContext) -> String {
    view.read_with(cx, |model, cx| {
        model
            .composer
            .view
            .as_ref()
            .expect("composer")
            .read(cx)
            .draft
            .text
            .clone()
    })
}

fn attached_composer(
    cx: &mut TestAppContext,
) -> (
    Entity<AppModel>,
    &mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(12).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (mut boot, requests) = stub_boot(state);
    boot.settings.composer.clear_after_sending = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    pane,
                    session,
                    attachment: attachment(),
                    created: false,
                },
            ),
            cx,
        );
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-i"
    } else {
        "ctrl-shift-i"
    });
    (view, cx, requests)
}

fn delivery(
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
) -> (Vec<u8>, async_channel::Sender<Result<(), String>>) {
    requests
        .try_iter()
        .find_map(|(_, work)| match work {
            Work::WriteInput {
                bytes, completion, ..
            } => Some((bytes, completion)),
            _ => None,
        })
        .expect("acknowledged terminal input")
}

#[gpui::test]
fn composer_delivery_preserves_edits_and_failures_then_clears_confirmed_draft(
    cx: &mut TestAppContext,
) {
    let (view, cx, requests) = attached_composer(cx);
    cx.simulate_input("first");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert_eq!(bytes, b"\x15first\r");
    cx.simulate_input(" edited");
    completion.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "first edited");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-shift-enter"
    } else {
        "ctrl-shift-enter"
    });
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert_eq!(bytes, b"\x15first edited");
    completion
        .try_send(Err("writer failed".into()))
        .expect("failure");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "first edited");
    view.read_with(cx, |model, cx| {
        assert!(
            model
                .composer
                .view
                .as_ref()
                .expect("composer")
                .read(cx)
                .error
                .as_ref()
                .expect("error")
                .contains("Draft preserved")
        );
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    delivery(&requests).1.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "");
}

#[gpui::test]
fn composer_broadcast_deduplicates_sessions_and_retains_partial_failure(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    let third = state.split_pane(second, Direction::Right).expect("split");
    for (pane, id) in [(first, 31), (second, 31), (third, 32)] {
        state
            .set_pane_session(pane, SessionId::new(id))
            .expect("session");
    }
    let (mut boot, requests) = stub_boot(state);
    boot.settings.composer.broadcast = true;
    boot.settings.composer.clear_after_sending = true;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        for (index, pane) in [first, second, third].into_iter().enumerate() {
            let mut attachment = attachment();
            attachment.channel =
                muxy_protocol::ChannelId(u32::try_from(index + 1).expect("channel"));
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane,
                        session: model.pane_session(pane).expect("session"),
                        attachment,
                        created: false,
                    },
                ),
                cx,
            );
        }
    });
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-i"
    } else {
        "ctrl-shift-i"
    });
    cx.simulate_input("broadcast");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    delivery(&requests).1.try_send(Ok(())).expect("first ack");
    cx.run_until_parked();
    delivery(&requests)
        .1
        .try_send(Err("second writer failed".into()))
        .expect("second failure");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "broadcast");
    view.read_with(cx, |model, cx| {
        let panel = model.composer.view.as_ref().expect("composer").read(cx);
        assert!(
            panel
                .error
                .as_ref()
                .expect("partial result")
                .starts_with("1/2 terminals sent.")
        );
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::WriteInput { .. }))
    );
}

#[gpui::test]
fn stored_clipboard_strategy_sends_an_image_path_without_a_clipboard_backend(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::composer::submission::ImageSubmissionStrategy;
    use muxy_core::capability::{Capability, Feature};

    let (view, cx, requests) = attached_composer(cx);
    let mut png = Vec::new();
    image::RgbaImage::new(1, 1)
        .write_to(&mut io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("png");
    view.update(cx, |model, cx| {
        model
            .capabilities
            .set(Feature::ClipboardImages, Capability::Unsupported);
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, cx| {
            composer.settings.image_strategy = ImageSubmissionStrategy::Clipboard;
            cx.emit(crate::views::composer::ComposerEvent::Image(png));
        });
    });
    cx.run_until_parked();
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-enter"
    });
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert!(!bytes.contains(&0x16));
    assert!(String::from_utf8_lossy(&bytes).contains(".png"));
    completion.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
}

#[gpui::test]
fn the_app_drawn_composer_menu_opens_at_its_button_and_hands_the_keyboard_back(
    cx: &mut TestAppContext,
) {
    use crate::views::overlays::Overlay;
    use gpui::Focusable;

    let (view, cx, _requests) = attached_composer(cx);
    let button = gpui::Bounds::new(gpui::point(px(120.0), px(300.0)), size(px(20.0), px(20.0)));
    let composer = view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, _| composer.sending = true);
        let choices = composer.read(cx).menu_items();
        model.open_composer_menu(&choices, button, cx);
        let Some(Overlay::Menu(menu)) = &model.overlay else {
            panic!("the composer menu is not open");
        };
        assert_eq!(menu.drops_from, Some(button));
        composer
    });

    cx.simulate_keystrokes("down down");
    cx.simulate_keystrokes("enter");
    view.update_in(cx, |model, window, cx| {
        assert!(model.overlay.is_none());
        assert!(!model.settings.composer.clear_after_sending);
        assert!(composer.focus_handle(cx).is_focused(window));
    });

    view.update(cx, |model, cx| {
        let choices = composer.read(cx).menu_items();
        model.open_composer_menu(&choices, button, cx);
    });
    cx.simulate_keystrokes("escape");
    view.update_in(cx, |model, window, cx| {
        assert!(model.overlay.is_none());
        assert!(composer.focus_handle(cx).is_focused(window));
    });
}

#[gpui::test]
fn a_floating_composer_closed_by_its_menu_row_leaves_the_keyboard_to_the_terminal(
    cx: &mut TestAppContext,
) {
    use crate::views::composer::menu::Command;

    let (view, cx, _requests) = attached_composer(cx);
    let button = gpui::Bounds::new(gpui::point(px(120.0), px(300.0)), size(px(20.0), px(20.0)));
    view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        composer.update(cx, |composer, cx| {
            composer.menu_action(Command::Presentation, cx);
        });
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        let choices = composer.read(cx).menu_items();
        model.open_composer_menu(&choices, button, cx);
        model.dismiss_overlay(cx);
        model.dismiss_composer(false, cx);
        assert!(model.composer.closing.is_some());
    });
    cx.run_until_parked();
    cx.executor()
        .advance_clock(crate::views::composer::TRANSITION * 2);
    cx.run_until_parked();
    view.update_in(cx, |model, window, cx| {
        let pane = model.active_pane().expect("pane");
        let terminal = model.terminal(&pane).expect("terminal");
        assert!(terminal.view.read(cx).focus.is_focused(window));
    });

    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-i"
    } else {
        "ctrl-shift-i"
    });
    view.update(cx, |model, cx| {
        let composer = model.composer.view.clone().expect("composer");
        let choices = composer.read(cx).menu_items();
        model.open_composer_menu(&choices, button, cx);
        model.close_composer(cx);
        assert!(model.overlay.is_none());
    });
}
