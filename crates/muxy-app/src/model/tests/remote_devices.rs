use super::preferences::settings_window;
use super::servers::{connect_remote, descriptor, entry, page, redraw, serve_remote, work};
use super::*;
use crate::picker::path_service::DirectoryItem;
use crate::picker::remote::RemoteFolders;
use crate::views::project_picker::ProjectPicker;
use crate::views::settings::SettingsEvent;
use muxy_app_core::settings::{ServerEntry, Settings};

/// A stub boot listing `servers`, both in settings and in its settings.toml.
fn boot_with(servers: Vec<ServerEntry>) -> (Boot, std::sync::mpsc::Receiver<(u64, Work)>, Remotes) {
    let (boot, local, remotes) = remote_boot(AppState::bootstrap().expect("state"), servers);
    let path = boot.state_path.with_file_name("settings.toml");
    std::fs::create_dir_all(path.parent().expect("profile")).expect("profile");
    let mut file = Settings::default();
    for server in &boot.settings.servers {
        file.add_server(server.clone(), &path).expect("listed");
    }
    (boot, local, remotes)
}

fn form(name: &str, host: &str) -> DeviceForm {
    DeviceForm {
        name: name.into(),
        host: host.into(),
        ..DeviceForm::default()
    }
}

fn listed(path: &std::path::Path) -> Vec<ServerEntry> {
    Settings::load(path).expect("settings").servers
}

fn connect_local(model: &mut AppModel, cx: &mut Context<AppModel>) {
    model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
    acknowledge_catalog(model, cx);
}

fn selector(text: String) -> &'static str {
    text.leak()
}

#[gpui::test]
fn remote_devices_are_added_edited_and_forgotten_in_settings_and_the_runtime(
    cx: &mut TestAppContext,
) {
    let other = ServerId::new();
    let (boot, _, remotes) = boot_with(vec![entry(other, "other")]);
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let (other_home, other_api) = (ProjectId::new(), ProjectId::new());
    let device = view.update(cx, |model, cx| {
        connect_local(model, cx);
        model.receive((other, 1, Update::Connected(vec![])), cx);
        let catalog = page(3, other_home, &[(other_api, "api")]);
        model.receive((other, 1, Update::Catalog(Ok(catalog))), cx);

        let invalid = model.save_remote_server(None, &form("box", "-oProxyCommand=sh"), cx);
        assert!(invalid.is_err());
        assert_eq!(listed(&path).len(), 1);

        let device = model
            .save_remote_server(None, &form(" ", " dev@box "), cx)
            .expect("added");
        let saved = listed(&path);
        let added = saved
            .iter()
            .find(|entry| entry.id == device)
            .expect("saved");
        assert_eq!(
            (added.name.as_str(), added.ssh.as_str()),
            ("dev@box", "dev@box")
        );
        assert_eq!(model.connection(device), ConnectionState::Connecting);
        device
    });
    let first = remotes.borrow_mut().remove(&device).expect("worker");
    assert!(
        first
            .try_iter()
            .any(|(generation, work)| generation == 1 && matches!(work, Work::Connect))
    );

    view.update(cx, |model, cx| {
        let home = connect_remote(model, device, vec![], &[], cx);
        work(&first);
        model
            .save_remote_server(Some(device), &form("box", "dev@box"), cx)
            .expect("renamed");
        assert_eq!(model.server_name(device), Some("box"));
        assert!(model.ready(device), "a new name keeps the connection");
        assert!(
            !work(&first)
                .iter()
                .any(|work| matches!(work, Work::Stop | Work::Connect))
        );

        model
            .save_remote_server(Some(device), &form("box", "dev@box2"), cx)
            .expect("moved");
        assert_eq!(listed(&path)[1].ssh, "dev@box2");
        assert_eq!(model.connection(device), ConnectionState::Connecting);
        assert_eq!(
            model.state.server_home(device).map(|home| home.id),
            Some(home)
        );
    });
    assert!(first.try_iter().any(|(_, work)| matches!(work, Work::Stop)));
    let second = remotes.borrow_mut().remove(&device).expect("new worker");
    assert!(
        second
            .try_iter()
            .any(|(generation, work)| generation == 3 && matches!(work, Work::Connect)),
        "the new worker counts on past the old one's generation"
    );

    view.update(cx, |model, cx| {
        model.forget_remote_server(device, cx).expect("forgotten");
        assert!(listed(&path).iter().all(|entry| entry.id != device));
        assert!(model.server_name(device).is_none());
        assert!(model.servers.get(device).is_none());
        assert!(model.state.server_home(device).is_none());
        assert!(
            model.state.project(other_api).is_some(),
            "other servers keep their projects"
        );
    });
    assert!(
        second
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Stop))
    );
}

