use std::collections::BTreeMap;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use super::*;
use gpui::{Entity, VisualTestContext};
use muxy_app_core::{Direction, PaneId};
use serde_json::Map;

fn open(state: AppState, cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext) {
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    (view, cx)
}

fn answer(replies: &Receiver<String>) -> Result<Value, String> {
    let reply: Value =
        serde_json::from_str(&replies.try_recv().expect("a reply")).expect("reply JSON");
    if reply["ok"] == true {
        Ok(reply["value"].clone())
    } else {
        Err(reply["error"].as_str().unwrap_or("").to_owned())
    }
}

fn send(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    owner: &str,
    verb: &str,
    args: Value,
) -> Receiver<String> {
    view.update_in(cx, |model, window, cx| {
        let (reply, replies) = std::sync::mpsc::sync_channel(1);
        let call = Call {
            owner: owner.into(),
            epoch: model.extensions.epoch_for(owner),
            generation: model.servers.local.generation,
            project: model.state.current_project().id,
            verb: verb.into(),
            args,
            reply: Reply::Script(1, reply),
            approved: true,
        };
        model.dispatch_extension(call, window, cx);
        replies
    })
}

fn call(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    verb: &str,
    args: Value,
) -> Result<Value, String> {
    answer(&send(view, cx, "tester", verb, args))
}

fn pump_until(cx: &mut VisualTestContext, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    for _ in 0..2000 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the extension work did not finish");
}

fn three_tabs() -> (AppState, [TabId; 3]) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tabs = [0, 1, 2].map(|_| state.open_terminal_tab(home).expect("tab"));
    state.select_tab(home, tabs[0]).expect("select");
    (state, tabs)
}

fn sent_since(view: &Entity<AppModel>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |model, _| {
        std::mem::take(&mut *model.extensions.events.sent.borrow_mut())
    })
}

