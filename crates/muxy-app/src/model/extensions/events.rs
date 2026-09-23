//! Workspace events with main's names and payloads. Every payload value is a
//! string, as on main; optional keys are omitted rather than sent empty.

use std::collections::{BTreeMap, BTreeSet};

use gpui::Context;
use muxy_app_core::{PaneContent, Project, ProjectId, Tab};
use muxy_protocol::{AgentProvider, AgentState, FilesAction, FilesRequest};
use serde_json::{Map, Value, json};

use super::AppModel;

#[derive(Default)]
pub(super) struct Tracker {
    initialized: bool,
    root: Option<ProjectId>,
    current: Option<ProjectId>,
    projects: Value,
    tabs: BTreeMap<String, Value>,
    panes: BTreeMap<String, Value>,
    selected: BTreeMap<String, String>,
    focused: Option<String>,
    branches: BTreeMap<ProjectId, String>,
    agents: BTreeMap<String, Value>,
    pub(super) watched: BTreeSet<ProjectId>,
    pub(super) watch_pending: bool,
}

impl Tracker {
    pub(super) fn disconnect(&mut self) {
        self.watched.clear();
        self.watch_pending = false;
    }
}

/// A payload whose values are all strings, like main's `[String: String]`.
pub(super) fn strings<'a>(pairs: impl IntoIterator<Item = (&'a str, String)>) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .map(|(key, value)| (key.to_owned(), Value::String(value)))
            .collect(),
    )
}

pub(super) fn provider_id(provider: AgentProvider) -> &'static str {
    match provider {
        AgentProvider::Claude => "claude",
        AgentProvider::Codex => "codex",
        AgentProvider::OpenCode => "opencode",
        AgentProvider::Cursor => "cursor",
        AgentProvider::Copilot => "copilot",
        AgentProvider::Droid => "droid",
        AgentProvider::Pi => "pi",
        AgentProvider::Grok => "grok",
        AgentProvider::Kiro => "kiro",
        AgentProvider::Xal => "xal",
        AgentProvider::Antigravity => "antigravity",
    }
}

fn agent_rank(state: AgentState) -> (u8, &'static str) {
    match state {
        AgentState::Working => (2, "working"),
        AgentState::Blocked => (1, "waiting"),
        AgentState::Idle | AgentState::Unknown => (0, "idle"),
    }
}

/// The top-level project that owns a project or worktree.
pub(super) fn root_of(project: &Project) -> ProjectId {
    project.parent_id.unwrap_or(project.id)
}

impl AppModel {
    fn tab_context(&self, project: &Project, tab: &Tab, cx: &gpui::App) -> Value {
        let pane = tab
            .displayed_pane(self.state.window().active_pane)
            .or_else(|| tab.panes.first());
        let mut payload = Map::new();
        let mut put = |key: &str, value: String| {
            payload.insert(key.into(), Value::String(value));
        };
        put("tabID", tab.id.to_string());
        put("projectID", root_of(project).to_string());
        put("worktreeID", project.id.to_string());
        put("areaID", project.id.to_string());
        put("title", self.webview_title(tab, cx).to_owned());
        put("projectPath", project.directory.display().to_string());
        match pane.map(|pane| (pane.id, &pane.content)) {
            Some((id, PaneContent::Webview(descriptor))) => {
                put("kind", "extensionWebView".into());
                put("extensionID", descriptor.owner.clone());
                put("tabTypeID", descriptor.kind.clone());
                put("tabInstanceID", id.to_string());
                if !descriptor.data.is_null() {
                    put("data", descriptor.data.to_string());
                }
            }
            Some((id, _)) => {
                put("kind", "terminal".into());
                put("paneID", id.to_string());
                if let Some(directory) = self
                    .terminal(&id)
                    .and_then(|view| view.view.read(cx).directory())
                {
                    put("cwd", directory.display().to_string());
                }
            }
            None => put("kind", "terminal".into()),
        }
        Value::Object(payload)
    }

    /// Delivers a workspace event to every enabled extension allowed to
    /// subscribe to it; `owner` restricts delivery to one extension.
    pub(in crate::model) fn emit_extension_event(
        &self,
        owner: Option<&str>,
        event: &str,
        payload: &Value,
        cx: &gpui::App,
    ) {
        let permitted = |name: &str| {
            owner.is_none_or(|owner| name == owner)
                && self
                    .extensions
                    .registry
                    .enabled(name)
                    .is_some_and(|extension| {
                        extension.allows_event(event) || self.runtime_command_event(name, event)
                    })
        };
        let source = format!("globalThis.__muxyEvent?.({}, {payload});", json!(event));
        for view in self.extension_views() {
            if permitted(&view.read(cx).source.owner) {
                view.read(cx).native.evaluate(&source);
            }
        }
        for running in self.extensions.scripts.values() {
            if permitted(&running.owner) {
                running.script.evaluate(source.clone());
            }
        }
    }

