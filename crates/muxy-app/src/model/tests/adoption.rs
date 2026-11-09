use super::servers::{connect_remote, entry, page, work};
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
fn every_servers_projects_are_listed_and_only_this_computers_are_searched_for_other_clients(
    cx: &mut TestAppContext,
) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, requests, remotes, home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        let local_home = model.state.home().id;
        assert!(model.project_listed(model.state.home()));
        assert!(model.project_listed(model.state.project(api).expect("api")));
        assert!(model.project_listed(model.state.project(home).expect("home")));
        assert_eq!(model.status_server(), ServerId::local());

        model.select_project(api, cx);
        assert_eq!(model.status_server(), remote);
        assert_eq!(model.state.current_project().id, api);
        assert!(model.project_listed(model.state.home()), "nothing switches");
        model.receive(listing(remote, api, Vec::new()), cx);
        model.receive(listing(ServerId::local(), local_home, Vec::new()), cx);
        let _ = work(&requests);
        let _ = work(&remotes.borrow()[&remote]);

        for server in [ServerId::local(), remote] {
            let changed = Update::Event(ClientEvent::SessionsChanged { revision: 7 });
            model.receive((server, 1, changed), cx);
        }
        let asked: Vec<_> = work(&remotes.borrow()[&remote])
            .into_iter()
            .filter_map(|work| match work {
                Work::ProjectSessions { project } => Some(project),
                _ => None,
            })
            .collect();
        assert_eq!(
            asked,
            [api],
            "another computer lists only the project on screen, never each of its projects"
        );
        assert!(
            asked_for_sessions(&requests, local_home),
            "this computer's are listed for other clients' terminals"
        );
    });
}

#[gpui::test]
fn terminals_other_clients_show_here_open_as_background_tabs_once(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        let local_home = model.state.home().id;
        model.select_project(api, cx);
        let active = model.state.window().active_pane;
        model.receive(
            listing(
                ServerId::local(),
                local_home,
                vec![
                    listed(local_home, 4, true, false),
                    listed(local_home, 5, false, false),
                    listed(local_home, 6, true, true),
                ],
            ),
            cx,
        );
        assert_eq!(
            sessions_in(model, local_home),
            [SessionId::new(4).expect("id")],
            "only another client's, never one nobody shows or this app's own"
        );
        assert_eq!(model.state.current_project().id, api, "focus stays");
        assert_eq!(model.state.window().active_pane, active);

        model.receive(
            listing(
                ServerId::local(),
                local_home,
                vec![listed(local_home, 4, true, false)],
            ),
            cx,
        );
        assert_eq!(sessions_in(model, local_home).len(), 1, "no second tab");

        model.receive(listing(remote, api, vec![listed(api, 9, true, false)]), cx);
        assert!(
            sessions_in(model, api).is_empty(),
            "another computer's stay in Existing Terminals"
        );
    });
}

#[gpui::test]
fn a_terminal_detached_here_is_not_reopened(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    let session = SessionId::new(4).expect("session");
    view.update(cx, |model, cx| {
        let local_home = model.state.home().id;
        let shown = || {
            listing(
                ServerId::local(),
                local_home,
                vec![listed(local_home, 4, true, false)],
            )
        };
        model.receive(shown(), cx);
        let pane = model
            .state
            .session_panes(ServerId::local(), session)
            .next()
            .expect("opened");
        model.detach_terminal(pane, cx);
        assert!(sessions_in(model, local_home).is_empty());
        assert!(
            model
                .state
                .dismissed_sessions(ServerId::local())
                .contains(&session)
        );
        assert!(
            store::load(&model.path)
                .expect("saved state")
                .dismissed_sessions(ServerId::local())
                .contains(&session),
            "it survives a restart"
        );

        model.receive(shown(), cx);
        assert!(sessions_in(model, local_home).is_empty(), "not reopened");
    });
}