#[gpui::test]
fn the_remote_section_adds_servers_and_manages_them_from_its_popover(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (mut boot, _, _) = boot_with(vec![entry(remote, "box")]);
    boot.settings.appearance.sidebar_expanded = true;
    let path = boot.state_path.with_file_name("settings.toml");
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&boot.settings.keymap, cx);
        cx.bind_keys(muxy_ui::text_input::key_bindings());
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    view.update(cx, |model, cx| {
        let failed = Update::ConnectFailed("Can't reach dev@box".into(), None);
        model.receive((remote, 1, failed), cx);
    });
    click(cx, "remote-servers-section");
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::RemoteServers)));
    });
    assert!(
        cx.debug_bounds(selector(format!("remote-server-{remote}")))
            .is_some()
    );
    assert!(
        cx.debug_bounds(selector(format!("remote-server-error-{remote}")))
            .is_some(),
        "the failure shows beside the server"
    );

    click(cx, "remote-add-server");
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::ServerForm(_))));
    });
    cx.simulate_input("other");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let added = view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none(), "saving closes the form");
        let added = model.settings.servers.last().expect("added").clone();
        assert_eq!(
            (added.name.as_str(), added.ssh.as_str()),
            ("other", "other")
        );
        added.id
    });
    assert_eq!(listed(&path).len(), 2);

    click(cx, "remote-servers-section");
    click(cx, selector(format!("remote-server-{remote}")));
    view.read_with(cx, |model, cx| {
        let Some(Overlay::Commands { palette, .. }) = &model.overlay else {
            panic!("the server's actions");
        };
        assert!(palette.read(cx).is_page(&format!("remote-{remote}")));
    });
    view.update(cx, |model, cx| {
        model.dismiss_overlay(cx);
        model.confirm_forget_server(remote, model.window, cx);
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Forget");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.server_name(remote).is_none());
        assert!(model.server_name(added).is_some());
        assert_eq!(model.error, None, "removing raises no alert");
    });
    assert!(listed(&path).iter().all(|entry| entry.id != remote));
}

#[gpui::test]
fn manage_servers_lists_each_server_with_its_actions(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (boot, _, _) = boot_with(vec![entry(remote, "box")]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| model.open_remote_servers(None, window, cx));
    });
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let Some(Overlay::Commands { palette, .. }) = &model.overlay else {
            panic!("the servers list");
        };
        assert!(palette.read(cx).is_page("remote_servers"));
    });
    assert!(cx.debug_bounds("picker-row-remote-add-server").is_some());
    assert!(
        cx.debug_bounds(selector(format!("picker-row-remote-{remote}")))
            .is_some()
    );
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.open_remote_servers(Some(remote), window, cx);
        });
    });
    cx.run_until_parked();
    for action in ["add-project", "connect", "edit", "remove"] {
        assert!(
            cx.debug_bounds(selector(format!("picker-row-{action}")))
                .is_some(),
            "{action}"
        );
    }
    assert!(
        cx.debug_bounds("picker-row-install").is_none(),
        "only releases install Muxy"
    );
}

#[gpui::test]
fn test_connection_reports_the_server_version_or_the_failure(cx: &mut TestAppContext) {
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot.workers.with_probe(|host| {
        if host.destination() == "dev@box" {
            Ok("2.1.0".into())
        } else {
            Err("Permission denied".into())
        }
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.test_remote_server(None, &form("", " dev@box "), cx);
        assert_eq!(
            model.server_probe("dev@box"),
            Some(&ConnectionTest::Testing)
        );
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(
            model.server_probe("dev@box"),
            Some(&ConnectionTest::Succeeded("2.1.0".into()))
        );
        model.test_remote_server(None, &form("", "dev@elsewhere"), cx);
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(
            model.server_probe("dev@elsewhere"),
            Some(&ConnectionTest::Failed("Permission denied".into()))
        );
        assert_eq!(
            model.server_probe("dev@box"),
            None,
            "only the last test shows"
        );
        model.test_remote_server(None, &form("", "-oProxyCommand=sh"), cx);
        assert!(matches!(
            model.server_probe("-oProxyCommand=sh"),
            Some(ConnectionTest::Failed(_))
        ));
    });
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let bounds = cx.debug_bounds(selector).expect(selector);
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn the_status_bar_shows_and_restarts_the_current_projects_server(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (boot, local_work, remotes) = boot_with(vec![entry(remote, "box")]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        model.receive(
            (
                remote,
                1,
                Update::ServerInfo(muxy_protocol::ServerInfo::current()),
            ),
            cx,
        );
        let api = ProjectId::new();
        connect_remote(model, remote, vec![], &[(api, "api")], cx);
        assert_eq!(model.status_server(), ServerId::local());
        model.select_project(api, cx);
        assert_eq!(model.status_server(), remote);
        assert_eq!(model.server_status(), ServerStatus::Connected);
        assert_eq!(model.server_label(remote), "box");
        assert_eq!(
            model.server_version(remote),
            Some(env!("CARGO_PKG_VERSION"))
        );
    });
    let remote_work = remotes.borrow_mut().remove(&remote).expect("worker");
    work(&remote_work);
    work(&local_work);
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    click(cx, "project-connection-status");
    assert!(cx.debug_bounds("server-popover-name").is_some());
    click(cx, "restart-project-server");
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Restart");
    cx.run_until_parked();
    assert!(
        work(&remote_work)
            .iter()
            .any(|work| matches!(work, Work::StopServer { restart: true }))
    );
    assert!(
        !work(&local_work)
            .iter()
            .any(|work| matches!(work, Work::StopServer { .. })),
        "this computer's server keeps running"
    );
    view.update(cx, |model, cx| {
        assert_eq!(model.server_status(), ServerStatus::Stopping);
        assert!(!model.server_control_enabled());
        let stopped = Update::ServerStopped {
            restart: true,
            result: Ok(()),
        };
        model.receive((remote, 1, stopped), cx);
        assert_eq!(model.server_status(), ServerStatus::Connecting);
    });
    assert!(
        remote_work
            .try_iter()
            .any(|(generation, work)| generation == 2 && matches!(work, Work::Connect))
    );
}

