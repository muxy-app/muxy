use super::*;
use muxy_app_core::activity::{ActivityIndicator, indicator};
use muxy_protocol::{
    ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity, AgentProvider, AgentState,
};

#[gpui::test]
fn hidden_agent_status_and_shared_reads_do_not_require_terminal_views(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tab = state.open_terminal_tab(home).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(42).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let other = state.open_terminal_tab(home).expect("other");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    cx.run_until_parked();
    let mut snapshot = ActivitySnapshot {
        revision: 1,
        agents: vec![AgentActivity {
            session,
            project: home,
            provider: AgentProvider::Codex,
            state: AgentState::Blocked,
        }],
        events: vec![ActivityEvent {
            id: 1,
            session,
            project: home,
            provider: AgentProvider::Codex,
            kind: ActivityKind::Attention,
            read: false,
            timestamp: 0,
        }],
    };
    for layout in [
        muxy_app_core::settings::AppLayout::ProjectFocused,
        muxy_app_core::settings::AppLayout::TabFocused,
    ] {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.appearance.layout = layout;
            model.select_tab(other, cx);
            assert!(model.terminal(&pane).is_none());
            model.receive_activity(Ok(snapshot.clone()), cx);
            assert_eq!(
                indicator(&model.activity.snapshot, |_| true),
                ActivityIndicator::Blocked
            );
            assert_eq!(
                model.unread_activity_count(),
                usize::from(!snapshot.events[0].read)
            );
        });
        cx.run_until_parked();
        snapshot.events[0].read = true;
        snapshot.revision += 1;
        view.update(cx, |model, cx| {
            model.receive_activity(Ok(snapshot.clone()), cx);
            assert_eq!(model.unread_activity_count(), 0);
            assert_eq!(
                indicator(&model.activity.snapshot, |_| true),
                ActivityIndicator::Blocked
            );
            model.navigate_activity(1, cx);
            assert_eq!(model.active_tab(), Some(tab));
            assert_eq!(model.active_pane(), Some(pane));
            model.disconnect(cx);
            assert!(model.activity.snapshot.agents.is_empty());
            assert_eq!(model.activity.snapshot.events.len(), 1);
        });
    }
}

#[gpui::test]
fn native_click_waits_for_connection_catalog_and_activity(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let target = state.open_terminal_tab(home).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(42).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let other = state.open_terminal_tab(home).expect("other");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Disconnected;
        model.navigate_activity(7, cx);
        assert_eq!(model.activity.navigation, Some(7));
        model.receive_connected(&[], cx);
        assert_eq!(model.activity.navigation, Some(7));
        model.receive_activity(
            Ok(ActivitySnapshot {
                events: vec![ActivityEvent {
                    id: 7,
                    session,
                    project: home,
                    provider: AgentProvider::Codex,
                    kind: ActivityKind::Attention,
                    read: false,
                    timestamp: 0,
                }],
                ..ActivitySnapshot::default()
            }),
            cx,
        );
        assert_eq!(model.active_tab(), Some(other));
        assert_eq!(model.activity.navigation, Some(7));
        model.catalog.pending = false;
        model.catalog.restore = None;
        model.resume_activity_navigation(cx);
        assert_eq!(model.activity.navigation, None);
        assert_eq!(model.active_tab(), Some(target));
        assert_eq!(model.active_pane(), Some(pane));
    });
}

fn title_update(session: SessionId, title: &str) -> ClientEvent {
    ClientEvent::SessionMetadata {
        session,
        metadata: muxy_protocol::SessionMetadata {
            title: title.into(),
            directory: muxy_protocol::ServerPath(b"/tmp/project".to_vec()),
            process: None,
        },
    }
}

