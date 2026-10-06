use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{Context, Entity};
use muxy_app_core::extensions::Extension;
use muxy_ui::tr;

use super::{ExtensionsView, Mutation, available_update};
use crate::{
    extensions::{io, marketplace},
    model::AppModel,
};

impl ExtensionsView {
    pub(crate) fn updating_all(&self) -> bool {
        self.mutation == Some(Mutation::UpdateAll)
    }

    pub(super) fn update_all(&mut self, cx: &mut Context<Self>) {
        self.update_all_with(
            marketplace::versions,
            |extension, directory| {
                let details = marketplace::detail(&extension.name)?;
                if available_update(extension, &details).is_none() {
                    return Err(
                        tr!("The extension changed. Check for updates and try again.").into(),
                    );
                }
                marketplace::download(&details, directory)
            },
            cx,
        );
    }

    pub(super) fn update_all_with(
        &mut self,
        versions: impl FnOnce(&[String]) -> Result<BTreeMap<String, String>, String> + Send + 'static,
        download: impl Fn(&Extension, &Path) -> Result<tempfile::TempDir, String>
        + Send
        + Sync
        + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.mutation.is_some() || self.loading.is_some() {
            return;
        }
        let Some(model) = self.model.upgrade() else {
            return;
        };
        let registry = &model.read(cx).extensions.registry;
        let installed: Vec<_> = registry
            .extensions
            .values()
            .filter(|extension| !registry.is_unpacked(&extension.name))
            .cloned()
            .collect();
        if installed.is_empty() {
            return;
        }
        let directory = registry.directory();
        let enabled: BTreeSet<_> = registry
            .active()
            .map(|extension| extension.name.clone())
            .collect();
        self.mutation = Some(Mutation::UpdateAll);
        self.completed = None;
        self.error = None;
        self.update_all_status = Some(tr!("Checking for extension updates…").into());
        let download = Arc::new(download);
        cx.spawn(async move |view, cx| {
            let names: Vec<_> = installed
                .iter()
                .map(|extension| extension.name.clone())
                .collect();
            let result = io::network(move || versions(&names)).await;
            let versions = match result {
                Ok(versions) => versions,
                Err(error) => {
                    let _ = view.update(cx, |view, cx| {
                        view.finish_update_all(Err(error), cx);
                    });
                    return;
                }
            };
            let updates: Vec<_> = installed
                .into_iter()
                .filter(|extension| {
                    versions
                        .get(&extension.name)
                        .is_some_and(|version| marketplace::is_update(&extension.version, version))
                })
                .collect();
            let total = updates.len();
            let mut result = BatchResult::default();
            for (index, extension) in updates.into_iter().enumerate() {
                let name = extension.name.clone();
                let _ = view.update(cx, |view, cx| {
                    view.update_all_status =
                        Some(tr!("Updating %@ (%@ of %@)…", &name, index + 1, total).into());
                    cx.notify();
                });
                let outcome = apply_update(
                    &model,
                    extension,
                    directory.clone(),
                    Arc::clone(&download),
                    cx,
                )
                .await;
                match outcome {
                    Ok(()) => {
                        result.updated += 1;
                        if enabled.contains(&name)
                            && model
                                .read_with(cx, |model, _| {
                                    model.extensions.registry.enabled(&name).is_none()
                                })
                                .unwrap_or(false)
                        {
                            result.disabled.push(name);
                        }
                    }
                    Err(error) => result.failed.push(format!("{name}: {error}")),
                }
            }
            let _ = view.update(cx, |view, cx| {
                view.finish_update_all(Ok(result), cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn finish_update_all(&mut self, result: Result<BatchResult, String>, cx: &mut Context<Self>) {
        self.mutation = None;
        match result {
            Ok(result) => {
                self.update_all_status = Some(result.message());
                self.error = (!result.failed.is_empty()).then(|| result.failed.join("\n"));
            }
            Err(error) => {
                self.update_all_status = None;
                self.error = Some(error);
            }
        }
        cx.notify();
    }
}

async fn apply_update(
    model: &Entity<AppModel>,
    extension: Extension,
    directory: PathBuf,
    download: Arc<
        impl Fn(&Extension, &Path) -> Result<tempfile::TempDir, String> + Send + Sync + 'static,
    >,
    cx: &mut gpui::AsyncApp,
) -> Result<(), String> {
    let current = extension.clone();
    let stage = io::network(move || download(&current, &directory)).await?;
    let task = model
        .update(cx, |model, cx| model.update_extension(extension, stage, cx))
        .map_err(|error| error.to_string())?;
    task.await
}

#[derive(Default)]
struct BatchResult {
    updated: usize,
    disabled: Vec<String>,
    failed: Vec<String>,
}

impl BatchResult {
    fn message(&self) -> String {
        let mut message = match self.updated {
            0 if self.failed.is_empty() => tr!("All extensions are up to date.").to_string(),
            1 => tr!("Updated 1 extension.").to_string(),
            count => tr!("Updated %@ extensions.", count).to_string(),
        };
        if !self.disabled.is_empty() {
            message.push(' ');
            message.push_str(&tr!(
                "Review permissions and enable when ready: %@.",
                self.disabled.join(", ")
            ));
        }
        message
    }
}
