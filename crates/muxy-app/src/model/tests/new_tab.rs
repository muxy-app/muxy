use super::*;
use muxy_core::shortcuts::ShortcutId;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

fn open_picker(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(ShortcutId::AddProject.definition().keys[0]);
    cx.run_until_parked();
}

fn load_folders(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(125));
    cx.run_until_parked();
}

fn open_home_terminal(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-enter"
    } else {
        "ctrl-shift-enter"
    });
    cx.run_until_parked();
}

fn replace_query(cx: &mut VisualTestContext, query: &str) {
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    });
    cx.simulate_input(query);
}

#[gpui::test]
fn saved_project_matches_win_over_folders_and_pending_searches(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    std::fs::create_dir(root.path().join("Needle saved")).expect("saved folder");
    std::fs::create_dir(root.path().join("Needle other")).expect("other folder");
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), root.path().join("Needle saved"))
        .expect("project");
    state.rename_project(project, "Needle").expect("rename");
    state.select_project(state.home().id).expect("home");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));

    open_picker(cx);
    cx.simulate_input("other");
    replace_query(cx, "Needle");
    load_folders(cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        assert!(model.state.current_project().tabs.is_empty());
        assert_eq!(model.state.projects().len(), 2);
        assert!(model.active_pane().is_none());
        assert!(model.overlay.is_none());
    });
    open_picker(cx);
    cx.simulate_input("Needle");
    open_home_terminal(cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, model.state.home().id);
        assert_eq!(model.state.home().tabs.len(), 1);
        assert!(
            model
                .state
                .project(project)
                .expect("project")
                .tabs
                .is_empty()
        );
        let pane = model.active_pane().expect("home pane");
        let mut saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved.prepare_creation(pane, root.path()).0,
            root.path()
                .join("Needle saved")
                .as_os_str()
                .as_encoded_bytes()
        );
    });
    std::fs::remove_dir(root.path().join("Needle saved")).expect("remove saved folder");
    view.update(cx, AppModel::refresh_project_statuses);
    open_picker(cx);
    cx.simulate_input("other");
    load_folders(cx);
    replace_query(cx, "Needle");
    load_folders(cx);
    open_home_terminal(cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.projects().len(), 2);
        assert_eq!(model.state.home().tabs.len(), 2);
        assert!(model.overlay.is_none());
        let pane = model.active_pane().expect("home pane");
        let mut saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved.prepare_creation(pane, root.path()).0,
            root.path()
                .join("Needle other")
                .as_os_str()
                .as_encoded_bytes()
        );
    });
}

#[gpui::test]
fn folder_search_runs_below_saved_project_matches_and_without_them(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    let saved = root.path().join("Saved");
    let folder = root.path().join("nested/Needle Unsaved");
    for path in [&saved, &folder] {
        std::fs::create_dir_all(path).expect("folder");
    }
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), saved)
        .expect("project");
    state
        .rename_project(project, "Needle Saved")
        .expect("rename");
    state.select_project(state.home().id).expect("home");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let project_row: &str = format!("picker-row-{project}").leak();

    open_picker(cx);
    cx.simulate_input("Needle");
    cx.run_until_parked();
    assert!(cx.debug_bounds(project_row).is_some());
    assert!(cx.debug_bounds("picker-row-search-0").is_none());
    load_folders(cx);
    let saved_row = cx.debug_bounds(project_row).expect("saved project");
    let folder_row = cx
        .debug_bounds("picker-row-search-0")
        .expect("matching folder");
    assert!(saved_row.bottom() <= folder_row.top());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        assert!(model.state.current_project().tabs.is_empty());
        assert!(model.overlay.is_none());
    });

    open_picker(cx);
    cx.simulate_input("Needle");
    load_folders(cx);
    cx.simulate_keystrokes("down tab");
    load_folders(cx);
    open_home_terminal(cx);
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.projects().len(), 2);
        assert_eq!(model.state.home().tabs.len(), 1);
        let pane = model.active_pane().expect("home pane");
        let mut saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved.prepare_creation(pane, root.path()).0,
            folder.as_os_str().as_encoded_bytes()
        );
    });

    open_picker(cx);
    cx.simulate_input("Needle");
    replace_query(cx, "Needle Unsaved");
    load_folders(cx);
    assert!(cx.debug_bounds("picker-row-search-0").is_some());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.projects().len(), 3);
        assert_eq!(model.state.current_project().directory, folder);
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_eq!(model.state.home().tabs.len(), 1);
    });
}

