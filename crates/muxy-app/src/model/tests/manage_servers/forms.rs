use super::*;

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
