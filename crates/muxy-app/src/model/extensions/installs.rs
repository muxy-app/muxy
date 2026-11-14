use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{AsyncApp, Context, Task, WeakEntity};
use muxy_app_core::extensions::Extension;
use muxy_ui::tr;

use super::AppModel;
use crate::extensions::{
    io,
    marketplace::{self, ExtensionDetails},
};

#[derive(Debug, PartialEq)]
pub(crate) enum ActiveInstall {
    Install { name: String },
    Update { name: String },
    UpdateAll,
}

#[derive(Debug, PartialEq)]
pub(crate) struct FinishedInstall {
    pub job: ActiveInstall,
    pub status: Option<String>,
    pub error: Option<String>,
}

/// The one marketplace job allowed at a time. It belongs to the model so it
/// outlives Settings, and its result waits here until a view has shown it.
#[derive(Default)]
pub(crate) struct Installs {
    pub active: Option<ActiveInstall>,
    pub status: Option<String>,
    pub finished: Option<FinishedInstall>,
    task: Option<Task<()>>,
}

impl AppModel {
    /// Returns whether the job started.
    pub(crate) fn install_marketplace_extension(
        &mut self,
        details: ExtensionDetails,
        cx: &mut Context<Self>,
    ) -> bool {
        self.install_marketplace_extension_with(details, marketplace::download, cx)
    }

    /// Returns whether the job started.
    pub(crate) fn install_marketplace_extension_with(
        &mut self,
        details: ExtensionDetails,
        download: impl FnOnce(&ExtensionDetails, &Path) -> Result<tempfile::TempDir, String>
        + Send
        + 'static,
        cx: &mut Context<Self>,
    ) -> bool {
        let name = details.name.clone();
        let registry = &self.extensions.registry;
        if self.extensions.installs.active.is_some() || registry.is_unpacked(&name) {
            return false;
        }
        let directory = registry.directory();
        let installed = registry.extensions.get(&name).cloned();
        let enabled = registry.enabled(&name).is_some();
        let (job, status) = if installed.is_some() {
            let status = tr!("Updating %@…", &name);
            (ActiveInstall::Update { name: name.clone() }, status)
        } else {
            let status = tr!("Installing %@…", &name);
            (ActiveInstall::Install { name: name.clone() }, status)
        };
        self.begin_install(job, status.into(), cx);
        self.extensions.installs.task = Some(cx.spawn(async move |model, cx| {
            let result =
                download_and_apply(&model, details, installed, directory, download, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_single_install(&name, enabled, result, cx);
            });
        }));
        true
    }

    /// Returns whether the job started.
    pub(crate) fn update_all_extensions(&mut self, cx: &mut Context<Self>) -> bool {
        self.update_all_extensions_with(
            marketplace::versions,
            |extension, directory| {
                let details = marketplace::detail(&extension.name)?;
                if marketplace::available_update(extension, &details).is_none() {
                    return Err(
                        tr!("The extension changed. Check for updates and try again.").into(),
                    );
                }
                marketplace::download(&details, directory)
            },
            cx,
        )
    }

    pub(crate) fn update_all_extensions_with(
        &mut self,
        versions: impl FnOnce(&[String]) -> Result<BTreeMap<String, String>, String> + Send + 'static,
        download: impl Fn(&Extension, &Path) -> Result<tempfile::TempDir, String>
        + Send
        + Sync
        + 'static,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.extensions.installs.active.is_some() {
            return false;
        }
        let registry = &self.extensions.registry;
        let installed: Vec<_> = registry
            .extensions
            .values()
            .filter(|extension| !registry.is_unpacked(&extension.name))
            .cloned()
            .collect();
        if installed.is_empty() {
            return false;
        }
        let directory = registry.directory();
        let enabled: BTreeSet<_> = registry
            .active()
            .map(|extension| extension.name.clone())
            .collect();
        self.begin_install(
            ActiveInstall::UpdateAll,
            tr!("Checking for extension updates…").into(),
            cx,
        );
        let download = Arc::new(download);
        self.extensions.installs.task = Some(cx.spawn(async move |model, cx| {
            let names: Vec<_> = installed
                .iter()
                .map(|extension| extension.name.clone())
                .collect();
            let versions = match io::network(move || versions(&names)).await {
                Ok(versions) => versions,
                Err(error) => {
                    let _ = model.update(cx, |model, cx| {
                        model.finish_install(None, Some(error), cx);
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
                let _ = model.update(cx, |model, cx| {
                    model.extensions.installs.status =
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
            let _ = model.update(cx, |model, cx| {
                let error = (!result.failed.is_empty()).then(|| result.failed.join("\n"));
                model.finish_install(Some(result.message()), error, cx);
            });
        }));
        true
    }

    fn begin_install(&mut self, job: ActiveInstall, status: String, cx: &mut Context<Self>) {
        let installs = &mut self.extensions.installs;
        installs.active = Some(job);
        installs.status = Some(status);
        installs.finished = None;
        cx.notify();
    }

    fn finish_single_install(
        &mut self,
        name: &str,
        was_enabled: bool,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        let registry = &self.extensions.registry;
        let result = result.and_then(|()| {
            if registry.extensions.contains_key(name) {
                return Ok(());
            }
            Err(
                tr!("The installed extension could not be loaded. Reload extensions to try again.")
                    .to_string(),
            )
        });
        let status = result.is_ok().then(|| {
            if matches!(
                self.extensions.installs.active,
                Some(ActiveInstall::Install { .. })
            ) {
                return tr!("Installed %@.", name).to_string();
            }
            BatchResult {
                updated: 1,
                disabled: (was_enabled && registry.enabled(name).is_none())
                    .then(|| name.to_owned())
                    .into_iter()
                    .collect(),
                failed: Vec::new(),
            }
            .message()
        });
        self.finish_install(status, result.err(), cx);
    }

    fn finish_install(
        &mut self,
        status: Option<String>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let installs = &mut self.extensions.installs;
        let Some(job) = installs.active.take() else {
            return;
        };
        installs.status = None;
        installs.task = None;
        installs.finished = Some(FinishedInstall { job, status, error });
        self.onboarding_install_finished(cx);
        cx.notify();
    }
}

async fn download_and_apply(
    model: &WeakEntity<AppModel>,
    details: ExtensionDetails,
    installed: Option<Extension>,
    directory: PathBuf,
    download: impl FnOnce(&ExtensionDetails, &Path) -> Result<tempfile::TempDir, String>
    + Send
    + 'static,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let name = details.name.clone();
    let staging = directory.clone();
    let stage = io::network(move || download(&details, &staging)).await?;
    let task = if let Some(installed) = installed {
        model.update(cx, |model, cx| model.update_extension(installed, stage, cx))
    } else {
        io::run(move || marketplace::install(&stage, &directory, &name)).await?;
        model.update(cx, AppModel::refresh_installed_extensions_task)
    };
    task.map_err(|error| error.to_string())?.await
}

async fn apply_update(
    model: &WeakEntity<AppModel>,
    extension: Extension,
    directory: PathBuf,
    download: Arc<
        impl Fn(&Extension, &Path) -> Result<tempfile::TempDir, String> + Send + Sync + 'static,
    >,
    cx: &mut AsyncApp,
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
