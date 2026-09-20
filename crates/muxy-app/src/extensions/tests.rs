#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Compatibility fixtures fail immediately"
)]

use muxy_ui::javascript::{Event, Script};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn runtime(source: &str) -> (Script, async_channel::Receiver<Event>) {
    Script::start(format!(
        "{}(\"files\", {});\n{source}",
        include_str!("script.js"),
        include_str!("bridge.js")
    ))
    .unwrap()
}

fn next(events: &async_channel::Receiver<Event>) -> muxy_ui::javascript::Call {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match events.try_recv() {
            Ok(Event::Call(call)) => return call,
            Ok(Event::Error(error)) => panic!("JavaScript failed: {error}"),
            Err(async_channel::TryRecvError::Empty) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("script did not call host: {error}"),
        }
    }
}

fn reply(call: &muxy_ui::javascript::Call, value: &Value) {
    call.reply
        .send(json!({"ok":true,"value":value}).to_string())
        .unwrap();
}

#[test]
fn cancelling_an_async_editor_dialog_prevents_closing_the_document() {
    let source = format!(
        r#"
        globalThis.window = globalThis;
        globalThis.document = {{ documentElement: {{ style: {{ setProperty() {{}} }} }}, addEventListener() {{}} }};
        window.webkit = {{ messageHandlers: {{ muxy: {{ postMessage(message) {{
            return Promise.resolve(JSON.parse(__muxyNative(JSON.stringify(message))));
        }} }} }} }};
        {}({{ owner: 'files', id: 'editor', surface: 'tab', data: null, theme: {{}} }}, {});
        muxy.lifecycle.onBeforeClose(async () => await muxy.dialog.confirm({{ buttons: ["Save", "Discard", "Cancel"] }}) === "Cancel");
        __muxyBeforeClose('close-1', 'tab', 'editor');
        "#,
        include_str!("../views/webview/bridge.js"),
        include_str!("bridge.js")
    );
    let (_script, events) = Script::start(source).unwrap();
    let ack = next(&events);
    let request: Value = serde_json::from_str(&ack.request).unwrap();
    assert_eq!(request["verb"], "lifecycle.ackBeforeClose");
    reply(&ack, &Value::Null);
    let dialog = next(&events);
    let request: Value = serde_json::from_str(&dialog.request).unwrap();
    assert_eq!(request["verb"], "dialog.confirm");
    reply(&dialog, &json!("Cancel"));
    let resolve = next(&events);
    let request: Value = serde_json::from_str(&resolve.request).unwrap();
    assert_eq!(request["verb"], "lifecycle.resolveBeforeClose");
    assert_eq!(request["args"]["prevent"], true);
    reply(&resolve, &Value::Null);
}

#[test]
fn unchanged_quick_open_script_executes_synchronously_and_opens_selected_file() {
    // Files 1.21.5, muxy-app/extensions eb37a0e. This fixture is the published script, unchanged.
    let (script, events) = runtime(include_str!("../../fixtures/extensions/quick-open.js"));
    let mut picker = false;
    let mut opened = false;
    for _ in 0..20 {
        let call = next(&events);
        let request: Value = serde_json::from_str(&call.request).unwrap();
        match request["verb"].as_str().unwrap() {
            "exec" => {
                assert!(request["args"]["argv"].is_array());
                reply(
                    &call,
                    &json!({"stdout":"src/main.rs\nREADME.md\n","stderr":"","exitCode":0}),
                );
            }
            "modal.open" => {
                picker = true;
                reply(&call, &json!({"requestID":"native:1"}));
            }
            "modal.feed" => {
                assert!(request["args"]["items"].is_array());
                reply(&call, &Value::Null);
            }
            "modal.finish" => {
                reply(&call, &Value::Null);
                script.evaluate(
                    "__muxiDeliverModalResult('native:1', {id:'src/main.rs',title:'src/main.rs'})"
                        .into(),
                );
            }
            "tabs.open" => {
                assert_eq!(request["args"]["extension"]["tabType"], "code-editor");
                assert_eq!(
                    request["args"]["extension"]["data"]["filePath"],
                    "src/main.rs"
                );
                opened = true;
                reply(&call, &json!("tab-id"));
            }
            "script.finished" => {
                reply(&call, &Value::Null);
                break;
            }
            other => panic!("unexpected quick-open call {other}: {request}"),
        }
    }
    assert!(picker && opened);
}

#[test]
fn unchanged_find_in_files_script_uses_query_callbacks_and_async_exec() {
    let (script, events) = runtime(include_str!("../../fixtures/extensions/find-in-files.js"));
    let mut started = false;
    let mut query_sent = false;
    let mut fed = false;
    for _ in 0..30 {
        let call = next(&events);
        let request: Value = serde_json::from_str(&call.request).unwrap();
        match request["verb"].as_str().unwrap() {
            "modal.open" => {
                assert_eq!(request["args"]["searchToolbar"], true);
                reply(&call, &json!({"requestID":"native:2"}));
            }
            "modal.feed" => {
                fed |= started;
                reply(&call, &Value::Null);
            }
            "modal.finish" => {
                reply(&call, &Value::Null);
                if !query_sent {
                    query_sent = true;
                    script.evaluate("__muxyDeliverModalQuery('native:2','1','needle',{})".into());
                } else if started {
                    break;
                }
            }
            "exec.start" => {
                started = true;
                assert!(request["args"]["argv"].is_array());
                let id = &request["args"]["id"];
                reply(&call, &id.clone());
                script.evaluate(format!("__muxyResolveExec({id},{{ok:true,value:{{stdout:'',stderr:'',exitCode:0,timedOut:false,truncated:false}}}})"));
            }
            "exec.cancel" => reply(&call, &json!(true)),
            "console" => {
                panic!("script console error: {request}");
            }
            other => panic!("unexpected search call {other}: {request}"),
        }
    }
    assert!(started && fed);
}

#[test]
fn unchanged_manifests_load_without_api_changes() {
    for source in [
        include_str!("../../fixtures/extensions/git.json"),
        include_str!("../../fixtures/extensions/files.json"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let package: Value = serde_json::from_str(source).unwrap();
        std::fs::write(root.path().join("package.json"), source).unwrap();
        let manifest = &package["muxy"];
        let resources = manifest["panels"]
            .as_array()
            .unwrap()
            .iter()
            .chain(manifest["tabTypes"].as_array().unwrap())
            .map(|item| item["entry"].as_str().unwrap())
            .chain(
                manifest["commands"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|command| {
                        command["action"]["entry"]
                            .as_str()
                            .or(command["action"]["script"].as_str())
                    }),
            );
        for relative in resources {
            let path = root.path().join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        let extension = muxy_app_core::extensions::Extension::load(root.path()).unwrap();
        assert_eq!(extension.name, package["name"]);
        assert!(!extension.manifest.commands.is_empty());
    }
}

#[test]
#[ignore = "downloads current Git and Files marketplace packages into a temporary profile"]
fn marketplace_git_and_files_install_without_modifying_the_packages() {
    let profile = tempfile::tempdir().unwrap();
    let packages = profile.path().join("extensions");
    for name in ["git", "files"] {
        let details = super::marketplace::detail(name).unwrap();
        let stage = super::marketplace::download(&details, &packages).unwrap();
        super::marketplace::install(&stage, &packages, name).unwrap();
    }
    let mut registry = muxy_app_core::extensions::Registry::load(profile.path());
    assert!(registry.errors.is_empty(), "{:?}", registry.errors);
    for name in ["git", "files"] {
        registry.set_enabled(name, true).unwrap();
        assert!(registry.enabled(name).is_some());
    }
}
