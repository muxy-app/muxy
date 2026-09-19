use gpui::{AppContext, Context, Focusable, Window};
use muxy_app_core::{
    PaneContent,
    modal::{ModalItem, ModalOptions, ModalToken},
};
use muxy_protocol::{FilesAction, FilesRequest, ProjectId};
use muxy_ui::javascript::{Event, Script};
use serde_json::{Value, json};
use std::io::Read;

use super::{AppModel, Call, Reply};
use crate::views::{native_modal::NativeModal, overlays::Overlay};

pub(super) struct Running {
    pub owner: String,
    pub script: Script,
}

pub(super) struct Picker {
    pub owner: String,
    script: u64,
    view: gpui::WeakEntity<NativeModal>,
    token: ModalToken,
}

impl AppModel {
    pub(crate) fn run_extension_command(
        &mut self,
        owner: &str,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(extension) = self.extensions.registry.enabled(owner).cloned() else {
            return;
        };
        let Some(command) = extension
            .manifest
            .commands
            .iter()
            .find(|command| command.id == id)
        else {
            return;
        };
        let result = if let Some(action) = &command.action {
            match action["kind"].as_str().unwrap_or("") {
                "togglePanel" | "openPanel" => {
                    if let Err(error)=self.authorize_page(owner,"panels.open"){Err(error)} else {
                        self.panel_operation(owner,if action["kind"]=="togglePanel" {"panels.toggle"}else{"panels.open"}, &json!({"panelID":action["panel"],"data":action["data"]}),window,cx).map(|_|())
                    }
                }
                "openTab" => self.authorize_page(owner,"tabs.open").and_then(|()| self.extension_open_tab(owner,&json!({"kind":"extensionWebView","extension":{"id":owner,"tabType":action["tabType"],"data":action["data"],"singleton":action["singleton"]}}),cx).map(|_|())),
                "openModal" => self.authorize_page(owner,"modal.openWebview").and_then(|()| self.open_extension_modal(&extension,action,window,cx).map(|_|())),
                "runScript" => self.authorize_page(owner,"runScript").and_then(|()| self.start_extension_script(&extension,action["script"].as_str().unwrap_or(""),cx)),
                _=>Err("unsupported command action".into()),
            }
        } else {
            self.emit_extension_event(Some(owner), &format!("command.{id}"), Value::Null, cx);
            Ok(())
        };
        if let Err(error) = result {
            self.extensions.log(format!("{owner}: {error}"));
            self.fail(error, cx);
        }
        self.sync_extension_events(cx);
        cx.notify();
    }

    fn start_extension_script(
        &mut self,
        extension: &muxy_app_core::extensions::Extension,
        path: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.extensions.scripts.len() + self.extensions.loading_scripts >= 16 {
            return Err("too many active extension scripts; reload extensions to stop them".into());
        }
        let extension = extension.clone();
        let path = path.to_owned();
        let owner = extension.name.clone();
        let epoch = self.extensions.epoch_for(&owner);
        let generation = self.generation;
        let project = self.state.current_project().id;
        self.extensions.loading_scripts += 1;
        let task = crate::extensions::io::run(move || {
            let path = extension.resource(&path)?;
            let mut source = String::new();
            std::fs::File::open(path)
                .map_err(|error| error.to_string())?
                .take(8 * 1024 * 1024 + 1)
                .read_to_string(&mut source)
                .map_err(|error| error.to_string())?;
            if source.len() > 8 * 1024 * 1024 {
                return Err("extension script is too large".into());
            }
            Ok(format!(
                "{}({}, {});\n{}",
                include_str!("../../extensions/script.js"),
                json!(extension.name),
                include_str!("../../extensions/bridge.js"),
                source
            ))
        });
        cx.spawn(async move |model, cx| {
            let source = task.await;
            let _ = model.update(cx, |model, cx| {
                model.extensions.loading_scripts =
                    model.extensions.loading_scripts.saturating_sub(1);
                if generation != model.generation
                    || epoch != model.extensions.epoch_for(&owner)
                    || model.extensions.registry.enabled(&owner).is_none()
                {
                    return;
                }
                let result = source.and_then(|source| {
                    model.start_loaded_extension_script(owner, project, source, cx)
                });
                if let Err(error) = result {
                    model.fail(error, cx);
                }
            });
        })
        .detach();
        Ok(())
    }

