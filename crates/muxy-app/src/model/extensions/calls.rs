use gpui::{Context, Window};
use muxy_app_core::PaneContent;
use muxy_app_core::extensions::{api, event_permission};
use muxy_protocol::{ExecRequest, FilesRequest, GitRequest, ServerPath};
use serde_json::{Value, json};

use super::{AppModel, Call, Reply, envelope, events::strings};

const GH_USER_TTL: std::time::Duration = std::time::Duration::from_secs(300);

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
                if let Reply::Script(id, _) = &call.reply
                    && !self.is_background(*id)
                {
                    self.extensions.scripts.remove(id);
                }
                Ok(Value::Null)
            }
            "console" => {
                self.console(&call);
                call.reply.send(Ok(Value::Null), cx);
                if self.settings_window.is_some() {
                    cx.notify();
                }
                return;
            }
            "events.subscribe" => self.subscribe_event(&call),
            "events.unsubscribe" => Ok(Value::Null),
            "events.emit" => {
                let background = matches!(&call.reply, Reply::Script(id, _)
                    if self.extensions.backgrounds.get(&call.owner)
                        .is_some_and(|background| background.script == *id));
                self.emit_local_event(
                    &call.owner,
                    background,
                    call.args["event"].as_str().unwrap_or(""),
                    &call.args["payload"],
                    cx,
                )
                .map(|()| Value::Null)
            }
            verb if verb.starts_with("storage.") => {
                self.extension_storage(call, cx);
                return;
            }
            verb if verb.starts_with("tabs.") => self.tabs_call(&call, cx),
            verb if verb.starts_with("panes.") => self.panes_call(&call, cx),
            "projects.create" => {
                self.create_extension_project(call, cx);
                return;
            }
            "projects.delete" => {
                let project = self
                    .find_project(call.args["identifier"].as_str().unwrap_or(""))
                    .map(|project| project.id);
                match project {
                    Some(project) => {
                        self.remove_project_confirmed(project, cx);
                        Ok(Value::Null)
                    }
                    None => Err("project not found".into()),
                }
            }
            "projects.attach" | "projects.detach" => self.workspaces_call(&call, cx),
            verb if verb.starts_with("workspaces.") => self.workspaces_call(&call, cx),
            verb if verb.starts_with("projects.") => self.projects_call(&call, cx),
            "worktrees.list" => Ok(self.worktree_list(call.project)),
            "worktrees.switch" | "git.worktree.switch" | "worktrees.refresh" => {
                self.extension_server_call(call, cx);
                return;
            }
            "agents.list" => Ok(Value::Array(self.agent_statuses().into_values().collect())),
            "gh.user" => {
                self.gh_user(call, cx);
                return;
            }
            "panels.open" | "panels.toggle" | "panels.close" => {
                self.panel_operation(&call.owner, &call.verb, &call.args, window, cx)
            }
            "popover.close" => {
                self.request_close_popover(&call.owner, cx);
                Ok(Value::Null)
            }
            "popover.resize" => self.resize_popover(&call.owner, &call.args, cx),
            "topbar.set" | "statusbar.set" => self.set_item(&call, cx),
            verb if verb.starts_with("shortcuts.") => self.shortcuts_call(&call, cx),
            "modal.open" | "modal.feed" | "modal.finish" | "modal.await" => {
                match self.script_modal(&call, window, cx) {
                    Some(result) => result,
                    None => return,
                }
            }
            "modal.openWebview" => self.script_webview_modal(&call, window, cx),
            "modal.closeWebview" => {
                self.dismiss_webview_modal(cx);
                Ok(Value::Null)
            }
            verb if verb.starts_with("dialog.") => {
                self.extension_dialog(call, window, cx);
                return;
            }
            "toast" | "notifications.notify" => self.extension_notify(&call, cx),
            "http.fetch" => {
                Self::http_fetch(call, cx);
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
                } else if self.extensions.cancelled_jobs.len() < 128 {
                    self.extensions
                        .cancelled_jobs
                        .insert((call.reply.key(), id.into()));
                }
                Ok(json!(true))
            }
            "exec" | "exec.start" => {
                self.extension_exec(call, cx);
                return;
            }
            verb if verb.starts_with("git.") || verb.starts_with("files.") => {
                self.extension_server_call(call, cx);
                return;
            }
            "browser.list" => Ok(json!([])),
            verb if verb.starts_with("browser.") => Err("the built-in browser is disabled".into()),
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        };
        call.reply.send(result, cx);
        self.sync_extension_events(cx);
        cx.notify();
    }

    /// Log lines from `console.*`: one per script call, or a batch from a page.
    fn console(&mut self, call: &Call) {
        let single = [call.args.clone()];
        let lines = call.args["lines"]
            .as_array()
            .map_or(&single[..], Vec::as_slice);
        for line in lines {
            let level = match line["level"].as_str() {
                Some("warn") => "warn",
                Some("err" | "error") => "err",
                _ => "log",
            };
            let message = line["message"].as_str().unwrap_or("");
            self.extension_log(&call.owner, format!("[{level}] {message}"));
        }
    }

    fn subscribe_event(&self, call: &Call) -> Result<Value, String> {
        let event = call.args["event"]
            .as_str()
            .ok_or("missing argument 'event'")?;
        let extension = self
            .extensions
            .registry
            .enabled(&call.owner)
            .ok_or_else(|| format!("extension {} not loaded", call.owner))?;
        if event.starts_with("extension.") {
            return if muxy_app_core::extensions::local_event(event) {
                Ok(json!(event))
            } else {
                Err("extension events must start with extension.".into())
            };
        }
        let declared = extension.manifest.events.contains(event)
            || event
                .strip_prefix("command.")
                .is_some_and(|id| extension.manifest.command(id).is_some())
            || self.runtime_command_event(&call.owner, event);
        if !declared {
            return Err(format!("event {event} not declared in manifest"));
        }
        if let Some(permission) = event_permission(event)
            && !extension.allows(permission)
        {
            return Err(format!("permission denied ({permission})"));
        }
        Ok(json!(event))
    }

    fn extension_storage(&mut self, call: Call, cx: &mut Context<Self>) {
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
    }

    fn extension_notify(&mut self, call: &Call, cx: &mut Context<Self>) -> Result<Value, String> {
        let title = call.args["title"].as_str().unwrap_or("").to_owned();
        let body = call.args["body"]
            .as_str()
            .or(call.args["message"].as_str())
            .unwrap_or("")
            .to_owned();
        if title.trim().is_empty() && body.trim().is_empty() {
            return Err("notification requires title or body".into());
        }
        self.show_toast(title.clone(), Some(body.clone()), cx);
        if let Some(notifications) = &self.notifications {
            self.extensions.next += 1;
            notifications.deliver(
                &format!("extension:{}:{}", call.owner, self.extensions.next),
                if title.is_empty() {
                    &call.owner
                } else {
                    &title
                },
                &body,
            );
        }
        let requested = call.args["paneID"].as_str();
        let target = self.state.projects().iter().find_map(|project| {
            project.tabs.iter().find_map(|tab| {
                tab.panes
                    .iter()
                    .find(|pane| {
                        requested.map_or_else(
                            || {
                                project.id == self.state.current_project().id
                                    && matches!(pane.content, PaneContent::Terminal { .. })
                            },
                            |requested| pane.id.to_string() == requested,
                        )
                    })
                    .map(|pane| (project, tab.id, pane.id))
            })
        });
        let (project, tab, pane) = match target {
            Some((project, tab, pane)) => (project, tab.to_string(), pane.to_string()),
            None => (self.state.current_project(), String::new(), String::new()),
        };
        let payload = strings([
            ("paneID", pane),
            ("projectID", super::events::root_of(project).to_string()),
            ("worktreeID", project.id.to_string()),
            ("worktreePath", project.directory.display().to_string()),
            ("tabID", tab),
            ("source", call.owner.clone()),
            ("title", title),
            ("body", body),
        ]);
        self.emit_extension_event(None, "notification.posted", &payload, cx);
        Ok(Value::Null)
    }

    fn server_exec(
        &mut self,
        request: ExecRequest,
        cx: &mut Context<Self>,
    ) -> Result<gpui::Task<Result<muxy_protocol::ExecResult, String>>, String> {
        let client = self
            .extensions
            .client
            .clone()
            .ok_or("server is disconnected")?;
        Ok(cx.background_executor().spawn(async move {
            client
                .exec_async(request)
                .await
                .map_err(|error| error.to_string())
        }))
    }

    /// `gh.user`: the signed-in GitHub CLI account, cached for five minutes.
    fn gh_user(&mut self, call: Call, cx: &mut Context<Self>) {
        if let Some((fetched, user)) = &self.extensions.gh_user
            && fetched.elapsed() < GH_USER_TTL
        {
            call.reply.send(Ok(user.clone()), cx);
            return;
        }
        self.extensions.next += 1;
        let home = self.state.home();
        let request = ExecRequest {
            job: self.extensions.next,
            project: home.id,
            argv: [
                "gh",
                "api",
                "user",
                "--jq",
                "{login: .login, name: .name, avatarUrl: .avatar_url}",
            ]
            .map(str::to_owned)
            .to_vec(),
            shell: None,
            cwd: home
                .directory
                .to_str()
                .map(|path| ServerPath(path.as_bytes().into())),
            env: std::collections::BTreeMap::new(),
            stdin: Vec::new(),
            timeout_ms: 30_000,
        };
        let task = match self.server_exec(request, cx) {
            Ok(task) => task,
            Err(error) => {
                call.reply.send(Err(error), cx);
                return;
            }
        };
        cx.spawn(async move |model, cx| {
            let missing = "GitHub CLI (gh) is not installed. Install it from cli.github.com.";
            let user = match task.await {
                Ok(result) if result.exit_code == 0 => {
                    serde_json::from_str::<Value>(&result.stdout)
                        .ok()
                        .filter(|user| user["login"].is_string())
                        .map(|user| {
                            json!({
                                "login": user["login"],
                                "name": user["name"].as_str().unwrap_or(""),
                                "avatarUrl": user["avatarUrl"].as_str().unwrap_or(""),
                            })
                        })
                        .ok_or_else(|| "Failed to parse GitHub user info.".to_owned())
                }
                Ok(result) if result.exit_code == 127 => Err(missing.into()),
                Ok(result) => Err(if result.stderr.trim().is_empty() {
                    result.stdout.trim().to_owned()
                } else {
                    result.stderr.trim().to_owned()
                }),
                Err(error) if error.contains("No such file") => Err(missing.into()),
                Err(error) => Err(error),
            };
            let _ = model.update(cx, |model, cx| {
                if let Ok(user) = &user {
                    model.extensions.gh_user = Some((std::time::Instant::now(), user.clone()));
                }
                call.reply.send(user, cx);
            });
        })
        .detach();
    }

    /// `projects.create`: optionally creates the folder on the server first.
    fn create_extension_project(&mut self, call: Call, cx: &mut Context<Self>) {
        let mut workspace = None;
        if let Some(identifier) = call.args["workspace"]
            .as_str()
            .map(str::trim)
            .filter(|w| !w.is_empty())
        {
            let Some(found) = self.find_workspace(identifier) else {
                call.reply
                    .send(Err(format!("workspace not found '{identifier}'")), cx);
                return;
            };
            workspace = Some((found.id, found.name.clone()));
        }
        let directory = super::workspace::standardized(call.args["path"].as_str().unwrap_or(""));
        let finish = move |model: &mut Self, call: Call, cx: &mut Context<Self>| {
            let path = call.args["path"].as_str().unwrap_or("").to_owned();
            let active = model.state.active_workspace().cloned();
            let result = model.add_extension_project(&path, cx).and_then(|id| {
                if let Some(name) = call.args["name"]
                    .as_str()
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                {
                    let name = name.to_owned();
                    if !model.edit_project(|state| state.rename_project(id, &name), cx) {
                        return Err("could not save project changes".into());
                    }
                }
                if let Some((workspace, name)) = &workspace
                    && !model.file_created_project(id, *workspace, active.as_ref(), cx)
                {
                    return Err(format!("project cannot be added to workspace '{name}'"));
                }
                let project = model
                    .state
                    .project(id)
                    .ok_or("project not found after creation")?;
                Ok(json!({
                    "id": id.to_string(),
                    "name": project.name,
                    "path": project.directory.display().to_string(),
                }))
            });
            call.reply.send(result, cx);
        };
        if directory.is_dir() {
            finish(self, call, cx);
            return;
        }
        if directory.exists() {
            call.reply.send(Err("path is not a directory".into()), cx);
            return;
        }
        if call.args["createIfMissing"].as_bool() != Some(true) {
            call.reply.send(
                Err("path does not exist, use --create to create it".into()),
                cx,
            );
            return;
        }
        self.extensions.next += 1;
        let request = ExecRequest {
            job: self.extensions.next,
            project: self.state.home().id,
            argv: vec![
                "/bin/mkdir".into(),
                "-p".into(),
                directory.display().to_string(),
            ],
            shell: None,
            cwd: None,
            env: std::collections::BTreeMap::new(),
            stdin: Vec::new(),
            timeout_ms: 30_000,
        };
        let task = match self.server_exec(request, cx) {
            Ok(task) => task,
            Err(error) => {
                call.reply.send(Err(error), cx);
                return;
            }
        };
        cx.spawn(async move |model, cx| {
            let created = task.await;
            let _ = model.update(cx, |model, cx| match created {
                Ok(result) if result.exit_code == 0 && model.call_live(&call, cx) => {
                    finish(model, call, cx);
                }
                Ok(_) => call
                    .reply
                    .send(Err("could not create directory".into()), cx),
                Err(error) => call.reply.send(Err(error), cx),
            });
        })
        .detach();
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
        let request = match exec_request(&call, job) {
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
        let running = self
            .extensions
            .jobs
            .values()
            .filter(|job| job.call.owner == call.owner)
            .count();
        if running >= 32 {
            call.reply.send(
                Err("exec: too many concurrent commands (limit 32)".into()),
                cx,
            );
            return;
        }
        if self.extensions.jobs.contains_key(&key) {
            call.reply.send(Err("duplicate command job ID".into()), cx);
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

pub(super) fn exec_request(call: &Call, job: u64) -> Result<ExecRequest, String> {
    let options = &call.args;
    let argv = options["argv"].as_array();
    let shell = options["shell"].as_str();
    match (argv, shell) {
        (None, None) => return Err("exec: exec requires argv or shell".into()),
        (Some(_), Some(_)) => {
            return Err("exec: exec accepts either argv or shell, not both".into());
        }
        (Some(argv), None) if argv.is_empty() => {
            return Err("exec: exec argv must be non-empty".into());
        }
        _ => (),
    }
    let arguments = argv
        .into_iter()
        .flatten()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| "argv must contain strings".to_owned())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let timeout = options["timeoutMs"]
        .as_u64()
        .unwrap_or(30_000)
        .clamp(1, 300_000);
    let mut request = ExecRequest {
        job,
        env: options["env"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
            .collect(),
        project: call.project,
        argv: arguments,
        shell: shell.map(str::to_owned),
        cwd: options["cwd"]
            .as_str()
            .map(|p| ServerPath(p.as_bytes().into())),
        stdin: options["stdin"].as_str().unwrap_or("").as_bytes().into(),
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