#[gpui::test]
fn extensions_rename_color_pin_and_move_any_tab(cx: &mut TestAppContext) {
    let (state, [a, b, c]) = three_tabs();
    let (view, cx) = open(state, cx);
    let order = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| {
            model
                .state
                .home()
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect::<Vec<_>>()
        })
    };
    let tab = |id: TabId| json!(id.to_string());

    assert_eq!(
        call(
            &view,
            cx,
            "tabs.rename",
            json!({"tabId": tab(a), "title": " Logs "})
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        call(
            &view,
            cx,
            "tabs.setColor",
            json!({"tabId": tab(b), "color": "blue"})
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        call(
            &view,
            cx,
            "tabs.pin",
            json!({"tabId": tab(c), "pinned": true})
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        call(
            &view,
            cx,
            "tabs.pin",
            json!({"tabId": tab(c), "pinned": true})
        ),
        Ok(Value::Null)
    );
    assert_eq!(order(&view, cx), [c, a, b]);

    let listed = call(&view, cx, "tabs.list", json!({})).expect("list");
    let entry = |id: TabId| {
        listed
            .as_array()
            .and_then(|tabs| tabs.iter().find(|entry| entry["id"] == tab(id)))
            .cloned()
            .expect("listed tab")
    };
    assert_eq!(entry(a)["customTitle"], "Logs");
    assert_eq!(
        entry(b)["color"],
        muxy_app_core::PROJECT_COLORS[8].1.to_lowercase()
    );
    assert_eq!(entry(c)["isPinned"], true);
    assert_eq!(entry(a)["paneIDs"].as_array().map(Vec::len), Some(1));

    assert_eq!(
        call(&view, cx, "tabs.move", json!({"tabId": tab(b), "index": 1})),
        Ok(Value::Null)
    );
    assert_eq!(order(&view, cx), [c, b, a]);
    assert_eq!(
        call(&view, cx, "tabs.move", json!({"tabId": tab(b), "index": 0})),
        Ok(Value::Null)
    );
    assert_eq!(order(&view, cx), [c, b, a], "pinned tabs stay first");

    assert_eq!(
        call(
            &view,
            cx,
            "tabs.rename",
            json!({"tabId": tab(a), "title": null})
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        call(
            &view,
            cx,
            "tabs.setColor",
            json!({"tabId": tab(b), "color": null})
        ),
        Ok(Value::Null)
    );
    view.read_with(cx, |model, _| {
        assert!(model.tab(a).is_some_and(|tab| tab.custom_title.is_none()));
        assert!(model.tab(b).is_some_and(|tab| tab.color.is_none()));
    });
}

#[gpui::test]
fn tab_controls_reject_bad_input_and_leave_tabs_alone(cx: &mut TestAppContext) {
    let (state, [a, ..]) = three_tabs();
    let (view, cx) = open(state, cx);
    let tab = |id: TabId| json!(id.to_string());
    let before = view.read_with(cx, |model, _| model.state.clone());
    assert_eq!(
        call(
            &view,
            cx,
            "tabs.rename",
            json!({"tabId": "nope", "title": "x"})
        ),
        Err("tab not found nope".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "tabs.setColor",
            json!({"tabId": tab(a), "color": "plaid"})
        ),
        Err("unknown color 'plaid'".into())
    );
    assert_eq!(
        call(&view, cx, "tabs.move", json!({"tabId": tab(a), "index": 9})),
        Err("index 9 is out of range".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "tabs.close",
            json!({"tabId": tab(a), "scope": "everything"})
        ),
        Err("invalid scope 'everything'".into())
    );
    assert_eq!(
        call(&view, cx, "tabs.dock", json!({"tabId": tab(a)})),
        Err("the tab is in the main window".into())
    );
    view.read_with(cx, |model, _| assert_eq!(model.state, before));
}

#[gpui::test]
fn extensions_open_a_tab_beside_another(cx: &mut TestAppContext) {
    let (state, [a, b, _]) = three_tabs();
    let (view, cx) = open(state, cx);
    let created = call(
        &view,
        cx,
        "tabs.new",
        json!({"beside": a.to_string(), "side": "right"}),
    )
    .expect("new tab");
    view.read_with(cx, |model, _| {
        let tabs: Vec<_> = model
            .state
            .home()
            .tabs
            .iter()
            .map(|tab| tab.id.to_string())
            .collect();
        assert_eq!(tabs[1], created.as_str().unwrap_or(""));
        assert_eq!(tabs[2], b.to_string());
    });
    assert_eq!(
        call(
            &view,
            cx,
            "tabs.new",
            json!({"beside": a.to_string(), "side": "above"})
        ),
        Err("invalid side 'above'".into())
    );
}

#[gpui::test]
fn extensions_zoom_focus_and_check_panes(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tab = state.open_terminal_tab(home).expect("tab");
    let first = state.window().active_pane.expect("pane");
    let second = state.split_pane(first, Direction::Right).expect("split");
    state.select_tab(home, tab).expect("select");
    let (view, cx) = open(state, cx);
    let pane = |id: PaneId| json!({"paneID": id.to_string()});
    let zoomed = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| model.tab(tab).and_then(|tab| tab.zoomed))
    };

    assert_eq!(call(&view, cx, "panes.focus", pane(first)), Ok(Value::Null));
    view.read_with(cx, |model, _| assert_eq!(model.active_pane(), Some(first)));

    let mut args = pane(second);
    args["zoomed"] = json!(true);
    assert_eq!(call(&view, cx, "panes.zoom", args.clone()), Ok(Value::Null));
    assert_eq!(zoomed(&view, cx), Some(second));
    assert_eq!(call(&view, cx, "panes.zoom", args), Ok(Value::Null));
    assert_eq!(
        zoomed(&view, cx),
        Some(second),
        "zooming twice keeps the zoom"
    );
    let mut args = pane(first);
    args["zoomed"] = json!(true);
    assert_eq!(call(&view, cx, "panes.zoom", args), Ok(Value::Null));
    assert_eq!(
        zoomed(&view, cx),
        Some(first),
        "zoom moves to the pane asked for"
    );
    assert_eq!(call(&view, cx, "panes.zoom", pane(first)), Ok(Value::Null));
    assert_eq!(zoomed(&view, cx), None, "without `zoomed` it toggles");
    let mut args = pane(second);
    args["zoomed"] = json!(false);
    assert_eq!(call(&view, cx, "panes.zoom", args), Ok(Value::Null));
    assert_eq!(zoomed(&view, cx), None);

    let listed = call(&view, cx, "panes.list", json!({})).expect("list");
    let entry = listed
        .as_array()
        .and_then(|panes| panes.iter().find(|entry| entry["id"] == second.to_string()))
        .cloned()
        .expect("listed pane");
    assert_eq!(entry["tabID"], tab.to_string());
    assert_eq!(entry["worktreeID"], home.to_string());

    assert_eq!(
        call(&view, cx, "panes.equalize", pane(first)),
        Ok(Value::Null)
    );
    assert_eq!(
        call(&view, cx, "panes.detach", pane(first)),
        Err("the terminal cannot be detached".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "panes.split",
            json!({"paneID": first.to_string(), "direction": "sideways"})
        ),
        Err("invalid direction 'sideways'".into())
    );
    let mut args = pane(first);
    args["scope"] = json!("everywhere");
    assert_eq!(
        call(&view, cx, "panes.float", args),
        Err("invalid scope 'everywhere'".into())
    );
    assert_eq!(
        call(&view, cx, "panes.focus", json!({"paneID": "nope"})),
        Err("pane not found nope".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "agents.focus",
            json!({"paneID": PaneId::new().to_string()})
        )
        .map_err(|error| error.starts_with("pane not found")),
        Err(true)
    );
}

#[gpui::test]
fn extensions_sort_and_restore_projects(cx: &mut TestAppContext) {
    let (view, cx) = open(AppState::bootstrap().expect("state"), cx);
    assert_eq!(
        call(
            &view,
            cx,
            "projects.sort",
            json!({"order": "nameDescending"})
        ),
        Ok(Value::Null)
    );
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.appearance.sidebar_project_order,
            muxy_app_core::settings::ProjectOrder::NameDescending
        );
    });
    assert_eq!(
        call(&view, cx, "projects.sort", json!({"order": "size"})),
        Err("invalid order 'size'".into())
    );
    assert_eq!(
        call(&view, cx, "projects.removed", json!({})),
        Ok(json!([]))
    );
    assert_eq!(
        call(&view, cx, "projects.restore", json!({"index": 0})),
        Err("no removed project at that index".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "projects.reveal",
            json!({"identifier": "nowhere"})
        ),
        Err("project not found nowhere".into())
    );
    assert_eq!(call(&view, cx, "projects.next", json!({})), Ok(Value::Null));
}