    fn start_loaded_extension_script(
        &mut self,
        owner: String,
        project: ProjectId,
        source: String,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (script, events) = Script::start(source).map_err(|e| e.to_string())?;
        self.extensions.next += 1;
        let id = self.extensions.next;
        let epoch = self.extensions.epoch_for(&owner);
        let generation = self.generation;
        self.extensions.scripts.insert(
            id,
            Running {
                owner: owner.clone(),
                script,
            },
        );
        let handle = self.window;
        cx.spawn(async move |model, cx| {
            while let Ok(event) = events.recv().await {
                if handle
                    .update(cx, |_, window, cx| {
                        let _ = model.update(cx, |model, cx| match event {
                            Event::Error(error) => {
                                model.extensions.log(format!("{owner}: {error}"));
                                model.fail(format!("{owner}: {error}"), cx);
                            }
                            Event::Call(request) => {
                                match serde_json::from_str::<Value>(&request.request) {
                                    Ok(body) => {
                                        let reply = Reply::Script(id, request.reply);
                                        match model.extension_project(&body["args"], project) {
                                            Ok(project) => model.extension_call(
                                                Call {
                                                    owner: owner.clone(),
                                                    epoch,
                                                    generation,
                                                    project,
                                                    verb: body["verb"]
                                                        .as_str()
                                                        .unwrap_or("")
                                                        .into(),
                                                    args: body["args"].clone(),
                                                    reply,
                                                },
                                                window,
                                                cx,
                                            ),
                                            Err(error) => reply.send(Err(error), cx),
                                        }
                                    }
                                    Err(error) => {
                                        let _ = request.reply.try_send(
                                            super::envelope(Err(error.to_string())).to_string(),
                                        );
                                    }
                                }
                            }
                        });
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Ok(())
    }

    pub(super) fn extension_evaluate(&self, target: &Reply, source: &str, cx: &gpui::App) {
        match target {
            Reply::Page(view, request) => {
                if let Some(view) = view.upgrade()
                    && view.read(cx).native.generation() == request.generation
                {
                    view.read(cx).native.evaluate(source);
                }
            }
            Reply::Script(id, _) => {
                if let Some(running) = self.extensions.scripts.get(id) {
                    running.script.evaluate(source.into());
                }
            }
        }
    }

    pub(super) fn script_modal(
        &mut self,
        call: &Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let Reply::Script(script, _) = &call.reply else {
            return Err("native picker requires a script context".into());
        };
        if call.verb == "modal.open" {
            let value =
                |key: &str, default: &str| call.args[key].as_str().unwrap_or(default).to_owned();
            let options = ModalOptions {
                placeholder: value("placeholder", "Search…"),
                empty_label: value("emptyLabel", "No items"),
                no_match_label: value("noMatchLabel", "No matches"),
                search_toolbar: call.args["searchToolbar"].as_bool().unwrap_or(false),
                dynamic: call.args["dynamic"].as_bool().unwrap_or(false),
            };
            let mut channels = None;
            let view = cx.new(|cx| {
                let (view, result, queries) =
                    NativeModal::new(options, self.theme.clone(), self.metrics, cx);
                channels = Some((result, queries));
                view
            });
            let token = view.read(cx).token();
            let id = format!("native:{}", token.id);
            let (result, queries) = channels.ok_or("could not create picker")?;
            self.extensions.native_modal = Some(Picker {
                owner: call.owner.clone(),
                script: *script,
                view: view.downgrade(),
                token,
            });
            view.focus_handle(cx).focus(window);
            self.overlay_subscription = None;
            self.overlay = Some(Overlay::Native(view.clone()));
            let reply = call.reply.clone();
            let request = id.clone();
            let weak = view.downgrade();
            cx.spawn(async move |model,cx|{
                let selected=result.recv().await.ok().flatten();
                let _=model.update(cx,|model,cx|{
                    if matches!(&model.overlay,Some(Overlay::Native(view)) if view.entity_id()==weak.entity_id()){model.overlay=None;model.focus_requested=true;}
                    let value=selected.map(|item|json!({"id":item.id,"title":item.title,"subtitle":item.subtitle}));
                    model.extension_evaluate(&reply,&format!("globalThis.__muxiDeliverModalResult?.({}, {});",json!(request),json!(value)),cx);
                    if model.extensions.native_modal.as_ref().is_some_and(|picker|picker.token.id==token.id){model.extensions.native_modal=None;}
                    cx.notify();
                });
            }).detach();
            let reply = call.reply.clone();
            let request = id.clone();
            cx.spawn(async move |model,cx|{
                while let Ok(query)=queries.recv().await {
                    let _=model.update(cx,|model,cx|{
                        if let Some(picker)=model.extensions.native_modal.as_mut().filter(|picker|picker.token.id==query.token.id){picker.token=query.token;}
                        model.extension_evaluate(&reply,&format!("globalThis.__muxyDeliverModalQuery?.({}, {}, {}, {});",json!(request),json!(query.token.revision.to_string()),json!(query.query),json!({"caseSensitive":query.options.case_sensitive,"wholeWord":query.options.whole_word,"regex":query.options.regex})),cx);
                    });
                }
            }).detach();
            return Ok(json!({"requestID":id}));
        }
        let picker = self
            .extensions
            .native_modal
            .as_ref()
            .filter(|picker| picker.script == *script)
            .ok_or("picker is no longer open")?;
        let mut token = picker.token;
        if let Some(revision) = call.args["queryID"]
            .as_str()
            .and_then(|value| value.parse::<u64>().ok())
        {
            token.revision = revision;
        }
        let view = picker.view.upgrade().ok_or("picker is closed")?;
        if call.verb == "modal.finish" {
            view.update(cx, |view, cx| view.finish(token, cx));
        } else {
            let items = call.args["items"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .take(muxy_app_core::modal::MAX_ITEMS)
                        .filter_map(|item| {
                            Some(ModalItem {
                                id: item["id"].as_str()?.into(),
                                title: item["title"].as_str()?.into(),
                                subtitle: item["subtitle"].as_str().map(str::to_owned),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            view.update(cx, |view, cx| view.feed(token, items, cx));
        }
        Ok(Value::Null)
    }

    pub(super) fn script_webview_modal(
        &mut self,
        call: &Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let extension = self
            .extensions
            .registry
            .enabled(&call.owner)
            .ok_or("extension disabled")?
            .clone();
        let (id, result) = self.open_extension_modal(&extension, &call.args, window, cx)?;
        let reply = call.reply.clone();
        let request = id.clone();
        cx.spawn(async move |model, cx| {
            let result = result.recv().await.unwrap_or(Value::Null);
            let _ = model.update(cx, |model, cx| {
                model.extension_evaluate(
                    &reply,
                    &format!(
                        "globalThis.__muxiDeliverModalResult?.({}, {});",
                        json!(request),
                        result
                    ),
                    cx,
                );
            });
        })
        .detach();
        Ok(json!({"requestID":id}))
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "Events own their transient JSON payload"
    )]
    pub(in crate::model) fn emit_extension_event(
        &self,
        owner: Option<&str>,
        event: &str,
        payload: Value,
        cx: &gpui::App,
    ) {
        let permitted = |name: &str| {
            owner.is_none_or(|owner| name == owner)
                && self
                    .extensions
                    .registry
                    .enabled(name)
                    .is_some_and(|extension| extension.allows_event(event))
        };
        let source = format!("globalThis.__muxyEvent?.({}, {});", json!(event), payload);
        for view in self
            .webviews
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
        {
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

    pub(in crate::model) fn sync_extension_events(&mut self, cx: &mut Context<Self>) {
        let project = self.state.current_project();
        if self.extensions.last_project != Some(project.id) {
            crate::diagnostics::event(
                "extension.project",
                format_args!(
                    "from={:?} to={:?}",
                    self.extensions.last_project, project.id
                ),
            );
            self.extensions.last_project = Some(project.id);
            let payload = json!({"projectID":project.id.to_string(),"projectPath":project.directory.to_str(),"worktreeID":project.id.to_string(),"path":project.directory.to_str()});
            self.emit_extension_event(None, "project.switched", payload.clone(), cx);
            self.emit_extension_event(None, "worktree.switched", payload, cx);
        }
        self.sync_extension_file_watches(cx);
        let mut tabs = std::collections::BTreeMap::new();
        for project in self.state.projects() {
            for tab in &project.tabs {
                for pane in &tab.panes {
                    if let PaneContent::Webview(descriptor) = &pane.content {
                        let value = json!({"tabID":tab.id.to_string(),"tabInstanceID":pane.id.to_string(),"projectID":project.id.to_string(),"extensionID":descriptor.owner,"tabType":descriptor.kind,"data":descriptor.data,"title":pane.title});
                        tabs.insert(pane.id.to_string(), value);
                    }
                }
            }
        }
        for (id, value) in &tabs {
            match self.extensions.last_tabs.get(id) {
                None => self.emit_extension_event(None, "tab.created", value.clone(), cx),
                Some(old) if old != value => {
                    self.emit_extension_event(None, "tab.updated", value.clone(), cx);
                }
                _ => (),
            }
        }
        for (id, value) in &self.extensions.last_tabs {
            if !tabs.contains_key(id) {
                self.emit_extension_event(None, "tab.closed", value.clone(), cx);
            }
        }
        self.extensions.last_tabs = tabs;
        let focused = self.active_pane().map(|id| id.to_string());
        if self.extensions.last_focused != focused {
            self.extensions.last_focused.clone_from(&focused);
            if let Some(value) = focused
                .as_ref()
                .and_then(|id| self.extensions.last_tabs.get(id))
            {
                self.emit_extension_event(None, "tab.focused", value.clone(), cx);
            }
        }
    }

    fn sync_extension_file_watches(&mut self, cx: &mut Context<Self>) {
        if self.extensions.watch_pending {
            return;
        }
        let Some(client) = self.extensions.client.clone() else {
            return;
        };
        let mut desired = std::collections::BTreeSet::new();
        if self
            .extensions
            .registry
            .active()
            .any(|ext| ext.allows("files:read") && ext.allows_event("file.changed"))
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
        if desired == self.extensions.watched {
            return;
        }
        let removed: Vec<_> = self
            .extensions
            .watched
            .difference(&desired)
            .copied()
            .collect();
        let added: Vec<_> = desired
            .difference(&self.extensions.watched)
            .copied()
            .collect();
        self.extensions.watched = desired;
        self.extensions.watch_pending = true;
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
                model.extensions.watch_pending = false;
                for error in errors {
                    model.extensions.log(format!("File watcher: {error}"));
                }
                model.sync_extension_file_watches(cx);
            });
        })
        .detach();
    }

    pub(in crate::model) fn extension_files_changed(
        &self,
        project: ProjectId,
        changes: muxy_protocol::FileChanges,
        cx: &gpui::App,
    ) {
        let root = self
            .state
            .projects()
            .iter()
            .find(|p| p.id == project)
            .and_then(|p| p.directory.to_str());
        if changes.rescan
            && let Some(project) = self.state.project(project)
        {
            for pane in project.tabs.iter().flat_map(|tab| &tab.panes) {
                if let PaneContent::Webview(descriptor) = &pane.content
                    && let Some(path) = descriptor.data["filePath"].as_str()
                {
                    self.emit_extension_event(
                        None,
                        "file.changed",
                        json!({"path":path,"projectPath":root}),
                        cx,
                    );
                }
            }
        }
        for path in changes.paths {
            if let Ok(path) = muxy_app_core::extensions::api::path_text(&path) {
                self.emit_extension_event(
                    None,
                    "file.changed",
                    json!({"path":path,"projectPath":root}),
                    cx,
                );
            }
        }
    }
}
