use super::preferences::settings_window;
use super::servers::{connect_remote, entry, page, serve_remote, work};
use super::*;
use crate::picker::remote::RemoteFolders;
use crate::views::settings::SettingsEvent;
use muxy_app_core::settings::{ServerEntry, Settings};

mod forms;
mod picker;

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

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(125));
    cx.run_until_parked();
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
    picker::check_folder_operations(&folders, directory.path())?;
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
            sandbox: None,
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