#[gpui::test]
fn remote_failures_stay_in_the_remote_section_without_alerts(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (boot, _, _) = boot_with(vec![entry(remote, "box")]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let failed = Update::ConnectFailed("Can't reach dev@box".into(), None);
        model.receive((remote, 1, failed), cx);
        assert_eq!(model.error, None);
        assert_eq!(model.server_error(remote), Some("Can't reach dev@box"));
        assert!(!model.send(remote, Work::ReadCatalog, cx));
        assert_eq!(
            model.error, None,
            "work for an offline server raises no alert"
        );
        let local = Update::ConnectFailed("The server isn't running".into(), None);
        model.receive((ServerId::local(), 1, local), cx);
        assert_eq!(
            model.error.as_deref(),
            Some("The server isn't running"),
            "this computer's server still alerts"
        );
    });
}

/// Connects this computer's server and `remote`, whose Home is /home/dev/Home.
fn connected_remote(
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext, ServerId, Remotes) {
    let remote = ServerId::new();
    let (mut boot, _, remotes) = boot_with(vec![entry(remote, "box")]);
    boot.settings.appearance.sidebar_expanded = true;
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&boot.settings.keymap, cx);
        cx.bind_keys(muxy_ui::text_input::key_bindings());
        cx.bind_keys(muxy_ui::picker::key_bindings());
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        connect_remote(model, remote, vec![], &[], cx);
    });
    (view, cx, remote, remotes)
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(125));
    cx.run_until_parked();
}

#[gpui::test]
fn remote_add_project_browses_the_remote_home_and_adds_typed_paths_there(cx: &mut TestAppContext) {
    let (view, cx, remote, remotes) = connected_remote(cx);
    let listed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let open = |cx: &mut VisualTestContext| {
        let listed = listed.clone();
        let folders = RemoteFolders::new("box".into(), "/home/dev/Home", move |path| {
            listed.lock().expect("listed").push(path.to_owned());
            match path {
                "/home/dev/Home" => Ok(vec![DirectoryItem::Directory("code".into())]),
                "/home/dev/Home/code" => Ok(vec![DirectoryItem::Directory("app".into())]),
                _ => Err("Only folders inside /home/dev/Home can be listed.".into()),
            }
        });
        cx.update(|window, cx| {
            view.update(cx, |model, cx| {
                let picker = cx.new(|cx| {
                    ProjectPicker::remote(folders, vec![], model.theme.clone(), model.metrics, cx)
                });
                let _ = window;
                model.show_project_picker(remote, picker, cx);
            });
        });
        settle(cx);
    };
    open(cx);
    assert!(
        listed
            .lock()
            .expect("listed")
            .contains(&"/home/dev/Home".to_owned())
    );
    assert!(
        cx.debug_bounds("picker-row-path-1").is_some(),
        "Home's folders are listed"
    );

    cx.simulate_input("code/app");
    settle(cx);
    assert!(
        listed
            .lock()
            .expect("listed")
            .contains(&"/home/dev/Home/code".to_owned())
    );
    cx.simulate_keystrokes("alt-enter");
    settle(cx);
    let remote_work = remotes.borrow_mut().remove(&remote).expect("worker");
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        let added = model.state.current_project();
        assert_eq!(added.server_id, remote);
        assert_eq!(added.directory, PathBuf::from("/home/dev/Home/code/app"));
    });
    assert!(
        work(&remote_work)
            .iter()
            .any(|work| matches!(work, Work::MutateProject(_)))
    );

    open(cx);
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("/srv/app");
    settle(cx);
    assert!(listed.lock().expect("listed").contains(&"/srv".to_owned()));
    cx.simulate_keystrokes("alt-enter");
    settle(cx);
    view.read_with(cx, |model, _| {
        let added = model.state.current_project();
        assert_eq!(
            (added.server_id, added.directory.clone()),
            (remote, PathBuf::from("/srv/app"))
        );
    });
}

#[gpui::test]
fn add_project_offers_remote_devices_only_when_there_are_some(cx: &mut TestAppContext) {
    let (boot, _, _) = boot_with(vec![]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            let position = gpui::point(px(10.0), px(10.0));
            model.choose_project_source(position, window, cx);
            assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
            model.dismiss_overlay(cx);
            model
                .save_remote_server(None, &form("box", "dev@box"), cx)
                .expect("added");
            model.choose_project_source(position, window, cx);
            assert!(matches!(model.overlay, Some(Overlay::Menu(_))));
        });
    });
}