#[gpui::test]
fn server_matches_keep_matching_folders_selectable_and_completable(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    let first = root.path().join("buildbox one");
    let second = root.path().join("buildbox two");
    for folder in [&first, &second] {
        std::fs::create_dir(folder).expect("folder");
    }
    let server = ServerId::new();
    let (mut boot, _requests, _remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![servers::entry(server, "buildbox")],
    );
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));

    open_picker(cx);
    cx.simulate_input("buildbox");
    load_folders(cx);
    cx.simulate_keystrokes("tab");
    load_folders(cx);
    assert!(cx.debug_bounds("picker-row-search-0").is_some());
    assert!(cx.debug_bounds("picker-row-search-1").is_some());
    cx.simulate_keystrokes("down tab");
    load_folders(cx);
    open_home_terminal(cx);
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.pending_remote.is_none());
        assert_eq!(model.state.projects().len(), 1);
        assert_eq!(model.state.home().tabs.len(), 1);
        let pane = model.active_pane().expect("home pane");
        let mut saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved.prepare_creation(pane, root.path()).0,
            first.as_os_str().as_encoded_bytes()
        );
    });

    open_picker(cx);
    cx.simulate_input("buildbox");
    load_folders(cx);
    cx.simulate_keystrokes("down down");
    cx.run_until_parked();
    let primary = cx
        .debug_bounds("picker-footer-confirm-path")
        .expect("Add Project");
    cx.simulate_click(primary.center(), Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.pending_remote.is_none());
        assert_eq!(model.state.projects().len(), 2);
        assert_eq!(model.state.current_project().directory, second);
        assert_eq!(model.state.current_project().tabs.len(), 1);
    });
}

#[gpui::test]
fn saved_local_paths_keep_trailing_spaces_when_opening_home_terminals(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    let folder = root.path().join("Saved ");
    std::fs::create_dir(&folder).expect("folder");
    let mut state = AppState::bootstrap().expect("state");
    state
        .add_project(ServerId::local(), folder.clone())
        .expect("project");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    open_picker(cx);
    cx.simulate_input("Saved");
    open_home_terminal(cx);
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.home().tabs.len(), 1);
        let pane = model.active_pane().expect("home pane");
        let mut saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved.prepare_creation(pane, root.path()).0,
            folder.as_os_str().as_encoded_bytes()
        );
    });
}

#[gpui::test]
fn unmatched_folders_add_projects_or_open_home_terminals_with_secondary_confirmation(
    cx: &mut TestAppContext,
) {
    let root = tempfile::tempdir().expect("root");
    let folder = root.path().join("nested/Needle Folder");
    std::fs::create_dir_all(&folder).expect("folder");
    let (mut boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
    });

    open_picker(cx);
    cx.simulate_input("Needle");
    load_folders(cx);
    open_home_terminal(cx);
    let home = view.update(cx, |model, cx| {
        let home = model.state.home().id;
        assert_eq!(model.state.current_project().id, home);
        assert_ne!(model.state.home().directory, folder);
        assert_eq!(model.state.home().tabs.len(), 1);
        assert_eq!(model.state.projects().len(), 1);
        assert!(model.state.project_intents(ServerId::local()).is_empty());
        assert!(model.overlay.is_none());
        model.start_attach(
            model.active_pane().expect("pane"),
            Size { cols: 80, rows: 24 },
            cx,
        );
        home
    });
    assert!(requests.try_iter().any(|(_, work)| matches!(
        work,
        Work::Attach { project, directory, session: None, .. }
            if project == home && directory == folder
    )));

    open_picker(cx);
    cx.simulate_input("Needle");
    load_folders(cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let project = view.update(cx, |model, cx| {
        assert_eq!(model.state.current_project().directory, folder);
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_eq!(model.state.home().tabs.len(), 1);
        assert_eq!(model.state.projects().len(), 2);
        assert_eq!(store::load(&model.path).expect("saved state"), model.state);
        assert!(model.overlay.is_none());
        acknowledge_catalog(model, cx);
        model.start_attach(
            model.active_pane().expect("first terminal"),
            Size { cols: 80, rows: 24 },
            cx,
        );
        model.state.current_project().id
    });
    assert!(requests.try_iter().any(|(_, work)| matches!(
        work,
        Work::Attach { project: owner, directory, session: None, .. }
            if owner == project && directory == folder
    )));
}

