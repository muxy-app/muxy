use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};
use muxy_app_core::AppState;
use muxy_app_core::extensions::Extension;

use super::{ExtensionsView, Mutation, Tab, permissions_note};
use crate::model::extensions::{ActiveInstall, FinishedInstall};
use crate::model::{AppModel, tests::stub_boot};

fn write_package(root: &Path, name: &str, version: &str, permissions: &[&str]) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("package.json"),
        serde_json::json!({
            "name": name,
            "version": version,
            "muxy": {"permissions": permissions},
        })
        .to_string(),
    )
    .unwrap();
}

fn stage_package(
    extension: &Extension,
    directory: &Path,
    permissions: &[&str],
) -> Result<tempfile::TempDir, String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let stage = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(directory)
        .map_err(|error| error.to_string())?;
    write_package(
        &stage.path().join("package"),
        &extension.name,
        "2.0.0",
        permissions,
    );
    Ok(stage)
}

fn newer_versions(names: &[String]) -> BTreeMap<String, String> {
    names
        .iter()
        .map(|name| (name.clone(), "2.0.0".to_owned()))
        .collect()
}

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

fn open_model<'a>(
    names: &[&str],
    cx: &'a mut TestAppContext,
) -> (Entity<AppModel>, &'a mut VisualTestContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let directory = model.read_with(cx, |model, _| model.extensions.registry.directory());
    for name in names {
        write_package(&directory.join(name), name, "1.0.0", &[]);
    }
    model.update(cx, AppModel::refresh_installed_extensions);
    let count = names.len();
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| model.extensions.registry.extensions.len()) == count
    });
    (model, cx)
}

fn open_view(model: &Entity<AppModel>, cx: &mut VisualTestContext) -> Entity<ExtensionsView> {
    let view = model.update(cx, |model, cx| {
        let weak = cx.weak_entity();
        let theme = model.theme.clone();
        cx.new(|cx| ExtensionsView::new(weak, theme, muxy_ui::theme::Metrics::new(1.15), cx))
    });
    cx.run_until_parked();
    view
}

fn finished(model: &Entity<AppModel>, cx: &mut VisualTestContext) {
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| model.extensions.installs.active.is_none())
    });
}

fn installed_version(model: &Entity<AppModel>, name: &str, cx: &VisualTestContext) -> String {
    model.read_with(cx, |model, _| {
        model.extensions.registry.extensions[name].version.clone()
    })
}

/// An Update All whose first download waits until the sender is used.
fn start_held_update_all(model: &Entity<AppModel>, cx: &mut VisualTestContext) -> Sender<()> {
    let (release, held): (Sender<()>, Receiver<()>) = channel();
    let held = Mutex::new(held);
    let started = model.update(cx, |model, cx| {
        model.update_all_extensions_with(
            |names| Ok(newer_versions(names)),
            move |extension, directory| {
                let _ = held.lock().unwrap().recv();
                stage_package(extension, directory, &[])
            },
            cx,
        )
    });
    assert!(started);
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model
                .extensions
                .installs
                .status
                .as_deref()
                .is_some_and(|status| status.starts_with("Updating"))
        })
    });
    release
}

#[gpui::test]
fn update_all_finishes_without_a_view(cx: &mut TestAppContext) {
    let (model, cx) = open_model(&["reader"], cx);
    let started = model.update(cx, |model, cx| {
        model.update_all_extensions_with(
            |names| Ok(newer_versions(names)),
            |extension, directory| stage_package(extension, directory, &[]),
            cx,
        )
    });
    assert!(started);
    finished(&model, cx);
    assert_eq!(installed_version(&model, "reader", cx), "2.0.0");
    model.read_with(cx, |model, _| {
        assert_eq!(
            model.extensions.installs.finished,
            Some(FinishedInstall {
                job: ActiveInstall::UpdateAll,
                status: Some("Updated 1 extension.".into()),
                error: None,
            })
        );
    });
}

#[gpui::test]
fn views_show_a_running_job_and_its_result_once(cx: &mut TestAppContext) {
    let (model, cx) = open_model(&["reader"], cx);
    let release = start_held_update_all(&model, cx);

    let during = open_view(&model, cx);
    during.read_with(cx, |view, cx| {
        assert!(view.busy(cx));
        assert_eq!(
            view.button_label(Mutation::UpdateAll.button(), "Update all", cx),
            "Updating…"
        );
        assert_eq!(
            model.read(cx).extensions.installs.status.as_deref(),
            Some("Updating reader (1 of 1)…")
        );
    });
    drop(during);
    cx.run_until_parked();

    release.send(()).unwrap();
    finished(&model, cx);

    let after = open_view(&model, cx);
    after.read_with(cx, |view, cx| {
        assert!(!view.busy(cx));
        assert_eq!(view.install_status.as_deref(), Some("Updated 1 extension."));
        assert_eq!(view.error, None);
        assert_eq!(view.selected.as_ref().map(|details| &details.name), None);
    });
    drop(after);
    cx.run_until_parked();

    let third = open_view(&model, cx);
    third.read_with(cx, |view, _| assert_eq!(view.install_status, None));
}

