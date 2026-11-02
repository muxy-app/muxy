use std::collections::BTreeMap;

use gpui::{Context, Task};
use muxy_app_core::extensions::Registry;

use super::AppModel;
use crate::extensions::{io, marketplace};

#[cfg(test)]
type Resolve = fn(&[String]) -> Result<BTreeMap<String, String>, String>;

#[derive(Default)]
pub(super) struct Checks {
    available: BTreeMap<String, String>,
    task: Option<Task<()>>,
    #[cfg(test)]
    resolve: Option<Resolve>,
}

impl Checks {
    pub(super) fn retain_installed(&mut self, registry: &Registry) {
        self.available.retain(|name, version| {
            !registry.is_unpacked(name)
                && registry
                    .extensions
                    .get(name)
                    .is_some_and(|extension| marketplace::is_update(&extension.version, version))
        });
    }
}

impl AppModel {
    pub(crate) fn extension_update_version(&self, name: &str) -> Option<&str> {
        self.extensions
            .checks
            .available
            .get(name)
            .map(String::as_str)
    }

    pub(crate) fn check_extension_updates(&mut self, cx: &mut Context<Self>) {
        #[cfg(not(test))]
        let resolve = marketplace::versions;
        #[cfg(test)]
        let resolve = self
            .extensions
            .checks
            .resolve
            .unwrap_or(|_| Ok(BTreeMap::new()));
        self.check_extension_updates_with(resolve, cx);
    }

    fn check_extension_updates_with(
        &mut self,
        resolve: impl FnOnce(&[String]) -> Result<BTreeMap<String, String>, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if !self.extensions_loaded() || self.extensions.checks.task.is_some() {
            return;
        }
        let registry = &self.extensions.registry;
        let names: Vec<_> = registry
            .extensions
            .keys()
            .filter(|name| !registry.is_unpacked(name))
            .cloned()
            .collect();
        if names.is_empty() {
            self.extensions.checks.available.clear();
            return;
        }
        self.extensions.checks.task = Some(cx.spawn(async move |model, cx| {
            let result = io::network(move || {
                let mut versions = resolve(&names)?;
                versions.retain(|name, _| names.binary_search(name).is_ok());
                Ok(versions)
            })
            .await;
            let _ = model.update(cx, |model, cx| {
                let extensions = &mut model.extensions;
                extensions.checks.task = None;
                match result {
                    Ok(versions) => {
                        extensions.checks.available = versions;
                        extensions.checks.retain_installed(&extensions.registry);
                        cx.notify();
                    }
                    Err(error) => crate::diagnostics::event(
                        "extension-update-check-failed",
                        format_args!("{error}"),
                    ),
                }
            });
        }));
    }
}

#[cfg(test)]
mod tests;