#[gpui::test]
fn typed_paths_open_one_initial_terminal_and_reuse_it_when_reopened(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    let folder = root.path().join("Alpha Folder");
    std::fs::create_dir(&folder).expect("folder");
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));

    open_picker(cx);
    cx.simulate_input(&root.path().join("Al").to_string_lossy());
    load_folders(cx);
    cx.simulate_keystrokes("tab");
    load_folders(cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let (project, pane) = view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().directory, folder);
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_eq!(store::load(&model.path).expect("saved state"), model.state);
        assert!(model.overlay.is_none());
        (
            model.state.current_project().id,
            model.active_pane().expect("first terminal"),
        )
    });

    open_picker(cx);
    assert!(view.read_with(cx, |model, _| matches!(
        model.overlay,
        Some(Overlay::Projects(_))
    )));
    cx.simulate_input(&folder.to_string_lossy());
    load_folders(cx);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_eq!(store::load(&model.path).expect("saved state"), model.state);
        assert_eq!(model.state.projects().len(), 2);
        assert!(model.overlay.is_none());
    });
}

#[gpui::test]
fn typed_paths_keep_home_and_persist_each_terminal_directory_before_connecting(
    cx: &mut TestAppContext,
) {
    let root = tempfile::tempdir().expect("root");
    let saved_folder = root.path().join("Saved Folder");
    let other_folder = root.path().join("Other Folder");
    std::fs::create_dir_all(saved_folder.join("Child")).expect("saved folder");
    std::fs::create_dir(&other_folder).expect("other folder");
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let home_directory = state.home().directory.clone();
    let project = state
        .add_project(ServerId::local(), saved_folder.clone())
        .expect("project");
    let (mut boot, requests) = stub_boot(state);
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let mut expected = Vec::new();

    for (query, complete, folder) in [
        (format!("{}/", saved_folder.display()), false, saved_folder),
        (
            root.path().join("Ot").to_string_lossy().into_owned(),
            true,
            other_folder,
        ),
    ] {
        open_picker(cx);
        cx.simulate_input(&query);
        load_folders(cx);
        if complete {
            cx.simulate_keystrokes("tab");
            load_folders(cx);
        }
        open_home_terminal(cx);
        let pane = view.read_with(cx, |model, _| {
            assert_eq!(model.state.current_project().id, home);
            assert_eq!(model.state.home().directory, home_directory);
            assert_eq!(model.state.home().tabs.len(), expected.len() + 1);
            assert_eq!(model.state.projects().len(), 2);
            assert_eq!(model.state.project_intents(ServerId::local()).len(), 1);
            assert!(
                model
                    .state
                    .project(project)
                    .expect("saved project")
                    .tabs
                    .is_empty()
            );
            assert!(model.overlay.is_none());
            let pane = model.active_pane().expect("pane");
            let mut saved = store::load(&model.path).expect("saved state");
            assert_eq!(saved, model.state);
            assert_eq!(
                saved.prepare_creation(pane, &home_directory).0,
                folder.as_os_str().as_encoded_bytes()
            );
            pane
        });
        expected.push((pane, folder));
    }
    view.update(cx, |model, cx| {
        model.new_tab(cx);
        expected.push((
            model.active_pane().expect("normal home pane"),
            home_directory,
        ));
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        for (pane, _) in &expected {
            model.start_attach(*pane, Size { cols: 80, rows: 24 }, cx);
        }
    });
    let attached: Vec<_> = requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::Attach {
                pane,
                project,
                directory,
                session: None,
                ..
            } => {
                assert_eq!(project, home);
                Some((pane, directory))
            }
            _ => None,
        })
        .collect();
    assert_eq!(attached.len(), expected.len());
    for expected in expected {
        assert!(attached.contains(&expected));
    }
}

fn compact_footer_bounds(cx: &mut VisualTestContext) -> [gpui::Bounds<gpui::Pixels>; 2] {
    assert!(cx.debug_bounds("picker-footer-finder").is_none());
    assert!(cx.debug_bounds("picker-footer-location").is_none());
    let actions = ["picker-footer-confirm-path", "picker-footer-open-terminal"]
        .map(|id| cx.debug_bounds(id).expect("footer action"));
    let footer = cx.debug_bounds("picker-footer").expect("footer");
    let panel = cx.debug_bounds("project-picker").expect("picker panel");
    assert!(footer.size.height <= px(29.0));
    assert!(footer.bottom() <= panel.bottom());
    for action in actions {
        assert_eq!(action.top(), actions[0].top());
        assert_eq!(action.size.height, px(20.0));
        assert!(action.left() >= panel.left());
        assert!(action.right() <= panel.right());
        assert!(action.top() >= footer.top());
        assert!(action.bottom() <= footer.bottom());
    }
    for pair in actions.windows(2) {
        assert!(pair[0].right() < pair[1].left());
    }
    actions
}

