use super::*;
use crate::boot::Boot;
use gpui::{TestAppContext, VisualTestContext};
use muxy_app_core::{
    AppState,
    composer::ComposerStore,
    settings::{Settings, TerminalSettings},
};

fn setup(
    cx: &mut TestAppContext,
) -> (
    Entity<AppModel>,
    Entity<ExtensionsView>,
    &mut VisualTestContext,
) {
    setup_in(&tempfile::tempdir().expect("profile").keep(), cx)
}

/// Like `setup`, with the app profile in `directory`.
fn setup_in<'a>(
    directory: &std::path::Path,
    cx: &'a mut TestAppContext,
) -> (
    Entity<AppModel>,
    Entity<ExtensionsView>,
    &'a mut VisualTestContext,
) {
    let (work, _) = std::sync::mpsc::channel();
    let (_, updates) = async_channel::unbounded();
    let boot = Boot {
        import_error: None,
        composer: ComposerStore::load_from(directory),
        state: AppState::bootstrap().expect("state"),
        state_path: directory.join("state.json"),
        settings: Settings::default(),
        terminal: TerminalSettings::default(),
        work,
        workers: crate::boot::Workers::with(|_, _| Ok(std::sync::mpsc::channel().0)),
        updates,
    };
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let view = cx.new(|cx| {
        ExtensionsView::new(
            model.downgrade(),
            model.read(cx).theme.clone(),
            model.read(cx).metrics,
            cx,
        )
    });
    (model, view, cx)
}

