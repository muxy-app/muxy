use super::*;
use muxy_app_core::settings::{AppLayout, ServerEntry};
use muxy_protocol::{ChannelId, ProjectDescriptor, ServerPath};

fn entry(id: ServerId, name: &str) -> ServerEntry {
    ServerEntry {
        id,
        name: name.into(),
        ssh: "dev@box".into(),
    }
}

fn descriptor(id: ProjectId, home: bool, name: &str) -> ProjectDescriptor {
    ProjectDescriptor {
        id,
        home,
        directory: ServerPath(format!("/home/dev/{name}").into_bytes()),
        name: name.into(),
        icon: None,
        logo: None,
        color: "#808080".into(),
        kind: None,
        parent_id: None,
    }
}

/// A catalog with a Home and `projects`, from a server with `identity`.
fn page(
    identity: u128,
    home: ProjectId,
    projects: &[(ProjectId, &str)],
) -> muxy_protocol::CatalogPage {
    muxy_protocol::CatalogPage {
        server: muxy_protocol::ServerIdentity::from_u128(identity),
        home,
        revision: 1,
        projects: std::iter::once(descriptor(home, true, "Home"))
            .chain(
                projects
                    .iter()
                    .map(|(id, name)| descriptor(*id, false, name)),
            )
            .collect(),
        next: None,
        legacy_home: None,
    }
}

/// Connects `server` as its worker would, with its catalog. Returns its Home.
fn connect_remote(
    model: &mut AppModel,
    server: ServerId,
    sessions: Vec<SessionInfo>,
    projects: &[(ProjectId, &str)],
    cx: &mut Context<AppModel>,
) -> ProjectId {
    let home = ProjectId::new();
    model.receive((server, 1, Update::Connected(sessions)), cx);
    model.receive(
        (server, 1, Update::Catalog(Ok(page(2, home, projects)))),
        cx,
    );
    assert_eq!(
        model.state.server_home(server).map(|home| home.id),
        Some(home)
    );
    home
}

fn work(requests: &std::sync::mpsc::Receiver<(u64, Work)>) -> Vec<Work> {
    requests.try_iter().map(|(_, work)| work).collect()
}

fn frame(text: &str) -> muxy_protocol::ScreenFrame {
    muxy_protocol::ScreenFrame {
        size: Size { cols: 80, rows: 24 },
        graphics: None,
        seq: 7,
        reset: false,
        rows: vec![muxy_protocol::Row {
            index: 0,
            runs: vec![muxy_protocol::Run {
                text: text.into(),
                width: u16::try_from(text.len()).expect("width"),
                style: muxy_protocol::Style::default(),
            }],
        }],
        cursor: muxy_protocol::Cursor {
            shape: muxy_protocol::CursorShape::default(),
            row: 0,
            col: 0,
            visible: true,
        },
        modes: muxy_protocol::Modes::default(),
    }
}

fn first_row(model: &AppModel, pane: PaneId, cx: &gpui::App) -> String {
    model.grids[&pane]
        .view
        .read(cx)
        .grid
        .as_ref()
        .expect("grid")
        .row_text(0)
}

/// An update from `server`'s first connection.
fn from(server: ServerId, update: Update) -> (ServerId, u64, Update) {
    (server, 1, update)
}

