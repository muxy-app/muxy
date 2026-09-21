mod calls;
mod commands;
mod local;
pub(crate) use commands::RunCommand;
mod scripts;
#[cfg(test)]
mod tests;
mod worktrees;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;

use gpui::{Context, Entity, Window};
use muxy_app_core::extensions::{Consent, Grants, Registry, required_permission};
use muxy_protocol::ProjectId;
use serde_json::{Value, json};

use super::{AppModel, ConnectionState};
use crate::views::webview::{Request, Webview};

pub(crate) struct Runtime {
    pub registry: Registry,
    grants: Result<Grants, String>,
    local: local::Local,
    local_revision: u64,
    pub epoch: u64,
    epochs: HashMap<String, u64>,
    client: Option<muxy_client::Client>,
    scripts: HashMap<u64, scripts::Running>,
    loading_scripts: usize,
    jobs: HashMap<(String, String), Job>,
    cancelled_jobs: std::collections::HashSet<(String, String)>,
    next: u64,
    pending: VecDeque<Call>,
    dialog: Option<muxy_ui::dialog::Confirmation>,
    pub logs: VecDeque<String>,
    native_modal: Option<scripts::Picker>,
    last_project: Option<ProjectId>,
    watched: std::collections::BTreeSet<ProjectId>,
    watch_pending: bool,
    last_tabs: BTreeMap<String, Value>,
    last_focused: Option<String>,
}

impl Runtime {
    pub(crate) fn new(profile: &Path) -> Self {
        Self {
            registry: Registry::empty(profile),
            grants: Err("extensions are loading".into()),
            local: local::Local::new(profile),
            local_revision: 0,
            epoch: 1,
            epochs: HashMap::new(),
            client: None,
            scripts: HashMap::new(),
            loading_scripts: 0,
            jobs: HashMap::new(),
            cancelled_jobs: std::collections::HashSet::new(),
            next: 0,
            pending: VecDeque::new(),
            dialog: None,
            logs: VecDeque::new(),
            native_modal: None,
            last_project: None,
            watched: std::collections::BTreeSet::new(),
            watch_pending: false,
            last_tabs: BTreeMap::new(),
            last_focused: None,
        }
    }

    pub(super) fn disconnect(&mut self) {
        if let Some(client) = self.client.take() {
            client.disconnect();
        }
        self.last_project = None;
        self.watched.clear();
        self.watch_pending = false;
    }

    fn epoch_for(&self, owner: &str) -> u64 {
        self.epochs.get(owner).copied().unwrap_or(0)
    }

    fn log(&mut self, message: String) {
        if self.logs.len() == 200 {
            self.logs.pop_front();
        }
        self.logs.push_back(message);
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.disconnect();
    }
}

struct Job {
    id: u64,
    call: Call,
}

#[derive(Clone)]
enum Reply {
    Page(gpui::WeakEntity<Webview>, Request),
    Script(u64, std::sync::mpsc::SyncSender<String>),
}

impl Reply {
    fn send(&self, result: Result<Value, String>, cx: &mut gpui::App) {
        match self {
            Self::Page(view, request) => {
                let _ = view.update(cx, |view, _| view.reply(request, result));
            }
            Self::Script(_, reply) => {
                let _ = reply.try_send(envelope(result).to_string());
            }
        }
    }

    fn key(&self) -> String {
        match self {
            Self::Page(view, request) => {
                format!("page:{:?}:{}", view.entity_id(), request.generation)
            }
            Self::Script(id, _) => format!("script:{id}"),
        }
    }
}

fn envelope(result: Result<Value, String>) -> Value {
    match result {
        Ok(value) => json!({"ok":true,"value":value}),
        Err(error) => json!({"ok":false,"cancelled":error == "cancelled","error":error}),
    }
}

#[derive(Clone)]
struct Call {
    owner: String,
    epoch: u64,
    generation: u64,
    project: ProjectId,
    verb: String,
    args: Value,
    reply: Reply,
}

