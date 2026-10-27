use super::*;
use muxy_protocol::ErrorCode;

#[gpui::test]
fn logo_picker_uses_the_target_project_directory_and_cancel_keeps_its_logo(
    cx: &mut TestAppContext,
) {
    let (mut state, first, second, _, _) = two_projects();
    let logo = include_bytes!("../../../../muxy-protocol/tests/fixtures/project-logo.png");
    state
        .set_project_logo(first, Some(logo.as_slice().into()))
        .expect("logo");
    let projects = state.projects().to_vec();
    let directory = state
        .project(first)
        .expect("target project")
        .directory
        .clone();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(model.state.current_project().id, second);
        model.choose_project_logo_with(
            first,
            |start, _| {
                assert_eq!(start, Some(directory.as_path()));
                std::future::ready(Ok(None))
            },
            cx,
        );
        assert!(model.project_logo_task.is_some());
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.projects(), projects);
        assert_eq!(model.state.current_project().id, second);
        assert!(model.overlay.is_none());
        assert!(model.error.is_none());
    });
}

#[gpui::test]
fn rejected_project_intent_advances_fifo_but_storage_failure_preserves_it(cx: &mut TestAppContext) {
    let (state, _, _, _, _) = two_projects();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        let pending = model.state.project_intents(ServerId::local()).to_vec();
        let error = |code| {
            muxy_client::ClientError::Server(muxy_protocol::ErrorReply {
                code,
                message: "rejected".into(),
            })
        };
        model.receive_project_mutation(
            ServerId::local(),
            pending[0].operation,
            Err(error(ErrorCode::PersistenceFailed)),
            cx,
        );
        assert_eq!(model.state.project_intents(ServerId::local()), pending);
        requests.try_iter().for_each(drop);
        model.receive_project_mutation(
            ServerId::local(),
            pending[0].operation,
            Err(error(ErrorCode::BadPath)),
            cx,
        );
        assert_eq!(
            model.state.project_intents(ServerId::local()),
            &pending[1..]
        );
        assert_eq!(
            store::load(&model.path)
                .expect("saved state")
                .project_intents(ServerId::local()),
            &pending[1..]
        );
        assert!(
            requests.try_iter().any(
                |(_, work)| matches!(work, Work::MutateProject(intent) if intent == pending[1])
            )
        );
        assert!(model.error.is_some());
    });
}

#[gpui::test]
fn one_x_projects_join_the_running_app_and_keep_what_is_there(cx: &mut TestAppContext) {
    let legacy = tempfile::tempdir().expect("1.x folder");
    let first = tempfile::tempdir().expect("first folder");
    let second = tempfile::tempdir().expect("second folder");
    let ids = [
        "01000000-0000-0000-0000-000000000001",
        "01000000-0000-0000-0000-000000000002",
    ];
    std::fs::write(
        legacy.path().join("projects.json"),
        serde_json::json!([
            {"id":ids[0],"name":"First","path":first.path(),"sortOrder":0,"pullRequestPrompt":"From 1.x"},
            {"id":ids[1],"name":"Second","path":second.path(),"sortOrder":1}
        ])
        .to_string(),
    )
    .expect("projects");
    std::fs::write(
        legacy.path().join("project-groups.json"),
        serde_json::json!([{"id":"02000000-0000-0000-0000-000000000001","name":"Work","sortOrder":0,"projectIDs":ids,"type":"local"}])
            .to_string(),
    )
    .expect("workspaces");
    let mut state = AppState::bootstrap().expect("state");
    let hand_added = state
        .add_project(ServerId::local(), first.path().into())
        .expect("project");
    state
        .rename_project(hand_added, "Hand added")
        .expect("rename");
    state.open_terminal_tab(hand_added).expect("tab");
    let tabs = state.project(hand_added).expect("project").tabs.clone();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        requests.try_iter().for_each(drop);
        let import = crate::backup::projects_from(legacy.path()).expect("1.x projects");
        assert_eq!(model.import_legacy_projects(&import, cx), Ok((1, 1)));

        let second: ProjectId = ids[1].parse().expect("ID");
        let kept = model.state.project(hand_added).expect("hand-added project");
        assert_eq!((kept.name.as_str(), &kept.tabs), ("Hand added", &tabs));
        assert!(model.state.project(ids[0].parse().expect("ID")).is_none());
        assert_eq!(
            model.state.workspaces()[0].projects,
            [hand_added, second].into()
        );
        assert_eq!(
            model.settings.ai.project_pr_prompts[&hand_added.to_string()],
            "From 1.x"
        );
        assert!(
            store::load(&model.path)
                .expect("saved state")
                .project(second)
                .is_some()
        );
        assert!(requests.try_iter().any(|(_, work)| matches!(
            work,
            Work::MutateProject(intent)
                if matches!(&intent.mutation, muxy_protocol::ProjectMutation::Create(project) if project.id == second)
        )));
    });
}

