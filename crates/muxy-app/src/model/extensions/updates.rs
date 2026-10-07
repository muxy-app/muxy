use gpui::Context;
use muxy_app_core::extensions::{Extension, Registry};
use muxy_ui::tr;

use super::AppModel;
use crate::extensions::{io, marketplace};

impl AppModel {
    pub(crate) fn update_extension(
        &mut self,
        expected: Extension,
        stage: tempfile::TempDir,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        let checks = self.extension_close_checks(Some(&expected.name), cx);
        let epoch = self.extensions.epoch;
        cx.spawn(async move |model, cx| {
            let (stage, next) = io::run(move || {
                let next = Extension::load(&stage.path().join("package"))?;
                Ok((stage, next))
            })
            .await?;
            if next.name != expected.name
                || !marketplace::is_update(&expected.version, &next.version)
            {
                return Err(tr!("The extension changed. Check for updates and try again.").into());
            }
            for view in checks {
                let check = view
                    .update(cx, crate::views::webview::Webview::before_close)
                    .map_err(|error| error.to_string())?;
                if check.recv().await.unwrap_or(true) {
                    return Err(tr!(
                        "The extension kept an open view. Save or close it before continuing."
                    )
                    .into());
                }
            }
            let (enabled, prepare) = model
                .update(cx, |model, cx| {
                    if model.extensions.epoch != epoch {
                        return Err(
                            tr!("The extension changed. Check for updates and try again.")
                                .to_string(),
                        );
                    }
                    check_installed(&model.extensions.registry, &expected)?;
                    let enabled = model.extensions.registry.enabled(&expected.name).is_some();
                    let current = expected.clone();
                    model.stop_extension(&expected.name, cx);
                    let task = model.change_extensions(
                        move |state| {
                            check_installed(&state.registry, &current)?;
                            state.registry.set_enabled(&current.name, false)
                        },
                        cx,
                    );
                    Ok((enabled, task))
                })
                .map_err(|error| error.to_string())??;
            prepare.await?;
            let task = model
                .update(cx, |model, cx| {
                    let flushed = model.extensions.logs.flush();
                    model.change_extensions(
                        move |state| {
                            let result = flushed
                                .and_then(|done| {
                                    done.recv_blocking().map_err(|error| error.to_string())
                                })
                                .and_then(|()| {
                                    check_installed(&state.registry, &expected)?;
                                    marketplace::update(
                                        &stage,
                                        &state.registry.directory(),
                                        &expected,
                                    )
                                });
                            let restore = enabled
                                && (result.is_err()
                                    || next
                                        .manifest
                                        .permissions
                                        .is_subset(&expected.manifest.permissions));
                            finish_update(&mut state.registry, &expected, restore, result)
                        },
                        cx,
                    )
                })
                .map_err(|error| error.to_string())?;
            task.await
        })
    }
}

fn check_installed(registry: &Registry, expected: &Extension) -> Result<(), String> {
    if registry.is_unpacked(&expected.name)
        || registry
            .extensions
            .get(&expected.name)
            .is_none_or(|current| {
                current.version != expected.version || current.directory != expected.directory
            })
    {
        return Err(tr!("The extension changed. Check for updates and try again.").into());
    }
    Ok(())
}