impl AppModel {
    pub(super) fn extension_client(&mut self, cx: &mut Context<Self>) {
        let (sender, receiver) = async_channel::bounded(1);
        let generation = self.generation;
        if !self.send(crate::boot::Work::ExtensionClient(sender), cx) {
            return;
        }
        let socket = self.path.with_file_name("server.sock");
        cx.spawn(async move |model, cx| {
            let Ok(Some(app_client)) = receiver.recv().await else {
                return;
            };
            let instance = app_client.server_info().instance;
            drop(app_client);
            let connected = crate::extensions::io::run(move || {
                let client =
                    muxy_client::Client::connect(&socket).map_err(|error| error.to_string())?;
                if client.server_info().instance != instance {
                    client.disconnect();
                    return Err("server changed while connecting extensions".into());
                }
                let events = client.events().ok_or("extension events already taken")?;
                let (sender, receiver) = async_channel::bounded(64);
                std::thread::Builder::new()
                    .name("extension-events".into())
                    .spawn(move || {
                        for event in events {
                            if matches!(
                                event,
                                muxy_client::ClientEvent::FilesChanged { .. }
                                    | muxy_client::ClientEvent::Disconnected
                            ) && sender.send_blocking(event).is_err()
                            {
                                break;
                            }
                        }
                    })
                    .map_err(|error| error.to_string())?;
                Ok((client, receiver))
            })
            .await;
            let (client, events) = match connected {
                Ok(connected) => connected,
                Err(error) => {
                    let _ = model.update(cx, |model, _| model.extensions.log(error));
                    return;
                }
            };
            let installed = model
                .update(cx, |model, cx| {
                    if model.generation != generation || model.connection != ConnectionState::Ready
                    {
                        return false;
                    }
                    model.extensions.client = Some(client.clone());
                    model.extensions.last_project = None;
                    model.sync_extension_events(cx);
                    true
                })
                .unwrap_or(false);
            if !installed {
                client.disconnect();
                return;
            }
            drop(client);
            while let Ok(event) = events.recv().await {
                if model
                    .update(cx, |model, cx| {
                        if model.generation != generation {
                            return;
                        }
                        match event {
                            muxy_client::ClientEvent::FilesChanged { project, changes } => {
                                model.extension_files_changed(project, changes, cx);
                            }
                            muxy_client::ClientEvent::Disconnected => {
                                model.extensions.disconnect();
                                model
                                    .extensions
                                    .log("extension server connection closed".into());
                                if model.connection == ConnectionState::Ready
                                    && model.quitting == super::Quitting::Idle
                                {
                                    model.extension_client(cx);
                                }
                            }
                            _ => (),
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn receive_extension_snapshot(&mut self, snapshot: local::Snapshot, cx: &mut Context<Self>) {
        if snapshot.revision <= self.extensions.local_revision {
            return;
        }
        let disabled: Vec<_> = self
            .extensions
            .registry
            .active()
            .filter(|extension| snapshot.registry.enabled(&extension.name).is_none())
            .map(|extension| extension.name.clone())
            .collect();
        for owner in disabled {
            self.stop_extension(&owner, cx);
        }
        self.extensions.local_revision = snapshot.revision;
        self.extensions.registry = snapshot.registry;
        self.extensions.grants = snapshot.grants;
        self.invalidate_webview_shortcuts();
        self.register_extension_surfaces(cx);
        self.sync_extension_events(cx);
        cx.notify();
    }

    fn change_extensions(
        &mut self,
        operation: impl FnOnce(&mut local::State) -> Result<(), String> + Send + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        let task = self.extensions.local.submit(move |state| {
            let result = operation(state);
            Ok((state.snapshot(), result))
        });
        cx.spawn(async move |model, cx| {
            let (snapshot, result) = task.await?;
            model
                .update(cx, |model, cx| {
                    model.receive_extension_snapshot(snapshot, cx);
                })
                .map_err(|error| error.to_string())?;
            result
        })
    }

    pub(crate) fn refresh_installed_extensions(&mut self, cx: &mut Context<Self>) {
        let task = self.refresh_installed_extensions_task(cx);
        cx.spawn(async move |model, cx| {
            if let Err(error) = task.await {
                let _ = model.update(cx, |model, cx| model.fail(error, cx));
            }
        })
        .detach();
    }

    pub(crate) fn refresh_installed_extensions_task(
        &mut self,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        self.change_extensions(
            |state| {
                state.registry.reload();
                Ok(())
            },
            cx,
        )
    }

    pub(crate) fn load_unpacked_extension(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        self.change_extensions(
            move |state| state.registry.load_unpacked(&path).map(|_| ()),
            cx,
        )
    }

    pub(crate) fn reload_extensions(
        &mut self,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        let checks = self.extension_close_checks(None, cx);
        let epoch = self.extensions.epoch;
        cx.spawn(async move |model, cx| {
            for view in checks {
                let Ok(check) = view.update(cx, Webview::before_close) else {
                    return Err(
                        "The extension view could not be closed. Try reloading again.".into(),
                    );
                };
                if check.recv().await.unwrap_or(true) {
                    return Err(
                        "The extension kept an open view. Save or close it before reloading."
                            .into(),
                    );
                }
            }
            let task = model
                .update(cx, |model, cx| {
                    if model.extensions.epoch != epoch {
                        return Err("Extensions changed while reloading. Try again.".to_owned());
                    }
                    model.stop_extensions(cx);
                    Ok(model.refresh_installed_extensions_task(cx))
                })
                .map_err(|error| error.to_string())??;
            task.await
        })
    }

    fn stop_extension(&mut self, owner: &str, cx: &mut Context<Self>) {
        self.extensions.epoch += 1;
        *self.extensions.epochs.entry(owner.into()).or_default() += 1;
        if self
            .extensions
            .native_modal
            .as_ref()
            .is_some_and(|picker| picker.owner == owner)
        {
            self.extensions.native_modal = None;
            if matches!(
                self.overlay,
                Some(crate::views::overlays::Overlay::Native(_))
            ) {
                self.overlay = None;
            }
        }
        self.extensions
            .scripts
            .retain(|_, running| running.owner != owner);
        self.extensions.pending.retain(|call| {
            if call.owner == owner {
                call.reply.send(Err("extension disabled".into()), cx);
                false
            } else {
                true
            }
        });
        self.prune_extension_jobs(cx);
        self.remove_owner_surfaces(Some(owner), cx);
    }

    pub(in crate::model) fn prune_extension_jobs(&mut self, cx: &mut Context<Self>) {
        let expired: Vec<_> = self
            .extensions
            .jobs
            .iter()
            .filter(|(_, job)| !self.call_live(&job.call, cx))
            .map(|(key, _)| key.clone())
            .collect();
        for key in expired {
            if let Some(job) = self.extensions.jobs.remove(&key)
                && let Some(client) = self.extensions.client.clone()
            {
                cx.spawn(async move |_, _| {
                    let _ = client.cancel_exec_async(job.id).await;
                })
                .detach();
            }
        }
    }

    pub(crate) fn set_extension_enabled(
        &mut self,
        owner: &str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        let owner = owner.to_owned();
        self.change_extensions(move |state| state.registry.set_enabled(&owner, enabled), cx)
    }

    pub(crate) fn reset_extension_permissions(
        &mut self,
        owner: &str,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        let owner = owner.to_owned();
        self.change_extensions(
            move |state| {
                state
                    .grants
                    .as_mut()
                    .map_err(|error| error.clone())?
                    .clear(&owner)
            },
            cx,
        )
    }

    pub(crate) fn remove_extension(
        &mut self,
        owner: &str,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        let owner = owner.to_owned();
        self.change_extensions(
            move |state| {
                state.registry.set_enabled(&owner, false)?;
                state
                    .grants
                    .as_mut()
                    .map_err(|error| error.clone())?
                    .clear(&owner)?;
                state.registry.remove(&owner)
            },
            cx,
        )
    }

    pub(super) fn stop_extensions(&mut self, cx: &mut Context<Self>) {
        self.stop_extension_tasks(cx);
        self.remove_extension_surfaces(cx);
    }

    pub(super) fn stop_extension_tasks(&mut self, cx: &mut Context<Self>) {
        self.extensions.epoch = self.extensions.epoch.wrapping_add(1);
        for owner in self.extensions.registry.extensions.keys() {
            *self.extensions.epochs.entry(owner.clone()).or_default() += 1;
        }
        self.extensions.dialog = None;
        for call in self.extensions.pending.drain(..) {
            call.reply.send(Err("extension runtime stopped".into()), cx);
        }
        let jobs: Vec<_> = self.extensions.jobs.drain().map(|(_, job)| job).collect();
        for job in jobs {
            let error = Err("extension runtime stopped".into());
            if job.call.verb == "exec.start" {
                self.extension_evaluate(
                    &job.call.reply,
                    &format!(
                        "globalThis.__muxyResolveExec?.({}, {});",
                        job.call.args["id"],
                        envelope(error)
                    ),
                    cx,
                );
            } else {
                job.call.reply.send(error, cx);
            }
            if let Some(client) = self.extensions.client.clone() {
                cx.spawn(async move |_, _| {
                    let _ = client.cancel_exec_async(job.id).await;
                })
                .detach();
            }
        }
        self.extensions.scripts.clear();
        self.extensions.cancelled_jobs.clear();
        if self.extensions.native_modal.take().is_some()
            && matches!(
                self.overlay,
                Some(crate::views::overlays::Overlay::Native(_))
            )
        {
            self.overlay = None;
        }
    }

    pub(super) fn authorize_page(&self, owner: &str, verb: &str) -> Result<(), String> {
        if owner == "foundation" && cfg!(debug_assertions) {
            return Ok(());
        }
        let extension = self
            .extensions
            .registry
            .enabled(owner)
            .ok_or("extension is disabled or unloaded")?;
        if let Some(permission) = required_permission(verb)
            && !extension.allows(permission)
        {
            return Err(format!("permission denied ({permission})"));
        }
        Ok(())
    }

    pub(super) fn extension_page_request(
        &mut self,
        view: &Entity<Webview>,
        request: &Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = view.read(cx).source.owner.clone();
        let args = request.body["args"].clone();
        let project = self.extension_project(&args, self.page_project(view, cx));
        match project {
            Ok(project) => self.extension_call(
                Call {
                    owner: owner.clone(),
                    epoch: self.extensions.epoch_for(&owner),
                    generation: self.generation,
                    project,
                    verb: request.body["verb"].as_str().unwrap_or("").into(),
                    args,
                    reply: Reply::Page(view.downgrade(), request.clone()),
                },
                window,
                cx,
            ),
            Err(error) => view.read(cx).reply(request, Err(error)),
        }
    }

    fn page_project(&self, view: &Entity<Webview>, _cx: &gpui::App) -> ProjectId {
        self.state
            .projects()
            .iter()
            .find(|project| {
                project.tabs.iter().flat_map(|tab| &tab.panes).any(|pane| {
                    self.webviews
                        .panes
                        .get(&pane.id)
                        .is_some_and(|surface| surface.view == *view)
                })
            })
            .map_or_else(|| self.state.current_project().id, |project| project.id)
    }

    fn extension_project(&self, args: &Value, default: ProjectId) -> Result<ProjectId, String> {
        let Some(identifier) = args["project"].as_str().filter(|id| !id.is_empty()) else {
            return Ok(default);
        };
        self.state
            .projects()
            .iter()
            .find(|project| {
                project.id.to_string() == identifier
                    || project.name == identifier
                    || project.directory.to_str() == Some(identifier)
            })
            .map(|project| project.id)
            .ok_or_else(|| "project was not found".into())
    }

    fn call_live(&self, call: &Call, cx: &gpui::App) -> bool {
        call.epoch == self.extensions.epoch_for(&call.owner)
            && call.generation == self.generation
            && self.extensions.registry.enabled(&call.owner).is_some()
            && match &call.reply {
                Reply::Page(view, request) => view
                    .upgrade()
                    .is_some_and(|view| view.read(cx).native.generation() == request.generation),
                Reply::Script(id, _) => self.extensions.scripts.contains_key(id),
            }
    }

    fn extension_call(&mut self, call: Call, window: &mut Window, cx: &mut Context<Self>) {
        if !self.call_live(&call, cx) {
            call.reply.send(Err("extension call expired".into()), cx);
            return;
        }
        if let Err(error) = self.authorize_page(&call.owner, &call.verb) {
            call.reply.send(Err(error), cx);
            return;
        }
        if let Some(key) = Grants::key(&call.owner, &call.verb, &call.args) {
            let decision = self
                .extensions
                .grants
                .as_ref()
                .map_or(Consent::Deny, |grants| grants.decision(&key));
            match decision {
                Consent::Deny => {
                    call.reply.send(Err("operation denied".into()), cx);
                    return;
                }
                Consent::Ask => {
                    if self.extensions.pending.len() >= 64 {
                        call.reply
                            .send(Err("too many permission requests".into()), cx);
                        return;
                    }
                    self.extensions.pending.push_back(call);
                    self.next_extension_consent(window, cx);
                    return;
                }
                Consent::Allow => (),
            }
        }
        self.dispatch_extension(call, window, cx);
    }

    fn next_extension_consent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.extensions.dialog.is_some() {
            return;
        }
        let Some(call) = self.extensions.pending.pop_front() else {
            return;
        };
        if !self.call_live(&call, cx) {
            call.reply.send(Err("extension call expired".into()), cx);
            self.next_extension_consent(window, cx);
            return;
        }
        if call.verb.starts_with("dialog.") {
            self.dispatch_extension(call, window, cx);
            return;
        }
        let key = Grants::key(&call.owner, &call.verb, &call.args).unwrap_or_default();
        match self
            .extensions
            .grants
            .as_ref()
            .map_or(Consent::Deny, |grants| grants.decision(&key))
        {
            Consent::Allow => {
                self.dispatch_extension(call, window, cx);
                self.next_extension_consent(window, cx);
                return;
            }
            Consent::Deny => {
                call.reply.send(Err("operation denied".into()), cx);
                self.next_extension_consent(window, cx);
                return;
            }
            Consent::Ask => (),
        }
        let project = self
            .state
            .project(call.project)
            .map(|project| project.directory.display().to_string())
            .unwrap_or_default();
        let detail = if call.verb.starts_with("exec") {
            call.args["shell"]
                .as_str()
                .map_or_else(|| call.args["argv"].to_string(), str::to_owned)
        } else {
            call.args.to_string()
        };
        let (sender, receiver) = async_channel::bounded(1);
        let prompt = muxy_ui::dialog::confirm(
            window,
            &format!("Allow {} to {}?", call.owner, call.verb),
            &format!(
                "Project: {project}\n\n{}",
                detail.chars().take(3000).collect::<String>()
            ),
            "Allow",
            Some("Remember this permission"),
            move |response| {
                let _ = sender.try_send(response);
            },
        );
        match prompt {
            Ok(prompt) => self.extensions.dialog = Some(prompt),
            Err(error) => {
                call.reply.send(Err(error.to_string()), cx);
                self.next_extension_consent(window, cx);
                return;
            }
        }
        self.complete_extension_consent(call, key, receiver, cx);
    }

    fn complete_extension_consent(
        &mut self,
        call: Call,
        key: String,
        receiver: async_channel::Receiver<muxy_ui::dialog::ConfirmationResponse>,
        cx: &mut Context<Self>,
    ) {
        let handle = self.window;
        let local = self.extensions.local.clone();
        cx.spawn(async move |model, cx| {
            let response = receiver.recv().await.unwrap_or_default();
            let live = model
                .update(cx, |model, cx| model.call_live(&call, cx))
                .unwrap_or(false);
            let remembered = if live
                && matches!(
                    response,
                    muxy_ui::dialog::ConfirmationResponse::Confirmed {
                        dont_ask_again: true
                    }
                ) {
                local
                    .submit(move |state| {
                        state
                            .grants
                            .as_mut()
                            .map_err(|error| error.clone())?
                            .remember(&key, Consent::Allow)?;
                        Ok(Some(state.snapshot()))
                    })
                    .await
            } else {
                Ok(None)
            };
            let _ = handle.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    model.extensions.dialog = None;
                    match remembered {
                        Ok(snapshot) => {
                            if let Some(snapshot) = snapshot {
                                model.receive_extension_snapshot(snapshot, cx);
                            }
                            if model.call_live(&call, cx)
                                && matches!(
                                    response,
                                    muxy_ui::dialog::ConfirmationResponse::Confirmed { .. }
                                )
                            {
                                model.dispatch_extension(call, window, cx);
                            } else {
                                call.reply.send(Err("operation denied".into()), cx);
                            }
                        }
                        Err(error) => call.reply.send(Err(error), cx),
                    }
                    model.next_extension_consent(window, cx);
                });
            });
        })
        .detach();
    }
}
