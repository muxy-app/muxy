use super::*;
use std::sync::{Arc, Mutex};

const HOST_PROMPT: &str = "The authenticity of host 'box (192.0.2.1)' can't be established.\nED25519 key fingerprint is SHA256:original.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";

#[gpui::test]
fn the_form_offers_host_trust_and_retests_only_after_approval(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let answers = Arc::new(Mutex::new(Vec::new()));
    let recorded = answers.clone();
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot.workers.with_probe(move |target| {
        let (socket, token) = askpass_of(target.askpass().expect("host key askpass"));
        let answer = ask(&socket, &token, HOST_PROMPT).expect("host key answer");
        recorded.lock().expect("answers").push(answer.clone());
        if answer == "yes" {
            Err(crate::boot::ProbeFailure::NotInstalled(
                "Missing Muxy".into(),
            ))
        } else {
            Err("Host key verification failed".into())
        }
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.open_server_form(None, cx);
        model.set_server_identity(std::path::Path::new("/tmp/key"), cx);
    });
    cx.run_until_parked();
    cx.simulate_input("box");
    click(cx, "server-test");
    assert_eq!(*answers.lock().expect("answers"), [""]);
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.server_probe("box"),
            Some(&ConnectionTest::UntrustedHost(HOST_PROMPT.into()))
        );
        assert!(model.settings.servers.is_empty());
    });
    click(cx, "server-trust");
    assert_eq!(*answers.lock().expect("answers"), ["", "yes"]);
    view.read_with(cx, |model, _| {
        assert!(matches!(
            model.server_probe("box"),
            Some(ConnectionTest::NotInstalled(_))
        ));
        assert!(model.settings.servers.is_empty());
    });
    assert!(cx.debug_bounds("server-install").is_some());
}

#[gpui::test]
fn a_different_fingerprint_requires_new_approval_and_edits_revoke_the_offer(
    cx: &mut TestAppContext,
) {
    let (_directory, socket) = short_socket();
    let prompt = Arc::new(Mutex::new(HOST_PROMPT.to_owned()));
    let offered = prompt.clone();
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot.workers.with_probe(move |target| {
        let (socket, token) = askpass_of(target.askpass().expect("askpass"));
        assert_eq!(
            ask(&socket, &token, &offered.lock().expect("prompt")).expect("answer"),
            ""
        );
        Err("Host key verification failed".into())
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.open_server_form(None, cx);
        model.test_remote_server(None, &form("", "box"), cx);
    });
    cx.run_until_parked();
    let replacement = HOST_PROMPT.replace("SHA256:original", "SHA256:different");
    *prompt.lock().expect("prompt") = replacement.clone();
    view.update(cx, |model, cx| {
        model.trust_test_server(None, &form("", "box"), cx);
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(
            model.server_probe("box"),
            Some(&ConnectionTest::UntrustedHost(replacement))
        );
        model.set_server_identity(std::path::Path::new("/tmp/key"), cx);
        model.trust_test_server(None, &form("", "box"), cx);
        assert!(model.server_probe.is_none());
    });
}

#[gpui::test]
fn a_changed_known_host_key_is_not_offered_for_trust(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot
        .workers
        .with_probe(|_| Err("The SSH host key for box changed".into()));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.test_remote_server(None, &form("", "box"), cx);
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        model.trust_test_server(None, &form("", "box"), cx);
        assert_eq!(
            model.server_probe("box"),
            Some(&ConnectionTest::Failed(
                "The SSH host key for box changed".into()
            ))
        );
    });
}

#[gpui::test]
fn manual_identity_is_preserved_when_the_destination_changes(cx: &mut TestAppContext) {
    let (boot, _, _) = boot_with(vec![]);
    cx.update(|cx| cx.bind_keys(muxy_ui::text_input::key_bindings()));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(900.0)));
    view.update(cx, |model, cx| model.open_server_form(None, cx));
    click(cx, "settings-field-server-identity");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("/tmp/chosen-key");
    click(cx, "settings-field-server-host");
    cx.simulate_input("other");
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let saved = model.settings.servers.last().expect("saved");
        assert_eq!(saved.identity_file.as_deref(), Some("/tmp/chosen-key"));
        assert_eq!(saved.ssh, "other");
    });
}