fn finish_update(
    registry: &mut Registry,
    expected: &Extension,
    restore: bool,
    result: Result<(), String>,
) -> Result<(), String> {
    registry.reload();
    let can_restore = !registry.is_unpacked(&expected.name)
        && registry
            .extensions
            .get(&expected.name)
            .is_some_and(|current| {
                result.is_ok()
                    || (current.version == expected.version
                        && current.directory == expected.directory)
            });
    if restore
        && can_restore
        && let Err(error) = registry.set_enabled(&expected.name, true)
    {
        return Err(match result {
            Err(original) => tr!(
                "%@ Could not restore the extension's enabled state: %@",
                original,
                error
            )
            .into(),
            Ok(()) => tr!(
                "The extension was updated but could not be enabled: %@",
                error
            )
            .into(),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{extensions::finish_extension, stub_boot};
    use gpui::TestAppContext;
    use muxy_app_core::AppState;
    use serde_json::json;
    use std::path::Path;

    fn package(root: &Path, version: &str, permissions: &[&str]) {
        std::fs::create_dir_all(root).expect("package folder");
        std::fs::write(
            root.join("package.json"),
            json!({
                "name":"reader", "version":version,
                "muxy":{"permissions":permissions}
            })
            .to_string(),
        )
        .expect("manifest");
    }

    #[gpui::test]
    fn updates_preserve_user_data_and_require_review_only_for_added_permissions(
        cx: &mut TestAppContext,
    ) {
        let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
        let profile = boot.state_path.parent().expect("profile").to_owned();
        let directory = profile.join("extensions");
        package(&directory.join("reader"), "1.0.0", &["files:read"]);
        std::fs::write(profile.join("extension-enabled.json"), r#"["reader"]"#).expect("enabled");
        for (folder, value) in [
            ("extension-settings", r#"{"theme":"dark"}"#),
            ("extension-storage", r#"{"saved":"value"}"#),
        ] {
            std::fs::create_dir_all(profile.join(folder)).expect("data folder");
            std::fs::write(profile.join(folder).join("reader.json"), value).expect("data");
        }
        std::fs::write(
            profile.join("extension-grants.json"),
            r#"{"reader:exec:argv:git":"Allow"}"#,
        )
        .expect("grants");
        let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        finish_extension(
            model.update(cx, AppModel::refresh_installed_extensions_task),
            cx,
        )
        .expect("load");

        for (version, permissions, enabled) in [
            ("1.1.0", vec!["files:read"], true),
            ("1.2.0", vec!["files:read", "files:write"], false),
            ("1.3.0", vec!["files:read"], false),
        ] {
            let stage = tempfile::tempdir_in(&directory).expect("stage");
            package(&stage.path().join("package"), version, &permissions);
            let before = model.read_with(cx, |model, _| model.extensions.epoch);
            let task = model.update(cx, |model, cx| {
                for index in 0..200 {
                    model.extension_log("reader", format!("pending {version} {index}"));
                }
                model.update_extension(
                    model.extensions.registry.extensions["reader"].clone(),
                    stage,
                    cx,
                )
            });
            finish_extension(task, cx).expect("update");
            let log = std::fs::read_to_string(directory.join("reader/logs/output.log"))
                .expect("preserved log");
            assert!(log.contains(&format!("pending {version} 199\n")));
            model.read_with(cx, |model, _| {
                assert_eq!(
                    model.extensions.registry.extensions["reader"].version,
                    version
                );
                assert_eq!(
                    model.extensions.registry.enabled("reader").is_some(),
                    enabled
                );
                assert!(model.extensions.epoch > before);
                assert_eq!(model.extensions.settings["reader"]["theme"], "dark");
                assert_eq!(model.extension_rules("reader").len(), 1);
            });
            assert_eq!(
                std::fs::read_to_string(profile.join("extension-storage/reader.json"))
                    .expect("storage"),
                r#"{"saved":"value"}"#
            );
            let reloaded = Registry::load(&profile);
            assert_eq!(reloaded.extensions["reader"].version, version);
            assert_eq!(reloaded.enabled("reader").is_some(), enabled);
        }
    }

    #[gpui::test]
    fn failed_update_restores_enabled_state_and_rejects_unpacked_folders(cx: &mut TestAppContext) {
        let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
        let profile = boot.state_path.parent().expect("profile").to_owned();
        let directory = profile.join("extensions");
        package(&directory.join("reader"), "1.0.0", &[]);
        std::fs::write(profile.join("extension-enabled.json"), r#"["reader"]"#).expect("enabled");
        let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        finish_extension(
            model.update(cx, AppModel::refresh_installed_extensions_task),
            cx,
        )
        .expect("load");
        let stage = tempfile::tempdir_in(&directory).expect("stage");
        package(&stage.path().join("package"), "1.1.0", &["files:write"]);
        std::fs::create_dir_all(directory.join("reader/logs")).expect("logs");
        std::fs::write(directory.join("reader/logs/output.log"), "output").expect("log");
        std::fs::write(stage.path().join("package/logs"), "not a directory").expect("conflict");
        let task = model.update(cx, |model, cx| {
            model.update_extension(
                model.extensions.registry.extensions["reader"].clone(),
                stage,
                cx,
            )
        });
        assert!(finish_extension(task, cx).is_err());
        model.read_with(cx, |model, _| {
            assert_eq!(
                model
                    .extensions
                    .registry
                    .enabled("reader")
                    .expect("still enabled")
                    .version,
                "1.0.0"
            );
        });
        finish_extension(
            model.update(cx, |model, cx| model.remove_extension("reader", cx)),
            cx,
        )
        .expect("remove");
        let unpacked = tempfile::tempdir().expect("unpacked");
        package(unpacked.path(), "1.0.0", &[]);
        finish_extension(
            model.update(cx, |model, cx| {
                model.load_unpacked_extension(unpacked.path().to_owned(), cx)
            }),
            cx,
        )
        .expect("load unpacked");
        let stage = tempfile::tempdir_in(&directory).expect("stage");
        package(&stage.path().join("package"), "1.1.0", &[]);
        let task = model.update(cx, |model, cx| {
            model.update_extension(
                model.extensions.registry.extensions["reader"].clone(),
                stage,
                cx,
            )
        });
        assert!(finish_extension(task, cx).is_err());
        assert_eq!(
            Extension::load(unpacked.path())
                .expect("untouched folder")
                .version,
            "1.0.0"
        );
        assert!(!directory.join("reader").exists());
    }
}