#[gpui::test]
fn opening_something_on_a_collapsed_server_expands_it_and_the_choice_is_kept(
    cx: &mut TestAppContext,
) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, home) = two_servers(remote, api, cx);
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar-server-local").is_some());
    assert!(cx.debug_bounds("add-project").is_some());
    let heading = format!("sidebar-server-{remote}");
    assert!(cx.debug_bounds(heading.clone().leak()).is_some());
    assert!(
        cx.debug_bounds(format!("add-project-{remote}").leak())
            .is_none(),
        "another computer starts collapsed"
    );
    view.read_with(cx, |model, _| {
        assert!(model.server_expanded(ServerId::local()));
        assert!(!model.server_expanded(remote));
        assert!(
            !model
                .sidebar_projects()
                .iter()
                .any(|project| project.id == api)
        );
    });

    let row = cx.debug_bounds(heading.clone().leak()).expect("heading");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(format!("add-project-{remote}").leak())
            .is_some()
    );
    view.update(cx, |model, cx| {
        assert!(
            model
                .sidebar_projects()
                .iter()
                .any(|project| project.id == api)
        );
        let saved =
            muxy_app_core::settings::Appearance::load(&model.path.with_file_name("settings.toml"))
                .expect("settings");
        assert!(saved.server_expanded(remote), "kept after a restart");

        model.toggle_server_group(remote, cx);
        model.toggle_server_group(ServerId::local(), cx);
        assert!(!model.server_expanded(ServerId::local()));
        assert!(model.sidebar_projects().is_empty());
        model.select_project(api, cx);
        assert!(model.server_expanded(remote), "opening it expands it");
        assert!(!model.server_expanded(ServerId::local()));
        assert_eq!(model.navigation_projects(), [home, api]);
    });
}

#[gpui::test]
fn a_tab_or_project_opened_on_a_collapsed_server_expands_it(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        assert!(!model.server_expanded(remote));
        model.new_tab_in(api, cx);
        assert_eq!(model.state.current_project().id, api);
        assert!(
            model.server_expanded(remote),
            "New Tab in Project expands it"
        );

        model.toggle_server_group(remote, cx);
        let added = model.add_project_on(remote, "/home/dev/web".into(), true, cx);
        assert!(added.is_some());
        assert!(model.server_expanded(remote), "Add Project expands it");
    });
}

#[gpui::test]
fn hiding_other_servers_leaves_only_this_computer_in_the_sidebar(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        model.toggle_server_group(remote, cx);
        assert!(
            model
                .sidebar_projects()
                .iter()
                .any(|project| project.id == api)
        );
        model.change_preference(crate::model::preferences::Change::SidebarServers(false), cx);
        assert!(!model.appearance.sidebar_remote_servers);
        assert_eq!(model.sidebar_servers(), [ServerId::local()]);
        assert!(
            model
                .sidebar_projects()
                .iter()
                .all(|project| project.server_id.is_local())
        );
        assert!(model.server_expanded(remote), "its choice is kept");
        assert!(model.project_listed(model.state.project(api).expect("api")));
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("sidebar-server-local").is_some());
    view.read_with(cx, |model, _| {
        let shown: Vec<_> = model
            .sidebar_groups()
            .iter()
            .map(|group| group.server)
            .collect();
        assert_eq!(shown, [ServerId::local()]);
    });
}

#[gpui::test]
fn a_disconnected_servers_empty_terminal_offers_connect(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, remotes, _home) = two_servers(remote, api, cx);
    let pane = view.update(cx, |model, cx| {
        model.new_tab_in(api, cx);
        model.disconnect_remote_servers(&[remote], cx);
        model.active_pane().expect("pane")
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(format!("pane-offline-{pane}").leak())
            .is_some()
    );
    let _ = work(&remotes.borrow()[&remote]);
    let connect = cx
        .debug_bounds(format!("pane-connect-{pane}").leak())
        .expect("Connect");
    cx.simulate_click(connect.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        work(&remotes.borrow()[&remote])
            .iter()
            .any(|work| matches!(work, Work::Connect))
    );
    assert_eq!(
        view.read_with(cx, |model, _| model.status_of(remote)),
        ServerStatus::Connecting
    );
}