    /// `extension.*` events stay inside one extension: pages talk to the
    /// background script, and the background script talks to its pages.
    pub(super) fn emit_local_event(
        &self,
        owner: &str,
        from_background: bool,
        event: &str,
        payload: &Value,
        cx: &gpui::App,
    ) -> Result<(), String> {
        if !muxy_app_core::extensions::local_event(event) {
            return Err("extension events must start with extension.".into());
        }
        if !serde_json::to_vec(payload).is_ok_and(|bytes| bytes.len() <= 65_536) {
            return Err("event payload must be JSON-serializable and at most 65536 bytes".into());
        }
        let source = format!("globalThis.__muxyEvent?.({}, {payload});", json!(event));
        if from_background {
            for view in self.extension_views() {
                if view.read(cx).source.owner == owner {
                    view.read(cx).native.evaluate(&source);
                }
            }
            return Ok(());
        }
        let background = self
            .extensions
            .backgrounds
            .get(owner)
            .and_then(|background| self.extensions.scripts.get(&background.script))
            .ok_or("background script unavailable")?;
        background.script.evaluate(source);
        Ok(())
    }

    pub(in crate::model) fn sync_extension_events(&mut self, cx: &mut Context<Self>) {
        let silent = !self.extensions.events.initialized;
        let mut emit = |model: &Self, name: &str, payload: Value| {
            if !silent {
                model.emit_extension_event(None, name, &payload, cx);
            }
        };
        let mut tabs = BTreeMap::new();
        let mut panes = BTreeMap::new();
        let mut selected = BTreeMap::new();
        for project in self.state.projects() {
            if let Some(tab) = self.state.window().selected_tab.get(&project.id) {
                selected.insert(project.id.to_string(), tab.to_string());
            }
            for tab in &project.tabs {
                let context = self.tab_context(project, tab, cx);
                for pane in &tab.panes {
                    if matches!(pane.content, PaneContent::Terminal { .. }) {
                        let mut pane_context = context.clone();
                        pane_context["paneID"] = json!(pane.id.to_string());
                        panes.insert(pane.id.to_string(), pane_context);
                    }
                }
                tabs.insert(tab.id.to_string(), context);
            }
        }
        let previous = std::mem::take(&mut self.extensions.events.panes);
        for (id, context) in &panes {
            if !previous.contains_key(id) {
                emit(self, "pane.created", context.clone());
            }
        }
        for (id, context) in &previous {
            if !panes.contains_key(id) {
                emit(self, "pane.closed", context.clone());
            }
        }
        let previous = std::mem::take(&mut self.extensions.events.tabs);
        for (id, context) in &tabs {
            if !previous.contains_key(id) {
                emit(self, "tab.created", context.clone());
            }
        }
        for (id, context) in &previous {
            if !tabs.contains_key(id) {
                emit(self, "tab.closed", context.clone());
            }
        }
        let visible = |context: &Value| {
            ["title", "projectPath", "cwd", "data"].map(|key| context.get(key).cloned())
        };
        for (id, context) in &tabs {
            if previous
                .get(id)
                .is_some_and(|old| visible(old) != visible(context))
            {
                emit(self, "tab.updated", context.clone());
            }
        }
        self.extensions.events.panes = panes;
        self.extensions.events.tabs = tabs;
        self.sync_project_events(&mut emit);
        let previous = std::mem::replace(&mut self.extensions.events.selected, selected);
        for (project, tab) in &self.extensions.events.selected {
            if previous.get(project) != Some(tab) {
                emit(
                    self,
                    "tab.focused",
                    strings([("areaID", project.clone()), ("tabID", tab.clone())]),
                );
            }
        }
        let focused = self.active_pane().map(|pane| pane.to_string());
        if self.extensions.events.focused != focused {
            self.extensions.events.focused.clone_from(&focused);
            if let (Some(tab), true) = (self.active_tab(), focused.is_some()) {
                let project = self.state.current_project();
                emit(
                    self,
                    "pane.focused",
                    strings([
                        ("projectID", root_of(project).to_string()),
                        ("worktreeID", project.id.to_string()),
                        ("areaID", project.id.to_string()),
                        ("tabID", tab.to_string()),
                    ]),
                );
            }
        }
        self.sync_agent_events(&mut emit);
        self.extensions.events.initialized = true;
        self.sync_extension_file_watches(cx);
    }