#[gpui::test]
fn folder_clicks_still_browse_and_footer_actions_choose_home_or_project(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    let folder = root.path().join("Alpha Folder");
    std::fs::create_dir_all(folder.join("Child")).expect("folder");
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));

    open_picker(cx);
    cx.simulate_input(&root.path().join("Missing").to_string_lossy());
    load_folders(cx);
    compact_footer_bounds(cx);
    cx.simulate_keystrokes("escape");

    for add_project in [false, true] {
        open_picker(cx);
        cx.simulate_input(&format!("{}/", root.path().display()));
        load_folders(cx);
        let child = cx.debug_bounds("picker-row-path-1").expect("child folder");
        cx.simulate_click(child.center(), Modifiers::default());
        load_folders(cx);
        assert!(view.read_with(cx, |model, _| matches!(
            model.overlay,
            Some(Overlay::Projects(_))
        )));
        let [primary, secondary] = compact_footer_bounds(cx);
        cx.simulate_click(
            if add_project {
                primary.center()
            } else {
                secondary.center()
            },
            Modifiers::default(),
        );
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert!(model.overlay.is_none());
            assert_eq!(model.state.home().tabs.len(), 1);
            assert_eq!(
                model.state.projects().len(),
                if add_project { 2 } else { 1 }
            );
            if add_project {
                assert_eq!(model.state.current_project().directory, folder);
                assert_eq!(model.state.current_project().tabs.len(), 1);
            } else {
                assert_eq!(model.state.current_project().id, model.state.home().id);
                let pane = model.active_pane().expect("pane");
                let mut saved = store::load(&model.path).expect("saved state");
                assert_eq!(
                    saved.prepare_creation(pane, root.path()).0,
                    folder.as_os_str().as_encoded_bytes()
                );
            }
        });
    }
}

#[gpui::test]
fn project_and_home_terminals_are_rolled_back_when_they_cannot_be_saved(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    let folder = root.path().join("Target");
    std::fs::create_dir(&folder).expect("folder");
    std::fs::write(root.path().join("blocked"), "file").expect("blocked storage");
    let (mut boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
    });
    open_picker(cx);
    cx.simulate_input(&folder.to_string_lossy());
    load_folders(cx);
    let (path, state) = view.update(cx, |model, _| {
        let path = std::mem::replace(&mut model.path, root.path().join("blocked/state.json"));
        (path, model.state.clone())
    });
    for key in [
        "enter",
        ShortcutId::PopoverSecondaryConfirm.definition().keys[0],
    ] {
        replace_query(cx, &folder.to_string_lossy());
        load_folders(cx);
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert_eq!(model.state, state);
            assert!(model.error.is_some());
            assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
            assert_eq!(store::load(&path).expect("saved state"), state);
        });
    }
    view.update(cx, |model, _| model.path = path);
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );
}

#[gpui::test]
fn invalid_terminal_paths_never_create_folders_projects_or_tabs(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    std::fs::write(root.path().join("notes.txt"), "notes").expect("file");
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));

    open_picker(cx);
    for key in ShortcutId::PopoverSecondaryConfirm.definition().keys {
        for name in ["Missing", "notes.txt"] {
            replace_query(cx, &root.path().join(name).to_string_lossy());
            load_folders(cx);
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            assert!(!cx.has_pending_prompt());
            view.read_with(cx, |model, _| {
                assert_eq!(model.state.projects().len(), 1);
                assert!(model.state.home().tabs.is_empty());
                assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
            });
        }
    }
    assert!(!root.path().join("Missing").exists());
    cx.simulate_keystrokes("escape");
    assert!(view.read_with(cx, |model, _| model.overlay.is_none()));
}

