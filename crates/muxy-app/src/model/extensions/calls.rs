use gpui::{Context, Window};
use muxy_app_core::extensions::api;
use muxy_app_core::{PaneContent, webview::WebviewDescriptor};
use muxy_protocol::{ExecRequest, FilesRequest, GitRequest, ServerPath};
use serde_json::{Value, json};

use super::{AppModel, Call, Reply, envelope};

impl AppModel {
    #[allow(
        clippy::too_many_lines,
        reason = "Keep the extension API routing table together"
    )]
    pub(super) fn dispatch_extension(
        &mut self,
        call: Call,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = match call.verb.as_str() {
            "script.finished" => {
                if let Reply::Script(id, _) = &call.reply {
                    self.extensions.scripts.remove(id);
                }
                Ok(Value::Null)
            }
            "console" => {
                self.extensions.log(format!(
                    "{}: {}",
                    call.owner,
                    call.args["message"].as_str().unwrap_or("")
                ));
                Ok(Value::Null)
            }
            "events.subscribe" => {
                if self
                    .extensions
                    .registry
                    .enabled(&call.owner)
                    .is_some_and(|extension| {
                        extension.allows_event(call.args["event"].as_str().unwrap_or(""))
                    })
                {
                    Ok(Value::Null)
                } else {
                    Err("event is not declared in the manifest".into())
                }
            }
            "events.emit" => {
                let name = call.args["event"].as_str().unwrap_or("");
                if name.starts_with("extension.") && name.len() > 10 && name.len() <= 200 {
                    self.emit_extension_event(
                        Some(&call.owner),
                        name,
                        call.args["payload"].clone(),
                        cx,
                    );
                    Ok(Value::Null)
                } else {
                    Err("extension events must start with extension.".into())
                }
            }
            verb if verb.starts_with("storage.") => {
                let operation = call.clone();
                let task = self.extensions.local.submit(move |state| {
                    let extension = state
                        .registry
                        .enabled(&operation.owner)
                        .ok_or("extension disabled")?;
                    state
                        .storage
                        .call(extension, &operation.verb, &operation.args)
                });
                cx.spawn(async move |model, cx| {
                    let result = task.await;
                    let _ = model.update(cx, |model, cx| {
                        if model.call_live(&call, cx) {
                            call.reply.send(result, cx);
                        } else {
                            call.reply.send(Err("extension call expired".into()), cx);
                        }
                    });
                })
                .detach();
                return;
            }
            "tabs.list" => Ok(Value::Array(self.extension_tabs())),
            "tabs.switch" => {
                let id = call.args["identifier"].as_str().unwrap_or("");
                let tab = self
                    .state
                    .projects()
                    .iter()
                    .flat_map(|project| &project.tabs)
                    .find(|tab| {
                        tab.id.to_string() == id
                            || tab.panes.iter().any(|pane| pane.id.to_string() == id)
                    })
                    .map(|tab| tab.id);
                match tab {
                    Some(tab) => {
                        self.select_tab(tab, cx);
                        Ok(Value::Null)
                    }
                    None => Err("tab was not found".into()),
                }
            }
            "tabs.open" => self.extension_open_tab(&call.owner, &call.args, cx),
            "panels.open" | "panels.toggle" | "panels.close" => {
                self.panel_operation(&call.owner, &call.verb, &call.args, window, cx)
            }
            "modal.open" | "modal.feed" | "modal.finish" => self.script_modal(&call, window, cx),
            "modal.openWebview" => self.script_webview_modal(&call, window, cx),
            "modal.closeWebview" => {
                if self
                    .webviews
                    .modal
                    .as_ref()
                    .is_some_and(|modal| modal.surface.view.read(cx).source.owner == call.owner)
                {
                    self.dismiss_webview_modal(cx);
                }
                Ok(Value::Null)
            }
            "dialog.confirm" | "dialog.alert" | "dialog.prompt" => {
                self.extension_dialog(call, window, cx);
                return;
            }
            "toast" | "notifications.notify" => {
                self.show_toast(
                    call.args["title"]
                        .as_str()
                        .unwrap_or(&call.owner)
                        .to_owned(),
                    call.args["body"]
                        .as_str()
                        .or(call.args["message"].as_str())
                        .map(str::to_owned),
                    cx,
                );
                if let Some(notifications) = &self.notifications {
                    self.extensions.next += 1;
                    notifications.deliver(
                        &format!("extension:{}:{}", call.owner, self.extensions.next),
                        call.args["title"].as_str().unwrap_or(&call.owner),
                        call.args["body"]
                            .as_str()
                            .or(call.args["message"].as_str())
                            .unwrap_or(""),
                    );
                }
                self.extensions.log(format!(
                    "{}: {} {}",
                    call.owner,
                    call.args["title"].as_str().unwrap_or(""),
                    call.args["body"]
                        .as_str()
                        .or(call.args["message"].as_str())
                        .unwrap_or("")
                ));
                Ok(Value::Null)
            }
            "worktrees.list" => {
                let root = self
                    .state
                    .projects()
                    .iter()
                    .find(|p| p.id == call.project)
                    .map(|p| p.parent_id.unwrap_or(p.id));
                Ok(json!(self.state.projects().iter().filter(|p| Some(p.id) == root || p.parent_id == root).map(|p| json!({"id":p.id.to_string(),"name":p.name,"path":p.directory.to_str(),"branch":self.git.projects.get(&p.id).and_then(|r|r.summary.as_ref()).and_then(|s|s.branch.as_ref()),"isActive":p.id == self.state.current_project().id})).collect::<Vec<_>>()))
            }
            "worktrees.switch" | "git.worktree.switch" => {
                self.extension_server_call(call, cx);
                return;
            }

            "exec.cancel" => {
                let id = call.args["id"].as_str().unwrap_or("");
                if let Some(job) = self
                    .extensions
                    .jobs
                    .get(&(call.reply.key(), id.into()))
                    .map(|job| job.id)
                    && let Some(client) = self.extensions.client.clone()
                {
                    cx.background_executor()
                        .spawn(async move {
                            let _ = client.cancel_exec_async(job).await;
                        })
                        .detach();
                    Ok(json!(true))
                } else {
                    if self.extensions.cancelled_jobs.len() < 128 {
                        self.extensions
                            .cancelled_jobs
                            .insert((call.reply.key(), id.into()));
                    }
                    Ok(json!(true))
                }
            }
            "exec" | "exec.start" => {
                self.extension_exec(call, cx);
                return;
            }
            verb if verb.starts_with("git.")
                || verb.starts_with("files.")
                || verb == "worktrees.refresh" =>
            {
                self.extension_server_call(call, cx);
                return;
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        };
        call.reply.send(result, cx);
        self.sync_extension_events(cx);
        cx.notify();
    }

    pub(super) fn extension_tabs(&self) -> Vec<Value> {
        self.state.current_project().tabs.iter().enumerate().map(|(index, tab)| json!({"index":index+1,"id":tab.id.to_string(),"kind":if tab.panes.iter().any(|p| matches!(p.content, PaneContent::Webview(_))) {"extensionWebView"} else {"terminal"},"title":tab.title(self.active_pane()),"isActive":Some(tab.id)==self.active_tab()})).collect()
    }

    pub(super) fn extension_open_tab(
        &mut self,
        owner: &str,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let target = &args["extension"];
        if args["kind"] != "extensionWebView" || target["id"].as_str() != Some(owner) {
            return Err("extension can only open its own registered tab types".into());
        }
        let id = self.open_webview_tab(
            WebviewDescriptor {
                owner: owner.into(),
                kind: api::text(target, "tabType")?.into(),
                data: target["data"].clone(),
            },
            target["singleton"].as_bool().unwrap_or(false),
            cx,
        )?;
        Ok(json!(self.pane_tab(id).map(|tab| tab.to_string())))
    }

    fn extension_server_call(&mut self, call: Call, cx: &mut Context<Self>) {
        let Some(client) = self.extensions.client.clone() else {
            call.reply.send(Err("server is disconnected".into()), cx);
            return;
        };
        let trace = crate::diagnostics::Span::new(
            "extension.request",
            format_args!(
                "owner={} verb={} project={:?}",
                call.owner, call.verb, call.project
            ),
        );
        let operation = call.clone();
        let refresh = super::worktrees::handles(&call.verb);
        let task = cx.background_executor().spawn(async move {
            trace.stage(format_args!("phase=started"));
            let result = if refresh {
                super::worktrees::call(&client, &operation).await
            } else {
                server_call(&client, &operation).await
            };
            let catalog = if result.is_ok() && refresh {
                Some(client.catalog_async().await)
            } else {
                None
            };
            trace.stage(format_args!("phase=reply success={}", result.is_ok()));
            (result, catalog, trace)
        });
        cx.spawn(async move |model, cx| {
            let (mut result, catalog, trace) = task.await;
            trace.stage(format_args!("phase=delivering"));
            let _ = model.update(cx, |model, cx| {
                if model.call_live(&call, cx) {
                    if let Some(catalog) = catalog {
                        match catalog {
                            Ok(page) => model.receive_catalog(Ok(page), cx),
                            Err(error) => {
                                result = Err(format!(
                                    "Operation finished, but could not refresh worktrees: {error}"
                                ));
                            }
                        }
                    }
                    if result.is_ok()
                        && matches!(
                            call.verb.as_str(),
                            "worktrees.switch" | "git.worktree.switch"
                        )
                    {
                        result = model.switch_extension_worktree(&call, cx);
                    }
                    if result.is_ok() {
                        model.retarget_extension_files(&call, result.as_ref().ok(), cx);
                    }
                    call.reply.send(result, cx);
                } else {
                    call.reply.send(Err("extension call expired".into()), cx);
                }
            });
        })
        .detach();
    }

    fn extension_exec(&mut self, call: Call, cx: &mut Context<Self>) {
        let Some(client) = self.extensions.client.clone() else {
            call.reply.send(Err("server is disconnected".into()), cx);
            return;
        };
        self.extensions.next += 1;
        let job = self.extensions.next;
        let request = exec_request(&call, job);
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                call.reply.send(Err(error), cx);
                return;
            }
        };
        let id = if call.verb == "exec.start" {
            call.args["id"].as_str().unwrap_or("").to_owned()
        } else {
            format!("sync:{job}")
        };
        let key = (call.reply.key(), id.clone());
        if self.extensions.cancelled_jobs.remove(&key) {
            call.reply.send(Err("cancelled".into()), cx);
            return;
        }
        if self.extensions.jobs.len() >= 32 || self.extensions.jobs.contains_key(&key) {
            call.reply
                .send(Err("too many command jobs or duplicate job ID".into()), cx);
            return;
        }
        self.extensions.jobs.insert(
            key.clone(),
            super::Job {
                id: job,
                call: call.clone(),
            },
        );
        if call.verb == "exec.start" {
            call.reply.send(Ok(json!(id)), cx);
        }
        self.extensions.log(format!(
            "{}: executing {}",
            call.owner,
            call.args["argv"]
                .as_array()
                .map_or("shell", |a| a.first().and_then(Value::as_str).unwrap_or(""))
        ));
        let trace = crate::diagnostics::Span::new(
            "extension.exec",
            format_args!("owner={} project={:?} job={job}", call.owner, call.project),
        );
        let task = cx.background_executor().spawn(async move {
            trace.stage(format_args!("phase=started"));
            client.exec_async(request).await.map_err(|error| error.to_string()).and_then(|result| {
                if result.cancelled { return Err("cancelled".into()); }
                Ok(json!({"stdout":result.stdout,"stderr":result.stderr,"exitCode":result.exit_code,"timedOut":result.timed_out,"truncated":result.truncated}))
            })
        });
        cx.spawn(async move |model, cx| {
            let result = task.await;
            let _ = model.update(cx, |model, cx| {
                model.extensions.jobs.remove(&key);
                if !model.call_live(&call, cx) {
                    call.reply.send(Err("extension call expired".into()), cx);
                    return;
                }
                if call.verb == "exec.start" {
                    model.extension_evaluate(
                        &call.reply,
                        &format!(
                            "globalThis.__muxyResolveExec?.({}, {});",
                            json!(id),
                            envelope(result)
                        ),
                        cx,
                    );
                } else {
                    call.reply.send(result, cx);
                }
            });
        })
        .detach();
    }

    fn extension_dialog(&mut self, call: Call, window: &mut Window, cx: &mut Context<Self>) {
        if self.extensions.dialog.is_some() {
            if self.extensions.pending.len() >= 64 {
                call.reply
                    .send(Err("too many pending extension dialogs".into()), cx);
            } else {
                self.extensions.pending.push_back(call);
            }
            return;
        }
        let prompt = call.verb == "dialog.prompt";
        let alert = call.verb == "dialog.alert";
        let text =
            |field: &str, default: &str| call.args[field].as_str().unwrap_or(default).to_owned();
        let buttons = if prompt {
            vec![text("confirm", "OK"), text("cancel", "Cancel")]
        } else if alert {
            vec!["OK".into()]
        } else {
            call.args["buttons"]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .take(8)
                        .map(str::to_owned)
                        .collect()
                })
                .filter(|values: &Vec<String>| !values.is_empty())
                .unwrap_or_else(|| vec!["OK".into(), "Cancel".into()])
        };
        let options = muxy_ui::dialog::DialogOptions {
            title: text("title", &call.owner),
            message: text("message", ""),
            buttons,
            default_button: (!prompt)
                .then(|| call.args["default"].as_str().map(str::to_owned))
                .flatten(),
            cancel_button: Some(text("cancel", "Cancel")),
            input: prompt.then(|| (text("default", ""), text("placeholder", ""))),
            style: text("style", "informational"),
        };
        let (sender, receiver) = async_channel::bounded(1);
        match muxy_ui::dialog::present(window, options, move |value| {
            let _ = sender.try_send(value);
        }) {
            Ok(dialog) => self.extensions.dialog = Some(dialog),
            Err(error) => {
                call.reply.send(Err(error.to_string()), cx);
                return;
            }
        }
        let handle = self.window;
        cx.spawn(async move |model, cx| {
            let value = receiver.recv().await.ok().flatten();
            let _ = handle.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    model.extensions.dialog = None;
                    if model.call_live(&call, cx) {
                        call.reply
                            .send(Ok(if alert { Value::Null } else { json!(value) }), cx);
                    } else {
                        call.reply.send(Err("extension call expired".into()), cx);
                    }
                    model.next_extension_consent(window, cx);
                });
            });
        })
        .detach();
    }

    fn retarget_extension_files(
        &mut self,
        call: &Call,
        result: Option<&Value>,
        cx: &mut Context<Self>,
    ) {
        let mut renames = Vec::new();
        if call.verb == "files.rename"
            && let (Some(from), Some(to)) = (
                call.args["path"].as_str(),
                result.and_then(|v| v["path"].as_str()),
            )
        {
            renames.push((from.to_owned(), to.to_owned()));
        }
        if call.verb == "files.move"
            && let (Some(from), Some(to)) = (
                call.args["paths"].as_array(),
                result.and_then(Value::as_array),
            )
        {
            for (from, to) in from.iter().zip(to) {
                if let (Some(from), Some(to)) = (from.as_str(), to.as_str()) {
                    renames.push((from.into(), to.into()));
                }
            }
        }
        if renames.is_empty() {
            return;
        }
        let changes: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|p| p.id == call.project)
            .flat_map(|p| &p.tabs)
            .flat_map(|t| &t.panes)
            .filter_map(|pane| {
                let PaneContent::Webview(descriptor) = &pane.content else {
                    return None;
                };
                let path = descriptor.data["filePath"].as_str()?;
                let (from, to) = renames.iter().find(|(from, _)| {
                    path == from
                        || path
                            .strip_prefix(from)
                            .is_some_and(|tail| tail.starts_with('/'))
                })?;
                let mut data = descriptor.data.clone();
                data["filePath"] = json!(format!("{to}{}", &path[from.len()..]));
                Some((pane.id, data))
            })
            .collect();
        for (pane, data) in changes {
            let _ = self.state.set_webview_data(pane, data);
        }
        self.save(cx);
        cx.notify();
    }
}