    fn sync_project_events(&mut self, emit: &mut impl FnMut(&Self, &str, Value)) {
        let project = self.state.current_project();
        let (root, current) = (root_of(project), project.id);
        if self.extensions.events.root != Some(root) {
            self.extensions.events.root = Some(root);
            emit(
                self,
                "project.switched",
                strings([("projectID", root.to_string())]),
            );
        }
        if self.extensions.events.current != Some(current) {
            self.extensions.events.current = Some(current);
            emit(
                self,
                "worktree.switched",
                strings([
                    ("projectID", root.to_string()),
                    ("worktreeID", current.to_string()),
                ]),
            );
        }
        let listing = Value::Array(
            self.state
                .projects()
                .iter()
                .filter(|project| project.parent_id.is_none())
                .map(|project| {
                    json!([
                        project.id.to_string(),
                        project.name,
                        project.icon,
                        project.color.as_str(),
                        project.logo.as_ref().map(|logo| logo.len()),
                        project.directory,
                    ])
                })
                .collect(),
        );
        if self.extensions.events.projects != listing {
            self.extensions.events.projects = listing;
            emit(self, "projects.changed", json!({}));
        }
        let mut branches = BTreeMap::new();
        for project in self.state.projects() {
            let Some(branch) = self
                .git
                .projects
                .get(&project.id)
                .and_then(|repository| repository.summary.as_ref())
                .and_then(|summary| summary.branch.clone())
            else {
                continue;
            };
            if self
                .extensions
                .events
                .branches
                .get(&project.id)
                .is_some_and(|previous| *previous != branch)
            {
                emit(
                    self,
                    "worktree.headChanged",
                    strings([
                        ("projectID", root_of(project).to_string()),
                        ("worktreeID", project.id.to_string()),
                        ("branch", branch.clone()),
                        ("path", project.directory.display().to_string()),
                    ]),
                );
            }
            branches.insert(project.id, branch);
        }
        self.extensions.events.branches = branches;
    }

    /// One entry per worktree: its most active agent, as `agents.list` reports.
    pub(super) fn agent_statuses(&self) -> BTreeMap<String, Value> {
        let mut best: BTreeMap<String, (u8, Value)> = BTreeMap::new();
        for agent in &self.activity.snapshot.agents {
            let Some((project, pane)) = self.state.projects().iter().find_map(|project| {
                project
                    .tabs
                    .iter()
                    .flat_map(|tab| &tab.panes)
                    .find(|pane| {
                        matches!(pane.content, PaneContent::Terminal { session: Some(session) } if session == agent.session)
                    })
                    .map(|pane| (project, pane.id))
            }) else {
                continue;
            };
            let (rank, status) = agent_rank(agent.state);
            let payload = strings([
                ("worktreeID", project.id.to_string()),
                ("projectID", root_of(project).to_string()),
                ("paneID", pane.to_string()),
                ("providerID", provider_id(agent.provider).to_owned()),
                ("status", status.to_owned()),
            ]);
            let entry = best
                .entry(project.id.to_string())
                .or_insert((rank, payload.clone()));
            if rank > entry.0 {
                *entry = (rank, payload);
            }
        }
        best.into_iter()
            .map(|(worktree, (_, payload))| (worktree, payload))
            .collect()
    }

    fn sync_agent_events(&mut self, emit: &mut impl FnMut(&Self, &str, Value)) {
        let agents = self.agent_statuses();
        for (worktree, payload) in &agents {
            if self.extensions.events.agents.get(worktree) != Some(payload) {
                emit(self, "agent.status", payload.clone());
            }
        }
        for (worktree, previous) in &self.extensions.events.agents {
            if !agents.contains_key(worktree) && previous["status"] != "idle" {
                let mut idle = previous.clone();
                idle["status"] = json!("idle");
                emit(self, "agent.status", idle);
            }
        }
        self.extensions.events.agents = agents;
    }