fn install_folder_checks(
    requests: &std::sync::mpsc::Receiver<(u64, Work)>,
    project: ProjectId,
    reply: Arc<Mutex<Result<bool, muxy_protocol::ErrorCode>>>,
    cx: &mut VisualTestContext,
) -> Arc<AtomicUsize> {
    use muxy_protocol::wire::{Decoder, Encoder};
    use muxy_protocol::{
        CONTROL, FilesAction, FilesReply, Message, ReplyBody, RequestBody, ServerPath,
    };

    let (client, server) = muxy_protocol::transport::socket_pair().expect("socket pair");
    let checks = Arc::new(AtomicUsize::new(0));
    let checked = checks.clone();
    std::thread::spawn(move || {
        let mut decoder = Decoder::new(server.try_clone().expect("reader"));
        let mut encoder = Encoder::new(server);
        while let Ok((_, message)) = decoder.next() {
            let response = match message {
                Message::Hello { .. } => Message::HelloReply {
                    versions: muxy_protocol::SUPPORTED.to_vec(),
                    server: muxy_protocol::ServerInfo::current(),
                    features: vec![],
                },
                Message::Request {
                    id,
                    body: RequestBody::Files(request),
                } => {
                    assert_eq!(request.project, project);
                    assert_eq!(request.action, FilesAction::Stat(ServerPath(vec![])));
                    checked.fetch_add(1, Ordering::SeqCst);
                    let body = match *reply.lock().expect("reply") {
                        Ok(is_directory) => {
                            ReplyBody::Files(FilesReply::Info(muxy_protocol::FileInfo {
                                name: ServerPath(vec![]),
                                path: ServerPath(vec![]),
                                is_directory,
                                size: 0,
                            }))
                        }
                        Err(code) => ReplyBody::Error(muxy_protocol::ErrorReply {
                            code,
                            message: "folder check failed".into(),
                        }),
                    };
                    Message::Reply { id, body }
                }
                other => panic!("unexpected request: {other:?}"),
            };
            if encoder.send(CONTROL, &response).is_err() {
                break;
            }
        }
    });
    let client = muxy_client::Client::from_stream(Box::new(client)).expect("client");
    let connection = requests
        .try_iter()
        .find_map(|(_, work)| match work {
            Work::ExtensionConnection(reply) => Some(reply),
            _ => None,
        })
        .expect("extension connection");
    connection.try_send(Ok(client)).expect("connected");
    cx.run_until_parked();
    assert_eq!(checks.load(Ordering::SeqCst), 1);
    checks
}

#[gpui::test]
fn saved_remote_projects_select_or_open_terminals_in_their_servers_home(cx: &mut TestAppContext) {
    use std::os::unix::ffi::OsStringExt;

    let root = tempfile::tempdir().expect("root");
    let server = ServerId::new();
    let project = ProjectId::new();
    let (mut boot, requests, remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![servers::entry(server, "buildbox")],
    );
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let home = view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        servers::connect_remote(model, server, vec![], &[(project, "api")], cx)
    });
    let odd = PathBuf::from(std::ffi::OsString::from_vec(b"/srv/caf\xe9 \xff ".to_vec()));
    view.update(cx, |model, cx| {
        let mut page = servers::page(2, home, &[(project, "api")]);
        page.revision = 2;
        page.projects[1].directory =
            muxy_protocol::ServerPath(odd.as_os_str().as_encoded_bytes().to_vec());
        model.receive((server, 1, Update::Catalog(Ok(page))), cx);
    });
    let checks = install_folder_checks(
        &remotes.borrow()[&server],
        project,
        Arc::new(Mutex::new(Ok(true))),
        cx,
    );

    open_picker(cx);
    cx.simulate_input("buildbox api");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        assert!(model.state.current_project().tabs.is_empty());
        assert!(model.overlay.is_none());
    });

    for (index, (query, path)) in [
        ("buildbox api", odd.as_path()),
        ("buildbox", std::path::Path::new("/home/dev/Home")),
    ]
    .into_iter()
    .enumerate()
    {
        open_picker(cx);
        cx.simulate_input(query);
        let checked = checks.load(Ordering::SeqCst);
        open_home_terminal(cx);
        assert_eq!(
            checks.load(Ordering::SeqCst),
            checked + usize::from(index == 0)
        );
        view.update(cx, |model, cx| {
            assert_eq!(model.state.current_project().id, home);
            assert_eq!(model.state.current_project().server_id, server);
            assert_eq!(model.state.current_project().tabs.len(), index + 1);
            assert!(
                model
                    .state
                    .project(project)
                    .expect("project")
                    .tabs
                    .is_empty()
            );
            assert!(model.state.home().tabs.is_empty());
            assert!(model.overlay.is_none());
            let pane = model.active_pane().expect("remote home pane");
            if index == 0 {
                let mut saved = store::load(&model.path).expect("saved state");
                assert_eq!(
                    saved
                        .prepare_creation(pane, std::path::Path::new("/wrong"))
                        .0,
                    path.as_os_str().as_encoded_bytes()
                );
            }
            model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
        });
        assert!(remotes.borrow()[&server].try_iter().any(|(_, work)| matches!(work,
            Work::Attach { project, directory, session: None, .. } if project == home && directory == path
        )));
    }
    assert!(
        !requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );
}