pub(super) fn two_projects() -> (AppState, ProjectId, ProjectId, PaneId, PaneId) {
    let mut state = AppState::bootstrap().expect("state");
    let first = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("first project");
    state.open_terminal_tab(first).expect("first tab");
    let first_pane = state.current_project().tabs[0].panes[0].id;
    let second = state
        .add_project(ServerId::local(), std::env::current_dir().expect("cwd"))
        .expect("second project");
    state.open_terminal_tab(second).expect("second tab");
    let second_pane = state.current_project().tabs[0].panes[0].id;
    (state, first, second, first_pane, second_pane)
}

#[gpui::test]
fn switching_projects_detaches_and_reattaches_in_the_owning_directory(cx: &mut TestAppContext) {
    let (mut state, first, second, first_pane, second_pane) = two_projects();
    let first_session = SessionId::new(41).expect("session");
    let second_session = SessionId::new(42).expect("session");
    state
        .set_pane_session(first_pane, Some(first_session))
        .expect("session");
    state
        .set_pane_session(second_pane, Some(second_session))
        .expect("session");
    state.select_project(first).expect("select");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected([first_session, second_session].map(|id| SessionInfo { project: ProjectId::from_u128(1),  id, directory: muxy_protocol::ServerPath(b"/tmp".to_vec()) }).to_vec())), cx);
        acknowledge_catalog(model, cx);
        model.receive((ServerId::local(), 1, Update::Attached { pane: first_pane, session: first_session, attachment: attachment(), created: false }), cx);
        requests.try_iter().for_each(drop);
        model.select_project(second, cx);
        model.start_attach(second_pane, Size { cols: 80, rows: 24 }, cx);
        let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
        assert!(work.iter().any(|work| matches!(work, Work::Detach(_))));
        assert!(work.iter().any(|work| matches!(work, Work::Attach { pane, session: Some(session), directory, .. } if *pane == second_pane && *session == second_session && *directory == std::env::current_dir().expect("cwd"))));
        assert_eq!(model.grids.len(), 1);
        assert!(model.grids.contains_key(&second_pane));
        assert!(model.snapshots.contains_key(&first_pane));
        model.navigate(false, cx);
        assert_eq!(model.state.current_project().id, first);
        model.navigate(true, cx);
        assert_eq!(model.state.current_project().id, second);
        model.select_project(first, cx);
        model.start_attach(first_pane, Size { cols: 80, rows: 24 }, cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::Attach { pane, session: Some(session), .. } if pane == first_pane && session == first_session)));
        assert_eq!(model.state.projects().len(), 3);
    });
}

#[gpui::test]
fn hidden_restore_creates_in_each_project_and_removal_discards_late_creations(
    cx: &mut TestAppContext,
) {
    let (mut state, first, second, first_pane, second_pane) = two_projects();
    state.select_project(state.home().id).expect("home");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(Vec::new())), cx);
        acknowledge_catalog(model, cx);
        let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
        for (id, pane) in [(first, first_pane), (second, second_pane)] {
            let expected = &model.state.project(id).expect("project").directory;
            assert!(work.iter().any(|work| matches!(work, Work::Attach { pane: target, directory, .. } if *target == pane && directory == expected)));
        }
        model.remove_project_confirmed(first, cx);
        let session = SessionId::new(51).expect("session");
        model.receive((ServerId::local(), 1, Update::Attached { pane: first_pane, session, attachment: attachment(), created: true }), cx);
        assert!(model.state.project(first).is_none());
        assert_eq!(model.state.pending_discards(ServerId::local()), [session]);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session)));
        assert_eq!(model.state.project(second).expect("second").tabs.len(), 1);
        assert!(model.grids.is_empty());
    });
}

