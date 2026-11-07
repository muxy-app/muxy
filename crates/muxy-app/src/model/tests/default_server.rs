use super::servers::{connect_remote, entry, work};
use super::*;
use muxy_protocol::{ProjectSession, ProjectSessions, SessionStatus};

fn info(project: ProjectId, id: u64) -> SessionInfo {
    SessionInfo {
        project,
        id: SessionId::new(id).expect("session"),
        directory: muxy_protocol::ServerPath(b"/home/dev".to_vec()),
    }
}

fn asked_for_sessions(
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
    project: ProjectId,
) -> bool {
    work(requests)
        .iter()
        .any(|work| matches!(work, Work::ProjectSessions { project: asked } if *asked == project))
}

/// A session of `project` as Existing Terminals lists it, shown by a phone
/// when `shown`, and by this app when `attached`.
fn listed(project: ProjectId, id: u64, shown: bool, attached: bool) -> ProjectSession {
    ProjectSession {
        info: info(project, id),
        status: SessionStatus::Live,
        owner: shown.then(|| muxy_protocol::SessionClient {
            id: muxy_protocol::ClientId::new(),
            kind: muxy_protocol::ClientKind::Mobile,
        }),
        attached,
        name: None,
    }
}

fn listing(
    server: ServerId,
    project: ProjectId,
    sessions: Vec<ProjectSession>,
) -> (ServerId, u64, Update) {
    (
        server,
        1,
        Update::ProjectSessions {
            project,
            result: Ok(ProjectSessions {
                revision: 1,
                sessions,
                next: None,
            }),
        },
    )
}

fn sessions_in(model: &AppModel, project: ProjectId) -> Vec<SessionId> {
    model
        .state
        .project(project)
        .expect("project")
        .tabs
        .iter()
        .flat_map(|tab| &tab.panes)
        .filter_map(muxy_app_core::Pane::session)
        .collect()
}

/// The window, its view, what this computer's and the other servers' workers
/// were asked, and the remote Home.
type TwoServers<'a> = (
    Entity<AppModel>,
    &'a mut VisualTestContext,
    std::sync::mpsc::Receiver<(u64, Work)>,
    Remotes,
    ProjectId,
);

/// A window with this computer's server and `remote`'s, which has `api`.
fn two_servers(remote: ServerId, api: ProjectId, cx: &mut TestAppContext) -> TwoServers<'_> {
    let (boot, requests, remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![entry(remote, "devbox")],
    );
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let home = view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        connect_remote(model, remote, vec![], &[(api, "api")], cx)
    });
    (view, cx, requests, remotes, home)
}

#[gpui::test]
fn the_default_server_decides_what_the_sidebar_and_status_bar_show(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, remotes, home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        let local_home = model.state.home().id;
        assert_eq!(model.default_server(), ServerId::local());
        assert!(model.project_listed(model.state.home()));
        assert!(!model.project_listed(model.state.project(api).expect("api")));
        let _ = work(&remotes.borrow()[&remote]);

        model.set_default_server(remote, cx);
        assert_eq!(model.default_server(), remote);
        assert_eq!(model.status_server(), remote);
        assert_eq!(model.state.current_project().id, home);
        assert!(model.project_listed(model.state.project(api).expect("api")));
        assert!(!model.project_listed(model.state.home()));
        let asked = work(&remotes.borrow()[&remote]);
        for project in [home, api] {
            assert!(
                asked.iter().any(|work| matches!(
                    work,
                    Work::ProjectSessions { project: listed } if *listed == project
                )),
                "it looks for other clients' terminals in each of its projects"
            );
        }
        let saved = store::load(&model.path).expect("saved state");
        assert_eq!(saved.default_server(), remote);

        model.new_home_tab_at(remote, std::path::Path::new("/home/dev"), cx);
        assert_eq!(model.state.current_project().id, home);
        model.set_default_server(ServerId::local(), cx);
        assert_eq!(model.state.current_project().id, local_home);
    });
}

#[gpui::test]
fn the_status_bar_popover_switches_the_default_server(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, home) = two_servers(remote, api, cx);
    cx.run_until_parked();
    let item = cx
        .debug_bounds("project-connection-status")
        .expect("server item");
    cx.simulate_click(item.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("server-picker").is_some());
    let local = format!("server-choice-{}", ServerId::local());
    assert!(cx.debug_bounds(local.leak()).is_some());
    let choice = cx
        .debug_bounds(format!("server-choice-{remote}").leak())
        .expect("devbox");
    cx.simulate_click(choice.center(), Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.default_server(), remote);
        assert_eq!(model.state.current_project().id, home);
    });
}

