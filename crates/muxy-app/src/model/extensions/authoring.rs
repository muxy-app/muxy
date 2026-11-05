//! Agents working on extensions: the picker's launch, and the `muxy extension`
//! commands they use to load what they build into this app.

use std::path::{Path, PathBuf};

use gpui::{Context, Task};
use muxy_app_core::ServerId;
use muxy_app_core::extensions::{Extension, workspace};
use muxy_core::capability::Feature;
use muxy_ui::picker::PickerStatus;
use muxy_ui::tr;
use serde_json::{Map, Value, json};

use super::AppModel;
use crate::ai::Provider;
use crate::extensions::authoring::{self, AgentTask, CliPlan, LocalTools};
use crate::extensions::publish::{self, GitHub, PublishFailure, PullRequest};
use crate::model::tabs::OpenTerminal;
use crate::views::overlays::Overlay;

/// The key in `[ai.providers]` that remembers the last agent used.
const AGENT_SETTING: &str = "extensions";
const LOG_LINES_IN_STATUS: usize = 20;

impl AppModel {
    /// The folder agents work in. Tests keep theirs beside their own profile.
    fn extension_workspace(&self) -> Result<PathBuf, String> {
        if cfg!(test) {
            return Ok(self.path.with_file_name("extension-workspace"));
        }
        muxy_core::dirs::extension_workspace().map_err(|error| error.to_string())
    }

    /// Brings the skill of an existing workspace up to date with this build.
    #[cfg(not(test))]
    pub(crate) fn refresh_extension_workspace(&self, cx: &mut Context<Self>) {
        let Ok(root) = self.extension_workspace() else {
            return;
        };
        cx.background_executor()
            .spawn(async move {
                let _ = workspace::refresh(&root);
            })
            .detach();
    }

    /// The agents to offer, the last one used first.
    pub(crate) fn extension_agents(&self) -> Vec<Provider> {
        let mut agents = self.ai.installed.clone();
        if let Some(last) = self.settings.ai.providers.get(AGENT_SETTING)
            && let Some(index) = agents.iter().position(|agent| agent.id == last)
        {
            let agent = agents.remove(index);
            agents.insert(0, agent);
        }
        agents
    }

    /// Stops a start that failed. A long reason needs more room than the picker
    /// has, so it goes where the app shows its other errors.
    fn fail_extension_agent(&mut self, error: String, cx: &mut Context<Self>) {
        self.close_overlay(cx);
        self.fail(error, cx);
    }

