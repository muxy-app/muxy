use super::*;
use crate::model::tests::{extensions::finish_extension, stub_boot};
use gpui::TestAppContext;
use muxy_app_core::{AppState, ServerId};

#[gpui::test]
fn exec_consent_preflight_validates_sync_and_async_commands(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.read_with(cx, |model, _| {
        assert_eq!(model.extensions.next, 0);
        for verb in ["exec", "exec.start"] {
            let (reply, _replies) = std::sync::mpsc::sync_channel(1);
            let mut call = Call {
                owner: "files".into(),
                epoch: model.extensions.epoch_for("files"),
                generation: model.servers.local.generation,
                project: model.state.home().id,
                verb: verb.into(),
                args: Value::Null,
                reply: Reply::Script(1, reply),
                approved: false,
            };
            for args in [
                json!({"argv": ["perl", "-e", "exec @ARGV", "--", "git", "ls-files", "-z"]}),
                json!({
                    "id": "1",
                    "argv": ["sh", "-c", "grep -rnF -f - -- . | head -n 120"],
                    "stdin": "test\n",
                    "timeoutMs": 350,
                }),
                json!({"shell": "git ls-files"}),
                json!({"argv": ["git", "show", "HEAD:src/main.rs"]}),
                json!({"argv": ["git", "show", "HEAD:src/main.rs"], "cwd": "/repo/worktree"}),
            ] {
                call.args = args;
                let expected = Request::for_call(&call.owner, verb, &call.args, "")
                    .expect("exec requires consent");
                assert_eq!(
                    model.consent_request(&call),
                    Ok(Some(expected)),
                    "{verb}: {}",
                    call.args
                );
                assert!(calls::exec_request(&call, 0).is_err());
                let request = calls::exec_request(&call, 42).expect("allocated exec request");
                assert_eq!(request.job, 42);
                assert_eq!(request.project, call.project);
                assert_eq!(
                    request.cwd,
                    call.args["cwd"]
                        .as_str()
                        .map(|path| { muxy_protocol::ServerPath(path.as_bytes().to_vec()) })
                );
                assert_eq!(request.env["MUXY_EXTENSION_ID"], "files");
            }
            for args in [
                json!({}),
                json!({"argv": []}),
                json!({"argv": [""]}),
                json!({"argv": ["git"], "shell": "git ls-files"}),
                json!({"argv": ["git", 1]}),
                json!({"argv": ["git"], "env": {"INVALID=KEY": "value"}}),
            ] {
                call.args = args;
                assert!(
                    model.consent_request(&call).is_err(),
                    "{verb} must reject invalid arguments before consent: {}",
                    call.args
                );
            }
            let null = "exec failed to launch: arguments and environment cannot contain null bytes";
            for (args, error) in [
                (json!({"argv": ["git", "\u{0}"]}), null),
                (json!({"shell": "git\u{0}"}), null),
                (json!({"argv": ["git"], "env": {"KEY": "a\u{0}b"}}), null),
                (json!({"argv": ["git"], "env": {"K\u{0}EY": "value"}}), null),
                (
                    json!({"argv": ["git"], "cwd": "/repo\u{0}"}),
                    "exec: cwd cannot contain null bytes",
                ),
            ] {
                call.args = args;
                assert_eq!(
                    model.consent_request(&call),
                    Err(error.into()),
                    "{verb}: {}",
                    call.args
                );
            }
        }
        assert_eq!(model.extensions.next, 0);
        assert!(model.extensions.jobs.is_empty());
    });
}

