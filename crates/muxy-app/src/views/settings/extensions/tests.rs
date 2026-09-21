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
    let directory = tempfile::tempdir().expect("profile").keep();
    let (work, _) = std::sync::mpsc::channel();
    let (_, updates) = async_channel::unbounded();
    let boot = Boot {
        composer: ComposerStore::load_from(&directory),
        state: AppState::bootstrap().expect("state"),
        state_path: directory.join("state.json"),
        settings: Settings::default(),
        terminal: TerminalSettings::default(),
        work,
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
                Some(Mutation::Enable),
                Some(Mutation::Disable),
                Some(Mutation::Remove),
                Some(Mutation::ResetPermissions),
            ] {
                view.mutation = mutation;
                let _ = view.details_body(&details, &installed, cx);
            }
        }
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
            view.button_label(
                "extension-reset-permissions",
                "Reset remembered permissions"
            ),
            "Resetting…"
        );
    });
    finish_mutation(&view, cx);
    view.read_with(cx, |view, _| {
        assert!(view.error.is_none());
        assert_eq!(
            view.button_label(
                "extension-reset-permissions",
                "Reset remembered permissions"
            ),
            "Permissions reset"
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