#[gpui::test]
fn remote_projects_are_marked_and_their_home_is_not_listed(cx: &mut TestAppContext) {
    let (view, cx, remote, _) = connected_remote(cx);
    let api = ProjectId::new();
    let (home, local_home) = view.update(cx, |model, cx| {
        let home = model.state.server_home(remote).expect("Home").id;
        let mut catalog = page(2, home, &[]);
        catalog.projects.push(descriptor(api, false, "api"));
        catalog.revision = 2;
        model.receive((remote, 1, Update::Catalog(Ok(catalog))), cx);
        let location = |id| model.remote_location(model.state.project(id).expect("project"));
        assert_eq!(location(api).as_deref(), Some("box · /home/dev/api"));
        let local_home = model.state.home().id;
        assert_eq!(model.navigation_projects(), [local_home, api]);
        (home, local_home)
    });
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    redraw(cx);
    assert!(
        cx.debug_bounds(selector(format!("project-remote-{api}")))
            .is_some()
    );
    for hidden in [home, local_home] {
        assert!(
            cx.debug_bounds(selector(format!("project-remote-{hidden}")))
                .is_none(),
            "no remote Home row, and no remote mark on this computer's Home"
        );
    }
}

#[gpui::test]
fn new_worktrees_of_remote_projects_go_under_the_remote_home(cx: &mut TestAppContext) {
    let (view, cx, remote, _) = connected_remote(cx);
    view.update(cx, |model, cx| {
        let home = model.state.server_home(remote).expect("Home").id;
        assert_eq!(
            model.remote_home(remote),
            Some(std::path::Path::new("/home/dev/Home"))
        );
        model.open_git_form(home, true, cx);
        let Some(Overlay::GitForm(form)) = &model.overlay else {
            panic!("worktree form");
        };
        let directory = model.worktree_directory(form, cx).expect("directory");
        assert!(
            directory.starts_with("/home/dev/Home/.muxy/worktrees/Home"),
            "{}",
            directory.display()
        );
    });
}

#[gpui::test]
fn a_remote_reached_over_ssh_lists_folders_and_adds_a_project_there(cx: &mut TestAppContext) {
    remote_add_project(cx).expect("remote Add Project over ssh");
}

fn remote_add_project(cx: &mut TestAppContext) -> Result {
    let directory = tempfile::Builder::new()
        .prefix("muxy-remote-")
        .tempdir_in("/tmp")?;
    let socket = directory.path().join("remote.sock");
    serve_remote(&socket)?;
    let folder = directory.path().canonicalize()?.join("app");
    std::fs::create_dir(&folder)?;
    let remote = ServerId::new();
    let (mut boot, _, _) = boot_with(vec![entry(remote, "box")]);
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&boot.settings.keymap, cx);
        cx.bind_keys(muxy_ui::text_input::key_bindings());
        cx.bind_keys(muxy_ui::picker::key_bindings());
    });
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
        connect_local(model, cx);
        model.open_remote_project_picker(remote, cx);
        assert!(model.overlay.is_none(), "the picker waits for the server");
    });
    wait(cx, &view, |model, _| {
        model.state.server_home(remote).is_some()
            && model.extensions.client(remote).is_some()
            && matches!(model.overlay, Some(Overlay::Projects(_)))
    })?;
    let (client, home) = view.read_with(cx, |model, _| {
        let home = model.state.server_home(remote).expect("Home");
        (
            model.extensions.client(remote).expect("client"),
            (home.id, home.directory.to_string_lossy().into_owned()),
        )
    });
    let folders = RemoteFolders::through(client, "box".into(), home.0, &home.1);
    let listed: Vec<_> = folders
        .list(&directory.path().to_string_lossy())?
        .iter()
        .map(|folder| folder.name().to_owned())
        .collect();
    assert_eq!(
        listed,
        ["app"],
        "any folder is listed, not only inside Home"
    );
    assert_eq!(
        folders.list("/does/not/exist").err().as_deref(),
        Some("folder does not exist")
    );
    assert!(
        view.read_with(cx, |model, _| matches!(
            model.overlay,
            Some(Overlay::Projects(_))
        )),
        "Add Project before connecting opened the picker once ready"
    );
    settle(cx);
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input(&folder.to_string_lossy());
    settle(cx);
    cx.simulate_keystrokes("alt-enter");
    settle(cx);
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    let expected = folder.to_string_lossy().into_owned().into_bytes();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let catalog = Client::connect(&socket)?.catalog()?;
        if catalog
            .projects
            .iter()
            .any(|project| project.directory.0 == expected)
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err("the remote server never listed the new project".into());
        }
        cx.run_until_parked();
        thread::sleep(Duration::from_millis(10));
    }
    view.update(cx, |model, _| model.stop_workers());
    Ok(())
}

