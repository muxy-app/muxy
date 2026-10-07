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