#[gpui::test]
fn expired_confirmation_does_not_persist_permission(cx: &mut TestAppContext) {
    let package = tempfile::tempdir().expect("extension folder");
    std::fs::write(
        package.path().join("package.json"),
        r#"{"name":"writer","version":"1.0.0","muxy":{"permissions":["files:write"]}}"#,
    )
    .expect("manifest");
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let profile = boot.state_path.parent().expect("profile").to_owned();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    finish_extension(
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        cx,
    )
    .expect("load");
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("writer", true, cx)
        }),
        cx,
    )
    .expect("enable");
    let request = Request::file("write", "file");
    let (reply, replies) = std::sync::mpsc::sync_channel(1);
    let (response, confirmation) = async_channel::bounded(1);
    view.update(cx, |model, cx| {
        let (script, _events) = muxy_ui::javascript::Script::start(String::new()).expect("script");
        model.extensions.scripts.insert(
            1,
            scripts::Running {
                owner: "writer".into(),
                script,
            },
        );
        let call = Call {
            owner: "writer".into(),
            epoch: model.extensions.epoch_for("writer"),
            generation: model.servers.local.generation,
            project: model.state.home().id,
            verb: "files.write".into(),
            args: json!({"path":"file","content":"text"}),
            reply: Reply::Script(1, reply),
            approved: false,
        };
        assert!(model.call_live(&call, cx));
        model.await_consent(1, call.clone(), request.clone(), confirmation, cx);
        response
            .try_send(muxy_ui::dialog::ConsentResponse::AllowAndRemember)
            .expect("confirm");
        model
            .extensions
            .epochs
            .insert("writer".into(), call.epoch + 1);
        assert!(!model.call_live(&call, cx));
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let response = loop {
        cx.run_until_parked();
        if let Ok(response) = replies.try_recv() {
            break response;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "confirmation timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(
        serde_json::from_str::<Value>(&response).expect("reply")["ok"],
        false
    );
    assert_eq!(
        Grants::load(&profile)
            .expect("grants")
            .decision("writer", &request),
        muxy_app_core::extensions::Consent::Ask
    );
}

#[gpui::test]
fn extension_toast_aliases_reach_the_app_without_native_notifications(cx: &mut TestAppContext) {
    let package = tempfile::tempdir().expect("extension folder");
    std::fs::write(
        package.path().join("package.json"),
        r#"{"name":"notifier","version":"1.0.0","muxy":{"permissions":["notifications:write"]}}"#,
    )
    .expect("manifest");
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    finish_extension(
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        cx,
    )
    .expect("load");
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("notifier", true, cx)
        }),
        cx,
    )
    .expect("enable");
    view.update(cx, |model, _| {
        model.notifications = None;
        let (script, _events) = muxy_ui::javascript::Script::start(String::new()).expect("script");
        model.extensions.scripts.insert(
            1,
            scripts::Running {
                owner: "notifier".into(),
                script,
            },
        );
    });
    for (verb, args, title, body) in [
        (
            "toast",
            json!({"title":"Saved", "body":"  File saved successfully  "}),
            "Saved",
            Some("File saved successfully"),
        ),
        (
            "notifications.notify",
            json!({"title":"Saved", "message":"Another file saved"}),
            "Saved",
            Some("Another file saved"),
        ),
        (
            "toast",
            json!({"title":"", "body":"Body only"}),
            "Body only",
            None,
        ),
    ] {
        let response = notify_extension(&view, cx, verb, args);
        assert_eq!(response["ok"], true);
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            let toast = model.notice_toast.as_ref().expect("in-app toast");
            assert_eq!(toast.message, title);
            assert_eq!(toast.body.as_deref(), body);
        });
    }
    view.update(cx, |model, _| {
        model
            .extensions
            .registry
            .extensions
            .get_mut("notifier")
            .expect("extension")
            .manifest
            .permissions
            .clear();
    });
    let response = notify_extension(&view, cx, "toast", json!({"title":"Unauthorized"}));
    view.read_with(cx, |model, _| {
        assert_eq!(model.notice.as_deref(), Some("Body only"));
    });
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"], "permission denied (notifications:write)");
}

