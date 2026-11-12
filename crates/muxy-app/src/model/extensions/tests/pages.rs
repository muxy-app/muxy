//! `muxy extension` commands that reach an extension's pages, commands and
//! events.

use std::path::Path;
use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use muxy_app_core::AppState;
use serde_json::{Map, Value, json};

use crate::model::AppModel;
use crate::model::tests::stub_boot;

fn open_model(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.add_window_view(|window, cx| AppModel::new(boot, window, cx))
}

/// Answers a socket command, through `page` for the page commands.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Every caller builds the arguments with json!"
)]
fn call(
    model: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    verb: &str,
    args: Value,
) -> Result<Value, String> {
    let args: Map<String, Value> = args.as_object().cloned().unwrap_or_default();
    let (reply, replied) = async_channel::bounded(1);
    let task = model.update_in(cx, |model, window, cx| {
        if super::super::is_page_verb(verb) {
            model.extension_page_cli_call(verb, &args, window, cx)
        } else {
            model.extension_cli_call(verb, &args, cx)
        }
    });
    cx.spawn(|_| async move {
        let _ = reply.send(task.await).await;
    })
    .detach();
    cx.executor().allow_parking();
    for _ in 0..2_000 {
        cx.run_until_parked();
        if let Ok(result) = replied.try_recv() {
            return result;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("{verb} did not answer");
}

fn write_manifest(root: &Path) {
    std::fs::write(
        root.join("package.json"),
        json!({"name": "hello", "version": "1.0.0", "muxy": {
            "events": ["settings.changed"],
            "commands": [{"id": "ping", "title": "Hello: Ping"}],
        }})
        .to_string(),
    )
    .unwrap();
}

fn sent_events(model: &Entity<AppModel>, cx: &mut VisualTestContext) -> Vec<String> {
    model.read_with(cx, |model, _| model.extensions.events.sent.take())
}

#[gpui::test]
fn commands_run_its_commands_and_send_it_only_the_events_it_lists(cx: &mut TestAppContext) {
    let (model, cx) = open_model(cx);
    let source = tempfile::tempdir().unwrap();
    write_manifest(source.path());
    let path = source.path().display().to_string();
    call(&model, cx, "extension.load", json!({"path": path})).unwrap();
    let off = call(
        &model,
        cx,
        "extension.run",
        json!({"name": "hello", "command": "ping"}),
    );
    assert!(off.unwrap_err().contains("turned off"));
    call(&model, cx, "extension.enable", json!({"name": "hello"})).unwrap();
    sent_events(&model, cx);

    call(
        &model,
        cx,
        "extension.run",
        json!({"name": "hello", "command": "ping"}),
    )
    .unwrap();
    assert_eq!(sent_events(&model, cx), ["command.ping"]);
    let missing = call(
        &model,
        cx,
        "extension.run",
        json!({"name": "hello", "command": "missing"}),
    );
    assert!(missing.unwrap_err().contains("no command 'missing'"));

    call(
        &model,
        cx,
        "extension.emit",
        json!({"name": "hello", "event": "settings.changed", "payload": {"key": "greeting"}}),
    )
    .unwrap();
    assert_eq!(sent_events(&model, cx), ["settings.changed"]);
    for args in [
        json!({"name": "hello", "event": "file.changed"}),
        json!({"name": "hello", "event": "settings.changed", "payload": [1]}),
        json!({"name": "nobody", "event": "settings.changed"}),
    ] {
        assert!(
            call(&model, cx, "extension.emit", args.clone()).is_err(),
            "{args}"
        );
    }
    assert!(sent_events(&model, cx).is_empty());

    assert_eq!(
        call(&model, cx, "extension.pages", json!({"name": "hello"})),
        Ok(json!([]))
    );
    let unknown = call(
        &model,
        cx,
        "extension.page.click",
        json!({"pageId": "999999", "selector": "#a"}),
    );
    assert!(unknown.unwrap_err().contains("no extension page 999999"));
    let surface = call(
        &model,
        cx,
        "extension.open",
        json!({"name": "hello", "surface": "panel", "id": "main"}),
    );
    assert!(surface.unwrap_err().contains("no panel 'main'"));
}
