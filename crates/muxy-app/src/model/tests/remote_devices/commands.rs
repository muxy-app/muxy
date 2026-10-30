use super::*;
use std::sync::{Arc, Mutex};

fn custom(command: &str) -> DeviceForm {
    DeviceForm {
        ssh_command: Some(command.into()),
        ..DeviceForm::default()
    }
}

#[gpui::test]
fn custom_probes_and_installation_use_the_same_target_without_app_credentials(
    cx: &mut TestAppContext,
) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let probing = calls.clone();
    let installing = calls.clone();
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot
        .workers
        .with_probe(move |target| {
            assert!(target.is_custom_command());
            assert!(target.askpass().is_none());
            let mut calls = probing.lock().expect("calls");
            calls.push(target.clone());
            if calls.len() == 1 {
                Err(crate::boot::ProbeFailure::NotInstalled(
                    "not installed".into(),
                ))
            } else {
                Ok("2.1.0".into())
            }
        })
        .with_run(move |target, command, archive| {
            let mut calls = installing.lock().expect("calls");
            assert_eq!(calls.first(), Some(target));
            assert!(command.contains("--version 2.1.0"));
            assert!(archive.is_none());
            calls.push(target.clone());
            Ok(String::new())
        });
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let form = DeviceForm {
        host: "ignored".into(),
        user: "ignored".into(),
        port: "invalid".into(),
        identity_file: "/private/ignored".into(),
        password_login: true,
        password: "never forwarded".into(),
        ..custom("vm ssh 'dev box' --")
    };
    view.update(cx, |model, cx| model.test_remote_server(None, &form, cx));
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert!(matches!(
            model.server_probe("vm ssh 'dev box' --"),
            Some(ConnectionTest::NotInstalled(_))
        ));
        model.install_test_server(
            None,
            &form,
            crate::remote_install::Source::Release("2.1.0".into()),
            cx,
        );
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(
            model.server_probe("vm ssh 'dev box' --"),
            Some(&ConnectionTest::Succeeded("2.1.0".into()))
        );
        let server = model.save_remote_server(None, &form, cx).expect("saved");
        let runtime = model.servers.get(server).expect("runtime");
        let calls = calls.lock().expect("calls");
        assert_eq!(calls.len(), 3);
        assert_eq!(
            runtime.target.as_ref(),
            calls
                .first()
                .cloned()
                .map(crate::boot::Target::Ssh)
                .as_ref()
        );
        assert!(!runtime.password_login);
        assert!(!model.servers.passwords.has(server));
    });
    let saved = listed(&path).pop().expect("entry");
    assert!(saved.ssh.is_empty());
    assert!(saved.identity_file.is_none());
    assert!(!saved.password_login);
    assert_eq!(saved.ssh_command, form.ssh_command);
    assert!(
        !std::fs::read_to_string(path)
            .expect("settings")
            .contains("never forwarded")
    );
}

#[gpui::test]
fn editing_a_custom_command_restarts_only_its_worker_and_keeps_its_projects(
    cx: &mut TestAppContext,
) {
    let mut entry = ServerEntry::new("VM".into(), String::new());
    entry.ssh_command = Some("vm ssh one".into());
    let server = entry.id;
    let (boot, _, remotes) = boot_with(vec![entry]);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let first = remotes.borrow_mut().remove(&server).expect("worker");
    view.update(cx, |model, cx| {
        connect_local(model, cx);
        assert_eq!(model.connection(server), ConnectionState::Connecting);
        let home = connect_remote(model, server, vec![], &[], cx);
        work(&first);
        let renamed = DeviceForm {
            name: "Renamed VM".into(),
            ..custom("vm ssh one")
        };
        model
            .save_remote_server(Some(server), &renamed, cx)
            .expect("renamed");
        assert!(model.ready(server));
        assert!(work(&first).is_empty());
        model
            .save_remote_server(Some(server), &custom("vm ssh two"), cx)
            .expect("edited");
        assert_eq!(
            model.state.server_home(server).map(|home| home.id),
            Some(home)
        );
        assert_eq!(model.connection(server), ConnectionState::Connecting);
        model.receive((server, 1, Update::Connected(vec![])), cx);
        assert_eq!(
            model.connection(server),
            ConnectionState::Connecting,
            "old worker cannot reconnect the new target"
        );
    });
    assert!(work(&first).iter().any(|work| matches!(work, Work::Stop)));
    let next = remotes.borrow_mut().remove(&server).expect("new worker");
    assert!(
        next.try_iter()
            .any(|(generation, work)| generation == 3 && matches!(work, Work::Connect))
    );
}

#[gpui::test]
fn the_custom_command_form_saves_and_reopens_without_requiring_a_host(cx: &mut TestAppContext) {
    let (boot, _, _) = boot_with(vec![]);
    cx.update(|cx| cx.bind_keys(muxy_ui::text_input::key_bindings()));
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| model.open_server_form(None, cx));
    click(cx, "settings-toggle-server-custom-command");
    cx.simulate_input("vm ssh 'dev box' --");
    click(cx, "server-save");
    let entry = listed(&path).pop().expect("saved command");
    assert_eq!(entry.ssh_command.as_deref(), Some("vm ssh 'dev box' --"));
    assert!(entry.ssh.is_empty());
    view.update(cx, |model, cx| model.open_server_form(Some(entry.id), cx));
    cx.run_until_parked();
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-a"
    } else {
        "ctrl-a"
    });
    cx.simulate_input("vm ssh 'other box' --");
    click(cx, "server-save");
    let edited = listed(&path).pop().expect("edited command");
    assert_eq!(edited.id, entry.id);
    assert_eq!(edited.ssh_command.as_deref(), Some("vm ssh 'other box' --"));
    assert!(edited.ssh.is_empty());
}