#[gpui::test]
fn tab_close_confirmation_keeps_its_target_after_project_switch(cx: &mut TestAppContext) {
    let (mut state, first, second, first_pane, _) = two_projects();
    let session = SessionId::new(61).expect("session");
    state
        .set_pane_session(first_pane, Some(session))
        .expect("session");
    state.select_project(first).expect("first");
    let tab = state.current_project().tabs[0].id;
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.close_tab(tab, cx);
        model.select_project(second, cx);
        model.receive_close_checked(tab, session, Ok(None), cx);
        assert!(model.state.project(first).expect("first").tabs.is_empty());
        assert_eq!(model.state.project(second).expect("second").tabs.len(), 1);
        assert_eq!(model.state.current_project().id, second);
        assert!(
            requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
        );
    });
}

#[gpui::test]
fn project_removal_requires_confirmation_and_persists_offline_cleanup(cx: &mut TestAppContext) {
    let (mut state, first, second, first_pane, _) = two_projects();
    let session = SessionId::new(71).expect("session");
    state
        .set_pane_session(first_pane, Some(session))
        .expect("session");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| model.confirm_remove_project(first, cx));
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| model.state.project(first).is_some()));
    view.update(cx, |model, cx| model.confirm_remove_project(first, cx));
    cx.run_until_parked();
    cx.simulate_prompt_answer("Remove");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.state.project(first).is_none());
        assert_eq!(model.state.current_project().id, second);
        assert_eq!(model.state.pending_discards(ServerId::local()), [session]);
        let saved = store::load(&model.path).expect("load");
        assert!(saved.project(first).is_none());
        assert_eq!(saved.pending_discards(ServerId::local()), [session]);
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Discard(_, _)))
    );
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
    });
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Discard(id, _) if id == session))
    );
}

#[gpui::test]
fn missing_folder_hides_its_terminal_until_refresh_finds_it_again(cx: &mut TestAppContext) {
    let directory = std::env::temp_dir().join(format!("muxy-project-status-{}", ProjectId::new()));
    std::fs::create_dir(&directory).expect("mkdir");
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), directory.clone())
        .expect("project");
    state.open_terminal_tab(project).expect("tab");
    let (boot, _) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    std::fs::remove_dir(&directory).expect("remove fixture");
    view.update(cx, |model, cx| {
        model.refresh_project_statuses(cx);
        assert!(model.active_pane().is_none());
        assert!(model.grids.is_empty());
        assert_eq!(model.state.current_project().tabs.len(), 1);
        model.new_tab(cx);
        assert_eq!(model.state.current_project().tabs.len(), 1);
    });
    std::fs::create_dir(&directory).expect("restore fixture");
    view.update(cx, |model, cx| {
        model.refresh_project_statuses(cx);
        assert!(model.active_pane().is_some());
        assert_eq!(model.grids.len(), 1);
    });
    std::fs::remove_dir(&directory).expect("cleanup");
}

#[gpui::test]
fn end_all_includes_hidden_projects_and_keeps_empty_project_records(cx: &mut TestAppContext) {
    let (mut state, first, second, first_pane, second_pane) = two_projects();
    let first_session = SessionId::new(81).expect("session");
    let second_session = SessionId::new(82).expect("session");
    state
        .set_pane_session(first_pane, Some(first_session))
        .expect("session");
    state
        .set_pane_session(second_pane, Some(second_session))
        .expect("session");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.end_all_and_quit(cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::EndAll(sessions) if sessions == [first_session, second_session])));
        model.finish_end_all(Ok(()), cx);
        assert!(model.state.project(first).expect("first").tabs.is_empty());
        assert!(model.state.project(second).expect("second").tabs.is_empty());
    });
}