fn exec_request(call: &Call, job: u64) -> Result<ExecRequest, String> {
    let args = &call.args;
    let arguments = if let Some(values) = args["argv"].as_array() {
        values
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "argv must contain strings".into())
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        Vec::new()
    };
    let timeout = args["timeoutMs"]
        .as_u64()
        .unwrap_or(30_000)
        .clamp(1, 300_000);
    let mut request = ExecRequest {
        job,
        env: if args["env"].is_null() {
            std::collections::BTreeMap::new()
        } else {
            serde_json::from_value(args["env"].clone()).map_err(|error| error.to_string())?
        },
        project: call.project,
        argv: arguments,
        shell: args["shell"].as_str().map(str::to_owned),
        cwd: args["cwd"]
            .as_str()
            .map(|p| ServerPath(p.as_bytes().into())),
        stdin: args["stdin"].as_str().unwrap_or("").as_bytes().into(),
        timeout_ms: u32::try_from(timeout).map_err(|e| e.to_string())?,
    };
    request
        .env
        .insert("MUXY_EXTENSION_ID".into(), call.owner.clone());
    request
        .validate()
        .map_err(|error| format!("invalid command: {error:?}"))?;
    Ok(request)
}

async fn server_call(client: &muxy_client::Client, call: &Call) -> Result<Value, String> {
    if call.verb.starts_with("files.") {
        return client
            .files_async(FilesRequest {
                project: call.project,
                action: api::files_action(&call.verb, &call.args)?,
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(api::files_reply);
    }
    let reply = client
        .git_async(GitRequest {
            project: call.project,
            action: api::git_action(&call.verb, &call.args)?,
        })
        .await
        .map_err(|e| e.to_string())?;
    api::git_reply(&call.verb, reply)
}