#[gpui::test]
fn a_second_start_does_nothing_while_a_job_runs(cx: &mut TestAppContext) {
    let (model, cx) = open_model(&["reader"], cx);
    let release = start_held_update_all(&model, cx);
    let (status, second) = model.update(cx, |model, cx| {
        let second = model.update_all_extensions_with(
            |_| Err("a second job ran".to_owned()),
            |_, _| Err("a second job ran".to_owned()),
            cx,
        );
        (model.extensions.installs.status.clone(), second)
    });
    assert!(!second);
    assert_eq!(status.as_deref(), Some("Updating reader (1 of 1)…"));
    release.send(()).unwrap();
    finished(&model, cx);
    model.read_with(cx, |model, _| {
        let result = model.extensions.installs.finished.as_ref().unwrap();
        assert_eq!(result.error, None);
    });
    assert_eq!(installed_version(&model, "reader", cx), "2.0.0");
}

#[gpui::test]
fn a_failed_download_reports_that_extension_and_others_update(cx: &mut TestAppContext) {
    let (model, cx) = open_model(&["broken", "reader"], cx);
    model.update(cx, |model, cx| {
        model.update_all_extensions_with(
            |names| Ok(newer_versions(names)),
            |extension, directory| {
                if extension.name == "broken" {
                    return Err("offline".to_owned());
                }
                stage_package(extension, directory, &[])
            },
            cx,
        )
    });
    finished(&model, cx);
    assert_eq!(installed_version(&model, "broken", cx), "1.0.0");
    assert_eq!(installed_version(&model, "reader", cx), "2.0.0");
    let view = open_view(&model, cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.error.as_deref(), Some("broken: offline"));
        assert_eq!(view.install_status.as_deref(), Some("Updated 1 extension."));
    });
}

#[gpui::test]
fn the_check_for_versions_failing_reports_only_the_error(cx: &mut TestAppContext) {
    let (model, cx) = open_model(&["reader"], cx);
    model.update(cx, |model, cx| {
        model.update_all_extensions_with(
            |_| Err("offline".to_owned()),
            |_, _| Err("no download expected".to_owned()),
            cx,
        )
    });
    finished(&model, cx);
    let view = open_view(&model, cx);
    view.read_with(cx, |view, _| {
        assert_eq!(view.error.as_deref(), Some("offline"));
        assert_eq!(view.install_status, None);
    });
}

#[gpui::test]
fn an_update_adding_permissions_stays_disabled(cx: &mut TestAppContext) {
    let (model, cx) = open_model(&["reader"], cx);
    model.update(cx, |model, cx| {
        model.set_extension_enabled("reader", true, cx).detach();
    });
    pump_until(cx, |cx| {
        model.read_with(cx, |model, _| {
            model.extensions.registry.enabled("reader").is_some()
        })
    });
    model.update(cx, |model, cx| {
        model.update_all_extensions_with(
            |names| Ok(newer_versions(names)),
            |extension, directory| stage_package(extension, directory, &["files:read"]),
            cx,
        )
    });
    finished(&model, cx);
    model.read_with(cx, |model, _| {
        assert!(model.extensions.registry.enabled("reader").is_none());
        assert_eq!(
            model
                .extensions
                .installs
                .finished
                .as_ref()
                .and_then(|finished| finished.status.as_deref()),
            Some("Updated 1 extension. Review permissions and enable when ready: reader.")
        );
    });
}

#[gpui::test]
fn only_the_view_that_started_a_job_opens_the_extension(cx: &mut TestAppContext) {
    let (model, cx) = open_model(&["reader"], cx);
    let finish = |model: &Entity<AppModel>, cx: &mut VisualTestContext| {
        model.update(cx, |model, cx| {
            model.extensions.installs.finished = Some(FinishedInstall {
                job: ActiveInstall::Install {
                    name: "reader".into(),
                },
                status: Some("Installed reader.".into()),
                error: None,
            });
            cx.notify();
        });
        cx.run_until_parked();
    };

    let later = open_view(&model, cx);
    finish(&model, cx);
    later.read_with(cx, |view, _| {
        assert!(view.selected.is_none());
        assert_eq!(view.install_status.as_deref(), Some("Installed reader."));
    });
    drop(later);
    cx.run_until_parked();

    let starter = open_view(&model, cx);
    starter.update(cx, |view, _| {
        view.tab = Tab::Marketplace;
        view.started_install = true;
    });
    finish(&model, cx);
    starter.read_with(cx, |view, _| {
        assert!(view.tab == Tab::Installed);
        assert_eq!(
            view.selected.as_ref().map(|details| details.name.as_str()),
            Some("reader")
        );
        assert_eq!(view.install_status, None);
        assert!(!view.started_install);
    });
}

#[test]
fn unknown_permissions_are_not_described_as_none() {
    let note = |permissions: Option<&BTreeSet<String>>| {
        permissions_note(permissions).map(|note| note.to_string())
    };
    assert_eq!(
        note(None).as_deref(),
        Some("The marketplace did not list this extension's permissions.")
    );
    assert_eq!(
        note(Some(&BTreeSet::new())).as_deref(),
        Some("No permissions requested.")
    );
    assert_eq!(note(Some(&BTreeSet::from(["files:read".to_owned()]))), None);
}