#[gpui::test]
fn server_form_tabs_through_visible_fields_in_both_directions(cx: &mut TestAppContext) {
    let (boot, _, _) = boot_with(vec![]);
    cx.update(|cx| {
        crate::views::workspace::bind_keys(&boot.settings.keymap, cx);
        cx.bind_keys(muxy_ui::text_input::key_bindings());
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(900.0)));
    view.update(cx, |model, cx| model.open_server_form(None, cx));
    cx.run_until_parked();
    cx.simulate_input("box");
    cx.simulate_keystrokes("shift-tab");
    cx.simulate_input("Build Server");
    cx.simulate_keystrokes("tab tab");
    cx.simulate_input("dev");
    cx.simulate_keystrokes("tab");
    cx.simulate_input("2222");
    cx.simulate_keystrokes("tab cmd-a");
    cx.simulate_input("/tmp/muxy-test-key");
    cx.simulate_keystrokes("tab cmd-a");
    cx.simulate_input("Renamed Server");
    cx.simulate_keystrokes("shift-tab shift-tab cmd-a");
    cx.simulate_input("2223");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let remote = view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        let entry = model.settings.servers.last().expect("saved server");
        assert_eq!(entry.name, "Renamed Server");
        assert_eq!(entry.ssh, "ssh://dev@box:2223");
        assert_eq!(entry.identity_file.as_deref(), Some("/tmp/muxy-test-key"));
        entry.id
    });

    let (_directory, socket) = short_socket();
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.open_server_form(Some(remote), cx);
    });
    click(cx, "settings-toggle-server-password-login");
    click(cx, "settings-field-server-identity");
    cx.simulate_keystrokes("tab");
    cx.simulate_input("test-password");
    cx.simulate_keystrokes("tab cmd-a");
    cx.simulate_input("Password Server");
    cx.simulate_keystrokes("shift-tab enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        let entry = model.settings.server(remote).expect("edited server");
        assert_eq!(entry.name, "Password Server");
        assert!(entry.password_login);
        assert!(model.servers.passwords.has(remote));
    });
}

#[gpui::test]
fn missing_muxy_is_reported_in_the_unsaved_form_and_edits_clear_it(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot.workers.with_probe(|_| {
        Err(crate::boot::ProbeFailure::NotInstalled(
            "Muxy is not installed".into(),
        ))
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(900.0)));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.open_server_form(None, cx);
        model.set_server_identity(std::path::Path::new("/tmp/key"), cx);
    });
    cx.run_until_parked();
    cx.simulate_input("box");
    click(cx, "server-test");
    assert!(cx.debug_bounds("server-install").is_some());
    view.read_with(cx, |model, _| {
        assert!(model.settings.servers.is_empty());
        assert!(matches!(
            model.server_probe("box"),
            Some(ConnectionTest::NotInstalled(_))
        ));
    });
    click(cx, "settings-field-server-user");
    cx.simulate_input("dev");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.server_probe.is_none()));
    click(cx, "server-test");
    assert!(cx.debug_bounds("server-install").is_some());
    click(cx, "settings-toggle-server-password-login");
    view.read_with(cx, |model, _| assert!(model.server_probe.is_none()));
    view.update(cx, |model, cx| {
        model.test_remote_server(None, &form("", "dev@box"), cx);
        model.dismiss_overlay(cx);
        model.open_server_form(None, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.server_probe.is_none()));
}

#[gpui::test]
fn a_new_probe_ignores_an_old_result_for_the_same_host(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot.workers.with_probe(|target| {
        let command = target.command("true").expect("ssh command");
        if command.get_args().any(|arg| arg == "/tmp/key") {
            Ok("2.1.0".into())
        } else {
            Err(crate::boot::ProbeFailure::NotInstalled("Missing".into()))
        }
    });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.test_remote_server(None, &form("", "box"), cx);
        model.test_remote_server(
            None,
            &DeviceForm {
                identity_file: "/tmp/key".into(),
                ..form("", "box")
            },
            cx,
        );
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.server_probe("box"),
            Some(&ConnectionTest::Succeeded("2.1.0".into()))
        );
    });
}