#[gpui::test]
fn terminals_other_clients_show_open_here_as_background_tabs_once(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, requests, _remotes, home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        model.set_default_server(remote, cx);
        let active = model.state.window().active_pane;
        model.receive(
            listing(
                remote,
                api,
                vec![
                    listed(api, 4, true, false),
                    listed(api, 5, false, false),
                    listed(api, 6, true, true),
                    listed(home, 7, true, false),
                ],
            ),
            cx,
        );
        assert_eq!(
            sessions_in(model, api),
            [SessionId::new(4).expect("id")],
            "only another client's, never one nobody shows or this app's own"
        );
        assert_eq!(model.state.current_project().id, home, "focus stays");
        assert_eq!(model.state.window().active_pane, active);

        model.receive(listing(remote, api, vec![listed(api, 4, true, false)]), cx);
        assert_eq!(sessions_in(model, api).len(), 1, "no second tab");

        let local_home = model.state.home().id;
        let _ = work(&requests);
        model.receive(
            listing(
                ServerId::local(),
                local_home,
                vec![listed(local_home, 9, true, false)],
            ),
            cx,
        );
        assert!(
            model.state.home().tabs.is_empty(),
            "only the default server's terminals open"
        );
        assert!(
            !asked_for_sessions(&requests, local_home),
            "this computer's projects aren't listed for adoption"
        );
    });
}

#[gpui::test]
fn a_terminal_detached_here_is_not_reopened(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    let session = SessionId::new(4).expect("session");
    view.update(cx, |model, cx| {
        model.set_default_server(remote, cx);
        model.receive(listing(remote, api, vec![listed(api, 4, true, false)]), cx);
        let pane = model
            .state
            .session_panes(remote, session)
            .next()
            .expect("opened");
        model.detach_terminal(pane, cx);
        assert!(sessions_in(model, api).is_empty());
        assert!(model.state.dismissed_sessions(remote).contains(&session));
        assert!(
            store::load(&model.path)
                .expect("saved state")
                .dismissed_sessions(remote)
                .contains(&session),
            "it survives a restart"
        );

        model.receive(listing(remote, api, vec![listed(api, 4, true, false)]), cx);
        assert!(sessions_in(model, api).is_empty(), "not reopened");
    });
}

#[gpui::test]
fn new_home_tab_waits_for_a_default_servers_home_instead_of_switching_back(
    cx: &mut TestAppContext,
) {
    let remote = ServerId::new();
    let (boot, requests, remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![entry(remote, "devbox")],
    );
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        model.set_default_server(remote, cx);
        let _ = work(&requests);
    });
    cx.dispatch_action(crate::views::workspace::NewHomeTab);
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(model.default_server(), remote);
        assert!(model.state.home().tabs.is_empty(), "no local terminal");
        assert!(model.pending_remote.is_some_and(
            |pending| pending.server == remote && pending.intent == RemoteIntent::HomeTab
        ));
        let home = connect_remote(model, remote, vec![], &[], cx);
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(model.state.project(home).expect("home").tabs.len(), 1);
    });
    let _ = remotes;
}

#[gpui::test]
fn a_forgotten_default_server_gives_way_to_this_computer(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        model.set_default_server(remote, cx);
        model
            .forget_remote_servers(&[remote], cx)
            .expect("forgotten");
        assert_eq!(model.default_server(), ServerId::local());
        assert_eq!(model.state.current_project().id, model.state.home().id);
        assert!(model.project_listed(model.state.current_project()));
    });
}

#[gpui::test]
fn the_agents_panel_shows_only_the_default_servers_agents(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    let agent = |project, id| muxy_protocol::AgentActivity {
        session: SessionId::new(id).expect("session"),
        project,
        provider: muxy_protocol::AgentProvider::Claude,
        state: muxy_protocol::AgentState::Working,
    };
    view.update(cx, |model, cx| {
        let local_home = model.state.home().id;
        let tab = model.state.open_terminal_tab(local_home).expect("tab");
        let pane = model
            .state
            .home()
            .tabs
            .iter()
            .find(|candidate| candidate.id == tab)
            .expect("tab")
            .panes[0]
            .id;
        model
            .state
            .set_pane_session(pane, Some(SessionId::new(7).expect("session")))
            .expect("session");
        model.receive(listing(remote, api, vec![listed(api, 4, true, false)]), cx);
        model.set_default_server(remote, cx);
        model.receive(listing(remote, api, vec![listed(api, 4, true, false)]), cx);
        for (server, snapshot) in [
            (ServerId::local(), agent(local_home, 7)),
            (remote, agent(api, 4)),
        ] {
            model.receive(
                (
                    server,
                    1,
                    Update::Activity(Ok(muxy_protocol::ActivitySnapshot {
                        agents: vec![snapshot],
                        ..muxy_protocol::ActivitySnapshot::default()
                    })),
                ),
                cx,
            );
        }
        let servers = |model: &AppModel| {
            model
                .running_agents()
                .into_iter()
                .map(|agent| agent.server)
                .collect::<Vec<_>>()
        };
        assert_eq!(servers(model), [remote]);
        model.set_default_server(ServerId::local(), cx);
        assert_eq!(servers(model), [ServerId::local()]);
    });
}