fn notify_extension(
    view: &Entity<AppModel>,
    cx: &mut gpui::VisualTestContext,
    verb: &str,
    args: Value,
) -> Value {
    let (reply, replies) = std::sync::mpsc::sync_channel(1);
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.extension_call(
                Call {
                    owner: "notifier".into(),
                    epoch: model.extensions.epoch_for("notifier"),
                    generation: model.servers.local.generation,
                    project: model.state.home().id,
                    verb: verb.into(),
                    args,
                    reply: Reply::Script(1, reply),
                    approved: false,
                },
                window,
                cx,
            );
        });
    });
    serde_json::from_str(&replies.try_recv().expect("reply")).expect("JSON")
}

/// Loads and enables an unpacked extension whose `muxy` manifest is `manifest`.
fn enabled<'a>(
    cx: &'a mut TestAppContext,
    name: &str,
    manifest: &str,
    state: AppState,
) -> (
    Entity<AppModel>,
    &'a mut gpui::VisualTestContext,
    tempfile::TempDir,
    std::sync::mpsc::Receiver<(u64, crate::boot::Work)>,
) {
    enabled_with(cx, name, manifest, &[], state)
}

/// Like `enabled`, with extra package files as `(path, contents)`.
fn enabled_with<'a>(
    cx: &'a mut TestAppContext,
    name: &str,
    manifest: &str,
    files: &[(&str, &str)],
    state: AppState,
) -> (
    Entity<AppModel>,
    &'a mut gpui::VisualTestContext,
    tempfile::TempDir,
    std::sync::mpsc::Receiver<(u64, crate::boot::Work)>,
) {
    let package = tempfile::tempdir().expect("extension folder");
    std::fs::write(
        package.path().join("package.json"),
        format!(r#"{{"name":"{name}","version":"1.0.0","muxy":{manifest}}}"#),
    )
    .expect("manifest");
    std::fs::write(package.path().join("index.html"), "").expect("entry");
    for (path, contents) in files {
        let path = package.path().join(path);
        std::fs::create_dir_all(path.parent().expect("package folder")).expect("package folder");
        std::fs::write(path, contents).expect("package file");
    }
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    finish_extension(
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        cx,
    )
    .expect("load");
    let owner = name.to_owned();
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled(&owner, true, cx)
        }),
        cx,
    )
    .expect("enable");
    (view, cx, package, requests)
}

/// Sends one API call as the extension's script `id` and waits for its reply.
fn script_call(
    view: &Entity<AppModel>,
    cx: &mut gpui::VisualTestContext,
    owner: &str,
    id: u64,
    verb: &str,
    args: Value,
) -> Value {
    let (reply, replies) = std::sync::mpsc::sync_channel(1);
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            model.extensions.scripts.entry(id).or_insert_with(|| {
                let (script, _events) =
                    muxy_ui::javascript::Script::start(String::new()).expect("script");
                scripts::Running {
                    owner: owner.into(),
                    script,
                }
            });
            model.extension_call(
                Call {
                    owner: owner.into(),
                    epoch: model.extensions.epoch_for(owner),
                    generation: model.servers.local.generation,
                    project: model.state.current_project().id,
                    verb: verb.into(),
                    args,
                    reply: Reply::Script(id, reply),
                    approved: false,
                },
                window,
                cx,
            );
        });
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if let Ok(response) = replies.try_recv() {
            return serde_json::from_str(&response).expect("JSON reply");
        }
        assert!(std::time::Instant::now() < deadline, "{verb} timed out");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[gpui::test]