#[gpui::test]
fn hidden_codex_title_and_finished_spinner_update_in_both_layouts(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tab = state.open_terminal_tab(home).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let session = SessionId::new(42).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let other = state.open_terminal_tab(home).expect("other");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    let spinner = format!("tab-progress-{tab}").leak();
    let provider = format!("tab-provider-{tab}-Codex").leak();
    for layout in [
        muxy_app_core::settings::AppLayout::ProjectFocused,
        muxy_app_core::settings::AppLayout::TabFocused,
    ] {
        view.update(cx, |model, cx| {
            model.appearance.layout = layout;
            model.appearance.sidebar_expanded = true;
            model.appearance.tab_focused_expanded.insert(home, true);
            model.select_tab(other, cx);
            assert!(model.terminal(&pane).is_none());
            model.activity.snapshot.agents = vec![AgentActivity {
                session,
                project: home,
                provider: AgentProvider::Codex,
                state: AgentState::Working,
            }];
            model.receive_event(title_update(session, "Fix tests"), cx);
            assert_eq!(model.state.home().tabs[0].title(None), "Fix tests");
            model.receive_progress(
                session,
                muxy_protocol::SessionProgress {
                    progress: Some(muxy_protocol::TerminalProgress {
                        state: muxy_protocol::ProgressState::Indeterminate,
                        percent: None,
                    }),
                    completed: 0,
                },
                cx,
            );
        });
        cx.run_until_parked();
        let spinner_bounds = cx.debug_bounds(spinner).expect("spinner");
        view.update(cx, |model, cx| {
            model.activity.snapshot.agents[0].state = AgentState::Idle;
            model.receive_event(title_update(session, "Tests passed"), cx);
            assert_eq!(model.state.home().tabs[0].title(None), "Tests passed");
            assert!(model.terminal(&pane).is_none());
        });
        cx.run_until_parked();
        let provider_bounds = cx.debug_bounds(provider).expect("provider");
        assert_eq!(spinner_bounds.size, provider_bounds.size);
        assert_eq!(spinner_bounds.center(), provider_bounds.center());
        view.update(cx, |model, cx| {
            for (process, directory, expected) in [
                (
                    Some(muxy_protocol::ForegroundProcess {
                        name: "codex".into(),
                        is_shell: false,
                    }),
                    "/tmp/project",
                    "codex",
                ),
                (
                    Some(muxy_protocol::ForegroundProcess {
                        name: "zsh".into(),
                        is_shell: true,
                    }),
                    "/tmp/other-project",
                    "other-project",
                ),
                (None, "/tmp/project", "project"),
            ] {
                model.receive_event(
                    ClientEvent::SessionMetadata {
                        session,
                        metadata: muxy_protocol::SessionMetadata {
                            title: String::new(),
                            directory: muxy_protocol::ServerPath(directory.as_bytes().to_vec()),
                            process,
                        },
                    },
                    cx,
                );
                assert_eq!(model.state.home().tabs[0].title(None), expected);
                assert!(model.terminal(&pane).is_none());
            }
        });
    }
}

#[gpui::test]
fn tab_provider_icon_follows_pane_focus_in_both_layouts(cx: &mut TestAppContext) {
    for layout in [
        muxy_app_core::settings::AppLayout::ProjectFocused,
        muxy_app_core::settings::AppLayout::TabFocused,
    ] {
        let mut state = AppState::bootstrap().expect("state");
        let home = state.home().id;
        let tab = state.open_terminal_tab(home).expect("tab");
        let codex = state.window().active_pane.expect("pane");
        let opencode = state.split_pane(codex, Direction::Right).expect("split");
        let shell = state.split_pane(opencode, Direction::Down).expect("split");
        let mut agents = Vec::new();
        for (pane, number, provider) in [
            (opencode, 43, AgentProvider::OpenCode),
            (codex, 42, AgentProvider::Codex),
        ] {
            let session = SessionId::new(number).expect("session");
            state
                .set_pane_session(pane, Some(session))
                .expect("session");
            agents.push(AgentActivity {
                session,
                project: home,
                provider,
                state: AgentState::Idle,
            });
        }
        state
            .set_pane_session(shell, SessionId::new(44))
            .expect("shell");
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| {
            let mut model = AppModel::new(boot, window, cx);
            model.appearance.layout = layout;
            model.appearance.sidebar_expanded = true;
            model.appearance.tab_focused_expanded.insert(home, true);
            model.activity.snapshot.agents = agents;
            model
        });
        cx.simulate_resize(size(px(1000.0), px(600.0)));
        cx.run_until_parked();
        let codex_icon: &'static str = format!("tab-provider-{tab}-Codex").leak();
        let opencode_icon: &'static str = format!("tab-provider-{tab}-OpenCode").leak();
        assert!(cx.debug_bounds(codex_icon).is_none());
        assert!(cx.debug_bounds(opencode_icon).is_none());
        for (pane, selector) in [(codex, codex_icon), (opencode, opencode_icon)] {
            view.update(cx, |model, cx| model.focus_pane(pane, cx));
            cx.run_until_parked();
            assert!(cx.debug_bounds(selector).is_some());
        }
    }
}
