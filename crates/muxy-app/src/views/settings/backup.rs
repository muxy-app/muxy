use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, div};
use muxy_ui::controls;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use super::{Category, SettingsEvent, SettingsView, window::SettingsWindow};

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Export,
    Choose,
    Legacy,
    Confirm,
    Cancel,
}

pub(super) fn rows(view: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    for (id, label, actions) in [
        (
            "backup-export",
            tr_key!("Export backup"),
            vec![(tr!("Export…"), Action::Export)],
        ),
        (
            "backup-restore",
            tr_key!("Restore backup"),
            vec![(tr!("Choose file…"), Action::Choose)],
        ),
        (
            "backup-legacy",
            tr_key!("Import from Muxy 1.x"),
            vec![
                (tr!("Import installed 1.x"), Action::Legacy),
                (tr!("Choose file…"), Action::Choose),
            ],
        ),
    ] {
        if !view.matches(Category::Backup, label) {
            continue;
        }
        let mut control = div().flex().flex_wrap().gap(view.metrics.spacing2());
        for (index, (label, action)) in actions.into_iter().enumerate() {
            let selector = format!("{id}-{index}");
            control = control.child(
                controls::button(
                    view.style(),
                    &selector,
                    &label,
                    !view.backup_busy,
                    cx.listener(move |_, _, _, cx| cx.emit(SettingsEvent::Backup(action))),
                )
                .debug_selector(move || selector.clone()),
            );
        }
        if id == "backup-restore" {
            control = control.child(controls::button(
                view.style(),
                "backup-confirm",
                &tr!("Restore on next launch"),
                !view.backup_busy && view.backup_import.is_some(),
                cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Backup(Action::Confirm))),
            ));
            control = control.child(controls::button(
                view.style(),
                "backup-cancel",
                &tr!("Cancel import"),
                !view.backup_busy,
                cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Backup(Action::Cancel))),
            ));
        }
        rows.push(view.row(id, label, control.into_any_element()));
        if id == "backup-restore" {
            let status = view
                .backup_import
                .as_ref()
                .map_or(view.backup_status.as_str(), |import| &import.summary);
            if !status.is_empty() {
                rows.push(view.note(status, false).into_any_element());
            }
        }
    }
    if view.matches(Category::Backup, tr_key!("Configuration files")) {
        let mut control = div().flex().flex_wrap().gap(view.metrics.spacing2());
        for name in ["settings.toml", "server.toml"] {
            control = control.child(controls::button(
                view.style(),
                &format!("backup-edit-{name}"),
                name,
                true,
                cx.listener(move |_, _, _, cx| cx.emit(SettingsEvent::OpenConfiguration(name))),
            ));
        }
        rows.push(view.row(
            "backup-files",
            tr_key!("Configuration files"),
            control.into_any_element(),
        ));
    }
    rows
}

impl SettingsWindow {
    #[allow(
        clippy::too_many_lines,
        reason = "Keep background backup actions and their completion handling together"
    )]
    pub(super) fn backup_action(&mut self, action: Action, cx: &mut Context<Self>) {
        if self.view.read(cx).backup_busy {
            return;
        }
        let Ok((profile, state)) = self.model.update(cx, |model, cx| {
            model.flush_preferences(cx);
            (
                model
                    .configuration_path("settings.toml")
                    .parent()
                    .map(std::path::Path::to_path_buf),
                model.state.clone(),
            )
        }) else {
            return;
        };
        let Some(profile) = profile else { return };
        self.view.update(cx, |view, cx| {
            view.backup_busy = true;
            if matches!(action, Action::Choose | Action::Legacy) {
                view.backup_import = None;
            }
            view.set_error("backup-restore", None, cx);
        });
        match action {
            Action::Export => {
                let prompt = cx.prompt_for_new_path(&profile, Some("Muxy-backup.muxy"));
                cx.spawn(async move |root, cx| {
                    let result = async {
                        let Some(mut path) = prompt
                            .await
                            .map_err(|error| error.to_string())?
                            .map_err(|error| error.to_string())?
                        else {
                            return Ok(None);
                        };
                        if path.extension().is_none() {
                            path.set_extension("muxy");
                        }
                        cx.background_executor()
                            .spawn(async move {
                                crate::backup::export(&profile, &path, &state)
                                    .map_err(|error| error.to_string())
                            })
                            .await?;
                        Ok(Some(tr!("Backup exported.").to_string()))
                    }
                    .await;
                    let _ = root.update(cx, |root, cx| root.backup_finished(result, cx));
                })
                .detach();
            }
            Action::Choose => {
                let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some(tr!("Choose Muxy backup or 1.x settings.json")),
                });
                cx.spawn(async move |root, cx| {
                    let result = async {
                        let paths = prompt
                            .await
                            .map_err(|error| error.to_string())?
                            .map_err(|error| error.to_string())?;
                        let Some(path) = paths.and_then(|paths| paths.into_iter().next()) else {
                            return Ok(None);
                        };
                        cx.background_executor()
                            .spawn(async move {
                                crate::backup::prepare(&profile, &path)
                                    .map(Some)
                                    .map_err(|error| error.to_string())
                            })
                            .await
                    }
                    .await;
                    let _ = root.update(cx, |root, cx| root.backup_prepared(result, cx));
                })
                .detach();
            }
            Action::Legacy => {
                cx.spawn(async move |root, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            crate::backup::prepare_installed(&profile)
                                .map(Some)
                                .map_err(|error| error.to_string())
                        })
                        .await;
                    let _ = root.update(cx, |root, cx| root.backup_prepared(result, cx));
                })
                .detach();
            }
            Action::Confirm | Action::Cancel => {
                let import = self.view.update(cx, |view, _| view.backup_import.take());
                cx.spawn(async move |root, cx| {
                    let result = cx.background_executor().spawn(async move {
                        if matches!(action, Action::Cancel) {
                            crate::backup::cancel_pending(&profile).map(|()| Some(tr!("Import cancelled.").to_string()))
                        } else if let Some(import) = import {
                                crate::backup::stage(&profile, &import).map(|()| Some(tr!("Import ready. Quit and reopen Muxy to restore it. A recovery copy will be saved in Backups. Restart the server from Settings to apply server settings.").to_string()))
                        } else { Ok(None) }.map_err(|error| error.to_string())
                    }).await;
                    let _ = root.update(cx, |root, cx| root.backup_finished(result, cx));
                }).detach();
            }
        }
    }

    fn backup_prepared(
        &mut self,
        result: Result<Option<crate::backup::PreparedImport>, String>,
        cx: &mut Context<Self>,
    ) {
        let result = result.map(|import| {
            import.map(|import| {
                let summary = import.summary.clone();
                self.view
                    .update(cx, |view, _| view.backup_import = Some(import));
                summary
            })
        });
        self.backup_finished(result, cx);
    }

    fn backup_finished(&mut self, result: Result<Option<String>, String>, cx: &mut Context<Self>) {
        self.view.update(cx, |view, cx| {
            view.backup_busy = false;
            match result {
                Ok(Some(message)) => view.backup_status = message,
                Ok(None) => (),
                Err(error) => view.set_error("backup-restore", Some(&error), cx),
            }
            view.results.dirty = true;
            cx.notify();
        });
    }
}

impl SettingsView {
    pub(crate) fn set_backup_pending(&mut self, pending: bool) {
        if pending {
            self.backup_status = tr!("An import is ready for the next launch. Quit and reopen Muxy to apply it, or cancel the import here.").to_string();
        }
    }
}