fn drawn(cx: &mut VisualTestContext, selector: String) -> bool {
    cx.debug_bounds(selector.leak()).is_some()
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn indeterminate() -> muxy_protocol::SessionProgress {
    muxy_protocol::SessionProgress {
        progress: Some(muxy_protocol::TerminalProgress {
            state: muxy_protocol::ProgressState::Indeterminate,
            percent: None,
        }),
        completed: 0,
    }
}

/// Shows a remote tab and the local Quick Terminal together, both attached
/// to session 1 on channel 1 of their own server.
fn attach_local_and_remote(
    model: &mut AppModel,
    remote: ServerId,
    session: SessionId,
    cx: &mut Context<AppModel>,
) -> (PaneId, PaneId) {
    model.receive(from(ServerId::local(), Update::Connected(vec![])), cx);
    acknowledge_catalog(model, cx);
    let home = connect_remote(model, remote, vec![], &[], cx);
    model.select_project(home, cx);
    model.new_tab(cx);
    let remote_pane = model.active_pane().expect("remote pane");
    let local_pane = model.state.ensure_quick_terminal();
    model.quick.visible = true;
    model.sync_visible(cx);
    for (server, pane) in [(ServerId::local(), local_pane), (remote, remote_pane)] {
        model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
        let attached = Update::Attached {
            pane,
            session,
            attachment: attachment(),
            created: true,
        };
        model.receive(from(server, attached), cx);
    }
    (local_pane, remote_pane)
}

#[gpui::test]
fn panes_with_the_same_session_and_channel_on_two_servers_stay_apart(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let state = AppState::bootstrap().expect("state");
    let (boot, local_work, remotes) = remote_boot(state, vec![entry(remote, "box")]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let session = SessionId::new(1).expect("session");
    let (local_pane, remote_pane) = view.update(cx, |model, cx| {
        attach_local_and_remote(model, remote, session, cx)
    });
    let remote_work = remotes.borrow_mut().remove(&remote).expect("remote worker");
    let (local, remote_sent) = (work(&local_work), work(&remote_work));
    let attached = |work: &[Work]| -> Vec<PaneId> {
        work.iter()
            .filter_map(|work| match work {
                Work::Attach { pane, .. } => Some(*pane),
                _ => None,
            })
            .collect()
    };
    assert_eq!(attached(&local), [local_pane]);
    assert_eq!(attached(&remote_sent), [remote_pane]);
    let lists = |work: &[Work], sessions: &[SessionId]| {
        work.iter()
            .any(|work| matches!(work, Work::References(listed) if listed == sessions))
    };
    assert!(lists(&local, &[session]) && lists(&remote_sent, &[session]));

    view.update(cx, |model, cx| {
        let before = first_row(model, local_pane, cx);
        let output = ClientEvent::Frame {
            channel: ChannelId(1),
            frame: frame("remote output"),
        };
        model.receive(from(remote, Update::Event(output)), cx);
        assert!(first_row(model, remote_pane, cx).starts_with("remote output"));
        assert_eq!(first_row(model, local_pane, cx), before);
        for pane in [local_pane, remote_pane] {
            model.grids[&pane].view.update(cx, |_, cx| {
                cx.emit(PaneEvent::Input(ChannelId(1), b"typed".to_vec()));
            });
        }
    });
    let (local, remote_sent) = (work(&local_work), work(&remote_work));
    let typed = |work: &[Work]| {
        work.iter()
            .filter(|work| matches!(work, Work::Input(ChannelId(1), bytes) if bytes == b"typed"))
            .count()
    };
    assert_eq!((typed(&local), typed(&remote_sent)), (1, 1));
    let acked = |work: &[Work]| work.iter().any(|work| matches!(work, Work::Ack(..)));
    assert!(acked(&remote_sent) && !acked(&local));

    view.update(cx, |model, cx| {
        let ended = ClientEvent::SessionEnded {
            session,
            reason: ExitReason::Ended,
        };
        model.receive(from(remote, Update::Event(ended)), cx);
        assert_eq!(model.state.pane_server(remote_pane), None);
        assert_eq!(model.pane_session(local_pane), Some(session));
    });
    let (local, remote_sent) = (work(&local_work), work(&remote_work));
    assert!(lists(&remote_sent, &[]));
    assert!(!local.iter().any(|work| matches!(work, Work::References(_))));
}

/// The local Home with a tab on session 42, a remote server `box` with a Home
/// and `api`, and the remote Home's tab on session 42 too.
struct Scene {
    /// Kept so this computer's stub worker stays open.
    _local_work: std::sync::mpsc::Receiver<(u64, Work)>,
    remotes: Remotes,
    remote: ServerId,
    local_home: ProjectId,
    local_tab: TabId,
    home: ProjectId,
    api: ProjectId,
    remote_tab: TabId,
    remote_pane: PaneId,
}

fn show_local_and_remote(
    cx: &mut TestAppContext,
) -> (Scene, Entity<AppModel>, &mut VisualTestContext) {
    let (remote, dormant) = (ServerId::new(), ServerId::new());
    let mut state = AppState::bootstrap().expect("state");
    let local_home = state.home().id;
    let local_tab = state.open_terminal_tab(local_home).expect("local tab");
    let session = SessionId::new(42).expect("session");
    let local_pane = state.home().tabs[0].panes[0].id;
    state
        .set_pane_session(local_pane, Some(session))
        .expect("session");
    let old = page(3, ProjectId::new(), &[(ProjectId::new(), "old")]);
    state.apply_catalog(dormant, &old).expect("dormant server");
    let (mut boot, local_work, remotes) = remote_boot(state, vec![entry(remote, "box")]);
    boot.settings.appearance.sidebar_expanded = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    let scene = view.update(cx, |model, cx| {
        let sessions = vec![SessionInfo {
            project: local_home,
            id: session,
            directory: ServerPath(b"/tmp".to_vec()),
        }];
        model.receive(from(ServerId::local(), Update::Connected(sessions)), cx);
        acknowledge_catalog(model, cx);
        let api = ProjectId::new();
        let home = connect_remote(model, remote, vec![], &[(api, "api")], cx);
        model.select_project(home, cx);
        model.new_tab(cx);
        let remote_pane = model.active_pane().expect("remote pane");
        let remote_tab = model.active_tab().expect("remote tab");
        let assigned = model.state.set_pane_session(remote_pane, Some(session));
        assigned.expect("session");
        Scene {
            _local_work: local_work,
            remotes,
            remote,
            local_home,
            local_tab,
            home,
            api,
            remote_tab,
            remote_pane,
        }
    });
    redraw(cx);
    (scene, view, cx)
}

#[gpui::test]
fn remote_projects_show_their_server_and_their_own_progress(cx: &mut TestAppContext) {
    let (scene, view, cx) = show_local_and_remote(cx);
    assert!(row_drawn(cx, 2));
    assert!(
        !row_drawn(cx, 3),
        "the dormant server's projects stay hidden"
    );
    for project in [scene.home, scene.api] {
        assert!(drawn(cx, format!("project-server-label-{project}")));
    }
    assert!(!drawn(
        cx,
        format!("project-server-label-{}", scene.local_home)
    ));

    view.update(cx, |model, cx| {
        model.appearance.layout = AppLayout::TabFocused;
        for project in [scene.local_home, scene.home] {
            model.appearance.tab_focused_expanded.insert(project, true);
        }
        model.select_project(scene.local_home, cx);
        let progress = ClientEvent::Progress {
            session: SessionId::new(42).expect("session"),
            progress: indeterminate(),
        };
        model.receive(from(scene.remote, Update::Event(progress)), cx);
    });
    redraw(cx);
    assert!(drawn(cx, format!("tab-progress-{}", scene.remote_tab)));
    assert!(!drawn(cx, format!("tab-progress-{}", scene.local_tab)));
}

#[gpui::test]
fn a_remote_going_offline_leaves_local_panes_live_and_reconnects_when_opened(
    cx: &mut TestAppContext,
) {
    let (scene, view, cx) = show_local_and_remote(cx);
    let remote_work = scene
        .remotes
        .borrow_mut()
        .remove(&scene.remote)
        .expect("remote worker");
    view.update(cx, |model, cx| {
        let offline = Update::Event(ClientEvent::Disconnected);
        model.receive(from(scene.remote, offline), cx);
        assert!(model.ready(ServerId::local()));
        assert_eq!(
            model.connection(scene.remote),
            ConnectionState::Disconnected
        );
        let remote_pane = model.grids[&scene.remote_pane].view.read(cx).state;
        assert_eq!(remote_pane, PaneState::Disconnected);
        assert_eq!(model.status(cx), PaneState::Disconnected);
        model.select_project(scene.local_home, cx);
        assert_eq!(model.status(cx), PaneState::Live);
        work(&remote_work);
        model.select_project(scene.home, cx);
        assert_eq!(model.connection(scene.remote), ConnectionState::Connecting);
    });
    assert!(
        remote_work
            .try_iter()
            .any(|(generation, work)| generation == 2 && matches!(work, Work::Connect))
    );
}

fn row_drawn(cx: &mut VisualTestContext, index: usize) -> bool {
    drawn(cx, format!("project-row-{index}"))
}

#[gpui::test]
fn a_remote_reached_over_ssh_lists_its_projects_and_runs_a_terminal(cx: &mut TestAppContext) {
    remote_terminal(cx).expect("remote terminal over ssh");
}

fn remote_terminal(cx: &mut TestAppContext) -> Result {
    let directory = tempfile::Builder::new()
        .prefix("muxy-remote-")
        .tempdir_in("/tmp")?;
    let socket = directory.path().join("remote.sock");
    serve_remote(&socket)?;
    let remote = ServerId::new();
    let (mut boot, _local_work, _) =
        remote_boot(AppState::bootstrap()?, vec![entry(remote, "box")]);
    let (updates, received) = async_channel::unbounded();
    let ssh = crate::boot::tests::fake_ssh(directory.path(), &socket)?;
    boot.workers = crate::boot::Workers::with(move |server, _| {
        crate::boot::worker(
            server,
            crate::boot::Target::Ssh(ssh.clone()),
            updates.clone(),
        )
    });
    boot.updates = received;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
    });
    wait(cx, &view, |model, _| {
        model.state.server_home(remote).is_some()
    })?;
    let pane = view.update(cx, |model, cx| {
        let home = model.state.server_home(remote).map(|home| home.id);
        model.select_project(home.expect("remote Home"), cx);
        model.new_tab(cx);
        let pane = model.active_pane().expect("pane");
        model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
        pane
    });
    wait(cx, &view, |model, cx| model.attachment(pane, cx).is_some())?;
    view.update(cx, |model, cx| {
        let (server, channel) = model.attachment(pane, cx).expect("attachment");
        assert_eq!(server, remote);
        model.send(
            remote,
            Work::Input(channel, b"echo remote-$((40 + 2))\r".to_vec()),
            cx,
        );
    });
    wait_text(cx, &view, "remote-42")?;
    assert_eq!(Client::connect(&socket)?.list_sessions()?.len(), 1);
    view.update(cx, |model, _| model.stop_workers());
    Ok(())
}