#[gpui::test]
fn installed_extension_opens_its_details_with_package_permissions(cx: &mut TestAppContext) {
    let (model, view, cx) = setup(cx);
    let directory = tempfile::tempdir().expect("package");
    std::fs::write(directory.path().join("package.json"), r#"{"name":"reader","version":"1.2.3","muxy":{"description":"Read project files","permissions":["files:read"]}}"#).expect("package");
    let extension = Extension::load(directory.path()).expect("extension");
    model.update(cx, |model, _| {
        model
            .extensions
            .registry
            .extensions
            .insert("reader".into(), extension);
    });
    view.update(cx, |view, cx| {
        view.tab = Tab::Marketplace;
        view.query = "reader".into();
        view.search
            .update(cx, |search, cx| search.set_text("reader", cx));
        view.selected = Some(json!({"name":"reader", "permissions":["exec"]}));
        view.mutation = Some(Mutation::Install);
        view.finish_install("reader", Ok(()), cx);
        assert!(view.tab == Tab::Installed);
        assert!(view.mutation.is_none());
        assert!(view.query.is_empty());
        assert_eq!(view.search.read(cx).text(), "");
        assert!(view.error.is_none());
        let details = view.selected.as_ref().expect("installed detail page");
        assert_eq!(details["name"], "reader");
        assert_eq!(details["version"], "1.2.3");
        assert_eq!(details["permissions"], json!(["files:read"]));
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(view.selected.is_some()));
    model.read_with(cx, |model, _| {
        assert!(model.extensions.registry.enabled("reader").is_none());
    });
}

#[gpui::test]
fn failed_install_keeps_marketplace_details_and_reports_error(cx: &mut TestAppContext) {
    let (_model, view, cx) = setup(cx);
    view.update(cx, |view, cx| {
        let details = json!({"name":"reader", "description":"Read files"});
        view.tab = Tab::Marketplace;
        view.selected = Some(details.clone());
        view.query = "reader".into();
        view.mutation = Some(Mutation::Install);
        assert_eq!(
            view.button_label("extension-install", "Install"),
            "Installing…"
        );
        view.finish_install("reader", Err("Download failed".into()), cx);
        assert!(view.mutation.is_none());
        assert_eq!(view.button_label("extension-install", "Install"), "Install");
        assert!(view.tab == Tab::Marketplace);
        assert_eq!(view.selected, Some(details.clone()));
        assert_eq!(view.query, "reader");
        assert_eq!(view.error.as_deref(), Some("Download failed"));
        view.finish_install("reader", Ok(()), cx);
        assert!(view.tab == Tab::Marketplace);
        assert_eq!(view.selected, Some(details));
        assert!(
            view.error
                .as_deref()
                .expect("load failure")
                .contains("could not be loaded")
        );
    });
}

#[gpui::test]
fn extension_detail_controls_build_without_duplicate_interaction_styles(cx: &mut TestAppContext) {
    let (_model, view, cx) = setup(cx);
    let extension = Extension {
        name: "reader".into(),
        version: "1.0.0".into(),
        directory: std::path::PathBuf::new(),
        manifest: muxy_app_core::extensions::Manifest::default(),
    };
    let details = installed_details(&extension);
    view.update(cx, |view, cx| {
        for installed in [
            Vec::new(),
            vec![(extension.clone(), false, false)],
            vec![(extension.clone(), true, false)],
            vec![(extension.clone(), false, true)],
        ] {
            for mutation in [
                None,
                Some(Mutation::Install),
                Some(Mutation::Update),
                Some(Mutation::Enable),
                Some(Mutation::Disable),
                Some(Mutation::Remove),
                Some(Mutation::ResetPermissions),
                Some(Mutation::RemoveRule),
            ] {
                view.mutation = mutation;
                let _ = view.details_body(&details, &installed, cx);
            }
        }
    });
}

#[test]
fn update_availability_uses_marketplace_metadata_without_replacing_local_permissions() {
    let extension = Extension {
        name: "reader".into(),
        version: "1.0.3".into(),
        directory: std::path::PathBuf::new(),
        manifest: muxy_app_core::extensions::Manifest::default(),
    };
    let remote = json!({"name":"reader", "current_version":"1.0.4", "permissions":["exec"]});
    assert_eq!(available_update(&extension, &remote), Some("1.0.4"));
    assert_eq!(installed_details(&extension)["permissions"], json!([]));
    for details in [
        installed_details(&extension),
        json!({"name":"reader", "current_version":"1.0.3"}),
        json!({"name":"reader", "current_version":"1.0.2"}),
        json!({"name":"other", "current_version":"1.0.4"}),
    ] {
        assert_eq!(available_update(&extension, &details), None);
    }
}

#[gpui::test]
fn update_controls_report_progress_and_keep_retry_details_on_failure(cx: &mut TestAppContext) {
    let (_model, view, cx) = setup(cx);
    view.update(cx, |view, cx| {
        view.selected = Some(json!({"name":"reader", "current_version":"1.0.4"}));
        view.loading = Some(Loading::Details("reader".into()));
        assert_eq!(
            view.button_label("extension-check-update", "Check for updates"),
            "Checking…"
        );
        view.loading = None;
        view.mutation = Some(Mutation::Update);
        assert_eq!(view.button_label("extension-update", "Update"), "Updating…");
        view.finish_install("reader", Err("Download failed".into()), cx);
        assert_eq!(view.button_label("extension-update", "Update"), "Update");
        assert_eq!(view.selected.as_ref().unwrap()["current_version"], "1.0.4");
        assert_eq!(view.error.as_deref(), Some("Download failed"));
    });
}

fn finish_mutation(view: &Entity<ExtensionsView>, cx: &VisualTestContext) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if view.read_with(cx, |view, _| view.mutation.is_none()) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "extension action timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[gpui::test]
fn extension_actions_report_progress_until_the_registry_is_updated(cx: &mut TestAppContext) {
    let (model, view, cx) = setup(cx);
    let package = model.read_with(cx, |model, _| {
        model.extensions.registry.directory().join("reader")
    });
    std::fs::create_dir_all(&package).expect("package folder");
    std::fs::write(
        package.join("package.json"),
        r#"{"name":"reader","version":"1.0.0","muxy":{}}"#,
    )
    .expect("manifest");

    view.update(cx, |view, cx| {
        view.error = Some("Previous failure".into());
        view.reload(cx);
        assert!(view.error.is_none());
        assert_eq!(
            view.button_label("extension-reload", "Reload"),
            "Reloading…"
        );
    });
    finish_mutation(&view, cx);
    view.read_with(cx, |view, _| {
        assert!(view.error.is_none());
        assert_eq!(view.button_label("extension-reload", "Reload"), "Reloaded");
    });
    model.read_with(cx, |model, _| {
        assert!(model.extensions.registry.extensions.contains_key("reader"));
    });

    for (enabled, label) in [(true, "Enabling…"), (false, "Disabling…")] {
        view.update(cx, |view, cx| {
            view.enable("reader", enabled, cx);
            view.enable("reader", !enabled, cx);
            assert_eq!(view.button_label("extension-enable", "Enable"), label);
            assert_eq!(
                view.button_label("extension-remove", "Uninstall"),
                "Uninstall"
            );
        });
        finish_mutation(&view, cx);
        view.read_with(cx, |view, _| assert!(view.error.is_none()));
        model.read_with(cx, |model, _| {
            assert_eq!(
                model.extensions.registry.enabled("reader").is_some(),
                enabled
            );
        });
    }

    view.update(cx, |view, cx| {
        view.reset_permissions("reader", cx);
        assert_eq!(
            view.button_label("extension-reset-permissions", "Clear all"),
            "Clearing…"
        );
    });
    finish_mutation(&view, cx);
    view.read_with(cx, |view, _| {
        assert!(view.error.is_none());
        assert_eq!(
            view.button_label("extension-reset-permissions", "Clear all"),
            "Cleared"
        );
    });

    view.update(cx, |view, cx| {
        view.selected = Some(json!({"name":"reader"}));
        view.remove("reader", cx);
        assert_eq!(
            view.button_label("extension-remove", "Uninstall"),
            "Uninstalling…"
        );
    });
    finish_mutation(&view, cx);
    view.read_with(cx, |view, _| {
        assert!(view.error.is_none());
        assert!(view.selected.is_none());
    });
    model.read_with(cx, |model, _| {
        assert!(!model.extensions.registry.extensions.contains_key("reader"));
    });
}

#[gpui::test]
fn failed_enable_restores_the_button_and_keeps_details_open(cx: &mut TestAppContext) {
    let (_model, view, cx) = setup(cx);
    view.update(cx, |view, cx| {
        view.selected = Some(json!({"name":"missing"}));
        view.enable("missing", true, cx);
        assert_eq!(view.button_label("extension-enable", "Enable"), "Enabling…");
    });
    finish_mutation(&view, cx);
    view.read_with(cx, |view, _| {
        assert!(view.error.is_some());
        assert!(view.selected.is_some());
        assert_eq!(view.button_label("extension-enable", "Enable"), "Enable");
    });
}

#[gpui::test]
fn remembered_rules_show_on_the_details_page_and_can_be_removed(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("profile").keep();
    std::fs::write(
        directory.join("extension-grants.json"),
        r#"{"reader:exec:argv:git":"Allow","reader:http.fetch:*":"Blocked","other:exec:argv:ls":"Deny"}"#,
    )
    .expect("grants");
    let (model, view, cx) = setup_in(&directory, cx);
    let package = model.read_with(cx, |model, _| {
        model.extensions.registry.directory().join("reader")
    });
    std::fs::create_dir_all(&package).expect("package folder");
    std::fs::write(
        package.join("package.json"),
        r#"{"name":"reader","version":"1.0.0","muxy":{}}"#,
    )
    .expect("manifest");
    view.update(cx, ExtensionsView::reload);
    finish_mutation(&view, cx);
    let rules = model.read_with(cx, |model, _| model.extension_rules("reader"));
    assert_eq!(
        rules.iter().map(Rule::id).collect::<Vec<_>>(),
        ["exec:argv:git", "http.fetch:*"]
    );
    let extension = model.read_with(cx, |model, _| {
        model.extensions.registry.extensions["reader"].clone()
    });
    view.update(cx, |view, cx| {
        let _ = view.details_body(
            &json!({"name":"reader"}),
            &[(extension.clone(), true, false)],
            cx,
        );
        view.remove_rule("reader", rules[0].clone(), cx);
        assert_eq!(view.mutation, Some(Mutation::RemoveRule));
    });
    finish_mutation(&view, cx);
    view.read_with(cx, |view, _| assert!(view.error.is_none()));
    model.read_with(cx, |model, _| {
        assert_eq!(model.extension_rules("reader"), [rules[1].clone()]);
        assert_eq!(
            model.extension_rules("other").len(),
            1,
            "other extensions keep their rules"
        );
    });
}

fn batch_package(root: &std::path::Path, name: &str, version: &str, permissions: &[&str]) {
    std::fs::create_dir_all(root).expect("package folder");
    std::fs::write(
        root.join("package.json"),
        json!({
            "name":name, "version":version, "muxy":{"permissions":permissions}
        })
        .to_string(),
    )
    .expect("manifest");
}

fn batch_profile() -> tempfile::TempDir {
    let profile = tempfile::tempdir().expect("profile");
    for (name, version) in [
        ("a-fail", "1.0.0"),
        ("b-update", "1.0.0"),
        ("c-disabled", "1.0.0"),
        ("current", "1.0.0"),
        ("newer", "3.0.0"),
        ("unpublished", "1.0.0"),
    ] {
        batch_package(
            &profile.path().join("extensions").join(name),
            name,
            version,
            &[],
        );
    }
    let dev = profile.path().join("developer");
    batch_package(&dev, "dev", "1.0.0", &[]);
    std::fs::write(
        profile.path().join("extension-folders.json"),
        json!({"dev":dev}).to_string(),
    )
    .expect("unpacked");
    std::fs::write(
        profile.path().join("extension-enabled.json"),
        r#"["a-fail","b-update"]"#,
    )
    .expect("enabled");
    profile
}

fn batch_versions(names: &[String]) -> std::collections::BTreeMap<String, String> {
    assert_eq!(
        names,
        [
            "a-fail",
            "b-update",
            "c-disabled",
            "current",
            "newer",
            "unpublished"
        ]
    );
    [
        ("a-fail", "2.0.0"),
        ("b-update", "2.0.0"),
        ("c-disabled", "2.0.0"),
        ("current", "1.0.0"),
        ("newer", "2.0.0"),
    ]
    .into_iter()
    .map(|(name, version)| (name.into(), version.into()))
    .collect()
}

#[gpui::test]
fn update_all_skips_unpacked_and_current_packages_and_continues_after_a_failure(
    cx: &mut TestAppContext,
) {
    let profile = batch_profile();
    let (model, view, cx) = setup_in(profile.path(), cx);
    view.update(cx, ExtensionsView::reload);
    finish_mutation(&view, cx);
    let downloaded = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let calls = downloaded.clone();
    view.update(cx, |view, cx| {
        view.query = "b-update".into();
        view.update_all_with(
            |names| Ok(batch_versions(names)),
            move |extension, directory| {
                calls.lock().expect("calls").push(extension.name.clone());
                if extension.name == "a-fail" {
                    return Err("Download failed".into());
                }
                if extension.name == "c-disabled" {
                    assert_eq!(
                        Extension::load(&directory.join("b-update"))
                            .expect("earlier update applied")
                            .version,
                        "2.0.0"
                    );
                }
                let stage = tempfile::tempdir_in(directory).expect("stage");
                let permissions = if extension.name == "b-update" {
                    vec!["files:write"]
                } else {
                    Vec::new()
                };
                batch_package(
                    &stage.path().join("package"),
                    &extension.name,
                    "2.0.0",
                    &permissions,
                );
                Ok(stage)
            },
            cx,
        );
        assert_eq!(
            view.button_label("extension-update-all", "Update all"),
            "Updating…"
        );
        assert_eq!(
            view.update_all_status.as_deref(),
            Some("Checking for extension updates…")
        );
        view.update_all_with(
            |_| panic!("duplicate check"),
            |_, _| panic!("duplicate download"),
            cx,
        );
        view.switch_tab(Tab::Marketplace, cx);
        assert!(view.tab == Tab::Installed);
    });
    finish_mutation(&view, cx);
    assert_eq!(
        *downloaded.lock().expect("calls"),
        ["a-fail", "b-update", "c-disabled"]
    );
    model.read_with(cx, |model, _| {
        let registry = &model.extensions.registry;
        for (name, version) in [
            ("a-fail", "1.0.0"),
            ("b-update", "2.0.0"),
            ("c-disabled", "2.0.0"),
            ("current", "1.0.0"),
            ("newer", "3.0.0"),
            ("unpublished", "1.0.0"),
            ("dev", "1.0.0"),
        ] {
            assert_eq!(registry.extensions[name].version, version);
        }
        assert!(registry.enabled("a-fail").is_some());
        assert!(registry.enabled("b-update").is_none());
        assert!(registry.enabled("c-disabled").is_none());
    });
    view.read_with(cx, |view, _| {
        assert_eq!(view.error.as_deref(), Some("a-fail: Download failed"));
        assert_eq!(
            view.update_all_status.as_deref(),
            Some("Updated 2 extensions. Review permissions and enable when ready: b-update.")
        );
        assert_eq!(
            view.button_label("extension-update-all", "Update all"),
            "Update all"
        );
        assert_eq!(view.query, "b-update");
        assert!(view.selected.is_none());
    });
}

#[gpui::test]
fn update_all_reports_check_failures_and_can_retry_when_everything_is_current(
    cx: &mut TestAppContext,
) {
    let profile = tempfile::tempdir().expect("profile");
    batch_package(
        &profile.path().join("extensions/reader"),
        "reader",
        "1.0.0",
        &[],
    );
    let (_model, view, cx) = setup_in(profile.path(), cx);
    view.update(cx, ExtensionsView::reload);
    finish_mutation(&view, cx);
    view.update(cx, |view, cx| {
        view.update_all_with(
            |_| Err("Offline".into()),
            |_, _| panic!("no download without versions"),
            cx,
        );
    });
    finish_mutation(&view, cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.error.as_deref(), Some("Offline"));
        assert!(view.update_all_status.is_none());
        assert_eq!(
            view.button_label("extension-update-all", "Update all"),
            "Update all"
        );
    });
    view.update(cx, |view, cx| {
        view.update_all_with(
            |_| Ok([("reader".into(), "1.0.0".into())].into()),
            |_, _| panic!("current extension must not download"),
            cx,
        );
        assert!(view.error.is_none());
    });
    finish_mutation(&view, cx);
    view.read_with(cx, |view, _| {
        assert!(view.error.is_none());
        assert_eq!(
            view.update_all_status.as_deref(),
            Some("All extensions are up to date.")
        );
    });
}