#[gpui::test]
fn sidebar_add_project_menu_and_remote_button_remain_separate_in_each_layout(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::settings::{AppLayout, SidebarCollapsedStyle};

    let root = tempfile::tempdir().expect("root");
    let server = ServerId::new();
    let (mut boot, _requests, _remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![servers::entry(server, "buildbox")],
    );
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    view.update(cx, |model, cx| {
        model.receive(
            (server, 1, Update::ConnectFailed("offline".into(), None)),
            cx,
        );
    });

    for (layout, expanded) in [
        (AppLayout::ProjectFocused, true),
        (AppLayout::TabFocused, true),
        (AppLayout::AgentsFocused, true),
        (AppLayout::ProjectFocused, false),
    ] {
        view.update(cx, |model, cx| {
            model.appearance.layout = layout;
            model.appearance.sidebar_expanded = expanded;
            model.appearance.sidebar_collapsed_style = SidebarCollapsedStyle::Icons;
            model.appearance.sidebar_expanded_width = Some(180.0);
            cx.notify();
        });
        cx.run_until_parked();
        let sidebar = cx.debug_bounds("workspace-sidebar").expect("sidebar");
        let remote = cx.debug_bounds("remote-servers-section").expect("remote");
        let add_project = cx.debug_bounds("sidebar-add-project").expect("add_project");
        for button in [remote, add_project] {
            assert!(button.left() >= sidebar.left());
            assert!(button.right() <= sidebar.right());
            assert!(button.top() >= sidebar.top());
            assert!(button.bottom() <= sidebar.bottom());
        }
        assert_eq!(remote.size, add_project.size);
        if expanded {
            assert!(remote.left() - sidebar.left() < remote.size.width);
            assert!(sidebar.right() - add_project.right() < add_project.size.width);
            assert_eq!(remote.top(), add_project.top());
        } else {
            assert!(remote.bottom() < add_project.top());
            assert_eq!(remote.left(), add_project.left());
        }
        cx.simulate_click(add_project.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("menu-label-Remote").is_some());
        assert!(cx.debug_bounds("menu-shortcut-Local").is_some());
        let local = cx.debug_bounds("menu-label-Local").expect("Local");
        cx.simulate_click(local.center(), Modifiers::none());
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
            assert!(model.state.home().tabs.is_empty());
        });
        open_picker(cx);
        assert!(view.read_with(cx, |model, _| model.overlay.is_none()));

        cx.simulate_click(remote.center(), Modifiers::none());
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert!(matches!(model.overlay, Some(Overlay::RemoteServers)));
            assert_eq!(model.remote_anchor.get(), Some(remote));
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(view.read_with(cx, |model, _| model.overlay.is_none()));
    }

    let add_project = cx.debug_bounds("sidebar-add-project").expect("add_project");
    cx.simulate_click(add_project.center(), Modifiers::none());
    cx.run_until_parked();
    let local = cx.debug_bounds("menu-label-Local").expect("Local");
    cx.simulate_click(local.center(), Modifiers::none());
    cx.run_until_parked();
    cx.simulate_input(&root.path().to_string_lossy());
    load_folders(cx);
    open_home_terminal(cx);
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.projects().len(), 1);
        assert_eq!(model.state.home().tabs.len(), 1);
        let pane = model.active_pane().expect("pane");
        let mut saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved
                .prepare_creation(pane, &model.state.home().directory)
                .0,
            root.path().as_os_str().as_encoded_bytes()
        );
    });
}

#[gpui::test]
fn the_command_palette_opens_the_unified_picker_and_the_old_shortcut_is_retired(
    cx: &mut TestAppContext,
) {
    let root = tempfile::tempdir().expect("root");
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));

    cx.simulate_keystrokes(ShortcutId::ToggleCommandPalette.definition().keys[0]);
    cx.simulate_input("Open Project");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
    });
    open_picker(cx);
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-shift-n"
    } else {
        "ctrl-alt-n"
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.state.home().tabs.is_empty());
    });
}

