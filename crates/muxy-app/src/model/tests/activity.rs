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
        let working_icon = cx.debug_bounds(provider);
        view.update(cx, |model, cx| {
            model.activity.snapshot.agents[0].state = AgentState::Idle;
            model.receive_event(title_update(session, "Tests passed"), cx);
            assert_eq!(model.state.home().tabs[0].title(None), "Tests passed");
            assert!(model.terminal(&pane).is_none());
        });
        cx.run_until_parked();
        let provider_bounds = cx.debug_bounds(provider).expect("provider");
        if layout == muxy_app_core::settings::AppLayout::TabFocused {
            assert_eq!(working_icon, Some(provider_bounds));
        } else {
            assert_eq!(spinner_bounds.size, provider_bounds.size);
            assert_eq!(spinner_bounds.center(), provider_bounds.center());
        }
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

#[gpui::test]
fn sidebar_status_keeps_provider_and_title_separate_at_every_scale(cx: &mut TestAppContext) {
    use muxy_app_core::settings::AppLayout;
    use muxy_protocol::{ProgressState, SessionProgress, TerminalProgress};
    for scale in [1.0, 1.5] {
        for (agent_state, unread, kind) in [
            (AgentState::Working, false, Some("progress")),
            (AgentState::Blocked, false, Some("blocked")),
            (AgentState::Idle, true, Some("completion")),
            (AgentState::Idle, false, None),
        ] {
            let mut state = AppState::bootstrap().expect("state");
            let home = state.home().id;
            let tab = state.open_terminal_tab(home).expect("tab");
            let pane = state.window().active_pane.expect("pane");
            let split = state.split_pane(pane, Direction::Right).expect("split");
            let session = SessionId::new(42).expect("session");
            let second = SessionId::new(43).expect("second session");
            state
                .set_pane_session(pane, Some(session))
                .expect("session");
            state
                .set_pane_session(split, Some(second))
                .expect("split session");
            state
                .set_tab_title(
                    tab,
                    Some("A long agent task title that must leave room for its status".into()),
                )
                .expect("title");
            state.open_terminal_tab(home).expect("other");
            let (boot, _requests) = stub_boot(state);
            let (view, window) = cx.add_window_view(|window, cx| {
                let mut model = AppModel::new(boot, window, cx);
                model.metrics = Metrics::new(scale);
                model.appearance.layout = AppLayout::TabFocused;
                model.appearance.sidebar_expanded = true;
                model.activity.snapshot.agents = vec![AgentActivity {
                    session,
                    project: home,
                    provider: AgentProvider::Codex,
                    state: agent_state,
                }];
                model.progress.insert(
                    session,
                    SessionProgress {
                        progress: Some(TerminalProgress {
                            state: ProgressState::Indeterminate,
                            percent: None,
                        }),
                        completed: 0,
                    },
                );
                if agent_state == AgentState::Blocked {
                    model.progress.insert(second, model.progress[&session]);
                }
                if unread {
                    model.activity.snapshot.events.push(ActivityEvent {
                        id: 1,
                        session,
                        project: home,
                        provider: AgentProvider::Codex,
                        kind: ActivityKind::Completed,
                        read: false,
                        timestamp: 0,
                    });
                }
                model
            });
            window.simulate_resize(size(px(1000.0), px(600.0)));
            window.run_until_parked();
            let provider = window
                .debug_bounds(format!("tab-provider-{tab}-Codex").leak())
                .expect("provider");
            let title = window
                .debug_bounds(format!("sidebar-tab-title-{tab}").leak())
                .expect("title");
            let accessory = window
                .debug_bounds(format!("sidebar-tab-accessory-{tab}").leak())
                .expect("accessory");
            let close = window
                .debug_bounds(format!("sidebar-close-{tab}").leak())
                .expect("close");
            assert_eq!(provider.size, size(px(14.0 * scale), px(14.0 * scale)));
            assert_eq!(accessory.size, size(px(20.0 * scale), px(20.0 * scale)));
            assert_eq!(close, accessory, "hover close uses the same slot");
            assert!(provider.right() <= title.left());
            assert!(title.right() <= accessory.left());
            for candidate in ["progress", "blocked", "completion"] {
                let bounds = window.debug_bounds(format!("tab-{candidate}-{tab}").leak());
                assert_eq!(
                    bounds.is_some(),
                    kind == Some(candidate),
                    "{agent_state:?}: {candidate}"
                );
                if let Some(bounds) = bounds {
                    assert_eq!(bounds.center(), accessory.center());
                }
            }
            view.read_with(window, |model, _| assert!(model.terminal(&pane).is_none()));
        }
    }
}

#[gpui::test]
fn pinned_sidebar_tabs_keep_provider_and_trailing_pin(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tab = state.open_terminal_tab(home).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    let session = SessionId::new(42).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    state.toggle_tab_pin(tab).expect("pin");
    let (boot, _requests) = stub_boot(state);
    let (_view, window) = cx.add_window_view(|window, cx| {
        let mut model = AppModel::new(boot, window, cx);
        model.appearance.layout = muxy_app_core::settings::AppLayout::TabFocused;
        model.appearance.sidebar_expanded = true;
        model.activity.snapshot.agents.push(AgentActivity {
            session,
            project: home,
            provider: AgentProvider::Codex,
            state: AgentState::Working,
        });
        model
    });
    window.run_until_parked();
    let provider = window
        .debug_bounds(format!("tab-provider-{tab}-Codex").leak())
        .expect("provider");
    let pin = window
        .debug_bounds(format!("sidebar-tab-pin-{tab}").leak())
        .expect("pin");
    let accessory = window
        .debug_bounds(format!("sidebar-tab-accessory-{tab}").leak())
        .expect("accessory");
    assert!(provider.right() < pin.left());
    assert_eq!(pin.center(), accessory.center());
    assert!(
        window
            .debug_bounds(format!("sidebar-close-{tab}").leak())
            .is_none()
    );
}

