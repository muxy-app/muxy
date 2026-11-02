use std::path::Path;
use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use muxy_app_core::AppState;

use super::*;
use crate::model::tests::stub_boot;
use crate::views::settings::SettingsView;

fn package(path: &Path, name: &str, version: &str) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(
        path.join("package.json"),
        serde_json::json!({"name": name, "version": version, "muxy": {}}).to_string(),
    )
    .unwrap();
}

fn pump_until(cx: &mut VisualTestContext, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    for _ in 0..2000 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("extension check did not finish");
}

fn idle(model: &Entity<AppModel>, cx: &mut VisualTestContext) {
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model.extensions_loaded() && model.extensions.checks.task.is_none()
        })
    });
}

fn open_model(
    resolve: Option<Resolve>,
    cx: &mut TestAppContext,
) -> (tempfile::TempDir, Entity<AppModel>, &mut VisualTestContext) {
    let root = tempfile::tempdir().unwrap();
    for name in ["editor", "reader"] {
        package(&root.path().join("extensions").join(name), name, "1.0.0");
    }
    let dev = root.path().join("dev");
    package(&dev, "local", "1.0.0");
    Registry::load(root.path()).load_unpacked(&dev).unwrap();
    let (mut boot, _requests) = stub_boot(AppState::bootstrap().unwrap());
    boot.state_path = root.path().join("state.json");
    let (model, cx) = cx.add_window_view(|window, cx| {
        let mut model = AppModel::new(boot, window, cx);
        model.extensions.checks.resolve = resolve;
        model
    });
    idle(&model, cx);
    (root, model, cx)
}

fn newer(names: &[String]) -> BTreeMap<String, String> {
    assert_eq!(names, ["editor", "reader"]);
    BTreeMap::from([
        ("editor".into(), "1.0.0".into()),
        ("reader".into(), "2.0.0".into()),
        ("local".into(), "2.0.0".into()),
        ("unknown".into(), "2.0.0".into()),
    ])
}

#[gpui::test]
fn startup_checks_disabled_packages_after_loading_and_excludes_developer_folders(
    cx: &mut TestAppContext,
) {
    let (_root, model, cx) = open_model(Some(|names| Ok(newer(names))), cx);
    model.read_with(cx, |model, _| {
        assert_eq!(model.extensions.registry.extensions.len(), 3);
        assert_eq!(model.extensions.registry.active().count(), 0);
        assert_eq!(model.extension_update_version("reader"), Some("2.0.0"));
        assert_eq!(model.extensions.checks.available.len(), 1);
        assert_eq!(
            model.extensions.registry.extensions["reader"].version,
            "1.0.0"
        );
        assert!(model.extensions.installs.active.is_none());
    });
}

#[gpui::test]
fn opening_extensions_settings_refreshes_available_updates(cx: &mut TestAppContext) {
    let (_root, model, cx) = open_model(None, cx);
    model.update(cx, |model, cx| {
        model.extensions.checks.resolve = Some(|names| Ok(newer(names)));
        model.show_settings(cx);
    });
    cx.run_until_parked();
    let settings = model.read_with(cx, |model, _| {
        assert!(model.extensions.checks.available.is_empty());
        model.settings_window.as_ref().unwrap().view.clone()
    });
    settings.update(cx, SettingsView::show_extensions);
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model.extension_update_version("reader").is_some()
        })
    });
    model.update(cx, |model, _| {
        model.extensions.checks.resolve =
            Some(|_| Ok(BTreeMap::from([("reader".into(), "3.0.0".into())])));
    });
    settings.update(cx, SettingsView::show_extensions);
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model.extension_update_version("reader") == Some("3.0.0")
        })
    });
}

#[gpui::test]
fn overlapping_checks_coalesce_and_late_results_ignore_removed_or_updated_packages(
    cx: &mut TestAppContext,
) {
    let (root, model, cx) = open_model(None, cx);
    let (release, held) = std::sync::mpsc::channel();
    model.update(cx, |model, cx| {
        model.check_extension_updates_with(
            move |names| {
                held.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(names
                    .iter()
                    .map(|name| (name.clone(), "2.0.0".into()))
                    .collect())
            },
            cx,
        );
        model.check_extension_updates_with(|_| panic!("duplicate request"), cx);
    });
    package(&root.path().join("extensions/reader"), "reader", "2.0.0");
    std::fs::remove_dir_all(root.path().join("extensions/editor")).unwrap();
    model.update(cx, AppModel::refresh_installed_extensions);
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model.extensions.registry.extensions.len() == 2
                && model.extensions.registry.extensions["reader"].version == "2.0.0"
        })
    });
    release.send(()).unwrap();
    idle(&model, cx);
    model.read_with(cx, |model, _| {
        assert!(model.extensions.checks.available.is_empty());
    });

    std::fs::remove_dir_all(root.path().join("extensions/reader")).unwrap();
    model.update(cx, AppModel::refresh_installed_extensions);
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model.extensions.registry.extensions.len() == 1
        })
    });
    model.update(cx, |model, cx| {
        model.check_extension_updates_with(|_| panic!("no marketplace packages"), cx);
        assert!(model.extensions.checks.task.is_none());
    });
}

#[gpui::test]
fn failed_checks_keep_known_updates_allow_retry_and_clear_after_installation(
    cx: &mut TestAppContext,
) {
    let (root, model, cx) = open_model(Some(|names| Ok(newer(names))), cx);
    let error_before = model.read_with(cx, |model, _| model.error.clone());
    model.update(cx, |model, cx| {
        model.check_extension_updates_with(|_| Err("offline".into()), cx);
    });
    idle(&model, cx);
    model.read_with(cx, |model, _| {
        assert_eq!(model.extension_update_version("reader"), Some("2.0.0"));
        assert_eq!(model.error, error_before);
    });
    model.update(cx, |model, cx| {
        model.check_extension_updates_with(
            |_| Ok(BTreeMap::from([("reader".into(), "3.0.0".into())])),
            cx,
        );
    });
    idle(&model, cx);
    model.read_with(cx, |model, _| {
        assert_eq!(model.extension_update_version("reader"), Some("3.0.0"));
    });
    package(&root.path().join("extensions/reader"), "reader", "3.0.0");
    model.update(cx, AppModel::refresh_installed_extensions);
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model.extension_update_version("reader").is_none()
        })
    });
}