fn show_remote_folder_picker(
    view: &Entity<AppModel>,
    server: ServerId,
    created: Arc<Mutex<Vec<String>>>,
    cx: &mut VisualTestContext,
) {
    use crate::picker::path_service::{DirectoryItem, TypedPathState};
    use crate::picker::remote::RemoteFolders;
    use crate::views::project_picker::ProjectPicker;

    let folders = RemoteFolders::new(
        "buildbox".into(),
        "/home/dev/Home",
        |_| Ok(vec![DirectoryItem::Directory("Workspace".into())]),
        |path, _| {
            Ok(match path {
                "/srv/Workspace" => TypedPathState::Directory,
                "/srv/notes.txt" => TypedPathState::NotDirectory,
                _ => TypedPathState::Missing,
            })
        },
        move |path, _| {
            created.lock().expect("created paths").push(path.to_owned());
            Ok(())
        },
    );

    view.update(cx, |model, cx| {
        let picker = cx.new(|cx| {
            ProjectPicker::remote(folders, vec![], model.theme.clone(), model.metrics, cx)
        });
        model.show_project_picker(server, picker, cx);
    });
    load_folders(cx);
}

#[gpui::test]
fn remote_paths_use_server_checks_and_only_primary_confirmation_can_create_projects(
    cx: &mut TestAppContext,
) {
    let server = ServerId::new();
    let (boot, local_requests, remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![servers::entry(server, "buildbox")],
    );
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let home = view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        servers::connect_remote(model, server, vec![], &[], cx)
    });
    let created = Arc::new(Mutex::new(Vec::new()));
    let show = |cx: &mut VisualTestContext| {
        show_remote_folder_picker(&view, server, created.clone(), cx);
    };
    show(cx);
    assert!(cx.debug_bounds("picker-footer-finder").is_none());
    assert!(cx.debug_bounds("picker-footer-location").is_none());
    for key in ShortcutId::PopoverSecondaryConfirm.definition().keys {
        for path in ["/srv/Missing", "/srv/notes.txt"] {
            replace_query(cx, path);
            load_folders(cx);
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            assert!(!cx.has_pending_prompt());
            assert!(created.lock().expect("created paths").is_empty());
            view.read_with(cx, |model, _| {
                assert_eq!(model.state.projects().len(), 2);
                assert!(model.state.project(home).expect("home").tabs.is_empty());
                assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
            });
        }
    }
    replace_query(cx, "/srv/Workspace");
    load_folders(cx);
    open_home_terminal(cx);
    view.update(cx, |model, cx| {
        assert_eq!(model.state.current_project().id, home);
        assert_eq!(
            model.state.current_project().directory,
            std::path::Path::new("/home/dev/Home")
        );
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_eq!(model.state.projects().len(), 2);
        assert!(model.state.project_intents(server).is_empty());
        assert!(model.state.home().tabs.is_empty());
        assert!(model.overlay.is_none());
        let pane = model.active_pane().expect("remote home pane");
        let mut saved = store::load(&model.path).expect("saved state");
        assert_eq!(
            saved
                .prepare_creation(pane, std::path::Path::new("/wrong"))
                .0,
            b"/srv/Workspace"
        );
        model.start_attach(pane, Size { cols: 80, rows: 24 }, cx);
    });
    assert!(remotes.borrow()[&server].try_iter().any(|(_, work)| matches!(work,
        Work::Attach { project, directory, session: None, .. } if project == home && directory == std::path::Path::new("/srv/Workspace")
    )));
    assert!(
        !local_requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );

    for path in ["/srv/Workspace", "/srv/Missing"] {
        show(cx);
        replace_query(cx, path);
        load_folders(cx);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        if path == "/srv/Missing" {
            assert!(cx.has_pending_prompt());
            assert!(created.lock().expect("created paths").is_empty());
            cx.simulate_prompt_answer("Cancel");
            cx.run_until_parked();
            assert!(created.lock().expect("created paths").is_empty());
            cx.simulate_keystrokes("enter");
            cx.run_until_parked();
            cx.simulate_prompt_answer("Create & Add");
            cx.run_until_parked();
        }
        view.read_with(cx, |model, _| {
            assert_eq!(model.state.current_project().server_id, server);
            assert_eq!(
                model.state.current_project().directory,
                std::path::Path::new(path)
            );
            assert_eq!(model.state.current_project().tabs.len(), 1);
            assert_eq!(model.state.project(home).expect("home").tabs.len(), 1);
            assert!(model.overlay.is_none());
        });
    }
    assert_eq!(*created.lock().expect("created paths"), ["/srv/Missing"]);
}

