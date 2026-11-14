use super::*;
use muxy_protocol::ChannelId;

mod close_behavior;

fn terminal_state() -> (AppState, PaneId) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    state.prepare_creation(pane, std::path::Path::new("/tmp"));
    state
        .set_pane_session(pane, SessionId::new(71))
        .expect("session");
    (state, pane)
}

fn attach_pane(model: &mut AppModel, pane: PaneId, channel: u32, cx: &mut Context<AppModel>) {
    let mut attached = attachment_on(channel);
    attached.process = Some(muxy_protocol::ForegroundProcess {
        name: "sleep".into(),
        is_shell: false,
    });
    model.receive_attached(
        ServerId::local(),
        pane,
        model.state.pane_session(pane).expect("session").1,
        attached,
        false,
        cx,
    );
}

fn non_destructive(work: &Work) -> bool {
    !matches!(
        work,
        Work::Discard(..) | Work::CancelCreation(_) | Work::CheckClose { .. } | Work::EndAll(_)
    )
}

#[gpui::test]
fn late_attachment_replies_after_detach_never_discard_the_existing_session(
    cx: &mut TestAppContext,
) {
    for succeeds in [false, true] {
        let (state, pane) = terminal_state();
        let (boot, requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            let (_, session) = model.state.pane_session(pane).expect("session");
            model.pending.insert(pane, ServerId::local());
            requests.try_iter().for_each(drop);
            model.detach_terminal(pane, cx);
            if succeeds {
                model.receive_attached(ServerId::local(), pane, session, attachment(), false, cx);
            } else {
                model.receive_attach_failed(
                    ServerId::local(),
                    pane,
                    Some(session),
                    false,
                    &muxy_client::ClientError::Timeout,
                    cx,
                );
            }
            assert!(model.state.home().tabs.is_empty());
            assert!(model.state.pending_discards(ServerId::local()).is_empty());
            assert!(
                model
                    .state
                    .pending_cancellations(ServerId::local())
                    .is_empty()
            );
            let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
            assert!(work.iter().all(non_destructive));
            if succeeds {
                assert!(work.iter().any(|work| matches!(work, Work::Detach(_))));
            }
        });
    }
}