/// A server on `socket`, where the fake ssh's relay reaches it.
fn serve_remote(socket: &std::path::Path) -> io::Result<()> {
    let listener = std::os::unix::net::UnixListener::bind(socket)?;
    let (events, kept) = std::sync::mpsc::channel();
    let registry = std::sync::Arc::new(muxy_server::Registry::new(
        muxy_server::ServerSettings {
            default_shell: Some(PathBuf::from("/bin/sh")),
            ..muxy_server::ServerSettings::default()
        },
        events,
    ));
    thread::spawn(move || {
        let _kept = kept;
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                return;
            };
            let registry = std::sync::Arc::clone(&registry);
            thread::spawn(move || {
                let (_kept, events) = std::sync::mpsc::channel();
                let _ = muxy_server::connection::serve(Box::new(stream), registry, events);
            });
        }
    });
    Ok(())
}

#[gpui::test]
fn a_restarted_server_lists_its_existing_sessions_once(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let listings = |requests: &std::sync::mpsc::Receiver<(u64, Work)>| {
        work(requests)
            .iter()
            .filter(|work| matches!(work, Work::ProjectSessions { .. }))
            .count()
    };
    view.update(cx, |model, cx| {
        let home = model.state.home().id;
        let page = |revision| Update::ProjectSessions {
            project: home,
            result: Ok(muxy_protocol::ProjectSessions {
                revision,
                sessions: Vec::new(),
                next: None,
            }),
        };
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        let changed = ClientEvent::SessionsChanged { revision: 100 };
        model.receive(local(1, Update::Event(changed)), cx);
        model.receive(local(1, page(100)), cx);
        listings(&requests);
        model.disconnect(ServerId::local(), cx);
        model.connect(cx);
        model.receive(local(2, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        assert_eq!(listings(&requests), 1);
        // The restarted server counts its session list revisions from the start.
        model.receive(local(2, page(5)), cx);
        assert_eq!(listings(&requests), 0, "the listing must not repeat");
    });
}
