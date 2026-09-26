mod background;
mod calls;
mod commands;
mod consent;
mod events;
mod http;
mod items;
mod local;
mod logs;
mod scripts;
mod surfaces;
#[cfg(test)]
mod tests;
mod workspace;
mod worktrees;

pub(crate) use commands::RunCommand;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use gpui::{Context, Entity, Window};
use muxy_app_core::PaneId;
use muxy_app_core::extensions::{Grants, Registry, Request, required_permission};
use muxy_protocol::ProjectId;
use serde_json::{Map, Value, json};

use super::{AppModel, ConnectionState};
use crate::views::webview::{Request as PageRequest, Webview};

pub(crate) struct Runtime {
    pub registry: Registry,
    grants: Result<Grants, String>,
    local: local::Local,
    local_revision: u64,
    pub epoch: u64,
    epochs: HashMap<String, u64>,
    client: Option<muxy_client::Client>,
    scripts: HashMap<u64, scripts::Running>,
    backgrounds: HashMap<String, background::Background>,
    loading_scripts: usize,
    loading_backgrounds: HashSet<String>,
    jobs: HashMap<(String, String), Job>,
    cancelled_jobs: HashSet<(String, String)>,
    next: u64,
    waiting: VecDeque<consent::Waiting>,
    sheet: Option<consent::Sheet>,
    pub logs: logs::Logs,
    native_modal: Option<scripts::Picker>,
    /// Stored overrides for each extension's declared settings.
    pub settings: BTreeMap<String, Map<String, Value>>,
    pub(crate) items: surfaces::Items,
    shortcuts: Vec<surfaces::Shortcut>,
    events: events::Tracker,
    /// Startup commands typed into new terminals once their sessions attach.
    pub(crate) startup: HashMap<PaneId, String>,
    gh_user: Option<(std::time::Instant, Value)>,
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
            backgrounds: HashMap::new(),
            loading_scripts: 0,
            loading_backgrounds: HashSet::new(),
            jobs: HashMap::new(),
            cancelled_jobs: HashSet::new(),
            next: 0,
            waiting: VecDeque::new(),
            sheet: None,
            logs: logs::Logs::new(),
            native_modal: None,
            settings: BTreeMap::new(),
            items: surfaces::Items::default(),
            shortcuts: Vec::new(),
            events: events::Tracker::default(),
            startup: HashMap::new(),
            gh_user: None,
        }
    }

    pub(super) fn disconnect(&mut self) {
        if let Some(client) = self.client.take() {
            client.disconnect();
        }
        self.events.disconnect();
    }

    fn epoch_for(&self, owner: &str) -> u64 {
        self.epochs.get(owner).copied().unwrap_or(0)
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
    Page(gpui::WeakEntity<Webview>, PageRequest),
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
    /// Set once runtime consent has been granted for this call.
    approved: bool,
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
                    let _ = model.update(cx, |model, _| {
                        model.extension_log("", format!("[muxy] {error}"));
                    });
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
                                model.extension_log(
                                    "",
                                    "[muxy] extension server connection closed".into(),
                                );
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
        self.extensions.settings = snapshot.settings;
        self.invalidate_webview_shortcuts();
        self.register_extension_surfaces(cx);
        self.load_extension_icons(cx);
        self.sync_backgrounds(cx);
        self.sync_extension_events(cx);
        self.close_hidden_popover(cx);
        self.sync_preferences(cx);
        cx.notify();
    }

    /// Whether the installed extensions have been read from disk yet.
    pub(crate) fn extensions_loaded(&self) -> bool {
        self.extensions.local_revision > 0
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
        self.stop_backgrounds(Some(owner));
        self.extensions
            .scripts
            .retain(|_, running| running.owner != owner);
        self.extensions
            .shortcuts
            .retain(|shortcut| shortcut.owner != owner);
        self.extensions.items.clear(Some(owner));
        if self
            .extensions
            .sheet
            .as_ref()
            .is_some_and(|sheet| sheet.owner == owner)
        {
            self.extensions.sheet = None;
        }
        let waiting = std::mem::take(&mut self.extensions.waiting);
        for waiting in waiting {
            if waiting.owner() == owner {
                waiting.fail("extension disabled", cx);
            } else {
                self.extensions.waiting.push_back(waiting);
            }
        }
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

    /// Stores (or with `None`, clears) the user's value for a declared setting.
    pub(crate) fn set_extension_setting(
        &mut self,
        owner: &str,
        key: &str,
        value: Option<Value>,
        cx: &mut Context<Self>,
    ) -> gpui::Task<Result<(), String>> {
        let (owner, key) = (owner.to_owned(), key.to_owned());
        self.change_extensions(
            move |state| {
                let extension = state
                    .registry
                    .extensions
                    .get(&owner)
                    .ok_or("extension is not installed")?;
                state.settings.set(extension, &key, value).map(|_| ())
            },
            cx,
        )
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

    /// Reloading stops everything extensions started, including background
    /// scripts and their runtime shortcuts and item changes.
    pub(super) fn stop_extensions(&mut self, cx: &mut Context<Self>) {
        self.stop_extension_tasks(cx);
        self.stop_backgrounds(None);
        self.extensions.shortcuts.clear();
        self.extensions.items.clear(None);
        self.invalidate_webview_shortcuts();
        self.remove_extension_surfaces(cx);
    }

    /// Cancels in-flight work, as when the server connection ends. Background
    /// scripts keep running, as open pages do.
    pub(super) fn stop_extension_tasks(&mut self, cx: &mut Context<Self>) {
        self.extensions.epoch = self.extensions.epoch.wrapping_add(1);
        for owner in self.extensions.registry.extensions.keys() {
            *self.extensions.epochs.entry(owner.clone()).or_default() += 1;
        }
        self.extensions.sheet = None;
        for waiting in self.extensions.waiting.drain(..).collect::<Vec<_>>() {
            waiting.fail("extension runtime stopped", cx);
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
        let backgrounds: HashSet<u64> = self
            .extensions
            .backgrounds
            .values()
            .map(|background| background.script)
            .collect();
        self.extensions
            .scripts
            .retain(|id, _| backgrounds.contains(id));
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

    /// Appends to an extension's log; an empty owner is the runtime's own log.
    pub(super) fn extension_log(&mut self, owner: &str, line: String) {
        let root = self
            .extensions
            .registry
            .extensions
            .get(owner)
            .map(|extension| extension.directory.clone());
        self.extensions.logs.append(owner, root.as_deref(), line);
    }

    pub(super) fn authorize_page(
        &self,
        owner: &str,
        verb: &str,
        args: &Value,
    ) -> Result<(), String> {
        if owner == "foundation" && cfg!(debug_assertions) {
            return Ok(());
        }
        let extension = self
            .extensions
            .registry
            .enabled(owner)
            .ok_or("extension is disabled or unloaded")?;
        if let Some(permission) = required_permission(verb, args)
            && !extension.allows(permission)
        {
            return Err(format!("permission denied ({permission})"));
        }
        Ok(())
    }

    /// Every live extension webview: tabs, panels, the modal, the popover, and the sidebar.
    pub(super) fn extension_views(&self) -> Vec<&Entity<Webview>> {
        self.webviews
            .panes
            .values()
            .map(|surface| &surface.view)
            .chain(
                self.webviews
                    .panels
                    .values()
                    .map(|panel| &panel.surface.view),
            )
            .chain(self.webviews.modal.iter().map(|modal| &modal.surface.view))
            .chain(
                self.webviews
                    .popover
                    .iter()
                    .map(|popover| &popover.surface.view),
            )
            .chain(
                self.webviews
                    .sidebar
                    .iter()
                    .map(|sidebar| &sidebar.surface.view),
            )
            .collect()
    }

    pub(super) fn extension_page_request(
        &mut self,
        view: &Entity<Webview>,
        request: &PageRequest,
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
                    approved: false,
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

    /// The project a call acts on: the `project` selector (a project's active
    /// worktree, like main) or the caller's default.
    fn extension_project(&self, args: &Value, default: ProjectId) -> Result<ProjectId, String> {
        let Some(identifier) = args["project"].as_str().filter(|id| !id.is_empty()) else {
            return Ok(default);
        };
        let project = self
            .find_project(identifier)
            .ok_or_else(|| format!("project not found {identifier}"))?;
        Ok(if project.parent_id.is_some() {
            project.id
        } else {
            self.preferred_worktree(project.id)
        })
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

    /// Checks main does before prompting, so users are never asked about calls
    /// that would fail anyway.
    fn consent_request(&self, call: &Call) -> Result<Option<Request>, String> {
        match call.verb.as_str() {
            "exec" | "exec.start" => {
                calls::exec_request(call, 1)?;
            }
            "panes.send" | "panes.sendKeys" | "panes.readScreen" => {
                Self::check_pane_request(&call.args)?;
            }
            "tabs.open" => self.check_tab_request(&call.args)?,
            "projects.delete" => {
                let project = self
                    .find_project(call.args["identifier"].as_str().unwrap_or(""))
                    .ok_or_else(|| {
                        format!(
                            "project not found {}",
                            call.args["identifier"].as_str().unwrap_or("")
                        )
                    })?;
                if project.home {
                    return Err("the home project cannot be deleted".into());
                }
                return Ok(Some(Request::project_delete(
                    &project.name,
                    &project.directory.display().to_string(),
                )));
            }
            "projects.create" => {
                if let Some(workspace) = call.args["workspace"]
                    .as_str()
                    .map(str::trim)
                    .filter(|workspace| !workspace.is_empty())
                    && self.find_workspace(workspace).is_none()
                {
                    return Err(format!("workspace not found '{workspace}'"));
                }
                let path = workspace::standardized(call.args["path"].as_str().unwrap_or(""));
                return Ok((call.args["createIfMissing"].as_bool() == Some(true)
                    && !path.exists())
                .then(|| Request::file("mkdir", &path.display().to_string())));
            }
            _ => (),
        }
        let location = self
            .state
            .project(call.project)
            .map(|project| project.directory.display().to_string())
            .unwrap_or_default();
        Ok(Request::for_call(
            &call.owner,
            &call.verb,
            &call.args,
            &location,
        ))
    }

    fn extension_call(&mut self, call: Call, window: &mut Window, cx: &mut Context<Self>) {
        if !self.call_live(&call, cx) {
            call.reply.send(Err("extension call expired".into()), cx);
            return;
        }
        if let Err(error) = self.authorize_page(&call.owner, &call.verb, &call.args) {
            call.reply.send(Err(error), cx);
            return;
        }
        if call.approved {
            self.dispatch_extension(call, window, cx);
            return;
        }
        if call.verb == "http.fetch" {
            self.check_http(call, cx);
            return;
        }
        match self.consent_request(&call) {
            Ok(Some(request)) => self.gate(call, request, window, cx),
            Ok(None) => self.dispatch_extension(call, window, cx),
            Err(error) => call.reply.send(Err(error), cx),
        }
    }
}