#[gpui::test]
fn saving_a_device_also_takes_in_devices_edited_by_hand(cx: &mut TestAppContext) {
    let (boot, _, remotes) = boot_with(vec![]);
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let by_hand = entry(ServerId::new(), "by hand");
    Settings::default()
        .add_server(by_hand.clone(), &path)
        .expect("hand edit");
    let saved = view.update(cx, |model, cx| {
        let saved = model
            .save_remote_server(None, &form("y", "dev@y"), cx)
            .expect("added");
        assert_eq!(model.connection(by_hand.id), ConnectionState::Connecting);
        saved
    });
    let worker = remotes.borrow_mut().remove(&by_hand.id).expect("worker");
    assert!(
        worker
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Connect))
    );

    Settings::default()
        .remove_server(by_hand.id, &path)
        .expect("hand edit");
    view.update(cx, |model, cx| {
        model
            .save_remote_server(Some(saved), &form("renamed", "dev@y"), cx)
            .expect("renamed");
        assert!(model.servers.get(by_hand.id).is_none());
        assert!(model.servers.get(saved).is_some());
    });
    assert!(
        worker
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Stop))
    );
}

#[gpui::test]
fn settings_server_control_stays_on_this_computer_while_a_remote_project_is_open(
    cx: &mut TestAppContext,
) {
    let remote = ServerId::new();
    let (boot, local_work, remotes) = boot_with(vec![entry(remote, "box")]);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = settings_window(boot, cx);
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        let api = ProjectId::new();
        connect_remote(model, remote, vec![], &[(api, "api")], cx);
        let loaded = muxy_protocol::ServerSettingsDoc {
            default_shell: None,
            history_budget_bytes: 0,
            shell_integration: true,
        };
        model.receive_server_settings(Ok(loaded), cx);
        model.select_project(api, cx);
        assert_eq!(model.status_server(), remote);
    });
    let remote_work = remotes.borrow_mut().remove(&remote).expect("worker");
    work(&remote_work);
    work(&local_work);
    let settings = view.read_with(cx, |model, _| {
        model
            .settings_window
            .as_ref()
            .expect("settings")
            .view
            .clone()
    });
    settings.update(cx, |_, cx| {
        cx.emit(SettingsEvent::ServerControl { restart: false });
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Stop");
    cx.run_until_parked();
    assert!(
        work(&local_work)
            .iter()
            .any(|work| matches!(work, Work::StopServer { restart: false }))
    );
    assert!(
        !work(&remote_work)
            .iter()
            .any(|work| matches!(work, Work::StopServer { .. }))
    );
}

#[test]
fn the_editor_builds_ssh_destinations_from_host_user_and_port() {
    let joined = |host, user, port| join_destination(host, user, port);
    assert_eq!(joined(" box ", "", "").as_deref(), Ok("box"));
    assert_eq!(joined("dev@box", "", "").as_deref(), Ok("dev@box"));
    assert_eq!(joined("box", "dev", "").as_deref(), Ok("dev@box"));
    assert_eq!(joined("box", "", "2222").as_deref(), Ok("ssh://box:2222"));
    assert_eq!(
        joined("fe80::1", "dev", "22").as_deref(),
        Ok("ssh://dev@[fe80::1]:22")
    );
    for (host, user, port) in [
        ("", "", ""),
        ("-oProxyCommand=sh", "", ""),
        ("dev@box", "other", ""),
        ("box", "a@b", ""),
        ("box", "", "0"),
        ("box", "", "70000"),
    ] {
        assert!(joined(host, user, port).is_err(), "{host} {user} {port}");
    }
    for destination in [
        "box",
        "dev@box",
        "ssh://box:2222",
        "ssh://dev@box:2222",
        "ssh://dev@[fe80::1]:22",
    ] {
        let (host, user, port) = split_destination(destination);
        assert_eq!(
            join_destination(&host, &user, &port).as_deref(),
            Ok(destination)
        );
    }
}

/// What the app's askpass socket answers for `token` and `prompt`.
fn ask(socket: &std::ffi::OsStr, token: &str, prompt: &str) -> io::Result<String> {
    use std::io::{Read, Write};
    let mut stream = std::os::unix::net::UnixStream::connect(socket)?;
    stream.write_all(format!("{token}\n{prompt}").as_bytes())?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer)?;
    Ok(answer)
}

/// The socket and token an askpass program asks with.
fn askpass_of(askpass: &muxy_client::Askpass) -> (std::ffi::OsString, String) {
    let value = |key: &str| {
        askpass
            .environment
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    (
        value(crate::askpass::SOCKET),
        value(crate::askpass::TOKEN).to_string_lossy().into_owned(),
    )
}

fn short_socket() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::Builder::new()
        .prefix("askpass")
        .tempdir_in("/tmp")
        .expect("socket folder");
    let socket = directory.path().join("askpass.sock");
    (directory, socket)
}