#[gpui::test]
fn update_all_does_not_check_an_empty_registry_or_interrupt_a_pending_request(
    cx: &mut TestAppContext,
) {
    let (_model, view, cx) = setup(cx);
    view.update(cx, |view, cx| {
        view.update_all_with(
            |_| panic!("empty registry"),
            |_, _| panic!("empty registry"),
            cx,
        );
        assert!(view.mutation.is_none());
        view.loading = Some(Loading::Search);
        view.update_all_with(|_| panic!("busy view"), |_, _| panic!("busy view"), cx);
        assert_eq!(view.loading, Some(Loading::Search));
        assert!(view.mutation.is_none());
    });
}

fn open_extension_settings(
    model: &Entity<AppModel>,
    cx: &mut VisualTestContext,
) -> Entity<ExtensionsView> {
    cx.update(|window, cx| model.update(cx, |model, cx| model.open_settings(window, cx)));
    model.read_with(cx, |model, cx| {
        model
            .settings_window
            .as_ref()
            .expect("settings")
            .view
            .read(cx)
            .extensions
            .clone()
            .expect("extensions")
    })
}

fn close_extension_settings(model: &Entity<AppModel>, cx: &mut VisualTestContext) {
    let window = model.read_with(cx, |model, _| {
        model.settings_window.as_ref().expect("settings").window
    });
    window
        .update(cx, |_, window, cx| {
            model.update(cx, AppModel::settings_closed);
            window.remove_window();
        })
        .expect("close settings");
}