fn extension_events_stay_inside_the_extension(cx: &mut TestAppContext) {
    let (view, cx, _package, _requests) = enabled(
        cx,
        "bus",
        r#"{"events": ["tab.switched"]}"#,
        AppState::bootstrap().expect("state"),
    );
    let missing = script_call(
        &view,
        cx,
        "bus",
        1,
        "events.emit",
        json!({"event": "extension.ready", "payload": {}}),
    );
    assert_eq!(missing["error"], "background script unavailable");
    view.update(cx, |model, _| {
        let (script, _events) = muxy_ui::javascript::Script::start(String::new()).expect("script");
        model.extensions.scripts.insert(
            2,
            scripts::Running {
                owner: "bus".into(),
                script,
            },
        );
        model.extensions.backgrounds.insert(
            "bus".into(),
            background::Background {
                script: 2,
                version: "1.0.0".into(),
            },
        );
    });
    let delivered = script_call(
        &view,
        cx,
        "bus",
        1,
        "events.emit",
        json!({"event": "extension.ready", "payload": {"n": 1}}),
    );
    assert_eq!(delivered["ok"], true);
    let from_background = script_call(
        &view,
        cx,
        "bus",
        2,
        "events.emit",
        json!({"event": "extension.done", "payload": null}),
    );
    assert_eq!(from_background["ok"], true);
    let invalid = script_call(
        &view,
        cx,
        "bus",
        1,
        "events.emit",
        json!({"event": "tab.switched", "payload": {}}),
    );
    assert_eq!(invalid["ok"], false);
    let subscribed = script_call(
        &view,
        cx,
        "bus",
        1,
        "events.subscribe",
        json!({"event": "extension.anything"}),
    );
    assert_eq!(subscribed["ok"], true);
    let undeclared = script_call(
        &view,
        cx,
        "bus",
        1,
        "events.subscribe",
        json!({"event": "project.switched"}),
    );
    assert_eq!(
        undeclared["error"],
        "event project.switched not declared in manifest"
    );
}

#[gpui::test]
fn terminal_tabs_type_their_startup_command_once_attached(cx: &mut TestAppContext) {
    let (view, cx, _package, requests) = enabled(
        cx,
        "launcher",
        r#"{"permissions": ["tabs:write"]}"#,
        AppState::bootstrap().expect("state"),
    );
    let request = Request::for_call(
        "launcher",
        "tabs.open",
        &json!({"kind": "terminal", "command": "npm test"}),
        "",
    )
    .expect("running a command needs consent");
    view.update(cx, |model, cx| {
        model.servers.local.connection = crate::model::ConnectionState::Ready;
        crate::model::tests::acknowledge_catalog(model, cx);
        model
            .extensions
            .grants
            .as_mut()
            .expect("grants")
            .remember(
                "launcher",
                &request,
                muxy_app_core::extensions::Choice::AllowAndRemember,
            )
            .expect("remember");
    });
    let opened = script_call(
        &view,
        cx,
        "launcher",
        1,
        "tabs.open",
        json!({"kind": "terminal", "command": " npm test "}),
    );
    assert_eq!(opened["ok"], true, "{opened}");
    cx.run_until_parked();
    let pane = view.read_with(cx, |model, _| {
        let pane = model.active_pane().expect("new terminal pane");
        assert_eq!(model.state.startup_command(pane), Some("npm test"));
        pane
    });
    let session = muxy_protocol::SessionId::new(7).expect("session");
    requests.try_iter().for_each(drop);
    view.update(cx, |model, cx| {
        model.receive_attached(
            ServerId::local(),
            pane,
            session,
            crate::model::tests::attachment(),
            true,
            cx,
        );
    });
    let typed: Vec<_> = requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            crate::boot::Work::Input(_, bytes) => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(typed, vec![b"npm test\r".to_vec()]);
    view.read_with(cx, |model, _| {
        assert!(model.state.startup_command(pane).is_none());
    });
    let outside = script_call(
        &view,
        cx,
        "launcher",
        1,
        "tabs.open",
        json!({"kind": "terminal", "directory": "../outside"}),
    );
    assert_eq!(
        outside["error"],
        "directory must be an existing folder inside the worktree"
    );
}