#[gpui::test]
fn extensions_drive_the_window_layout_and_report_it(cx: &mut TestAppContext) {
    let (view, cx) = open(AppState::bootstrap().expect("state"), cx);
    view.update(cx, AppModel::sync_extension_events);
    sent_since(&view, cx);

    assert_eq!(
        call(&view, cx, "ui.setLayout", json!({"layout": "agents"})),
        Ok(Value::Null)
    );
    assert_eq!(
        call(&view, cx, "ui.setSidebar", json!({"visible": false})),
        Ok(Value::Null)
    );
    let state = call(&view, cx, "ui.state", json!({})).expect("state");
    assert_eq!(state["layout"], "agents");
    assert_eq!(state["sidebar"], false);
    assert!(sent_since(&view, cx).contains(&"ui.changed".to_owned()));

    assert_eq!(
        call(&view, cx, "ui.setSidebar", json!({"visible": false})),
        Ok(Value::Null)
    );
    view.read_with(cx, |model, _| assert!(!model.appearance.sidebar_expanded));
    assert_eq!(
        call(&view, cx, "ui.setLayout", json!({"layout": "grid"})),
        Err("invalid layout 'grid'".into())
    );
    assert_eq!(
        call(&view, cx, "ui.openSettings", json!({"section": "secrets"})),
        Err("unknown settings section 'secrets'".into())
    );
    assert_eq!(call(&view, cx, "ui.back", json!({})), Ok(json!(false)));
}

#[gpui::test]
fn extensions_list_and_run_built_in_commands_but_never_quit(cx: &mut TestAppContext) {
    let (state, _) = three_tabs();
    let (view, cx) = open(state, cx);
    let listed = call(&view, cx, "commands.list", json!({})).expect("list");
    let ids: Vec<_> = listed
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|command| command["id"].as_str())
        .collect();
    assert!(ids.contains(&"new_tab"), "{ids:?}");
    for reserved in [
        "quit",
        "end_all_sessions_and_quit",
        "install_command_line_tool",
        "toggle_full_screen",
        "browser_copy_url",
    ] {
        assert!(!ids.contains(&reserved), "{reserved}");
        assert_eq!(
            call(&view, cx, "commands.run", json!({"id": reserved})),
            Err(format!("command not found {reserved}"))
        );
    }

    let count = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.read_with(cx, |model, _| model.state.home().tabs.len())
    };
    let before = count(&view, cx);
    assert_eq!(
        call(&view, cx, "commands.run", json!({"id": "new_tab"})),
        Ok(Value::Null)
    );
    assert_eq!(count(&view, cx), before + 1);
    assert_eq!(
        call(&view, cx, "commands.run", json!({"id": "command.missing"})),
        Err("command not found command.missing".into())
    );
    assert_eq!(
        call(&view, cx, "commands.other", json!({"id": "new_tab"})),
        Err("unsupported extension API: commands.other".into())
    );
    assert_eq!(count(&view, cx), before + 1);
}