#[gpui::test]
fn test_connection_answers_a_typed_password_only_while_it_runs(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let asked = seen.clone();
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot.workers.with_probe(move |target| {
        let (socket, token) = askpass_of(target.askpass().ok_or("no askpass")?);
        let answer =
            ask(&socket, &token, "dev@box's password: ").map_err(|error| error.to_string())?;
        *asked.lock().expect("seen") = Some((socket, token, answer));
        Ok("2.1.0".into())
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        let missing = DeviceForm {
            password_login: true,
            ..form("", "dev@box")
        };
        model.test_remote_server(None, &missing, cx);
        assert!(matches!(
            model.server_probe("dev@box"),
            Some(ConnectionTest::Failed(error)) if error.contains("Enter the password")
        ));
        let typed = DeviceForm {
            password: "hunter2".into(),
            ..missing
        };
        model.test_remote_server(None, &typed, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.server_probe("dev@box"),
            Some(&ConnectionTest::Succeeded("2.1.0".into()))
        );
    });
    let (socket, token, answer) = seen.lock().expect("seen").clone().expect("probed");
    assert_eq!(answer, "hunter2");
    assert_eq!(
        ask(&socket, &token, "Password: ").expect("asked"),
        "",
        "the typed password is gone after the test"
    );
}

#[gpui::test]
fn passwords_are_asked_when_connecting_and_forgotten_when_refused(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let remote = ServerId::new();
    let mut server = entry(remote, "box");
    server.password_login = true;
    let (boot, _, remotes) = boot_with(vec![server]);
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&boot.settings.keymap, cx);
        cx.bind_keys(muxy_ui::text_input::key_bindings());
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let worker = remotes.borrow_mut().remove(&remote).expect("worker");
    assert!(
        work(&worker).is_empty(),
        "launching neither connects nor asks"
    );
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        assert!(model.overlay.is_none());
        assert!(model.remote_servers()[0].needs_password);
        model.connect_remote_server(remote, cx);
        assert!(matches!(model.overlay, Some(Overlay::Password(_))));
    });
    cx.run_until_parked();
    cx.simulate_input("hunter2");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert!(model.overlay.is_none());
        assert!(!model.remote_servers()[0].needs_password);
        assert_eq!(model.connection(remote), ConnectionState::Connecting);
        let refused = Update::ConnectFailed(
            "Permission denied".into(),
            Some(muxy_client::RemoteReason::AuthenticationFailed),
        );
        model.receive((remote, 1, refused), cx);
        assert!(
            model.remote_servers()[0].needs_password,
            "a refused password is forgotten"
        );
        assert!(model.server_failed_login(remote));
        assert_eq!(model.error, None);
        model.connect_remote_server(remote, cx);
        assert!(
            matches!(model.overlay, Some(Overlay::Password(_))),
            "connecting asks again"
        );
    });
    let connects = work(&worker)
        .iter()
        .filter(|work| matches!(work, Work::Connect))
        .count();
    assert_eq!(connects, 1, "only the typed password connected");
}

#[gpui::test]
fn a_new_key_file_or_login_reconnects_through_a_new_worker(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let (boot, _, remotes) = boot_with(vec![]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let device = view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model
            .save_remote_server(None, &form("box", "dev@box"), cx)
            .expect("added")
    });
    let mut worker = remotes.borrow_mut().remove(&device).expect("worker");
    let restarted = |worker: &mut std::sync::mpsc::Receiver<(u64, Work)>| {
        let stopped = worker
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Stop));
        if let Some(new) = remotes.borrow_mut().remove(&device) {
            *worker = new;
        }
        stopped
    };
    let save = |form: &DeviceForm, cx: &mut VisualTestContext| {
        view.update(cx, |model, cx| {
            model
                .save_remote_server(Some(device), form, cx)
                .expect("saved");
        });
    };
    let keyed = DeviceForm {
        identity_file: "~/.ssh/box".into(),
        ..form("box", "dev@box")
    };
    save(&keyed, cx);
    assert!(restarted(&mut worker), "a new key file");
    let renamed = DeviceForm {
        name: "renamed".into(),
        ..keyed.clone()
    };
    save(&renamed, cx);
    assert!(!restarted(&mut worker), "a rename keeps the worker");
    let password = DeviceForm {
        password_login: true,
        password: "hunter2".into(),
        ..keyed.clone()
    };
    save(&password, cx);
    assert!(restarted(&mut worker), "logging in with a password");
    view.read_with(cx, |model, _| {
        assert!(!model.remote_servers()[0].needs_password);
    });
    let kept = DeviceForm {
        password: String::new(),
        ..password
    };
    save(&kept, cx);
    assert!(
        !restarted(&mut worker),
        "an empty field keeps the typed password"
    );
    save(&keyed, cx);
    assert!(restarted(&mut worker), "back to keys");
    view.read_with(cx, |model, _| assert!(!model.servers.passwords.has(device)));
}

#[test]
fn the_install_command_runs_this_releases_installer() {
    assert_eq!(
        remote_servers::install_command("2.0.0-beta-9"),
        "sh -c 'curl -fsSL https://github.com/muxy-app/muxy/releases/download/v2.0.0-beta-9/install-muxy.sh | sh -s -- --version 2.0.0-beta-9'"
    );
    assert_eq!(released_version(), None, "test builds aren't releases");
}

