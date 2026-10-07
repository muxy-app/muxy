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
                    sandbox: None,
                    pane,
                    session,
                    attachment: attachment(),
                    created: false,
                },
            ),
            cx,
        );
    });
    cx.simulate_keystrokes("cmd-i");
    (view, cx, requests)
}

fn delivery(
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
) -> (
    Vec<u8>,
    async_channel::Sender<std::result::Result<(), String>>,
) {
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
    cx.simulate_keystrokes("cmd-enter");
    cx.run_until_parked();
    let (bytes, completion) = delivery(&requests);
    assert_eq!(bytes, b"\x15first\r");
    cx.simulate_input(" edited");
    completion.try_send(Ok(())).expect("ack");
    cx.run_until_parked();
    assert_eq!(composer_text(&view, cx), "first edited");
    cx.simulate_keystrokes("cmd-shift-enter");
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
    cx.simulate_keystrokes("cmd-enter");
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
                        sandbox: None,
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
    cx.simulate_keystrokes("cmd-i");
    cx.simulate_input("broadcast");
    cx.simulate_keystrokes("cmd-enter");
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