#[gpui::test]
fn panel_and_popover_calls_report_main_errors(cx: &mut TestAppContext) {
    let (view, cx, _package, _requests) = enabled(
        cx,
        "surfaces",
        r#"{
            "permissions": ["panels:write"],
            "panels": [{"id": "side", "entry": "index.html"}]
        }"#,
        AppState::bootstrap().expect("state"),
    );
    let missing = script_call(&view, cx, "surfaces", 1, "panels.open", json!({}));
    assert_eq!(missing["error"], "missing argument 'panel'");
    let unknown = script_call(
        &view,
        cx,
        "surfaces",
        1,
        "panels.toggle",
        json!({"panelID": "other"}),
    );
    assert_eq!(unknown["error"], "unknown panel 'other'");
    let closed = script_call(
        &view,
        cx,
        "surfaces",
        1,
        "panels.close",
        json!({"panelID": "other"}),
    );
    assert_eq!(closed["ok"], true, "closing never fails");
    let width = script_call(
        &view,
        cx,
        "surfaces",
        1,
        "popover.resize",
        json!({"height": 10}),
    );
    assert_eq!(width["error"], "missing argument 'width'");
    let negative = script_call(
        &view,
        cx,
        "surfaces",
        1,
        "popover.resize",
        json!({"width": -1, "height": 10}),
    );
    assert_eq!(
        negative["error"],
        "popover.resize requires positive width and height"
    );
    let browser = script_call(&view, cx, "surfaces", 1, "browser.list", json!({}));
    assert_eq!(browser["ok"], false, "browser APIs need their permission");
}

/// Runs the app until `done` holds, failing after five seconds.
fn wait_for(
    view: &Entity<AppModel>,
    cx: &mut gpui::VisualTestContext,
    what: &str,
    done: impl Fn(&AppModel) -> bool,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if view.read_with(cx, |model, _| done(model)) {
            return;
        }
        assert!(std::time::Instant::now() < deadline, "{what} timed out");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn logged(model: &AppModel, owner: &str, line: &str) -> bool {
    model
        .extensions
        .logs
        .tail(owner)
        .any(|logged| logged == line)
}

#[gpui::test]
fn background_scripts_follow_the_extension_not_the_connection(cx: &mut TestAppContext) {
    let manifest = r#"{"background": "background.js"}"#;
    let (view, cx, package, _requests) = enabled_with(
        cx,
        "ports",
        manifest,
        &[("background.js", "setInterval(() => {}, 60000);")],
        AppState::bootstrap().expect("state"),
    );
    wait_for(&view, cx, "background start", |model| {
        logged(model, "ports", "[muxy] started ports v1.0.0")
    });
    view.update(cx, |model, cx| model.disconnect(ServerId::local(), cx));
    view.read_with(cx, |model, _| {
        assert!(
            model.background_running("ports"),
            "disconnects keep background scripts"
        );
    });
    std::fs::write(
        package.path().join("package.json"),
        format!(r#"{{"name":"ports","version":"1.1.0","muxy":{manifest}}}"#),
    )
    .expect("updated manifest");
    finish_extension(
        view.update(cx, AppModel::refresh_installed_extensions_task),
        cx,
    )
    .expect("refresh");
    wait_for(&view, cx, "background restart", |model| {
        logged(model, "ports", "[muxy] started ports v1.1.0")
    });
    view.read_with(cx, |model, _| {
        assert!(logged(model, "ports", "[muxy] exited cleanly"));
    });
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("ports", false, cx)
        }),
        cx,
    )
    .expect("disable");
    view.read_with(cx, |model, _| assert!(!model.background_running("ports")));
}

/// Stands in for the native consent sheet, which reports a cancel when it
/// closes without an answer.
struct ConsentSheet(async_channel::Sender<muxy_ui::dialog::ConsentResponse>);

impl Drop for ConsentSheet {
    fn drop(&mut self) {
        let _ = self.0.try_send(muxy_ui::dialog::ConsentResponse::Cancel);
    }
}