#[gpui::test]
fn each_server_counts_its_own_projects_without_tabs(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = true;
        model.set_layout(muxy_app_core::settings::AppLayout::TabFocused, cx);
        model.toggle_server_group(remote, cx);
        let counts: Vec<_> = model
            .tab_sidebar_groups()
            .iter()
            .map(|(group, count)| (group.server, *count))
            .collect();
        assert_eq!(counts, [(ServerId::local(), 1), (remote, 2)]);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("tab-sidebar-projects-without-tabs")
            .is_some()
    );
    assert!(
        cx.debug_bounds(format!("tab-sidebar-projects-without-tabs-{remote}").leak())
            .is_some()
    );
}

#[gpui::test]
fn the_agents_panel_shows_every_servers_agents(cx: &mut TestAppContext) {
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
        for (project, id) in [(local_home, 7), (api, 4)] {
            let tab = model.state.open_terminal_tab(project).expect("tab");
            let pane = model
                .state
                .project(project)
                .expect("project")
                .tabs
                .iter()
                .find(|candidate| candidate.id == tab)
                .expect("tab")
                .panes[0]
                .id;
            model
                .state
                .set_pane_session(pane, Some(SessionId::new(id).expect("session")))
                .expect("session");
        }
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
        let mut servers: Vec<_> = model
            .running_agents()
            .into_iter()
            .map(|agent| agent.server)
            .collect();
        servers.sort();
        let mut expected = vec![ServerId::local(), remote];
        expected.sort();
        assert_eq!(servers, expected);
    });
}

/// Reconnects `server` to whichever server answers with `catalog`.
fn reconnect(
    model: &mut AppModel,
    server: ServerId,
    catalog: muxy_protocol::CatalogPage,
    cx: &mut Context<AppModel>,
) {
    let generation = model.generation(server);
    let offline = Update::Event(ClientEvent::Disconnected);
    model.receive((server, generation, offline), cx);
    model.connect_server(server, cx);
    let generation = model.generation(server);
    model.receive((server, generation, Update::Connected(vec![])), cx);
    model.receive((server, generation, Update::Catalog(Ok(catalog))), cx);
}

#[gpui::test]
fn dismissals_are_forgotten_only_once_the_known_server_answers(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, home) = two_servers(remote, api, cx);
    let session = SessionId::new(4).expect("session");
    view.update(cx, |model, cx| {
        model.state.dismiss_sessions(remote, [session]);
        reconnect(model, remote, page(9, ProjectId::new(), &[]), cx);
        assert!(
            model.state.dismissed_sessions(remote).contains(&session),
            "another server's list changes nothing"
        );
        reconnect(model, remote, page(2, home, &[(api, "api")]), cx);
        assert!(
            model.state.dismissed_sessions(remote).is_empty(),
            "the known server no longer runs it"
        );
    });
}

#[gpui::test]
fn new_home_tab_opens_on_this_computer_from_another_computers_project(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| model.select_project(api, cx));
    cx.dispatch_action(crate::views::workspace::NewHomeTab);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, model.state.home().id);
        assert_eq!(model.state.home().tabs.len(), 1);
        assert!(model.pending_remote.is_none());
    });
}

#[gpui::test]
fn new_tab_opens_in_the_current_project_on_any_server(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (view, cx, _requests, _remotes, _home) = two_servers(remote, api, cx);
    view.update(cx, |model, cx| {
        model.select_project(api, cx);
        model.new_tab(cx);
        assert_eq!(model.state.current_project().id, api);
        assert_eq!(model.state.project(api).expect("api").tabs.len(), 1);
        assert!(model.state.home().tabs.is_empty(), "no local terminal");
    });
}