#[gpui::test]
fn a_device_without_muxy_installs_it_over_ssh_and_then_connects(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let ran = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (mut boot, _, remotes) = boot_with(vec![entry(remote, "box")]);
    let (log, result) = (
        ran.clone(),
        std::sync::Arc::new(std::sync::Mutex::new(Ok(()))),
    );
    let outcome = result.clone();
    boot.workers = boot.workers.with_run(move |target, command| {
        log.lock()
            .expect("ran")
            .push((target.destination().to_owned(), command.to_owned()));
        outcome.lock().expect("result").clone()
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let missing = muxy_client::RemoteReason::NotInstalled;
    view.update(cx, |model, cx| {
        let failed = Update::ConnectFailed("Muxy isn't installed on dev@box".into(), Some(missing));
        model.receive((remote, 1, failed), cx);
        assert!(model.remote_servers()[0].missing);
        *result.lock().expect("result") = Err("Error: Linux requires glibc 2.35 or newer.".into());
        model.install_server(remote, "2.0.0-beta-9", cx);
        assert_eq!(model.remote_servers()[0].install, Some(Install::Running));
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(
            model.remote_servers()[0].install,
            Some(Install::Failed(
                "Error: Linux requires glibc 2.35 or newer.".into()
            ))
        );
        *result.lock().expect("result") = Ok(());
        model.install_server(remote, "2.0.0-beta-9", cx);
    });
    let worker = remotes.borrow_mut().remove(&remote).expect("worker");
    work(&worker);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let device = &model.remote_servers()[0];
        assert_eq!(device.install, None);
        assert!(!device.missing);
        assert_eq!(model.connection(remote), ConnectionState::Connecting);
    });
    assert!(
        work(&worker)
            .iter()
            .any(|work| matches!(work, Work::Connect)),
        "it connects once installed"
    );
    let ran = ran.lock().expect("ran").clone();
    assert_eq!(ran.len(), 2);
    assert_eq!(ran[0].0, "dev@box");
    assert!(
        ran[0]
            .1
            .contains("install-muxy.sh | sh -s -- --version 2.0.0-beta-9")
    );
}

#[gpui::test]
fn a_local_build_explains_how_to_install_itself_on_a_server(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (mut boot, _, _) = boot_with(vec![entry(remote, "box")]);
    boot.settings.appearance.sidebar_expanded = true;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    view.update(cx, |model, cx| {
        let missing = Some(muxy_client::RemoteReason::NotInstalled);
        let failed = Update::ConnectFailed("Muxy isn't installed on dev@box".into(), missing);
        model.receive((remote, 1, failed), cx);
    });
    click(cx, "remote-servers-section");
    assert!(
        cx.debug_bounds(selector(format!("remote-server-hint-{remote}")))
            .is_some()
    );
}

fn extension_connections(worker: &std::sync::mpsc::Receiver<(u64, Work)>) -> usize {
    work(worker)
        .iter()
        .filter(|work| matches!(work, Work::ExtensionConnection(_)))
        .count()
}

#[gpui::test]
fn add_project_waits_for_the_extension_connection_without_opening_another(cx: &mut TestAppContext) {
    let (view, cx, remote, remotes) = connected_remote(cx);
    let worker = remotes.borrow_mut().remove(&remote).expect("worker");
    assert_eq!(extension_connections(&worker), 1, "connecting opens one");
    view.update(cx, |model, cx| {
        model.open_remote_project_picker(remote, cx);
        model.open_remote_project_picker(remote, cx);
        assert!(model.pending_remote_picker.is_some());
        assert!(model.overlay.is_none());
    });
    assert_eq!(
        extension_connections(&worker),
        0,
        "the one opening is waited for, not opened again"
    );
}

#[gpui::test]
fn a_waiting_add_project_is_dropped_once_the_user_moves_on(cx: &mut TestAppContext) {
    let (view, cx, remote, _) = connected_remote(cx);
    view.update(cx, |model, cx| {
        model.pending_remote_picker = Some((remote, Instant::now()));
        model.overlay = Some(Overlay::RemoteServers);
        model.resume_remote_picker(remote, cx);
        assert!(
            model.pending_remote_picker.is_none(),
            "something else is open"
        );
        model.dismiss_overlay(cx);

        let long_ago = Instant::now()
            .checked_sub(Duration::from_secs(120))
            .expect("two minutes ago");
        model.pending_remote_picker = Some((remote, long_ago));
        model.resume_remote_picker(remote, cx);
        assert!(model.pending_remote_picker.is_none(), "the wait was long");
        assert!(model.overlay.is_none());

        model.pending_remote_picker = Some((remote, Instant::now()));
        model.ask_password(remote, cx);
        model.dismiss_overlay(cx);
        assert!(
            model.pending_remote_picker.is_none(),
            "cancelling the password cancels the wait"
        );
    });
}

#[gpui::test]
fn the_remote_home_itself_is_not_added_as_a_project(cx: &mut TestAppContext) {
    let (view, cx, remote, _) = connected_remote(cx);
    let before = view.read_with(cx, |model, _| model.state.current_project().id);
    let folders = RemoteFolders::new("box".into(), "/home/dev/Home", |_| Ok(Vec::new()));
    view.update(cx, |model, cx| {
        let picker = cx.new(|cx| {
            ProjectPicker::remote(folders, vec![], model.theme.clone(), model.metrics, cx)
        });
        model.show_project_picker(remote, picker, cx);
    });
    settle(cx);
    cx.simulate_keystrokes("alt-enter");
    settle(cx);
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
        assert_eq!(model.state.current_project().id, before);
    });
}