#[gpui::test]
fn browsing_for_a_key_clears_completed_and_pending_connection_results(cx: &mut TestAppContext) {
    let (_directory, socket) = short_socket();
    let (mut boot, _, _) = boot_with(vec![]);
    boot.workers = boot
        .workers
        .with_probe(|_| Err(crate::boot::ProbeFailure::NotInstalled("Missing".into())));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.open_server_form(None, cx);
        model.test_remote_server(None, &form("", "box"), cx);
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert!(matches!(
            model.server_probe("box"),
            Some(ConnectionTest::NotInstalled(_))
        ));
        model.set_server_identity(std::path::Path::new("/tmp/key-b"), cx);
        assert!(model.server_probe.is_none());
        model.test_remote_server(None, &form("", "box"), cx);
        model.set_server_identity(std::path::Path::new("/tmp/key-c"), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.server_probe.is_none()));
}

#[gpui::test]
fn an_unsaved_server_install_can_retry_and_retests_with_temporary_credentials(
    cx: &mut TestAppContext,
) {
    let (_directory, socket) = short_socket();
    let ran = Arc::new(Mutex::new(Vec::new()));
    let outcome = Arc::new(Mutex::new(Err("download failed".to_owned())));
    let (mut boot, _, remotes) = boot_with(vec![]);
    let (log, result) = (ran.clone(), outcome.clone());
    boot.workers = boot
        .workers
        .with_run(move |target, command| {
            assert_eq!(target.destination(), "ssh://dev@box:2222");
            let ssh = target.command("true").expect("ssh command");
            let args: Vec<_> = ssh.get_args().collect();
            assert!(args.windows(2).any(|args| args == ["-i", "/tmp/key"]));
            let (socket, token) = askpass_of(target.askpass().expect("password login"));
            assert_eq!(
                ask(&socket, &token, "dev@box's password: ").expect("password"),
                "secret"
            );
            log.lock()
                .expect("log")
                .push((socket, token, command.to_owned()));
            result.lock().expect("result").clone()
        })
        .with_probe(|target| {
            let (socket, token) = askpass_of(target.askpass().expect("password login"));
            assert_eq!(
                ask(&socket, &token, "dev@box's password: ").expect("password"),
                "secret"
            );
            Ok("2.1.0".into())
        });
    let path = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let submitted = DeviceForm {
        user: "dev".into(),
        port: "2222".into(),
        identity_file: "/tmp/key".into(),
        password_login: true,
        password: "secret".into(),
        ..form("Build", "box")
    };
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.open_server_form(None, cx);
        model.install_test_server(
            None,
            &submitted,
            crate::remote_install::Source::Release("2.1.0".into()),
            cx,
        );
        assert_eq!(
            model.server_probe("ssh://dev@box:2222"),
            Some(&ConnectionTest::Installing)
        );
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert_eq!(
            model.server_probe("ssh://dev@box:2222"),
            Some(&ConnectionTest::InstallFailed("download failed".into()))
        );
        *outcome.lock().expect("outcome") = Ok(String::new());
        model.install_test_server(
            None,
            &submitted,
            crate::remote_install::Source::Release("2.1.0".into()),
            cx,
        );
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.server_probe("ssh://dev@box:2222"),
            Some(&ConnectionTest::Succeeded("2.1.0".into()))
        );
        assert!(matches!(model.overlay, Some(Overlay::ServerForm(_))));
        assert!(model.settings.servers.is_empty());
        assert!(listed(&path).is_empty());
        assert!(remotes.borrow().is_empty());
    });
    let ran = ran.lock().expect("ran");
    assert_eq!(ran.len(), 2);
    for (socket, token, command) in ran.iter() {
        assert!(command.contains("install-muxy.sh | sh -s -- --version 2.1.0"));
        assert_eq!(
            ask(socket, token, "Password: ").expect("expired password"),
            ""
        );
    }
}

