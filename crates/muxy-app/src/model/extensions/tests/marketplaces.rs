use std::path::Path;
use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use muxy_app_core::AppState;
use muxy_app_core::extensions::Marketplace;

use crate::extensions::marketplace::ExtensionDetails;
use crate::model::AppModel;
use crate::model::tests::stub_boot;

fn pump_until(cx: &mut VisualTestContext, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    for _ in 0..2000 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the extension job did not finish");
}

fn attempt(
    model: &Entity<AppModel>,
    details: ExtensionDetails,
    cx: &mut VisualTestContext,
) -> Option<String> {
    let started = model.update(cx, |model, cx| {
        model.install_marketplace_extension_with(
            details,
            |_: &ExtensionDetails, _: &Path| Err("downloaded".to_owned()),
            cx,
        )
    });
    assert!(started);
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| model.extensions.installs.active.is_none())
    });
    model.update(cx, |model, _| {
        model
            .extensions
            .installs
            .finished
            .take()
            .and_then(|finished| finished.error)
    })
}

#[gpui::test]
fn an_extension_is_never_installed_over_one_from_another_marketplace(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let directory = model.read_with(cx, |model, _| model.extensions.registry.directory());
    std::fs::create_dir_all(directory.join("git")).expect("folder");
    std::fs::write(
        directory.join("git/package.json"),
        serde_json::json!({"name": "git", "version": "1.0.0", "muxy": {}}).to_string(),
    )
    .expect("package");
    let theirs = Marketplace::parse("someone/extensions").expect("repository");
    let added = theirs.clone();
    model.update(cx, |model, cx| {
        model
            .change_extensions(
                move |state| {
                    state.registry.reload();
                    state.registry.add_marketplace(added)
                },
                cx,
            )
            .detach();
    });
    let check = theirs.clone();
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            let registry = &model.extensions.registry;
            registry.extensions.contains_key("git") && registry.has_marketplace(&check)
        })
    });
    let installed = model.read_with(cx, |model, _| {
        model.extensions.registry.extensions["git"].clone()
    });

    let mut replacement = ExtensionDetails::from(&installed);
    replacement.current_version = Some("2.0.0".into());
    replacement.source = theirs;
    let error = attempt(&model, replacement.clone(), cx).expect("refused");
    assert!(
        error.contains("already installed from muxy-app/extensions"),
        "{error}"
    );
    model.read_with(cx, |model, _| {
        let registry = &model.extensions.registry;
        assert_eq!(registry.extensions["git"].version, "1.0.0");
        assert!(registry.source("git").is_official());
    });

    let mut unlisted = replacement;
    unlisted.name = "notes".into();
    unlisted.source = Marketplace::parse("nobody/extensions").expect("repository");
    let error = attempt(&model, unlisted, cx).expect("refused");
    assert!(error.contains("nobody/extensions"), "{error}");
    assert!(!error.contains("downloaded"), "{error}");
}
