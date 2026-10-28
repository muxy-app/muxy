use super::*;
use muxy_core::shortcuts::ShortcutId;

fn open_picker(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(ShortcutId::NewTabInProject.definition().keys[0]);
    cx.run_until_parked();
}

fn load_folders(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(125));
    cx.run_until_parked();
}

fn confirm_path(cx: &mut VisualTestContext) {
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
    std::fs::create_dir(root.path().join("Saved")).expect("saved folder");
    std::fs::create_dir(root.path().join("Needle other")).expect("other folder");
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), root.path().join("Saved"))
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
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_eq!(model.state.projects().len(), 2);
        assert!(model.active_pane().is_some());
        assert!(model.overlay.is_none());
    });
}

#[gpui::test]
fn unmatched_folders_open_in_home_unless_secondary_confirmation_adds_the_project(
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
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
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
    confirm_path(cx);
    let project = view.update(cx, |model, cx| {
        assert_eq!(model.state.current_project().directory, folder);
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert_eq!(model.state.home().tabs.len(), 1);
        assert_eq!(model.state.projects().len(), 2);
        assert!(model.overlay.is_none());
        acknowledge_catalog(model, cx);
        model.start_attach(
            model.active_pane().expect("pane"),
            Size { cols: 80, rows: 24 },
            cx,
        );
        model.state.current_project().id
    });
    assert!(requests.try_iter().any(|(_, work)| matches!(
        work,
        Work::Attach { project: target, directory, session: None, .. }
            if target == project && directory == folder
    )));
}

#[gpui::test]
fn typed_paths_complete_and_reuse_the_project_for_each_new_terminal(cx: &mut TestAppContext) {
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
    confirm_path(cx);
    let project = view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().directory, folder);
        assert_eq!(model.state.current_project().tabs.len(), 1);
        assert!(model.overlay.is_none());
        model.state.current_project().id
    });

    open_picker(cx);
    assert!(view.read_with(cx, |model, _| matches!(
        model.overlay,
        Some(Overlay::Projects(_))
    )));
    cx.simulate_input(&folder.to_string_lossy());
    load_folders(cx);
    confirm_path(cx);
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.state.current_project().tabs.len(), 2);
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
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
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
        let primary = cx
            .debug_bounds("picker-footer-open-terminal")
            .expect("home action");
        let secondary = cx
            .debug_bounds("picker-footer-confirm-path")
            .expect("add project action");
        assert!(primary.right() <= secondary.left());
        assert!(secondary.right() - primary.left() <= px(464.0));
        cx.simulate_click(
            if add_project {
                secondary.center()
            } else {
                primary.center()
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
fn home_terminals_are_rolled_back_when_their_starting_directory_cannot_be_saved(
    cx: &mut TestAppContext,
) {
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
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.update(cx, |model, _| {
        model.path = path;
        assert_eq!(model.state, state);
        assert!(model.error.is_some());
        assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
    });
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
    for key in std::iter::once("enter").chain(
        ShortcutId::PopoverSecondaryConfirm
            .definition()
            .keys
            .iter()
            .copied(),
    ) {
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

#[gpui::test]
fn terminal_picker_keeps_remote_projects_and_server_home_targets(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().expect("root");
    let server = ServerId::new();
    let project = ProjectId::new();
    let (mut boot, _requests, _remotes) = remote_boot(
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

    for (query, expected) in [("buildbox api", project), ("buildbox", home)] {
        open_picker(cx);
        cx.simulate_input(query);
        confirm_path(cx);
        view.read_with(cx, |model, _| {
            assert_eq!(model.state.current_project().id, expected);
            assert_eq!(model.state.current_project().server_id, server);
            assert_eq!(model.state.current_project().tabs.len(), 1);
            assert!(model.state.home().tabs.is_empty());
            assert!(model.overlay.is_none());
        });
    }
}

#[gpui::test]
fn the_command_palette_opens_the_same_terminal_picker_and_shortcut_toggles_it(
    cx: &mut TestAppContext,
) {
    let root = tempfile::tempdir().expect("root");
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.projects.search_root = Some(root.path().into());
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));

    cx.simulate_keystrokes(ShortcutId::ToggleCommandPalette.definition().keys[0]);
    cx.simulate_input("New Tab in Project");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert!(matches!(&model.overlay, Some(Overlay::Projects(picker)) if picker.read(cx).opens_terminal()));
    });
    open_picker(cx);
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.state.home().tabs.is_empty());
    });
}