#[gpui::test]
fn development_install_button_uploads_only_after_confirmation_and_retests(cx: &mut TestAppContext) {
    use std::sync::atomic::{AtomicBool, Ordering};

    let (_directory, socket) = short_socket();
    let installed = Arc::new(AtomicBool::new(false));
    let (uploaded, ready) = (installed.clone(), installed.clone());
    let (mut boot, _, remotes) = boot_with(vec![]);
    boot.workers = boot
        .workers
        .with_run_input(move |target, command, input| {
            assert_eq!(target.destination(), "box");
            assert!(command.ends_with("muxy-install --dev-archive"));
            assert!(
                input
                    .expect("archive")
                    .ends_with("target/linux-dev/muxy-dev-linux-x86_64.tar.gz")
            );
            uploaded.store(true, Ordering::SeqCst);
            Ok(String::new())
        })
        .with_probe(move |_| {
            if ready.load(Ordering::SeqCst) {
                Ok("development".into())
            } else {
                Err(crate::boot::ProbeFailure::NotInstalled("Missing".into()))
            }
        });
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    view.update(cx, |model, cx| {
        model.servers.passwords.set_socket(socket);
        model.open_server_form(None, cx);
        model.set_server_identity(std::path::Path::new("/tmp/key"), cx);
    });
    cx.run_until_parked();
    cx.simulate_input("box");
    click(cx, "server-test");
    click(cx, "server-install");
    assert!(!installed.load(Ordering::SeqCst));
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(!installed.load(Ordering::SeqCst));
    click(cx, "server-install");
    view.update(cx, |model, cx| {
        model.set_server_identity(std::path::Path::new("/tmp/other-key"), cx);
    });
    cx.simulate_prompt_answer("Install");
    cx.run_until_parked();
    assert!(!installed.load(Ordering::SeqCst));
    click(cx, "server-test");
    click(cx, "server-install");
    cx.simulate_prompt_answer("Install");
    cx.run_until_parked();
    assert!(installed.load(Ordering::SeqCst));
    redraw(cx);
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.server_probe("box"),
            Some(&ConnectionTest::Succeeded("development".into()))
        );
        assert!(matches!(model.overlay, Some(Overlay::ServerForm(_))));
        assert!(model.settings.servers.is_empty());
        assert!(remotes.borrow().is_empty());
    });
}

#[gpui::test]
fn remote_badges_do_not_overlap_focused_sidebar_controls(cx: &mut TestAppContext) {
    use muxy_app_core::settings::AppLayout;
    let (view, cx, remote, _) = connected_remote(cx);
    let project = ProjectId::new();
    view.update(cx, |model, cx| {
        let home = model.state.server_home(remote).expect("home").id;
        let mut catalog = page(2, home, &[(project, "remote-project")]);
        catalog.revision = 2;
        model.receive((remote, 1, Update::Catalog(Ok(catalog))), cx);
        model.select_project(project, cx);
    });
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    for layout in [AppLayout::TabFocused, AppLayout::AgentsFocused] {
        for focused in [false, true] {
            view.update(cx, |model, cx| {
                model.appearance.layout = layout;
                model.appearance.sidebar_focus = focused;
                cx.notify();
            });
            redraw(cx);
            let header = cx
                .debug_bounds(selector(format!("tab-project-{project}")))
                .expect("header");
            cx.simulate_event(gpui::MouseMoveEvent {
                position: header.center(),
                ..Default::default()
            });
            redraw(cx);
            let badge = cx
                .debug_bounds(selector(format!("project-remote-{project}")))
                .expect("badge");
            let add = cx
                .debug_bounds(selector(format!("project-new-tab-{project}")))
                .expect("new tab");
            assert!(add.right() <= badge.left(), "{layout:?}, focused={focused}");
            assert!(badge.right() <= header.right());
        }
    }
}