#[gpui::test]
fn running_a_custom_command_asks_first(cx: &mut TestAppContext) {
    let (view, cx) = open(AppState::bootstrap().expect("state"), cx);
    view.update(cx, |model, _| {
        model
            .settings
            .commands
            .push(muxy_app_core::settings::CustomCommand {
                id: "deploy".into(),
                name: "Deploy".into(),
                command: "make deploy".into(),
            });
        let (reply, _replies) = std::sync::mpsc::sync_channel(1);
        let mut call = Call {
            owner: "tester".into(),
            epoch: 0,
            generation: model.servers.local.generation,
            project: model.state.home().id,
            verb: "commands.run".into(),
            args: json!({"id": "command.deploy"}),
            reply: Reply::Script(1, reply),
            approved: false,
        };
        let request = model.consent_request(&call).expect("valid").expect("asks");
        assert_eq!(
            request.gate,
            muxy_app_core::extensions::Gate::TabsRunCommand
        );
        assert_eq!(request.pattern, "command:make deploy");
        call.args = json!({"id": "command.missing"});
        assert_eq!(
            model
                .consent_request(&call)
                .map(|request| request.is_some()),
            Err("command not found command.missing".into())
        );
        call.args = json!({"id": "new_tab"});
        assert!(matches!(model.consent_request(&call), Ok(None)));
        call.verb = "composer.submit".into();
        assert_eq!(
            model
                .consent_request(&call)
                .map(|request| request.is_some()),
            Err("the composer is closed".into())
        );
    });
}

#[gpui::test]
fn extensions_read_and_change_preferences(cx: &mut TestAppContext) {
    let (view, cx) = open(AppState::bootstrap().expect("state"), cx);
    view.update(cx, AppModel::sync_extension_events);
    sent_since(&view, cx);

    assert_eq!(
        call(
            &view,
            cx,
            "preferences.set",
            json!({"key": "tips", "value": false})
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        call(
            &view,
            cx,
            "preferences.set",
            json!({"key": "interfaceSize", "value": "huge"})
        ),
        Ok(Value::Null)
    );
    let values = call(&view, cx, "preferences.get", json!({})).expect("preferences");
    assert_eq!(values["tips"], false);
    assert_eq!(values["interfaceSize"], "huge");
    assert!(sent_since(&view, cx).contains(&"preferences.changed".to_owned()));

    for (key, value, error) in [
        ("tips", json!("no"), "tips must be true or false"),
        (
            "interfaceSize",
            json!("tiny"),
            "invalid interface size 'tiny'",
        ),
        (
            "theme.dark",
            json!("No Such Theme"),
            "unknown theme 'No Such Theme'",
        ),
        ("shell", json!("/bin/sh"), "unknown preference 'shell'"),
    ] {
        assert_eq!(
            call(
                &view,
                cx,
                "preferences.set",
                json!({"key": key, "value": value})
            ),
            Err(error.into())
        );
    }
    let themes = call(&view, cx, "preferences.themes", json!({})).expect("themes");
    let first = themes[0]["name"].as_str().expect("a theme").to_owned();
    assert_eq!(
        call(
            &view,
            cx,
            "preferences.set",
            json!({"key": "theme.light", "value": first})
        ),
        Ok(Value::Null)
    );
    view.read_with(cx, |model, _| {
        assert_eq!(model.appearance.light_theme, first);
    });
}

#[gpui::test]
fn extensions_use_the_clipboard_and_open_only_web_links(cx: &mut TestAppContext) {
    let (view, cx) = open(AppState::bootstrap().expect("state"), cx);
    assert_eq!(
        call(&view, cx, "clipboard.writeText", json!({"text": "copied"})),
        Ok(Value::Null)
    );
    assert_eq!(
        call(&view, cx, "clipboard.readText", json!({})),
        Ok(json!("copied"))
    );
    for url in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "https://example.com/a b",
        "",
    ] {
        assert_eq!(
            call(&view, cx, "app.openExternal", json!({"url": url})),
            Err("only http and https URLs can be opened".into()),
            "{url}"
        );
    }
    let info = call(&view, cx, "app.info", json!({})).expect("info");
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert!(info["theme"]["colorScheme"].is_string());
}

#[gpui::test]
fn extensions_edit_the_composer_draft(cx: &mut TestAppContext) {
    let (view, cx) = open(AppState::bootstrap().expect("state"), cx);
    assert_eq!(
        call(&view, cx, "composer.submit", json!({})),
        Err("the composer is closed".into())
    );
    assert_eq!(
        call(&view, cx, "composer.setDraft", json!({"text": "hello"})),
        Ok(Value::Null)
    );
    view.read_with(cx, |model, _| assert!(model.composer.view.is_some()));
    assert_eq!(
        call(&view, cx, "composer.getDraft", json!({})),
        Ok(json!("hello"))
    );
    assert_eq!(
        call(&view, cx, "composer.close", json!({})),
        Ok(Value::Null)
    );
    view.read_with(cx, |model, _| assert!(model.composer.view.is_none()));
}

