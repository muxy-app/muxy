use super::*;
use muxy_app_core::project_layouts::{Config, Descriptor};
use muxy_protocol::ServerPath;

fn descriptor() -> Descriptor {
    Descriptor {
        name: "dev".into(),
        path: ServerPath(b".muxy/layouts/dev.yaml".to_vec()),
    }
}

fn open_picker(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
    project: ProjectId,
) -> u64 {
    requests.try_iter().for_each(drop);
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.open_layout_picker(project, window, cx);
        });
    });
    let request = requests
        .try_iter()
        .find_map(|(_, work)| match work {
            Work::ProjectLayouts {
                project: target,
                request,
            } if target == project => Some(request),
            _ => None,
        })
        .expect("layout listing request");
    view.update(cx, |model, cx| {
        model.receive_project_layouts(project, request, Ok(vec![descriptor()]), cx);
    });
    request
}

fn choose(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    project: ProjectId,
    request: u64,
    config: &str,
) {
    view.update(cx, |model, cx| {
        model.choose_project_layout("0", cx);
        model.receive_project_layout(project, request, &descriptor(), Config::parse(config), cx);
    });
    cx.run_until_parked();
}

#[gpui::test]
fn project_layouts_require_confirmation_and_preserve_other_projects(cx: &mut TestAppContext) {
    let (mut state, first, second, first_pane, _) = projects::two_projects();
    let old = state.project(first).expect("project").tabs[0].id;
    state.toggle_tab_pin(old).expect("pin");
    let session = SessionId::new(81).expect("session");
    state
        .set_pane_session(first_pane, Some(session))
        .expect("session");
    state.prepare_creation(first_pane, &std::env::temp_dir());
    let other_tabs = state.project(second).expect("other").tabs.clone();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    let request = open_picker(&view, cx, &requests, first);
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.project(first).expect("project").tabs[0].id, old);
    });
    choose(
        &view,
        cx,
        first,
        request,
        "panes: [{tab: nvim}, {tab: 'npm test'}]",
    );
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.project(first).expect("project").tabs[0].id, old);
    });
    let request = open_picker(&view, cx, &requests, first);
    choose(
        &view,
        cx,
        first,
        request,
        "panes: [{tab: nvim}, {tab: 'npm test'}]",
    );
    requests.try_iter().for_each(drop);
    cx.simulate_prompt_answer("Apply Layout");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, first);
        assert_eq!(model.state.project(second).expect("other").tabs, other_tabs);
        let tab = &model.state.project(first).expect("project").tabs[0];
        assert_ne!(tab.id, old);
        assert_eq!(tab.panes.len(), 2);
        assert_eq!(store::load(&model.path).expect("saved"), model.state);
        assert!(
            model
                .state
                .pending_discards(ServerId::local())
                .contains(&session)
        );
        assert!(
            model
                .state
                .pending_cancellations(ServerId::local())
                .is_empty()
        );
    });
    let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
    assert!(
        work.iter()
            .any(|work| matches!(work, Work::Discard(id, _) if *id == session))
    );
    assert_eq!(
        work.iter()
            .filter(|work| matches!(work, Work::Attach { project, .. } if *project == first))
            .count(),
        2
    );
}

#[gpui::test]
fn project_layouts_reject_bad_files_stale_replies_and_failed_saves(cx: &mut TestAppContext) {
    let (state, first, _, _, _) = projects::two_projects();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    let original = view.read_with(cx, |model, _| {
        model.state.project(first).expect("project").tabs.clone()
    });
    let previous_request = open_picker(&view, cx, &requests, first);
    view.update(cx, |model, cx| {
        model.choose_project_layout("0", cx);
        model.dismiss_overlay(cx);
    });
    let request = open_picker(&view, cx, &requests, first);
    view.update(cx, |model, cx| {
        model.receive_project_layout(
            first,
            previous_request,
            &descriptor(),
            Config::parse("tab: wrong"),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    choose(&view, cx, first, request, "panes: []");
    assert!(!cx.has_pending_prompt());
    view.read_with(cx, |model, _| {
        assert!(
            model
                .error
                .as_deref()
                .is_some_and(|error| error.contains("non-empty"))
        );
        assert_eq!(model.state.project(first).expect("project").tabs, original);
    });
    choose(&view, cx, first, request, "tab: nvim");
    assert!(cx.has_pending_prompt());
    let path = view.read_with(cx, |model, _| model.path.clone());
    view.update(cx, |model, _| model.path = std::env::temp_dir());
    requests.try_iter().for_each(drop);
    cx.simulate_prompt_answer("Apply Layout");
    cx.run_until_parked();
    view.update(cx, |model, _| {
        assert_eq!(model.state.project(first).expect("project").tabs, original);
        model.path = path;
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. } | Work::Discard(..)))
    );
}

#[gpui::test]
fn project_layout_commands_survive_restore_and_run_once_even_for_hidden_tabs(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let panes = state
        .apply_project_layout(
            state.home().id,
            &Config::parse("tabs: ['echo first', 'echo hidden']").expect("config"),
        )
        .expect("layout");
    let state =
        serde_json::from_str(&serde_json::to_string(&state).expect("encode")).expect("restore");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
        requests.try_iter().for_each(drop);
        model.receive_attach_failed(
            ServerId::local(),
            panes[1],
            None,
            false,
            &muxy_client::ClientError::Io(io::Error::other("temporary failure")),
            cx,
        );
        assert_eq!(model.state.startup_command(panes[1]), Some("echo hidden"));
        model.receive_attached(
            ServerId::local(),
            panes[1],
            SessionId::new(71).expect("session"),
            attachment(),
            true,
            cx,
        );
        assert!(model.state.startup_command(panes[1]).is_none());
        assert!(
            store::load(&model.path)
                .expect("saved")
                .startup_command(panes[1])
                .is_none()
        );
        model.receive_attached(
            ServerId::local(),
            panes[1],
            SessionId::new(71).expect("session"),
            attachment(),
            false,
            cx,
        );
    });
    let commands: Vec<_> = requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::Input(_, bytes) => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(commands, [b"echo hidden\r".to_vec()]);
}

#[gpui::test]
fn project_layout_replacement_rejects_panes_added_during_confirmation(cx: &mut TestAppContext) {
    let (state, project, _, first_pane, _) = projects::two_projects();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    let request = open_picker(&view, cx, &requests, project);
    choose(&view, cx, project, request, "tab: nvim");
    assert!(cx.has_pending_prompt());
    let original = view.update(cx, |model, _| {
        model
            .state
            .split_pane(first_pane, Direction::Right)
            .expect("split");
        model.state.project(project).expect("project").tabs.clone()
    });
    requests.try_iter().for_each(drop);
    cx.simulate_prompt_answer("Apply Layout");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.project(project).expect("project").tabs,
            original
        );
        assert!(
            model
                .error
                .as_deref()
                .is_some_and(|error| error.contains("changed"))
        );
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Discard(..)))
    );
}