#[gpui::test]
fn update_all_preserves_busy_state_and_results_across_settings_close_and_reopen(
    cx: &mut TestAppContext,
) {
    let profile = tempfile::tempdir().expect("profile");
    batch_package(
        &profile.path().join("extensions/reader"),
        "reader",
        "1.0.0",
        &[],
    );
    let (model, unused, cx) = setup_in(profile.path(), cx);
    drop(unused);
    let view = open_extension_settings(&model, cx);
    view.update(cx, ExtensionsView::reload);
    finish_mutation(&view, cx);
    let (entered, started) = async_channel::bounded(1);
    let (release, resume) = async_channel::bounded(1);
    view.update(cx, |view, cx| {
        view.update_all_with(
            |_| Ok([("reader".into(), "2.0.0".into())].into()),
            move |_, _| {
                entered.try_send(()).expect("started");
                resume.recv_blocking().map_err(|error| error.to_string())?;
                Err("Download failed".into())
            },
            cx,
        );
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while started.try_recv().is_err() {
        cx.run_until_parked();
        assert!(std::time::Instant::now() < deadline, "download started");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let weak = view.downgrade();
    let id = view.entity_id();
    drop(view);
    close_extension_settings(&model, cx);
    cx.run_until_parked();
    let reopened = open_extension_settings(&model, cx);
    assert_eq!(reopened.entity_id(), id);
    reopened.update(cx, |view, cx| {
        assert!(view.updating_all());
        assert_eq!(
            view.update_all_status.as_deref(),
            Some("Updating reader (1 of 1)…")
        );
        view.update_all_with(
            |_| panic!("duplicate batch"),
            |_, _| panic!("duplicate download"),
            cx,
        );
        view.reload(cx);
        assert_eq!(view.mutation, Some(Mutation::UpdateAll));
    });
    drop(reopened);
    close_extension_settings(&model, cx);
    release.try_send(()).expect("release download");
    let retained = weak.upgrade().expect("retained while closed");
    finish_mutation(&retained, cx);
    drop(retained);
    let reopened = open_extension_settings(&model, cx);
    assert_eq!(reopened.entity_id(), id);
    reopened.read_with(cx, |view, _| {
        assert!(!view.updating_all());
        assert_eq!(view.error.as_deref(), Some("reader: Download failed"));
        assert_eq!(
            view.update_all_status.as_deref(),
            Some("Updated 0 extensions.")
        );
    });
    drop(reopened);
    close_extension_settings(&model, cx);
    cx.run_until_parked();
    assert!(
        weak.upgrade().is_none(),
        "finished view is released after its result was shown"
    );
}