#[gpui::test]
fn pinning_a_tab_sends_tab_updated(cx: &mut TestAppContext) {
    let (state, [a, ..]) = three_tabs();
    let (view, cx) = open(state, cx);
    view.update(cx, AppModel::sync_extension_events);
    sent_since(&view, cx);
    assert_eq!(
        call(&view, cx, "tabs.pin", json!({"tabId": a.to_string()})),
        Ok(Value::Null)
    );
    assert_eq!(sent_since(&view, cx), ["tab.updated"]);
}

#[gpui::test]
fn extensions_read_and_change_their_own_settings(cx: &mut TestAppContext) {
    let (view, cx) = open(AppState::bootstrap().expect("state"), cx);
    let directory = view.read_with(cx, |model, _| model.extensions.registry.directory());
    let root = directory.join("greeter");
    std::fs::create_dir_all(&root).expect("directory");
    std::fs::write(
        root.join("package.json"),
        json!({
            "name": "greeter",
            "version": "1.0.0",
            "muxy": {
                "events": ["settings.changed"],
                "settings": [
                    {"key": "greeting", "title": "Greeting", "type": "string", "defaultValue": "Hello"},
                    {"key": "loud", "title": "Shout", "type": "bool"}
                ]
            },
        })
        .to_string(),
    )
    .expect("package");
    view.update(cx, AppModel::refresh_installed_extensions);
    pump_until(cx, |cx| {
        view.read_with(cx, |model, _| {
            model.extensions.registry.extensions.contains_key("greeter")
        })
    });
    let enabled = view.update(cx, |model, cx| {
        model.set_extension_enabled("greeter", true, cx)
    });
    cx.background_executor.block_test(enabled).expect("enabled");
    pump_until(cx, |cx| {
        view.read_with(cx, |model, _| {
            model.extensions.registry.enabled("greeter").is_some()
        })
    });
    let mut ask = |verb: &str, args: Value| {
        let replies = send(&view, cx, "greeter", verb, args);
        let mut reply = None;
        pump_until(cx, |_| {
            reply = replies.try_recv().ok();
            reply.is_some()
        });
        let reply: Value = serde_json::from_str(&reply.unwrap_or_default()).expect("reply");
        if reply["ok"] == true {
            Ok(reply["value"].clone())
        } else {
            Err(reply["error"].as_str().unwrap_or("").to_owned())
        }
    };

    assert_eq!(
        ask("settings.get", json!({"key": "greeting"})),
        Ok(json!("Hello"))
    );
    assert_eq!(
        ask("settings.all", json!({})),
        Ok(json!({"greeting": "Hello", "loud": null}))
    );
    assert_eq!(
        ask("settings.set", json!({"key": "greeting", "value": "Hi"})),
        Ok(Value::Null)
    );
    assert_eq!(
        ask("settings.get", json!({"key": "greeting"})),
        Ok(json!("Hi"))
    );
    assert_eq!(
        ask("settings.set", json!({"key": "loud", "value": "yes"})),
        Err("setting 'loud' has another type".into())
    );
    assert_eq!(
        ask("settings.get", json!({"key": "secret"})),
        Err("setting 'secret' not declared in manifest".into())
    );
    assert_eq!(
        ask("settings.reset", json!({"key": "greeting"})),
        Ok(Value::Null)
    );
    assert_eq!(
        ask("settings.get", json!({"key": "greeting"})),
        Ok(json!("Hello"))
    );
    assert!(sent_since(&view, cx).contains(&"settings.changed".to_owned()));
}

#[test]
fn changed_settings_reports_each_stored_value_that_differs() {
    let settings = |pairs: &[(&str, &str, Value)]| {
        let mut all: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
        for (owner, key, value) in pairs {
            all.entry((*owner).into())
                .or_default()
                .insert((*key).into(), value.clone());
        }
        all
    };
    let before = settings(&[
        ("a", "x", json!(1)),
        ("a", "y", json!(2)),
        ("b", "z", json!(true)),
    ]);
    let after = settings(&[
        ("a", "x", json!(1)),
        ("a", "y", json!(3)),
        ("c", "w", json!("new")),
    ]);
    assert_eq!(
        events::changed_settings(&before, &after),
        [
            ("a".to_owned(), "y".to_owned()),
            ("b".to_owned(), "z".to_owned()),
            ("c".to_owned(), "w".to_owned()),
        ]
    );
}