#[gpui::test]
fn a_server_target_connects_to_browse_on_enter_without_creating_a_terminal(
    cx: &mut TestAppContext,
) {
    let root = tempfile::tempdir().expect("root");
    let server = ServerId::new();
    let (mut boot, _requests, _remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![servers::entry(server, "buildbox")],
    );
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    open_picker(cx);
    cx.simulate_input("buildbox");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.pending_remote.is_some_and(
            |pending| pending.server == server && pending.intent == RemoteIntent::AddProject
        ));
        assert!(
            model
                .state
                .projects()
                .iter()
                .all(|project| project.tabs.is_empty())
        );
    });
}

#[gpui::test]
fn the_add_project_dropdown_is_available_without_saved_servers(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    let add_project = cx.debug_bounds("sidebar-add-project").expect("Add Project");
    cx.simulate_click(add_project.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("menu-label-Local").is_some());
    assert!(cx.debug_bounds("menu-label-Remote").is_some());
    assert!(cx.debug_bounds("menu-shortcut-Local").is_some());
    view.read_with(cx, |model, cx| {
        assert_eq!(
            crate::views::menu::Command::AddProject.shortcut(model, cx),
            Some(
                Keystroke::parse(ShortcutId::AddProject.definition().keys[0])
                    .expect("shortcut")
                    .to_string()
            )
        );
    });
    let remote = cx.debug_bounds("menu-label-Remote").expect("Remote");
    cx.simulate_click(remote.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("menu-label-Manage Remote Servers…")
            .is_some()
    );
}

#[gpui::test]
fn remote_projects_that_become_missing_cannot_open_home_terminals_from_stale_picker_rows(
    cx: &mut TestAppContext,
) {
    let root = tempfile::tempdir().expect("root");
    let server = ServerId::new();
    let project = ProjectId::new();
    let (mut boot, local_requests, remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![servers::entry(server, "buildbox")],
    );
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        servers::connect_remote(model, server, vec![], &[(project, "api")], cx);
    });
    open_picker(cx);
    cx.simulate_input("buildbox api");
    let (state, persisted) = view.update(cx, |model, cx| {
        model
            .state
            .set_remote_project_status(project, ProjectStatus::Missing)
            .expect("remote status");
        model.sync_visible(cx);
        cx.notify();
        (
            model.state.clone(),
            std::fs::read(&model.path).expect("persisted state"),
        )
    });
    open_home_terminal(cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.state, state);
        assert_eq!(
            std::fs::read(&model.path).expect("persisted state"),
            persisted
        );
        assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
    });
    assert!(
        !local_requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );
    assert!(
        !remotes.borrow()[&server]
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );
}

#[gpui::test]
fn saved_remote_projects_are_checked_again_before_persisting_home_terminals(
    cx: &mut TestAppContext,
) {
    let root = tempfile::tempdir().expect("root");
    let server = ServerId::new();
    let project = ProjectId::new();
    let (mut boot, local_requests, remotes) = remote_boot(
        AppState::bootstrap().expect("state"),
        vec![servers::entry(server, "buildbox")],
    );
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive(local(1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        servers::connect_remote(model, server, vec![], &[(project, "api")], cx);
    });
    let reply = Arc::new(Mutex::new(Ok(true)));
    let checks = install_folder_checks(&remotes.borrow()[&server], project, reply.clone(), cx);
    let (state, persisted) = view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.project(project).expect("project").status(),
            ProjectStatus::Available
        );
        (
            model.state.clone(),
            std::fs::read(&model.path).expect("persisted state"),
        )
    });
    for (index, result) in [
        Err(muxy_protocol::ErrorCode::BadPath),
        Ok(false),
        Err(muxy_protocol::ErrorCode::Unauthorized),
    ]
    .into_iter()
    .enumerate()
    {
        open_picker(cx);
        cx.simulate_input("buildbox api");
        *reply.lock().expect("reply") = result;
        open_home_terminal(cx);
        assert_eq!(checks.load(Ordering::SeqCst), index + 2);
        view.read_with(cx, |model, _| {
            assert_eq!(model.state, state);
            assert_eq!(
                std::fs::read(&model.path).expect("persisted state"),
                persisted
            );
            assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
            assert!(model.close_prompt.is_none());
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
    }
    assert!(
        !local_requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );
    assert!(
        !remotes.borrow()[&server]
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Attach { .. }))
    );
}
