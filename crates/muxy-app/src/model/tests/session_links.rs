use super::*;
use muxy_app_core::Direction;
use muxy_app_core::session_link::SessionLink;
use muxy_protocol::ServerIdentity;
use servers::work;

type Requests = std::sync::mpsc::Receiver<(u64, Work)>;

fn session(id: u64) -> SessionId {
    SessionId::new(id).expect("session")
}

fn link(id: u64) -> SessionLink {
    SessionLink {
        session: session(id),
        server: None,
        split: None,
    }
}

fn locate(
    model: &mut AppModel,
    server: ServerId,
    id: u64,
    project: Option<ProjectId>,
    cx: &mut Context<AppModel>,
) {
    let update = Update::SessionLocated {
        session: session(id),
        result: Ok(project.map(|project| SessionInfo {
            project,
            id: session(id),
            directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
        })),
    };
    model.receive((server, model.generation(server), update), cx);
}

fn located(requests: &Requests) -> Vec<SessionId> {
    work(requests)
        .into_iter()
        .filter_map(|work| match work {
            Work::LocateSession(session) => Some(session),
            _ => None,
        })
        .collect()
}

fn tab_count(model: &AppModel, project: ProjectId) -> usize {
    model
        .state
        .project(project)
        .map_or(0, |project| project.tabs.len())
}

fn shown(model: &AppModel, server: ServerId, id: u64) -> Option<(ProjectId, TabId, PaneId)> {
    model
        .session_pane(server, session(id))
        .map(|(project, tab, pane)| (project.id, tab, pane))
}

fn ready(
    state: AppState,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext, Requests) {
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    let _ = work(&requests);
    (view, cx, requests)
}

fn project_with_terminal(state: &mut AppState, id: u64) -> (ProjectId, PaneId) {
    let project = state
        .add_project(ServerId::local(), crate::test_support::project_directory())
        .expect("project");
    state.open_terminal_tab(project).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    state
        .set_pane_session(pane, Some(session(id)))
        .expect("session");
    (project, pane)
}

#[gpui::test]
fn a_link_jumps_to_the_tab_that_shows_its_terminal(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let (project, pane) = project_with_terminal(&mut state, 7);
    state.open_terminal_tab(project).expect("second tab");
    state.select_project(state.home().id).expect("home");
    let (view, cx, requests) = ready(state, cx);
    view.update(cx, |model, cx| {
        model.open_session_link(link(7), cx);
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(tab_count(model, project), 2);
        assert!(located(&requests).is_empty());
    });
}

#[gpui::test]
fn a_link_to_a_terminal_no_tab_shows_opens_it_in_its_project(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let (project, _) = project_with_terminal(&mut state, 7);
    let home = state.home().id;
    state.select_project(home).expect("home");
    let (view, cx, requests) = ready(state, cx);
    view.update(cx, |model, cx| {
        model.open_session_link(link(8), cx);
        assert_eq!(located(&requests), [session(8)]);
        assert!(shown(model, ServerId::local(), 8).is_none());
        assert_eq!(model.state.current_project().id, home);

        locate(model, ServerId::local(), 8, Some(project), cx);
        let (owner, _, pane) = shown(model, ServerId::local(), 8).expect("shown");
        assert_eq!(owner, project);
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(tab_count(model, project), 2);

        model.select_project(home, cx);
        model.open_session_link(link(8), cx);
        assert!(located(&requests).is_empty());
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(tab_count(model, project), 2);
    });
}

