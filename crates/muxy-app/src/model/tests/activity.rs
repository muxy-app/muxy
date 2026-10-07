use super::*;
use muxy_protocol::{ActivityEvent, ActivityKind, ActivitySnapshot, AgentProvider};

#[gpui::test]
fn detached_sessions_do_not_claim_alerts_or_reopen_from_notification_clicks(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    state.open_terminal_tab(home).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    let session = SessionId::new(42).expect("session");
    let sibling = state.split_pane(pane, Direction::Right).expect("split");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.window_active = false;
        assert!(model.servers.local.activity.pane_sessions.is_empty());
        model.receive_attached(ServerId::local(), pane, session, attachment(), true, cx);
        model.receive_attached(ServerId::local(), sibling, session, attachment(), false, cx);
        model.servers.local.activity.loaded = true;
        let mut snapshot = ActivitySnapshot {
            events: [42, 99]
                .into_iter()
                .map(|id| ActivityEvent {
                    id,
                    session: SessionId::new(id).expect("session"),
                    project: home,
                    provider: AgentProvider::Codex,
                    kind: ActivityKind::Completed,
                    read: false,
                    timestamp: 0,
                })
                .collect(),
            ..ActivitySnapshot::default()
        };
        model.receive_activity(ServerId::local(), Ok(snapshot.clone()), cx);
        assert_eq!(
            model.servers.local.activity.pane_sessions,
            vec![session],
            "track ownership before delivering alerts"
        );
        let claims: Vec<_> = requests
            .try_iter()
            .filter_map(|(_, work)| match work {
                Work::ClaimActivity(ids) => Some(ids),
                _ => None,
            })
            .collect();
        assert_eq!(
            claims,
            vec![vec![42]],
            "only an open pane can claim an alert"
        );
        model.detach_terminal(pane, cx);
        snapshot.events[0].id = 100;
        model.receive_activity(ServerId::local(), Ok(snapshot.clone()), cx);
        assert!(
            requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::ClaimActivity(ids) if ids == vec![100])),
            "the remaining pane still owns the session's alerts"
        );
        model.detach_terminal(sibling, cx);
        assert!(model.state.home().tabs.is_empty());
        model.navigate_activity(ServerId::local(), 100, cx);
        assert!(
            model.state.home().tabs.is_empty(),
            "stale clicks cannot reopen detached panes"
        );
        snapshot.events[0].id = 101;
        model.receive_activity(ServerId::local(), Ok(snapshot), cx);
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::ClaimActivity(_)))
        );
    });
}