/// An `exec` call from script 1 of the `runner` extension.
fn runner_exec(model: &AppModel, argv: &Value, reply: std::sync::mpsc::SyncSender<String>) -> Call {
    Call {
        owner: "runner".into(),
        epoch: model.extensions.epoch_for("runner"),
        generation: model.servers.local.generation,
        project: model.state.home().id,
        verb: "exec".into(),
        args: json!({ "argv": argv }),
        reply: Reply::Script(1, reply),
        approved: false,
    }
}

/// Shows a consent sheet `id` for running `argv`, returning where its answer goes.
fn prompt_runner(
    view: &Entity<AppModel>,
    cx: &mut gpui::VisualTestContext,
    id: u64,
    argv: &Value,
) -> async_channel::Sender<muxy_ui::dialog::ConsentResponse> {
    let (reply, _replies) = std::sync::mpsc::sync_channel(1);
    let (answer, answers) = async_channel::bounded(1);
    view.update(cx, |model, cx| {
        let call = runner_exec(model, argv, reply);
        let request = Request::for_call("runner", "exec", &call.args, "").expect("gated");
        let sheet = Box::new(ConsentSheet(answer.clone()));
        model.extensions.sheet = Some(consent::Sheet::new(id, "runner", false, sheet));
        model.await_consent(id, call, request, answers, cx);
    });
    answer
}

#[gpui::test]
fn gated_decisions_are_recorded_in_the_audit_log(cx: &mut TestAppContext) {
    let (view, cx, _package, _requests) = enabled(
        cx,
        "runner",
        r#"{"permissions": ["commands:exec"]}"#,
        AppState::bootstrap().expect("state"),
    );
    let audit = view.read_with(cx, |model, _| model.extension_audit_log());
    let entries = move || -> Vec<Value> {
        std::fs::read_to_string(&audit)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).expect("audit entry"))
            .collect()
    };
    let call = |cx: &mut gpui::VisualTestContext, argv: Value, sheet: Option<consent::Sheet>| {
        let (reply, replies) = std::sync::mpsc::sync_channel(1);
        cx.update(|window, cx| {
            view.update(cx, |model, cx| {
                if sheet.is_some() {
                    model.extensions.sheet = sheet;
                }
                model.extension_call(runner_exec(model, &argv, reply), window, cx);
            });
        });
        replies
    };
    view.update(cx, |model, _| {
        let (script, _events) = muxy_ui::javascript::Script::start(String::new()).expect("script");
        let running = scripts::Running {
            owner: "runner".into(),
            script,
        };
        model.extensions.scripts.insert(1, running);
    });
    prompt_runner(&view, cx, 1, &json!(["git", "status"]))
        .try_send(muxy_ui::dialog::ConsentResponse::DenyAndRemember)
        .expect("answer");
    wait_for(&view, cx, "the remembered answer", |_| entries().len() == 1);
    let denied = call(cx, json!(["git", "log"]), None);
    assert!(
        denied
            .try_recv()
            .expect("reply")
            .contains("user denied consent for exec")
    );
    wait_for(&view, cx, "the saved rule", |_| entries().len() == 2);
    drop(prompt_runner(&view, cx, 2, &json!(["rm", "-r", "build"])));
    view.update(cx, |model, _| model.extensions.sheet = None);
    wait_for(&view, cx, "the prompt that timed out", |_| {
        entries().len() == 3
    });
    let dialog = consent::Sheet::new(3, "other", true, Box::new(()));
    let stopped = call(cx, json!(["make"]), Some(dialog));
    finish_extension(
        view.update(cx, |model, cx| {
            model.set_extension_enabled("runner", false, cx)
        }),
        cx,
    )
    .expect("disable");
    assert!(
        stopped
            .try_recv()
            .expect("reply")
            .contains("extension disabled")
    );
    wait_for(&view, cx, "the queued prompt", |_| entries().len() == 4);
    let entries = entries();
    assert_eq!(
        entries[0],
        json!({
            "timestamp": entries[0]["timestamp"],
            "extensionID": "runner",
            "verb": "exec",
            "payloadSummary": "git status",
            "decision": "deny",
            "ruleID": "exec:argv:git",
            "source": "exec",
        })
    );
    assert!(
        entries[0]["timestamp"]
            .as_str()
            .is_some_and(|time| time.ends_with('Z'))
    );
    let fields = |entry: &Value| {
        ["payloadSummary", "decision", "ruleID"]
            .map(|key| entry[key].as_str().unwrap_or("").to_owned())
    };
    assert_eq!(
        fields(&entries[1]),
        ["git log", "deny", "exec:argv:git"],
        "a saved rule decides without asking"
    );
    assert_eq!(fields(&entries[2]), ["rm -r build [timeout]", "deny", ""]);
    assert_eq!(
        fields(&entries[3]),
        ["make [cancelled]", "deny", ""],
        "a prompt still waiting when its extension stops"
    );
}