#[gpui::test]
fn a_split_link_opens_beside_the_selected_pane_of_the_terminals_project(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let (project, source) = project_with_terminal(&mut state, 7);
    let empty = state
        .add_project(ServerId::local(), crate::test_support::directory())
        .expect("project");
    let home = state.home().id;
    state.select_project(home).expect("home");
    let (view, cx, requests) = ready(state, cx);
    view.update(cx, |model, cx| {
        model.open_session_link(
            SessionLink {
                split: Some(Direction::Right),
                ..link(9)
            },
            cx,
        );
        assert_eq!(located(&requests), [session(9)]);
        locate(model, ServerId::local(), 9, Some(project), cx);
        let (_, tab, pane) = shown(model, ServerId::local(), 9).expect("shown");
        let (_, source_tab, _) = shown(model, ServerId::local(), 7).expect("source");
        assert_eq!(tab, source_tab);
        assert_eq!(tab_count(model, project), 1);
        assert_eq!(
            model
                .tab(tab)
                .and_then(|tab| tab.layout.neighbor(source, Direction::Right)),
            Some(pane)
        );
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.active_pane(), Some(pane));

        model.open_session_link(
            SessionLink {
                split: Some(Direction::Down),
                ..link(10)
            },
            cx,
        );
        locate(model, ServerId::local(), 10, Some(empty), cx);
        assert_eq!(
            shown(model, ServerId::local(), 10).map(|(owner, _, _)| owner),
            Some(empty)
        );
        assert_eq!(tab_count(model, empty), 1);
        assert_eq!(model.state.current_project().id, empty);
    });
}

#[gpui::test]
fn a_link_to_the_quick_terminal_shows_it_instead_of_opening_a_tab(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let quick = state.ensure_quick_terminal();
    state
        .set_pane_session(quick, Some(session(7)))
        .expect("session");
    let home = state.home().id;
    let (view, cx, requests) = ready(state, cx);
    view.update(cx, |model, cx| {
        let tabs = tab_count(model, home);
        model.open_session_link(link(7), cx);
        assert!(located(&requests).is_empty());
        assert_eq!(tab_count(model, home), tabs);
        assert_eq!(
            model.quick.panel.is_some(),
            model.supports(muxy_core::capability::Feature::QuickTerminal)
        );
    });
}

#[gpui::test]
fn a_link_that_starts_the_app_waits_for_the_server_and_restored_tabs(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.open_session_link(link(7), cx);
        assert!(located(&requests).is_empty());
        model.receive(local(1, Update::Connected(vec![])), cx);
        assert!(
            located(&requests).is_empty(),
            "the tabs are not restored yet"
        );
        acknowledge_catalog(model, cx);
        assert_eq!(located(&requests), [session(7)]);
    });
}

#[gpui::test]
fn links_to_unknown_servers_or_ended_terminals_change_nothing(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let (view, cx, requests) = ready(state, cx);
    view.update(cx, |model, cx| {
        let tabs = tab_count(model, home);
        model.open_session_link(
            SessionLink {
                server: Some(ServerIdentity::from_u128(99)),
                ..link(7)
            },
            cx,
        );
        assert!(model.error.is_some());
        assert!(located(&requests).is_empty());

        model.error = None;
        model.open_session_link(
            SessionLink {
                server: Some(ServerIdentity::from_u128(1)),
                ..link(7)
            },
            cx,
        );
        assert_eq!(located(&requests), [session(7)]);
        locate(model, ServerId::local(), 8, Some(home), cx);
        assert!(
            shown(model, ServerId::local(), 8).is_none(),
            "an answer for another link"
        );
        locate(model, ServerId::local(), 7, None, cx);
        assert!(model.error.is_some());
        assert!(shown(model, ServerId::local(), 7).is_none());
        assert_eq!(tab_count(model, home), tabs);
    });
}

#[cfg(not(windows))]
#[gpui::test]
fn a_link_with_a_server_opens_that_servers_terminal(cx: &mut TestAppContext) {
    let (remote, api) = (ServerId::new(), ProjectId::new());
    let (boot, requests, remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![servers::entry(remote, "devbox")],
    );
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        servers::connect_remote(model, remote, vec![], &[(api, "api")], cx);
        let _ = work(&requests);
        let _ = work(&remotes.borrow()[&remote]);

        model.open_session_link(
            SessionLink {
                server: model.state.server_identity(remote),
                ..link(7)
            },
            cx,
        );
        assert!(located(&requests).is_empty());
        assert_eq!(located(&remotes.borrow()[&remote]), [session(7)]);
        locate(model, remote, 7, Some(api), cx);
        assert_eq!(
            shown(model, remote, 7).map(|(owner, _, _)| owner),
            Some(api)
        );
        assert_eq!(model.state.current_project().id, api);
    });
}