    fn show_extension_agent_getting_ready(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::ExtensionAgent(open)) = &self.overlay {
            open.picker.update(cx, |picker, cx| {
                picker.set_status(
                    PickerStatus::Loading(tr!("Getting the workspace ready…")),
                    cx,
                );
            });
        }
    }

    /// Starts `provider` on the open picker's task, once it can reach `muxy`.
    pub(crate) fn choose_extension_agent(&mut self, provider: Provider, cx: &mut Context<Self>) {
        let Some(Overlay::ExtensionAgent(picker)) = &self.overlay else {
            return;
        };
        if picker.attempt != 0 {
            return;
        }
        if let Some(reason) = self.capabilities.reason(Feature::CommandLineTool) {
            self.fail_extension_agent(reason.into_owned(), cx);
            return;
        }
        self.extensions.next += 1;
        let attempt = self.extensions.next;
        if let Some(Overlay::ExtensionAgent(picker)) = &mut self.overlay {
            picker.attempt = attempt;
        }
        match Self::muxy_for_agents() {
            CliPlan::Ready(directory) => {
                self.launch_extension_agent(provider, directory, attempt, cx);
            }
            CliPlan::Unavailable(reason) => self.fail_extension_agent(reason, cx),
            CliPlan::AskToInstall => self.ask_to_install_muxy(provider, attempt, cx),
        }
    }

    /// Whether the picker that started `attempt` is still open. Cancelling or
    /// choosing again abandons it, whatever it was still waiting for.
    pub(crate) fn extension_attempt_current(&self, attempt: u64) -> bool {
        matches!(
            &self.overlay,
            Some(Overlay::ExtensionAgent(picker)) if picker.attempt == attempt
        )
    }

    fn muxy_for_agents() -> CliPlan {
        let current = muxy_core::executable::current_path();
        let tool = current
            .as_ref()
            .map_err(ToString::to_string)
            .and_then(|path| {
                crate::cli_install::installed_tool(path).map_err(|error| error.to_string())
            });
        let unbundled = current
            .ok()
            .filter(|_| cfg!(debug_assertions))
            .map(|path| path.with_file_name("muxy"))
            .filter(|path| path.is_file());
        authoring::plan_cli(
            tool,
            crate::ai::find_program("muxy").as_deref(),
            crate::ai::find_in_login_shell("muxy").as_deref(),
            unbundled.as_deref(),
        )
    }

    /// Agents can't load what they build without `muxy`, so declining stops here.
    fn ask_to_install_muxy(&mut self, provider: Provider, attempt: u64, cx: &mut Context<Self>) {
        let title = tr!("Install the muxy Command?");
        let message = tr!(
            "Agents use the muxy command to load and reload the extensions they build. Muxy will link it at ~/.local/bin/muxy."
        );
        let install = tr!("Install and Continue");
        let cancel = tr!("Cancel");
        self.ask_dialog(
            self.window,
            async move |parent, cx| {
                muxy_ui::dialog::choose(parent, &title, &message, &[&install, &cancel], cx).await
            },
            move |model, answer, cx| {
                if !model.extension_attempt_current(attempt) {
                    return;
                }
                if matches!(answer, Ok(Some(0))) {
                    model.show_extension_agent_getting_ready(cx);
                    Self::install_muxy_then_launch(provider, attempt, cx);
                } else {
                    model.close_overlay(cx);
                }
            },
            cx,
        );
    }

    fn install_muxy_then_launch(provider: Provider, attempt: u64, cx: &mut Context<Self>) {
        let installing = crate::extensions::io::run(|| {
            crate::cli_install::install().map_err(|error| error.to_string())
        });
        cx.spawn(async move |model, cx| {
            let installed = installing.await;
            let _ = model.update(cx, |model, cx| {
                if !model.extension_attempt_current(attempt) {
                    return;
                }
                match installed {
                    Ok(path) => {
                        let directory = crate::ai::find_in_login_shell("muxy")
                            .is_none()
                            .then(|| path.parent().map(Path::to_owned))
                            .flatten();
                        model.launch_extension_agent(provider, directory, attempt, cx);
                    }
                    Err(error) => model.fail_extension_agent(
                        tr!("Couldn't install the muxy command: %@", error).to_string(),
                        cx,
                    ),
                }
            });
        })
        .detach();
    }

    fn launch_extension_agent(
        &mut self,
        provider: Provider,
        muxy_directory: Option<PathBuf>,
        attempt: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::ExtensionAgent(picker)) = &self.overlay else {
            return;
        };
        let task = picker.task.clone();
        let root = match self.extension_workspace() {
            Ok(root) => root,
            Err(error) => {
                self.fail_extension_agent(error, cx);
                return;
            }
        };
        self.show_extension_agent_getting_ready(cx);
        let preparing = {
            let task = task.clone();
            let root = root.clone();
            crate::extensions::io::network(move || {
                workspace::prepare(&root).map_err(|error| error.to_string())?;
                if let AgentTask::Clone { name, installed } = &task {
                    authoring::clone_extension(&LocalTools, &root, name, installed)?;
                }
                Ok(())
            })
        };
        cx.spawn(async move |model, cx| {
            let prepared = preparing.await;
            let _ = model.update(cx, |model, cx| {
                if !model.extension_attempt_current(attempt) {
                    return;
                }
                if let Err(error) = prepared.and_then(|()| {
                    model.start_extension_agent(
                        provider,
                        &task,
                        &root,
                        muxy_directory.as_deref(),
                        cx,
                    )
                }) {
                    model.fail_extension_agent(error, cx);
                }
            });
        })
        .detach();
    }

    fn start_extension_agent(
        &mut self,
        provider: Provider,
        task: &AgentTask,
        root: &Path,
        muxy_directory: Option<&Path>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let executable = provider
            .executable_path()
            .ok_or_else(|| tr!("%@ CLI is not installed", provider.name).to_string())?;
        self.start_extension_agent_at(provider, task, root, &executable, muxy_directory, cx)
    }

    fn start_extension_agent_at(
        &mut self,
        provider: Provider,
        task: &AgentTask,
        root: &Path,
        executable: &Path,
        muxy_directory: Option<&Path>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let start = task.start(root);
        let command = authoring::launch_line(provider, executable, &start.prompt, muxy_directory);
        authoring::typed_safely(&command)?;
        let project = self
            .state
            .server_home(ServerId::local())
            .ok_or_else(|| "this computer has no Home project".to_owned())?
            .id;
        let opened = self.open_terminal(
            OpenTerminal {
                project,
                title: Some(&start.title),
                command: Some(&command),
                directory: Some(&start.directory),
            },
            cx,
        )?;
        if opened.is_none() {
            if let Some(Overlay::ExtensionAgent(open)) = &mut self.overlay {
                open.attempt = 0;
            }
            self.update_extension_agents(cx);
            return Ok(());
        }
        self.close_overlay(cx);
        self.remember_extension_agent(provider, cx);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn extension_epoch_for_test(&self, owner: &str) -> u64 {
        self.extensions.epoch_for(owner)
    }

    #[cfg(test)]
    pub(crate) fn forget_extensions_loaded_for_test(&mut self) {
        self.extensions.local_revision = 0;
    }

    #[cfg(test)]
    pub(crate) fn start_agent_for_test(
        &mut self,
        provider: Provider,
        task: &AgentTask,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let root = self.extension_workspace()?;
        workspace::prepare(&root).map_err(|error| error.to_string())?;
        self.start_extension_agent_at(provider, task, &root, Path::new("/test/agent"), None, cx)
    }

    fn remember_extension_agent(&mut self, provider: Provider, cx: &mut Context<Self>) {
        match muxy_app_core::settings::Settings::save_ai_entry(
            &self.path.with_file_name("settings.toml"),
            "providers",
            AGENT_SETTING,
            Some(provider.id),
        ) {
            Ok(saved) => self.settings.ai = saved,
            Err(error) => self.fail(
                tr!("Could not save AI settings: %@", error.to_string()).to_string(),
                cx,
            ),
        }
    }

    /// The source folder a publish or edit works in.
    pub(crate) fn extension_source(&self, name: &str) -> Result<PathBuf, String> {
        self.extensions
            .registry
            .source_folder(name)
            .map(Path::to_owned)
            .ok_or_else(|| {
                tr!(
                    "%@ isn't an unpacked extension. Only unpacked folders can be published.",
                    name
                )
                .to_string()
            })
    }

    /// Offers an agent's help with what stopped a publish, before opening the
    /// picker, whose compact rows have no room to explain it.
    pub(crate) fn offer_publish_help(
        &mut self,
        name: &str,
        failure: &PublishFailure,
        cx: &mut Context<Self>,
    ) {
        let (title, action) = match failure {
            PublishFailure::NotReady(_) => (
                tr!("%@ Isn't Ready to Publish", name),
                tr!("Fix with Agent…"),
            ),
            _ => (
                tr!("Publishing %@ Needs the GitHub CLI", name),
                tr!("Continue with Agent…"),
            ),
        };
        let message = match failure {
            PublishFailure::NeedsGitHub(reason) => format!(
                "{reason}\n\n{}",
                tr!(
                    "An agent can guide you through it, and explains what publishing does before it changes anything."
                )
            ),
            _ => failure.describe(),
        };
        let Ok(folder) = self.extension_source(name) else {
            return;
        };
        let task = AgentTask::Publish {
            name: name.to_owned(),
            folder,
        };
        let cancel = tr!("Cancel");
        self.ask_dialog(
            self.window,
            async move |parent, cx| {
                muxy_ui::dialog::choose(parent, &title, &message, &[&action, &cancel], cx).await
            },
            move |model, answer, cx| {
                if matches!(answer, Ok(Some(0))) {
                    model.open_extension_agent(task, cx);
                }
            },
            cx,
        );
    }

    /// Checks, asks the user, then opens or updates a pull request.
    pub(crate) fn publish_extension(
        &mut self,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Task<Result<PullRequest, PublishFailure>> {
        let source = match self.extension_source(name) {
            Ok(source) => source,
            Err(error) => return Task::ready(Err(PublishFailure::Failed(error))),
        };
        let planning = {
            let source = source.clone();
            crate::extensions::io::network(move || Ok(publish::plan(&LocalTools, &source)))
        };
        cx.spawn(async move |model, cx| {
            let plan = planning.await.map_err(PublishFailure::Failed)??;
            let (approved, approval) = async_channel::bounded(1);
            let message = plan.confirmation();
            let title = tr!(
                "Publish %@ %@?",
                &plan.readiness.name,
                &plan.readiness.version
            );
            model
                .update(cx, |model, cx| {
                    let publish_label = tr!("Publish");
                    let cancel = tr!("Cancel");
                    model.ask_dialog(
                        model.window,
                        async move |parent, cx| {
                            muxy_ui::dialog::choose(
                                parent,
                                &title,
                                &message,
                                &[&publish_label, &cancel],
                                cx,
                            )
                            .await
                        },
                        move |_, answer, _| {
                            let _ = approved.try_send(matches!(answer, Ok(Some(0))));
                        },
                        cx,
                    );
                })
                .map_err(|error| PublishFailure::Failed(error.to_string()))?;
            if !approval.recv().await.unwrap_or(false) {
                return Err(PublishFailure::Declined);
            }
            crate::extensions::io::network(move || {
                publish::publish(
                    &LocalTools,
                    &source,
                    &plan.readiness,
                    &plan.login,
                    plan.update,
                )
            })
            .await
            .map_err(PublishFailure::Failed)
        })
    }

    /// A `muxy extension` command from the app socket.
    pub(crate) fn extension_cli_call(
        &mut self,
        verb: &str,
        args: &Map<String, Value>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Value, String>> {
        if !self.extensions_loaded() {
            return self.extension_cli_call_once_loaded(verb.to_owned(), args.clone(), cx);
        }
        let name = args.get("name").and_then(Value::as_str).unwrap_or_default();
        let path = args.get("path").and_then(Value::as_str);
        match verb {
            "extension.list" => Task::ready(Ok(self.extension_list())),
            "extension.status" => Task::ready(self.extension_status(name)),
            "extension.logs" => {
                let lines = args.get("lines").and_then(Value::as_u64).unwrap_or(100);
                Task::ready(self.extension_logs(name, usize::try_from(lines).unwrap_or(usize::MAX)))
            }
            "extension.load" => match path {
                Some(path) => Self::load_extension_folder(PathBuf::from(path), cx),
                None => Task::ready(Err("expected a folder".into())),
            },
            "extension.enable" => {
                let task = self.set_extension_enabled(name, true, cx);
                acknowledged(task, cx)
            }
            "extension.disable" => {
                let task = self.stop_then(name, false, cx);
                acknowledged(task, cx)
            }
            "extension.unload" => {
                if self.extensions.registry.is_unpacked(name) {
                    let task = self.stop_then(name, true, cx);
                    acknowledged(task, cx)
                } else {
                    Task::ready(Err(tr!(
                        "%@ isn't an unpacked extension. Remove marketplace extensions in Settings → Extensions.",
                        name
                    )
                    .to_string()))
                }
            }
            "extension.reload" => {
                let task = self.reload_extension(name, cx);
                acknowledged(task, cx)
            }
            "extension.validate" => self.validate_extension(name, path, cx),
            "extension.publish" => {
                let task = self.publish_extension(name, cx);
                cx.background_executor().spawn(async move {
                    match task.await {
                        Ok(request) => Ok(json!({
                            "url": request.url,
                            "branch": request.branch,
                            "updated": request.updated,
                        })),
                        Err(failure) => Err(failure.describe()),
                    }
                })
            }
            _ => Task::ready(Err(format!("unknown command {verb}"))),
        }
    }

    /// The command that started the app can arrive before it has read its
    /// extensions, and would otherwise see none.
    fn extension_cli_call_once_loaded(
        &mut self,
        verb: String,
        args: Map<String, Value>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Value, String>> {
        let refresh = self.refresh_installed_extensions_task(cx);
        cx.spawn(async move |model, cx| {
            refresh.await?;
            model
                .update(cx, |model, cx| model.extension_cli_call(&verb, &args, cx))
                .map_err(|error| error.to_string())?
                .await
        })
    }

    fn extension_entry(&self, name: &str) -> Option<Value> {
        let registry = &self.extensions.registry;
        let source = registry.source_folder(name);
        let extension = registry.extensions.get(name);
        if extension.is_none() && source.is_none() {
            return None;
        }
        let path = source
            .map(Path::to_owned)
            .or_else(|| extension.map(|extension| extension.directory.clone()))?;
        Some(json!({
            "name": name,
            "version": extension.map_or("", |extension| extension.version.as_str()),
            "description": extension.map_or("", |extension| extension.manifest.description.as_str()),
            "enabled": registry.enabled(name).is_some(),
            "unpacked": source.is_some(),
            "path": path,
            "permissions": extension.map(|extension| &extension.manifest.permissions),
        }))
    }

    fn extension_list(&self) -> Value {
        let extensions: Vec<_> = self
            .extensions
            .registry
            .extensions
            .keys()
            .filter_map(|name| self.extension_entry(name))
            .collect();
        json!({
            "extensions": extensions,
            "load_errors": self.extensions.registry.errors,
        })
    }

    fn extension_status(&self, name: &str) -> Result<Value, String> {
        let mut entry = self.extension_entry(name).ok_or_else(|| not_loaded(name))?;
        let path = entry["path"].as_str().unwrap_or_default().to_owned();
        let errors: Vec<_> = self
            .extensions
            .registry
            .errors
            .iter()
            .filter(|error| {
                error.starts_with(&format!("{path}:")) || error.starts_with(&format!("{name}:"))
            })
            .cloned()
            .collect();
        entry["load_errors"] = json!(errors);
        if self
            .extensions
            .registry
            .extensions
            .get(name)
            .is_some_and(|extension| extension.manifest.background.is_some())
        {
            entry["background_running"] = json!(self.background_running(name));
        }
        entry["log"] = json!(self.extension_log_tail(name, LOG_LINES_IN_STATUS));
        Ok(entry)
    }

    fn extension_log_tail(&self, name: &str, lines: usize) -> Vec<String> {
        let all: Vec<_> = self.extensions.logs.tail(name).map(str::to_owned).collect();
        all[all.len().saturating_sub(lines)..].to_vec()
    }

    fn extension_logs(&self, name: &str, lines: usize) -> Result<Value, String> {
        if self.extension_entry(name).is_none() {
            return Err(not_loaded(name));
        }
        Ok(json!({ "lines": self.extension_log_tail(name, lines) }))
    }

    /// Loading the folder that is already loaded reloads it, so an agent can
    /// repeat itself. The same name from another folder is an error.
    fn load_extension_folder(path: PathBuf, cx: &mut Context<Self>) -> Task<Result<Value, String>> {
        let reading = crate::extensions::io::run(move || {
            let extension = Extension::load(&path)?;
            let folder = path.canonicalize().map_err(|error| error.to_string())?;
            Ok((extension.name, folder))
        });
        cx.spawn(async move |model, cx| {
            let (name, folder) = reading.await?;
            let change = model
                .update(cx, |model, cx| {
                    let registry = &model.extensions.registry;
                    if let Some(loaded) = registry.source_folder(&name) {
                        if loaded != folder {
                            return Err(tr!(
                                "An extension named %@ is already loaded from %@. Unload it first.",
                                &name,
                                loaded.display().to_string()
                            )
                            .to_string());
                        }
                        return Ok(model.reload_extension(&name, cx));
                    }
                    if registry.extensions.contains_key(&name) {
                        return Err(tr!(
                            "A marketplace extension named %@ is installed. Choose another package name.",
                            &name
                        )
                        .to_string());
                    }
                    Ok(model.load_unpacked_extension(folder, cx))
                })
                .map_err(|error| error.to_string())??;
            change.await?;
            Ok(json!({ "name": name }))
        })
    }

    /// Reloads one extension, leaving the others running. It is only stopped
    /// once its folder has been read: one that no longer loads, or now names
    /// another package, is reported and leaves the running copy alone.
    pub(crate) fn reload_extension(
        &mut self,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        let registry = &self.extensions.registry;
        let folder = match registry.source_folder(name) {
            Some(folder) => folder.to_owned(),
            None if registry.extensions.contains_key(name) => registry.directory().join(name),
            None => return Task::ready(Err(not_loaded(name))),
        };
        let name = name.to_owned();
        let checks = self.extension_close_checks(Some(&name), cx);
        let reading = {
            let name = name.clone();
            crate::extensions::io::run(move || {
                let extension = Extension::load(&folder)?;
                if extension.name == name {
                    Ok(())
                } else {
                    Err(tr!(
                        "Its package name changed to %@. Unload it and load the folder again.",
                        extension.name
                    )
                    .to_string())
                }
            })
        };
        cx.spawn(async move |model, cx| {
            reading.await?;
            close_views(checks, cx).await?;
            let reloading = {
                let name = name.clone();
                model
                    .update(cx, |model, _| {
                        model.extensions.local.submit(move |state| {
                            let result = state.registry.reload_one(&name);
                            Ok((state.snapshot(), result))
                        })
                    })
                    .map_err(|error| error.to_string())?
            };
            let (snapshot, result) = reloading.await?;
            model
                .update(cx, |model, cx| {
                    if result.is_ok() {
                        model.stop_extension(&name, cx);
                    }
                    model.receive_extension_snapshot(snapshot, cx);
                })
                .map_err(|error| error.to_string())?;
            result
        })
    }

    /// Lets the extension's pages close, then disables or unloads it.
    fn stop_then(
        &mut self,
        name: &str,
        unload: bool,
        cx: &mut Context<Self>,
    ) -> Task<Result<(), String>> {
        if self.extension_entry(name).is_none() {
            return Task::ready(Err(not_loaded(name)));
        }
        let name = name.to_owned();
        let checks = self.extension_close_checks(Some(&name), cx);
        cx.spawn(async move |model, cx| {
            close_views(checks, cx).await?;
            let task = model
                .update(cx, |model, cx| {
                    if unload {
                        model.remove_extension(&name, cx)
                    } else {
                        model.set_extension_enabled(&name, false, cx)
                    }
                })
                .map_err(|error| error.to_string())?;
            task.await
        })
    }

    fn validate_extension(
        &self,
        name: &str,
        path: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Task<Result<Value, String>> {
        let source = match path {
            Some(path) => PathBuf::from(path),
            None => match self.extension_source(name) {
                Ok(source) => source,
                Err(error) => return Task::ready(Err(error)),
            },
        };
        let checking = crate::extensions::io::network(move || {
            let mut readiness = publish::check(&source);
            let github = publish::github_account(&LocalTools);
            if matches!(github, GitHub::User(_)) && readiness.problems.is_empty() {
                let published = publish::published_version(&LocalTools, &readiness.name)?;
                publish::check_against_store(&mut readiness, published.as_ref());
            }
            Ok(json!({
                "name": readiness.name,
                "version": readiness.version,
                "problems": readiness.problems,
                "files": readiness.files,
                "github": match github {
                    GitHub::Missing => "missing".to_owned(),
                    GitHub::SignedOut => "signed-out".to_owned(),
                    GitHub::Unreachable(_) => "unreachable".to_owned(),
                    GitHub::User(login) => login,
                },
            }))
        });
        cx.background_executor().spawn(checking)
    }
}

fn not_loaded(name: &str) -> String {
    tr!("%@ isn't loaded. Run muxy extension list.", name).to_string()
}

async fn close_views(
    checks: Vec<gpui::Entity<crate::views::webview::Webview>>,
    cx: &mut gpui::AsyncApp,
) -> Result<(), String> {
    for view in checks {
        let check = view
            .update(cx, crate::views::webview::Webview::before_close)
            .map_err(|error| error.to_string())?;
        if check.recv().await.unwrap_or(true) {
            return Err(tr!(
                "The extension kept an open view. Save or close it before continuing."
            )
            .to_string());
        }
    }
    Ok(())
}

fn acknowledged(
    task: Task<Result<(), String>>,
    cx: &mut Context<AppModel>,
) -> Task<Result<Value, String>> {
    cx.background_executor()
        .spawn(async move { task.await.map(|()| Value::Null) })
}
