use gpui::{AppContext, Context, Focusable, Window};
use muxy_app_core::extensions::{Action, Extension};
use muxy_app_core::modal::{ModalItem, ModalOptions, ModalToken};
use muxy_protocol::ProjectId;
use muxy_ui::javascript::{Event, Script};
use serde_json::{Value, json};
use std::io::Read;

use super::{AppModel, Call, Reply, events::strings};
use crate::views::{native_modal::NativeModal, overlays::Overlay};

pub(super) struct Running {
    pub owner: String,
    pub script: Script,
}

/// The native picker an extension opened; one at a time app-wide.
pub(super) struct Picker {
    pub owner: String,
    id: String,
    opener: Reply,
    view: gpui::WeakEntity<NativeModal>,
    token: ModalToken,
    result: Option<Value>,
    waiter: Option<Reply>,
}

/// Wraps script source in the runtime that provides `muxy`, `console`, and timers.
pub(super) fn runtime(owner: &str, surface: &str, source: &str) -> String {
    format!(
        "{}({}, {}, {});\n{}",
        include_str!("../../extensions/script.js"),
        json!(owner),
        include_str!("../../extensions/bridge.js"),
        json!({"surface": surface, "persistent": surface == "background"}),
        source
    )
}

pub(super) fn read_source(extension: &Extension, path: &str) -> Result<String, String> {
    let path = extension.resource(path)?;
    let mut source = String::new();
    std::fs::File::open(path)
        .map_err(|error| error.to_string())?
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut source)
        .map_err(|error| error.to_string())?;
    if source.len() > 8 * 1024 * 1024 {
        return Err("extension script is too large".into());
    }
    Ok(source)
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
        let Some(command) = extension.manifest.command(id) else {
            if self.runtime_command_event(owner, &format!("command.{id}")) {
                self.emit_extension_event(
                    Some(owner),
                    &format!("command.{id}"),
                    &strings([("extension", owner.to_owned()), ("command", id.to_owned())]),
                    cx,
                );
            }
            return;
        };
        if let Some(permission) = command.action.permission()
            && !extension.allows(permission)
        {
            self.extension_log(
                owner,
                format!("[muxy] command {id} blocked: missing {permission} permission"),
            );
            return;
        }
        let result = match &command.action {
            Action::Event => {
                self.emit_extension_event(
                    Some(owner),
                    &format!("command.{id}"),
                    &strings([("extension", owner.to_owned()), ("command", id.to_owned())]),
                    cx,
                );
                Ok(())
            }
            Action::TogglePanel { panel, toggle } => self
                .panel_operation(
                    owner,
                    if *toggle {
                        "panels.toggle"
                    } else {
                        "panels.open"
                    },
                    &json!({ "panelID": panel }),
                    window,
                    cx,
                )
                .map(|_| ()),
            Action::OpenTab { tab_type, data } => self
                .extension_open_tab(
                    &json!({"kind":"extensionWebView","extension":{"id":owner,"tabType":tab_type,"data":data}}),
                    cx,
                )
                .map(|_| ()),
            Action::OpenModal {
                entry,
                width,
                height,
                dismiss_on_outside_click,
                data,
            } => self
                .open_extension_modal(
                    &extension,
                    &json!({"entry":entry,"width":width,"height":height,"dismissOnOutsideClick":dismiss_on_outside_click,"data":data}),
                    window,
                    cx,
                )
                .map(|_| ()),
            Action::RunScript { script } => self.start_extension_script(&extension, script, cx),
            Action::OpenPopover { .. } => Ok(()),
        };
        if let Err(error) = result {
            self.extension_log(owner, format!("[muxy] command {id} failed: {error}"));
            self.fail(error, cx);
        }
        self.sync_extension_events(cx);
        cx.notify();
    }

    fn start_extension_script(
        &mut self,
        extension: &Extension,
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
            read_source(&extension, &path).map(|source| runtime(&extension.name, "script", &source))
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
                    model.start_loaded_extension_script(owner.clone(), Some(project), source, cx)
                });
                if let Err(error) = result {
                    model.extension_log(&owner, format!("[muxy] runScript failed: {error}"));
                    model.fail(error, cx);
                }
            });
        })
        .detach();
        Ok(())
    }

    /// Runs prepared source on its own `JavaScriptCore` thread. `project` fixes
    /// the default project for its calls; background scripts use the current one.
    pub(super) fn start_loaded_extension_script(
        &mut self,
        owner: String,
        project: Option<ProjectId>,
        source: String,
        cx: &mut Context<Self>,
    ) -> Result<u64, String> {
        let (script, events) = Script::start(source).map_err(|e| e.to_string())?;
        self.extensions.next += 1;
        let id = self.extensions.next;
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
                        let _ = model.update(cx, |model, cx| {
                            model.script_event(id, &owner, project, event, window, cx);
                        });
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Ok(id)
    }

    /// Handles one script event. A call belongs to the current epoch and
    /// connection, so a background script keeps working after a reconnect;
    /// removing a script from `scripts` is what stops its calls.
    fn script_event(
        &mut self,
        id: u64,
        owner: &str,
        project: Option<ProjectId>,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            Event::Error(error) => {
                self.extension_log(owner, format!("[err] {error}"));
                if !self.is_background(id) {
                    self.extension_log(owner, "[muxy] runScript failed".into());
                    self.fail(format!("{owner}: {error}"), cx);
                }
            }
            Event::Call(request) => match serde_json::from_str::<Value>(&request.request) {
                Ok(body) => {
                    let reply = Reply::Script(id, request.reply);
                    let default = project.unwrap_or_else(|| self.state.current_project().id);
                    match self.extension_project(&body["args"], default) {
                        Ok(project) => self.extension_call(
                            Call {
                                owner: owner.into(),
                                epoch: self.extensions.epoch_for(owner),
                                generation: self.generation,
                                project,
                                verb: body["verb"].as_str().unwrap_or("").into(),
                                args: body["args"].clone(),
                                reply,
                                approved: false,
                            },
                            window,
                            cx,
                        ),
                        Err(error) => reply.send(Err(error), cx),
                    }
                }
                Err(error) => {
                    let _ = request
                        .reply
                        .try_send(super::envelope(Err(error.to_string())).to_string());
                }
            },
        }
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

    fn deliver_modal_result(&mut self, id: &str, value: Value, cx: &mut Context<Self>) {
        let Some(picker) = self
            .extensions
            .native_modal
            .as_mut()
            .filter(|picker| picker.id == id)
        else {
            return;
        };
        match &picker.opener {
            Reply::Script(..) => {
                let opener = picker.opener.clone();
                self.extension_evaluate(
                    &opener,
                    &format!(
                        "globalThis.__muxiDeliverModalResult?.({}, {value});",
                        json!(id)
                    ),
                    cx,
                );
            }
            Reply::Page(..) => {
                if let Some(waiter) = picker.waiter.take() {
                    waiter.send(Ok(value), cx);
                } else {
                    picker.result = Some(value);
                    return;
                }
            }
        }
        self.extensions.native_modal = None;
    }

    pub(super) fn script_modal(
        &mut self,
        call: &Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Result<Value, String>> {
        match call.verb.as_str() {
            "modal.open" => Some(self.open_picker(call, window, cx)),
            "modal.await" => {
                let id = call.args["requestID"].as_str().unwrap_or("");
                if let Some(picker) = self
                    .extensions
                    .native_modal
                    .as_mut()
                    .filter(|picker| picker.id == id && picker.opener.key() == call.reply.key())
                {
                    if let Some(value) = picker.result.take() {
                        self.extensions.native_modal = None;
                        Some(Ok(value))
                    } else {
                        picker.waiter = Some(call.reply.clone());
                        None
                    }
                } else {
                    Some(Ok(Value::Null))
                }
            }
            _ => Some(self.feed_picker(call, cx)),
        }
    }

    fn open_picker(
        &mut self,
        call: &Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let value = |key: &str, default: &str| {
            call.args[key]
                .as_str()
                .filter(|text| !text.is_empty())
                .unwrap_or(default)
                .chars()
                .take(muxy_app_core::modal::MAX_FIELD_CHARACTERS)
                .collect::<String>()
        };
        let options = ModalOptions {
            placeholder: value("placeholder", "Search…"),
            empty_label: value("emptyLabel", "No items"),
            no_match_label: value("noMatchLabel", "No matches"),
            search_toolbar: call.args["searchToolbar"].as_bool().unwrap_or(false),
            dynamic: call.args["dynamic"].as_bool().unwrap_or(false),
        };
        if let Some(previous) = self
            .extensions
            .native_modal
            .as_ref()
            .map(|picker| picker.id.clone())
        {
            self.deliver_modal_result(&previous, Value::Null, cx);
            self.extensions.native_modal = None;
        }
        let mut channels = None;
        let view = cx.new(|cx| {
            let (view, result, queries) =
                NativeModal::new(options, self.theme.clone(), self.metrics, cx);
            channels = Some((result, queries));
            view
        });
        let token = view.read(cx).token();
        self.extensions.next += 1;
        let id = format!("{}:{}", call.owner, self.extensions.next);
        let (result, queries) = channels.ok_or("could not create picker")?;
        self.extensions.native_modal = Some(Picker {
            owner: call.owner.clone(),
            id: id.clone(),
            opener: call.reply.clone(),
            view: view.downgrade(),
            token,
            result: None,
            waiter: None,
        });
        view.focus_handle(cx).focus(window);
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Native(view.clone()));
        let request = id.clone();
        let weak = view.downgrade();
        cx.spawn(async move |model, cx| {
            let selected = result.recv().await.ok().flatten();
            let _ = model.update(cx, |model, cx| {
                if matches!(&model.overlay, Some(Overlay::Native(view)) if view.entity_id() == weak.entity_id())
                {
                    model.overlay = None;
                    model.focus_requested = true;
                }
                let value = selected.map_or(Value::Null, |item| {
                    json!({"id": item.id, "title": item.title, "subtitle": item.subtitle})
                });
                model.deliver_modal_result(&request, value, cx);
                cx.notify();
            });
        })
        .detach();
        let opener = call.reply.clone();
        let request = id.clone();
        cx.spawn(async move |model, cx| {
            while let Ok(query) = queries.recv().await {
                let _ = model.update(cx, |model, cx| {
                    if let Some(picker) = model
                        .extensions
                        .native_modal
                        .as_mut()
                        .filter(|picker| picker.token.id == query.token.id)
                    {
                        picker.token = query.token;
                    }
                    model.extension_evaluate(
                        &opener,
                        &format!(
                            "globalThis.__muxyDeliverModalQuery?.({}, {}, {}, {});",
                            json!(request),
                            json!(query.token.revision.to_string()),
                            json!(query.query),
                            json!({"caseSensitive":query.options.case_sensitive,"wholeWord":query.options.whole_word,"regex":query.options.regex})
                        ),
                        cx,
                    );
                });
            }
        })
        .detach();
        Ok(json!({"requestID": id}))
    }

    fn feed_picker(&mut self, call: &Call, cx: &mut Context<Self>) -> Result<Value, String> {
        let picker = self
            .extensions
            .native_modal
            .as_ref()
            .filter(|picker| picker.opener.key() == call.reply.key())
            .ok_or("picker is no longer open")?;
        let mut token = picker.token;
        if let Some(revision) = call.args["queryID"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| call.args["queryID"].as_u64().map(|value| value.to_string()))
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
                                id: item["id"].as_str().filter(|id| !id.is_empty())?.into(),
                                title: item["title"].as_str().filter(|t| !t.is_empty())?.into(),
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
}
