//! Background scripts: one long-lived `JavaScriptCore` context per enabled
//! extension that declares `background`. They start with the extension, keep
//! running across server reconnects like pages do, restart when the extension
//! is updated, and stop when it is disabled, reloaded, or removed.

use gpui::Context;

use super::AppModel;
use super::scripts::{read_source, runtime};

pub(super) struct Background {
    pub(super) script: u64,
    pub(super) version: String,
}

impl AppModel {
    pub(super) fn is_background(&self, script: u64) -> bool {
        self.extensions
            .backgrounds
            .values()
            .any(|background| background.script == script)
    }

    /// Stops one extension's background script, or every one with `None`.
    pub(super) fn stop_backgrounds(&mut self, owner: Option<&str>) {
        let stopped: Vec<_> = self
            .extensions
            .backgrounds
            .keys()
            .filter(|name| owner.is_none_or(|owner| owner == name.as_str()))
            .cloned()
            .collect();
        for name in stopped {
            if let Some(background) = self.extensions.backgrounds.remove(&name) {
                self.extensions.scripts.remove(&background.script);
                self.extension_log(&name, "[muxy] exited cleanly".into());
            }
        }
    }

    pub(super) fn sync_backgrounds(&mut self, cx: &mut Context<Self>) {
        let wanted: Vec<_> = self
            .extensions
            .registry
            .active()
            .filter(|extension| extension.manifest.background.is_some())
            .cloned()
            .collect();
        let stale: Vec<_> = self
            .extensions
            .backgrounds
            .iter()
            .filter(|(owner, background)| {
                !wanted.iter().any(|extension| {
                    &extension.name == *owner && extension.version == background.version
                })
            })
            .map(|(owner, _)| owner.clone())
            .collect();
        for owner in stale {
            self.stop_backgrounds(Some(&owner));
        }
        for extension in wanted {
            if self.extensions.backgrounds.contains_key(&extension.name)
                || !self
                    .extensions
                    .loading_backgrounds
                    .insert(extension.name.clone())
            {
                continue;
            }
            let owner = extension.name.clone();
            let version = extension.version.clone();
            let epoch = self.extensions.epoch_for(&owner);
            let task = crate::extensions::io::run(move || {
                let path = extension.manifest.background.clone().unwrap_or_default();
                read_source(&extension, &path)
                    .map(|source| runtime(&extension.name, "background", &source))
            });
            cx.spawn(async move |model, cx| {
                let source = task.await;
                let _ = model.update(cx, |model, cx| {
                    model.extensions.loading_backgrounds.remove(&owner);
                    let current = model
                        .extensions
                        .registry
                        .enabled(&owner)
                        .is_some_and(|extension| extension.version == version);
                    if epoch != model.extensions.epoch_for(&owner)
                        || !current
                        || model.extensions.backgrounds.contains_key(&owner)
                    {
                        model.sync_backgrounds(cx);
                        return;
                    }
                    match source.and_then(|source| {
                        model.start_loaded_extension_script(owner.clone(), None, source, cx)
                    }) {
                        Ok(script) => {
                            model.extensions.backgrounds.insert(
                                owner.clone(),
                                Background {
                                    script,
                                    version: version.clone(),
                                },
                            );
                            model.extension_log(
                                &owner,
                                format!("[muxy] started {owner} v{version}"),
                            );
                        }
                        Err(error) => {
                            model.extension_log(&owner, format!("[muxy] failed to start: {error}"));
                        }
                    }
                });
            })
            .detach();
        }
    }

    /// Whether the extension's background script is running.
    pub(crate) fn background_running(&self, owner: &str) -> bool {
        self.extensions.backgrounds.contains_key(owner)
    }
}