    fn sync_extension_file_watches(&mut self, cx: &mut Context<Self>) {
        if self.extensions.events.watch_pending {
            return;
        }
        let Some(client) = self.extensions.client.clone() else {
            return;
        };
        let mut desired = BTreeSet::new();
        if self
            .extensions
            .registry
            .active()
            .any(|extension| extension.allows_event("file.changed"))
        {
            desired.insert(self.state.current_project().id);
            for project in self.state.projects() {
                if desired.len() == 32 {
                    break;
                }
                if project.tabs.iter().flat_map(|tab| &tab.panes).any(|pane| matches!(&pane.content, PaneContent::Webview(d) if self.extensions.registry.enabled(&d.owner).is_some_and(|ext| ext.allows_event("file.changed")))) {
                    desired.insert(project.id);
                }
            }
        }
        if desired == self.extensions.events.watched {
            return;
        }
        let removed: Vec<_> = self
            .extensions
            .events
            .watched
            .difference(&desired)
            .copied()
            .collect();
        let added: Vec<_> = desired
            .difference(&self.extensions.events.watched)
            .copied()
            .collect();
        self.extensions.events.watched = desired;
        self.extensions.events.watch_pending = true;
        let generation = self.generation;
        let trace = crate::diagnostics::Span::new(
            "extension.watch",
            format_args!("generation={generation} added={added:?} removed={removed:?}"),
        );
        let task = cx.spawn(async move |_, _| {
            trace.stage(format_args!("phase=started"));
            let mut errors = Vec::new();
            for (projects, action) in [(removed, FilesAction::Unwatch), (added, FilesAction::Watch)]
            {
                for project in projects {
                    if let Err(error) = client
                        .files_async(FilesRequest {
                            project,
                            action: action.clone(),
                        })
                        .await
                    {
                        errors.push(error.to_string());
                    }
                }
            }
            trace.stage(format_args!("phase=reply errors={}", errors.len()));
            errors
        });
        cx.spawn(async move |model, cx| {
            let errors = task.await;
            let _ = model.update(cx, |model, cx| {
                if generation != model.generation {
                    return;
                }
                model.extensions.events.watch_pending = false;
                for error in errors {
                    model.extension_log("", format!("[muxy] file watcher: {error}"));
                }
                model.sync_extension_file_watches(cx);
            });
        })
        .detach();
    }

    fn file_changed(&self, project: &Project, relative: &str, cx: &gpui::App) {
        let root = project.directory.display().to_string();
        let path = if relative.is_empty() {
            root.clone()
        } else {
            format!("{}/{relative}", root.trim_end_matches('/'))
        };
        self.emit_extension_event(
            None,
            "file.changed",
            &strings([("path", path), ("projectPath", root)]),
            cx,
        );
    }

    pub(in crate::model) fn extension_files_changed(
        &self,
        project: ProjectId,
        changes: muxy_protocol::FileChanges,
        cx: &gpui::App,
    ) {
        let Some(project) = self.state.project(project) else {
            return;
        };
        if changes.rescan {
            for pane in project.tabs.iter().flat_map(|tab| &tab.panes) {
                if let PaneContent::Webview(descriptor) = &pane.content
                    && let Some(path) = descriptor.data["filePath"].as_str()
                {
                    self.file_changed(project, path, cx);
                }
            }
            self.file_changed(project, "", cx);
        }
        for path in changes.paths {
            if let Ok(path) = muxy_app_core::extensions::api::path_text(&path) {
                self.file_changed(project, path, cx);
            }
        }
    }

    /// New AI-hook notifications reach extensions as `notification.posted`,
    /// with the agent provider as their source.
    pub(in crate::model) fn activity_notifications_posted(&self, ids: &[u64], cx: &gpui::App) {
        for event in self
            .activity
            .snapshot
            .events
            .iter()
            .filter(|event| ids.contains(&event.id))
        {
            let located = self.state.projects().iter().find_map(|project| {
                project.tabs.iter().find_map(|tab| {
                    tab.panes
                        .iter()
                        .find(|pane| {
                            matches!(pane.content,
                                PaneContent::Terminal { session: Some(session) } if session == event.session)
                        })
                        .map(|pane| (project, tab.id, pane.id))
                })
            });
            let Some((project, tab, pane)) = located else {
                continue;
            };
            let payload = strings([
                ("paneID", pane.to_string()),
                ("projectID", root_of(project).to_string()),
                ("worktreeID", project.id.to_string()),
                ("worktreePath", project.directory.display().to_string()),
                ("tabID", tab.to_string()),
                ("source", provider_id(event.provider).to_owned()),
                (
                    "title",
                    format!("{} · {}", event.provider.name(), project.name),
                ),
                ("body", event.kind.description().to_owned()),
            ]);
            self.emit_extension_event(None, "notification.posted", &payload, cx);
        }
    }

    /// Repository state lives under `.git`, which main reports as file changes.
    pub(in crate::model) fn extension_git_changed(&self, project: ProjectId, cx: &gpui::App) {
        if let Some(project) = self
            .state
            .project(project)
            .filter(|project| self.extensions.events.watched.contains(&project.id))
        {
            self.file_changed(project, ".git/index", cx);
        }
    }
}