#[gpui::test]
fn workspace_events_reach_permitted_extension_scripts(cx: &mut TestAppContext) {
    let (view, cx, _package, _requests) = enabled(
        cx,
        "watcher",
        r#"{"events": ["tab.created"]}"#,
        AppState::bootstrap().expect("state"),
    );
    let (script, events) = muxy_ui::javascript::Script::start(
        "globalThis.__muxyEvent = (name, payload) => __muxyNative(JSON.stringify({ name, payload }));"
            .into(),
    )
    .expect("script");
    view.update(cx, |model, cx| {
        model.extensions.scripts.insert(
            9,
            scripts::Running {
                owner: "watcher".into(),
                script,
            },
        );
        model.sync_extension_events(cx);
        model.new_tab(cx);
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let delivered = loop {
        cx.run_until_parked();
        if let Ok(muxy_ui::javascript::Event::Call(call)) = events.try_recv() {
            let _ = call.reply.send("null".into());
            break serde_json::from_str::<Value>(&call.request).expect("event");
        }
        assert!(
            std::time::Instant::now() < deadline,
            "tab.created timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(delivered["name"], "tab.created");
    let payload = delivered["payload"].as_object().expect("payload");
    assert!(payload.values().all(Value::is_string), "{payload:?}");
    assert_eq!(payload["kind"], "terminal");
    std::thread::sleep(std::time::Duration::from_millis(50));
    cx.run_until_parked();
    assert!(
        events.try_recv().is_err(),
        "undeclared events such as tab.focused are not delivered"
    );
}

#[gpui::test]
fn project_and_agent_lists_use_main_shapes(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    let (view, cx, _package, _requests) = enabled(
        cx,
        "lister",
        r#"{"permissions": ["projects:read", "agents:read"]}"#,
        state,
    );
    let projects = script_call(&view, cx, "lister", 1, "projects.list", json!({}));
    assert_eq!(projects["value"][1]["worktreesEnabled"], true);
    view.update(cx, |model, cx| {
        model.toggle_worktree_visibility(project, cx);
    });
    let projects = script_call(&view, cx, "lister", 1, "projects.list", json!({}));
    assert_eq!(
        projects["value"][1]["worktreesEnabled"], false,
        "worktreesEnabled follows the project's Worktrees setting"
    );
    let home = &projects["value"][0];
    for key in [
        "id",
        "name",
        "path",
        "isActive",
        "sortOrder",
        "iconColor",
        "icon",
        "logo",
        "worktreesEnabled",
    ] {
        assert!(
            home.get(key).is_some(),
            "projects.list is missing {key}: {home}"
        );
    }
    assert_eq!(
        home["worktreesEnabled"], false,
        "the home project has no worktrees"
    );
    let agents = script_call(&view, cx, "lister", 1, "agents.list", json!({}));
    assert_eq!(agents["value"], json!([]));
    let rename = script_call(
        &view,
        cx,
        "lister",
        1,
        "projects.rename",
        json!({"identifier": "home", "name": "Other"}),
    );
    assert_eq!(rename["error"], "permission denied (projects:write)");
}
