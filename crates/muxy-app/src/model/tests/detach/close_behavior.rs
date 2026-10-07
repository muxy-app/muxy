use super::*;
use muxy_app_core::settings::CloseBehavior;

#[gpui::test]
fn configured_close_detaches_a_whole_split_tab_or_only_the_shortcut_pane(cx: &mut TestAppContext) {
    for whole_tab in [false, true] {
        let (mut state, first) = terminal_state();
        let second = state.split_pane(first, Direction::Right).expect("split");
        state.prepare_creation(second, std::path::Path::new("/tmp"));
        state
            .set_pane_session(second, SessionId::new(72))
            .expect("session");
        let (mut boot, requests) = stub_boot(state);
        boot.settings.window.close_behavior = CloseBehavior::Detach;
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            attach_pane(model, first, 1, cx);
            attach_pane(model, second, 2, cx);
            requests.try_iter().for_each(drop);
        });
        cx.run_until_parked();
        if whole_tab {
            view.update(cx, |model, cx| {
                model.close_tab(model.active_tab().expect("tab"), cx);
            });
        } else {
            cx.simulate_keystrokes("cmd-w");
        }
        view.read_with(cx, |model, _| {
            assert_eq!(model.state.home().tabs.is_empty(), whole_tab);
            if !whole_tab {
                assert_eq!(model.active_pane(), Some(first));
                assert_eq!(model.state.home().tabs[0].panes.len(), 1);
            }
            assert!(
                model
                    .state
                    .pending_cancellations(ServerId::local())
                    .is_empty()
            );
            assert!(model.state.pending_discards(ServerId::local()).is_empty());
            assert!(model.close_prompt.is_none());
            assert_eq!(store::load(&model.path).expect("layout"), model.state);
        });
        let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
        assert!(work.iter().all(non_destructive));
        assert!(
            work.iter()
                .any(|work| matches!(work, Work::Detach(ChannelId(2))))
        );
        assert_eq!(
            work.iter()
                .any(|work| matches!(work, Work::Detach(ChannelId(1)))),
            whole_tab
        );
    }
}

#[gpui::test]
fn configured_detach_during_creation_preserves_late_sessions_and_releases_ownership(
    cx: &mut TestAppContext,
) {
    for succeeds in [false, true] {
        let mut state = AppState::bootstrap().expect("state");
        state.open_terminal_tab(state.home().id).expect("tab");
        let pane = state.window().active_pane.expect("pane");
        state.prepare_creation(pane, std::path::Path::new("/tmp"));
        let (mut boot, requests) = stub_boot(state);
        boot.settings.window.close_behavior = CloseBehavior::Detach;
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            model.pending.insert(pane, ServerId::local());
            model.close_tab(model.active_tab().expect("tab"), cx);
            assert!(model.detached_pending.contains(&pane));
            requests.try_iter().for_each(drop);
            let session = SessionId::new(73).expect("session");
            if succeeds {
                model.receive_attached(ServerId::local(), pane, session, attachment(), true, cx);
            } else {
                model.receive_attach_failed(
                    ServerId::local(),
                    pane,
                    Some(session),
                    true,
                    &muxy_client::ClientError::Timeout,
                    cx,
                );
            }
            assert!(model.detached_pending.is_empty());
            assert!(model.state.home().tabs.is_empty());
            assert!(
                model
                    .state
                    .pending_cancellations(ServerId::local())
                    .is_empty()
            );
            assert!(model.state.pending_discards(ServerId::local()).is_empty());
            let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
            assert!(work.iter().all(non_destructive));
            assert!(
                work.iter().any(
                    |work| matches!(work, Work::References(references) if references.is_empty())
                )
            );
            assert_eq!(
                work.iter().any(|work| matches!(work, Work::Detach(_))),
                succeeds
            );
        });
    }
}