#[gpui::test]
fn project_indicators_include_terminal_progress_completion_and_detached_agents(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::settings::AppLayout;
    use muxy_protocol::{ProgressState, SessionProgress, TerminalProgress};
    for (layout, expanded) in [
        (AppLayout::ProjectFocused, false),
        (AppLayout::ProjectFocused, true),
        (AppLayout::TabFocused, false),
        (AppLayout::TabFocused, true),
    ] {
        for kind in ["progress", "completion", "blocked", "unread"] {
            let mut state = AppState::bootstrap().expect("state");
            let home = state.home().id;
            state.open_terminal_tab(home).expect("tab");
            let pane = state.window().active_pane.expect("pane");
            let session = SessionId::new(42).expect("session");
            state
                .set_pane_session(pane, Some(session))
                .expect("session");
            state.open_terminal_tab(home).expect("other");
            let (boot, _requests) = stub_boot(state);
            let (_view, window) = cx.add_window_view(|window, cx| {
                let mut model = AppModel::new(boot, window, cx);
                model.appearance.layout = layout;
                model.appearance.sidebar_expanded = layout == AppLayout::TabFocused || expanded;
                model.appearance.tab_focused_expanded.insert(home, expanded);
                match kind {
                    "progress" => {
                        model.progress.insert(
                            session,
                            SessionProgress {
                                progress: Some(TerminalProgress {
                                    state: ProgressState::Running,
                                    percent: Some(42),
                                }),
                                completed: 0,
                            },
                        );
                    }
                    "completion" => {
                        model.completions.insert(pane);
                    }
                    "blocked" => {
                        model.activity.snapshot.agents.push(AgentActivity {
                            session: SessionId::new(99).expect("detached"),
                            project: home,
                            provider: AgentProvider::Codex,
                            state: AgentState::Blocked,
                        });
                    }
                    "unread" => {
                        model.activity.snapshot.events.push(ActivityEvent {
                            id: 1,
                            session: SessionId::new(99).expect("detached"),
                            project: home,
                            provider: AgentProvider::Codex,
                            kind: ActivityKind::Completed,
                            read: false,
                            timestamp: 0,
                        });
                    }
                    _ => unreachable!(),
                }
                model
            });
            window.run_until_parked();
            let bounds = window.debug_bounds(format!("project-activity-{home}-{kind}").leak());
            let visible_tab_status = layout == AppLayout::TabFocused
                && expanded
                && matches!(kind, "progress" | "completion");
            assert_eq!(
                bounds.is_some(),
                !visible_tab_status,
                "{layout:?} expanded={expanded}, {kind}"
            );
            if let Some(bounds) = bounds {
                let row = if layout == AppLayout::TabFocused {
                    window
                        .debug_bounds(format!("tab-project-{home}").leak())
                        .expect("project")
                } else {
                    window.debug_bounds("project-row-0").expect("project")
                };
                assert!(row.contains(&bounds.center()), "badge stays on the project");
                if layout == AppLayout::TabFocused || expanded {
                    assert_eq!(row.center().y, bounds.center().y);
                }
            }
        }
    }
}

#[gpui::test]
fn project_rollups_follow_tab_groups_and_project_sidebar_width(cx: &mut TestAppContext) {
    use muxy_app_core::settings::AppLayout;
    let directory = tempfile::tempdir().expect("worktree");
    std::fs::write(directory.path().join(".git"), "gitdir: /tmp/unused").expect("marker");
    for (layout, expanded) in [
        (AppLayout::TabFocused, false),
        (AppLayout::TabFocused, true),
        (AppLayout::ProjectFocused, false),
        (AppLayout::ProjectFocused, true),
    ] {
        let mut state = AppState::bootstrap().expect("state");
        let home = state.home().id;
        let parent = state.add_project(std::env::temp_dir()).expect("parent");
        let child = state
            .add_project(directory.path().to_owned())
            .expect("child");
        state.select_project(home).expect("home");
        let mut stored = serde_json::to_value(state).expect("serialize");
        let child_record = stored["projects"]
            .as_array_mut()
            .expect("projects")
            .iter_mut()
            .find(|record| record["id"] == serde_json::to_value(child).expect("id"))
            .expect("child");
        child_record["parent_id"] = serde_json::to_value(parent).expect("parent id");
        child_record["kind"] =
            serde_json::to_value(muxy_app_core::ProjectKind::Worktree).expect("kind");
        let (boot, _requests) = stub_boot(serde_json::from_value(stored).expect("state"));
        let (_view, window) = cx.add_window_view(|window, cx| {
            let mut model = AppModel::new(boot, window, cx);
            model.appearance.layout = layout;
            model.appearance.sidebar_expanded = layout == AppLayout::TabFocused || expanded;
            model
                .appearance
                .tab_focused_expanded
                .insert(parent, expanded);
            model.activity.snapshot.agents.push(AgentActivity {
                session: SessionId::new(99).expect("detached"),
                project: child,
                provider: AgentProvider::Codex,
                state: AgentState::Blocked,
            });
            model
        });
        window.run_until_parked();
        let parent_status =
            window.debug_bounds(format!("project-activity-{parent}-blocked").leak());
        let child_status = window.debug_bounds(format!("project-activity-{child}-blocked").leak());
        assert_eq!(parent_status.is_some(), !expanded, "{layout:?}: parent");
        assert_eq!(
            child_status.is_some(),
            layout == AppLayout::ProjectFocused || expanded,
            "{layout:?}: child"
        );
    }
}