#[gpui::test]
fn install_failures_show_in_the_popover_until_the_server_connects(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (mut boot, _, _) = boot_with(vec![entry(remote, "box")]);
    boot.settings.appearance.sidebar_expanded = true;
    boot.workers = boot
        .workers
        .with_run(|_, _| Err("Error: Linux requires glibc 2.35 or newer.".into()));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    view.update(cx, |model, cx| {
        let missing = Some(muxy_client::RemoteReason::NotInstalled);
        let failed = Update::ConnectFailed("Muxy isn't installed on dev@box".into(), missing);
        model.receive((remote, 1, failed), cx);
        model.install_server(remote, "2.0.0-beta-9", cx);
    });
    cx.run_until_parked();
    click(cx, "remote-servers-section");
    assert!(
        cx.debug_bounds(selector(format!("remote-server-error-{remote}")))
            .is_some()
    );
    view.update(cx, |model, cx| {
        let server = &model.remote_servers()[0];
        assert!(matches!(&server.install, Some(Install::Failed(error)) if error.contains("glibc")));
        model.dismiss_overlay(cx);
        model.connect_remote_server(remote, cx);
        let generation = model.generation(remote);
        model.receive((remote, generation, Update::Connected(vec![])), cx);
        assert_eq!(model.remote_servers()[0].install, None);
    });
}

#[gpui::test]
fn a_remote_catalog_failure_raises_no_alert(cx: &mut TestAppContext) {
    let (view, cx, remote, _remotes) = connected_remote(cx);
    view.update(cx, |model, cx| {
        model.set_banner_error(None);
        let failed = Update::Catalog(Err(muxy_client::ClientError::Timeout));
        model.receive((remote, 1, failed), cx);
        assert_eq!(model.error, None);
        assert!(
            model
                .server_error(remote)
                .is_some_and(|error| error.contains("refresh"))
        );
    });
}

#[gpui::test]
fn a_password_that_cannot_be_kept_saves_nothing(cx: &mut TestAppContext) {
    let (boot, _, _) = boot_with(vec![]);
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model
            .servers
            .passwords
            .set_socket(PathBuf::from("/does/not/exist/askpass.sock"));
        let typed = DeviceForm {
            password_login: true,
            password: "hunter2".into(),
            ..form("box", "dev@box")
        };
        assert!(model.save_remote_server(None, &typed, cx).is_err());
        assert!(model.settings.servers.is_empty());
    });
    assert!(listed(&path).is_empty(), "saving again can't add it twice");
}

#[gpui::test]
fn the_remote_popover_opens_above_the_section_from_its_left_edge(cx: &mut TestAppContext) {
    let remote = ServerId::new();
    let (mut boot, _, _) = boot_with(vec![entry(remote, "box")]);
    boot.settings.appearance.sidebar_expanded = true;
    let (_view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    click(cx, "remote-servers-section");
    let section = cx.debug_bounds("remote-servers-section").expect("section");
    let popover = cx.debug_bounds("remote-servers-popover").expect("popover");
    // Popovers keep 8px from the window's edge.
    assert_eq!(popover.left(), section.left().max(px(8.0)));
    assert!(popover.bottom() <= section.top());
}

#[gpui::test]
fn a_hidden_remote_home_is_never_the_project_shown(cx: &mut TestAppContext) {
    let (view, cx, remote, _remotes) = connected_remote(cx);
    view.update(cx, |model, cx| {
        let home = model.state.server_home(remote).expect("Home").id;
        let local_home = model.state.home().id;
        model.select_project(home, cx);
        assert_eq!(model.state.current_project().id, local_home);
        model
            .state
            .select_project(home)
            .expect("remembered from before");
        model.sync_visible(cx);
        assert_eq!(model.state.current_project().id, local_home);
        model.new_tab(cx);
        let pane = model.active_pane().expect("pane");
        assert_eq!(
            model.state.pane_server(pane),
            Some(ServerId::local()),
            "a new tab opens on this computer"
        );
    });
}

#[gpui::test]
fn the_remote_picker_refuses_a_folder_its_listing_shows_is_missing(cx: &mut TestAppContext) {
    let (view, cx, remote, _remotes) = connected_remote(cx);
    let before = view.read_with(cx, |model, _| model.state.projects().len());
    let folders = RemoteFolders::new("box".into(), "/home/dev/Home", |path| match path {
        "/home/dev/Home" => Ok(vec![DirectoryItem::Directory("code".into())]),
        _ => Ok(Vec::new()),
    });
    view.update(cx, |model, cx| {
        let picker = cx.new(|cx| {
            ProjectPicker::remote(folders, vec![], model.theme.clone(), model.metrics, cx)
        });
        model.show_project_picker(remote, picker, cx);
    });
    settle(cx);
    cx.simulate_input("nope");
    settle(cx);
    cx.simulate_keystrokes("alt-enter");
    settle(cx);
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Projects(_))));
        assert_eq!(model.state.projects().len(), before, "nothing was added");
    });
}
