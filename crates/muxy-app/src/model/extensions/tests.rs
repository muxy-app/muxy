use super::*;
use crate::model::tests::stub_boot;
use gpui::TestAppContext;
use muxy_app_core::AppState;

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
