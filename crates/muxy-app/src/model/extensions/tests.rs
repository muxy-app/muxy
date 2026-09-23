use super::*;
use crate::model::tests::{extensions::finish_extension, stub_boot};
use gpui::TestAppContext;
use muxy_app_core::AppState;

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
    let key = "writer:files.write";
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
            generation: model.generation,
            project: model.state.home().id,
            verb: "files.write".into(),
            args: json!({"path":"file","content":"text"}),
            reply: Reply::Script(1, reply),
        };
        assert!(model.call_live(&call, cx));
        model.complete_extension_consent(call.clone(), key.into(), confirmation, cx);
        response
            .try_send(muxy_ui::dialog::ConfirmationResponse::Confirmed {
                dont_ask_again: true,
            })
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
        Grants::load(&profile).expect("grants").decision(key),
        Consent::Ask
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
                    generation: model.generation,
                    project: model.state.home().id,
                    verb: verb.into(),
                    args,
                    reply: Reply::Script(1, reply),
                },
                window,
                cx,
            );
        });
    });
    serde_json::from_str(&replies.try_recv().expect("reply")).expect("JSON")
}
