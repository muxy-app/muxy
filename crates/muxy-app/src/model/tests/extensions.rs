use super::*;

#[gpui::test]
fn extensions_request_server_client_on_initial_connection_and_reconnect(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        for generation in 1..=2 {
            assert!(model.connection == ConnectionState::Connecting);
            assert_eq!(model.generation, generation);
            requests.try_iter().for_each(drop);
            model.receive((generation, Update::Connected(Vec::new())), cx);
            assert!(model.connection == ConnectionState::Ready);
            assert!(model.error.is_none());
            let clients: Vec<_> = requests
                .try_iter()
                .filter_map(|(request_generation, work)| match work {
                    Work::ExtensionClient(reply) => {
                        assert_eq!(request_generation, generation);
                        Some(reply)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(clients.len(), 1, "extensions must receive the ready client");
            assert!(!clients[0].is_closed());
            if generation == 1 {
                model.disconnect(cx);
                model.connect(cx);
            }
        }
    });
}

#[gpui::test]
fn extension_permissions_and_shortcuts_follow_enable_disable_and_unload(cx: &mut TestAppContext) {
    let package = tempfile::tempdir().expect("extension folder");
    std::fs::write(
        package.path().join("package.json"),
        r#"{
        "name":"reader","version":"1.0.0","muxy":{
            "permissions":["files:read"],
            "commands":[{"id":"read","title":"Read file","defaultShortcut":"cmd+e"}]
        }
    }"#,
    )
    .expect("manifest");
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    finish_extension(
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        cx,
    )
    .expect("load");
    view.update(cx, |model, _| {
        assert!(model.authorize_page("reader", "files.read").is_err());
    });
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("reader", true, cx)
        }),
        cx,
    )
    .expect("enable");
    view.update(cx, |model, _| {
        assert!(model.authorize_page("reader", "files.read").is_ok());
        assert!(model.authorize_page("reader", "files.write").is_err());
        assert!(model.authorize_page("reader", "exec.start").is_err());
        assert_eq!(model.extension_keystrokes().len(), 1);
    });
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("reader", false, cx)
        }),
        cx,
    )
    .expect("disable");
    view.update(cx, |model, _| {
        assert!(model.authorize_page("reader", "files.read").is_err());
        assert!(model.extension_keystrokes().is_empty());
    });
    finish_extension(
        view.update(cx, |model, cx| model.remove_extension("reader", cx)),
        cx,
    )
    .expect("unload");
    view.update(cx, |model, _| {
        assert!(model.extensions.registry.extensions.is_empty());
    });
    assert!(package.path().join("package.json").is_file());
}

pub(in crate::model) fn finish_extension(
    task: Task<std::result::Result<(), String>>,
    cx: &VisualTestContext,
) -> std::result::Result<(), String> {
    let mut task = std::pin::pin!(task);
    let mut poll = std::task::Context::from_waker(std::task::Waker::noop());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if let std::task::Poll::Ready(result) = Future::poll(task.as_mut(), &mut poll) {
            return result;
        }
        if Instant::now() >= deadline {
            return Err("extension operation timed out".into());
        }
        thread::sleep(Duration::from_millis(2));
    }
}