#[gpui::test]
fn removal_save_failure_keeps_projects_and_never_discards(cx: &mut TestAppContext) {
    let (mut state, first, _, first_pane, _) = two_projects();
    state
        .set_pane_session(first_pane, Some(SessionId::new(91).expect("session")))
        .expect("session");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        let temporary = PathBuf::from(format!("{}.tmp", model.path.display()));
        std::fs::create_dir(&temporary).expect("block save");
        let previous = model.state.clone();
        model.remove_project_confirmed(first, cx);
        assert_eq!(model.state, previous);
        assert!(model.error.is_some());
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Discard(_, _)))
        );
        std::fs::remove_dir(temporary).expect("cleanup");
    });
}

#[gpui::test]
fn command_o_opens_the_picker_and_existing_paths_select_the_project(cx: &mut TestAppContext) {
    let root = std::env::temp_dir().join(format!("muxy-open-project-{}", ProjectId::new()));
    std::fs::create_dir_all(root.join("Alpha")).expect("mkdir");
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), root.join("Alpha"))
        .expect("project");
    state.select_project(state.home().id).expect("home");
    let (mut boot, requests) = stub_boot(state);
    boot.settings.projects.search_root = Some(root.clone());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-o"
    } else {
        "ctrl-shift-o"
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| matches!(
        model.overlay,
        Some(Overlay::Projects(_))
    )));
    cx.simulate_input(&root.join("Alpha").to_string_lossy());
    cx.executor().advance_clock(Duration::from_millis(125));
    cx.run_until_parked();
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-shift-enter"
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.state.projects().len(), 2);
        assert!(model.overlay.is_none());
    });
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[gpui::test]
fn folders_from_the_command_line_select_their_project_or_add_it(cx: &mut TestAppContext) {
    let root = std::env::temp_dir().join(format!("muxy-cli-folder-{}", ProjectId::new()));
    std::fs::create_dir_all(root.join("Alpha")).expect("mkdir");
    std::fs::create_dir_all(root.join("Beta Gamma")).expect("mkdir");
    std::fs::write(root.join("notes.txt"), "").expect("file");
    let mut state = AppState::bootstrap().expect("state");
    let alpha = state
        .add_project(ServerId::local(), root.join("Alpha"))
        .expect("project");
    state.select_project(state.home().id).expect("home");
    let (boot, _requests) = stub_boot(state);
    let (folders, opened) = async_channel::unbounded();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |_, cx| AppModel::open_folders_from(opened, cx));
    let url = |name: &str| format!("file://{}/{name}/", root.display()).replace(' ', "%20");
    folders.try_send(vec![url("Alpha")]).expect("send");
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(model.state.current_project().id, model.state.home().id);
        acknowledge_catalog(model, cx);
        assert_eq!(model.state.current_project().id, alpha);
        assert_eq!(model.state.projects().len(), 2);
    });
    folders
        .try_send(vec![
            format!("file://{}/notes.txt", root.display()),
            format!("file://{}/Missing/", root.display()),
            "https://example.com/".into(),
            url("Beta Gamma"),
        ])
        .expect("send");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.current_project().directory,
            root.join("Beta Gamma")
        );
        assert_eq!(model.state.projects().len(), 3);
        assert!(model.error.is_none());
    });
    std::os::unix::fs::symlink(root.join("Alpha"), root.join("Alias")).expect("symlink");
    for name in ["Alias", "Beta Gamma"] {
        folders.try_send(vec![url(name)]).expect("send");
        cx.run_until_parked();
    }
    folders.try_send(vec![url("Alias")]).expect("send");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, alpha);
        assert_eq!(model.state.projects().len(), 3);
    });
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// On another computer, as macOS file systems refuse such names here.
#[gpui::test]
fn a_project_whose_folder_name_is_not_text_opens_like_any_other(cx: &mut TestAppContext) {
    use std::os::unix::ffi::OsStringExt;
    let odd = PathBuf::from(std::ffi::OsString::from_vec(b"/srv/caf\xe9 \xff".to_vec()));
    let mut state = AppState::bootstrap().expect("state");
    let server = ServerId::new();
    state
        .apply_catalog(server, &servers::page(2, ProjectId::new(), &[]))
        .expect("remote catalog");
    let project = state.add_project(server, odd.clone()).expect("project");
    let (boot, _requests, _remotes) = remote_boot(state, vec![servers::entry(server, "box")]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.select_project(project, cx);
        model.sync_extension_events(cx);
        assert_eq!(model.state.current_project().directory, odd);
    });
}

