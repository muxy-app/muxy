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
