use super::*;

#[gpui::test]
fn extension_permissions_shortcuts_and_toolbar_follow_enable_disable_and_unload(
    cx: &mut TestAppContext,
) {
    let package = tempfile::tempdir().expect("extension folder");
    std::fs::write(
        package.path().join("package.json"),
        r#"{
        "name":"reader","version":"1.0.0","muxy":{
            "permissions":["files:read"],
            "commands":[{"id":"read","title":"Read file","defaultShortcut":"cmd+e"}],
            "topbarItems":[{"id":"read","icon":{"symbol":"folder"},"command":"read"}]
        }
    }"#,
    )
    .expect("manifest");
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    boot.settings.appearance.layout = muxy_app_core::settings::AppLayout::TabFocused;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    finish_extension(
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        cx,
    )
    .expect("load");
    view.update(cx, |model, _| {
        assert!(
            model
                .authorize_page("reader", "files.read", &serde_json::Value::Null)
                .is_err()
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("extension-toolbar-item").is_none());
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("reader", true, cx)
        }),
        cx,
    )
    .expect("enable");
    view.update(cx, |model, _| {
        assert!(
            model
                .authorize_page("reader", "files.read", &serde_json::Value::Null)
                .is_ok()
        );
        assert!(
            model
                .authorize_page("reader", "files.write", &serde_json::Value::Null)
                .is_err()
        );
        assert!(
            model
                .authorize_page("reader", "exec.start", &serde_json::Value::Null)
                .is_err()
        );
        assert_eq!(model.extension_keystrokes().len(), 1);
    });
    cx.run_until_parked();
    let button = cx
        .debug_bounds("extension-toolbar-item")
        .expect("enabled extension toolbar button in tab-focused layout");
    assert!(button.size.width > px(0.0) && button.size.height > px(0.0));
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("reader", false, cx)
        }),
        cx,
    )
    .expect("disable");
    view.update(cx, |model, _| {
        assert!(
            model
                .authorize_page("reader", "files.read", &serde_json::Value::Null)
                .is_err()
        );
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

#[gpui::test]
fn installed_registry_refresh_finishes_after_the_package_is_available(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let directory = view.read_with(cx, |model, _| model.extensions.registry.directory());
    let package = directory.join("reader");
    std::fs::create_dir_all(&package).expect("package folder");
    std::fs::write(
        package.join("package.json"),
        r#"{"name":"reader","version":"1.0.0","muxy":{"permissions":["files:read"]}}"#,
    )
    .expect("manifest");
    finish_extension(
        view.update(cx, AppModel::refresh_installed_extensions_task),
        cx,
    )
    .expect("refresh installed extensions");
    view.read_with(cx, |model, _| {
        let registry = &model.extensions.registry;
        assert!(registry.extensions.contains_key("reader"));
        assert!(registry.enabled("reader").is_none());
    });
}