#[gpui::test]
fn folders_opened_at_launch_find_projects_added_while_the_app_was_closed(cx: &mut TestAppContext) {
    let root = std::env::temp_dir().join(format!("muxy-cli-launch-{}", ProjectId::new()));
    std::fs::create_dir_all(root.join("Shared")).expect("mkdir");
    let mut elsewhere = AppState::bootstrap().expect("state");
    let shared = elsewhere
        .add_project(ServerId::local(), root.join("Shared"))
        .expect("project");
    let added_elsewhere = elsewhere.project(shared).expect("project").descriptor();
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (folders, opened) = async_channel::unbounded();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |_, cx| AppModel::open_folders_from(opened, cx));
    folders
        .try_send(vec![format!("file://{}/Shared/", root.display())])
        .expect("send");
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(model.state.projects().len(), 1);
        let mut projects: Vec<_> = model
            .state
            .projects()
            .iter()
            .map(muxy_app_core::Project::descriptor)
            .collect();
        projects.push(added_elsewhere);
        let page = muxy_protocol::CatalogPage {
            server: muxy_protocol::ServerIdentity::from_u128(1),
            home: model.state.home().id,
            revision: model.state.catalog_revision(ServerId::local()) + 1,
            projects,
            next: None,
            legacy_home: None,
        };
        model.receive_catalog(ServerId::local(), Ok(page), cx);
        assert_eq!(model.state.current_project().id, shared);
        assert_eq!(model.state.projects().len(), 2);
        assert!(model.state.project_intents(ServerId::local()).is_empty());
    });
    std::fs::remove_dir_all(root).expect("cleanup");
}

#[gpui::test]
fn folders_arriving_after_connection_failure_open_with_saved_projects(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("temporary folder");
    let existing = root.path().join("Existing");
    let new = root.path().join("New");
    for folder in [&existing, &new] {
        std::fs::create_dir(folder).expect("mkdir");
    }
    let mut state = AppState::bootstrap().expect("state");
    state
        .add_project(ServerId::local(), existing.clone())
        .expect("saved project");
    state.select_project(state.home().id).expect("home");
    let (boot, requests) = stub_boot(state);
    let (folders, opened) = async_channel::unbounded();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        AppModel::open_folders_from(opened, cx);
        let generation = model.servers.local.generation;
        model.receive(
            local(generation, Update::ConnectFailed("offline".into(), None)),
            cx,
        );
        assert!(!model.servers.local.catalog.identified);
    });
    requests.try_iter().for_each(drop);
    for folder in [&existing, &new] {
        let url = reqwest::Url::from_directory_path(folder).expect("file URL");
        folders.try_send(vec![url.into()]).expect("send");
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert_eq!(&model.state.current_project().directory, folder);
            assert!(model.pending_folders.is_empty());
        });
    }
    view.read_with(cx, |model, _| assert_eq!(model.state.projects().len(), 3));
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Connect))
    );
}

#[gpui::test]
fn creating_a_project_folder_requires_the_explicit_create_confirmation(cx: &mut TestAppContext) {
    let root = std::env::temp_dir().join(format!("muxy-create-project-{}", ProjectId::new()));
    std::fs::create_dir_all(&root).expect("root");
    let (mut boot, _) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.projects.search_root = Some(root.clone());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-o"
    } else {
        "ctrl-shift-o"
    });
    cx.simulate_input(&root.join("New").to_string_lossy());
    cx.executor().advance_clock(Duration::from_millis(125));
    cx.run_until_parked();
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-shift-enter"
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    assert!(!root.join("New").exists());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(!root.join("New").exists());
    assert_eq!(
        view.read_with(cx, |model, _| model.state.projects().len()),
        1
    );
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-shift-enter"
    });
    cx.run_until_parked();
    cx.simulate_prompt_answer("Create & Add");
    cx.run_until_parked();
    assert!(root.join("New").is_dir());
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().directory, root.join("New"));
        assert!(model.state.current_project().tabs.is_empty());
        assert!(model.overlay.is_none());
    });
    std::fs::remove_dir_all(root).expect("cleanup");
}
