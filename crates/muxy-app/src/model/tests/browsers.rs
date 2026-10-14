use std::cell::RefCell;

use super::*;
use muxy_app_core::PaneContent;
use serde_json::{Value, json};

/// Sends a `browser.*` request as the CLI does and waits for the reply.
fn call(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    verb: &str,
    args: &Value,
) -> std::result::Result<Value, String> {
    let args = args.as_object().cloned().unwrap_or_default();
    let reply = view.update_in(cx, |model, window, cx| {
        model.browser_call(verb, &args, window, cx)
    });
    let result = Rc::new(RefCell::new(None));
    let slot = Rc::clone(&result);
    cx.update(|_, cx| {
        cx.spawn(async move |_| {
            *slot.borrow_mut() = Some(reply.await);
        })
        .detach();
    });
    cx.run_until_parked();
    result.borrow_mut().take().expect("reply")
}

fn browser_pane(model: &AppModel) -> Option<PaneId> {
    model.browser_panes().first().map(|(_, _, pane, _)| *pane)
}

#[gpui::test]
fn new_browser_tabs_open_on_the_start_page_and_requests_drive_them(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, AppModel::new_browser_tab);
    cx.run_until_parked();
    let pane = view.read_with(cx, |model, _| {
        let pane = browser_pane(model).expect("browser pane");
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(model.state.browser(pane).and_then(|b| b.url.clone()), None);
        pane
    });
    let tab = pane.to_string();

    let listed = call(&view, cx, "browser.list", &json!({})).expect("list");
    assert_eq!(listed[0]["tabId"], json!(tab));
    assert_eq!(listed[0]["url"], json!(""));
    assert_eq!(listed[0]["active"], json!(true));
    assert_eq!(listed[0]["profile"], json!("Default"));

    call(
        &view,
        cx,
        "browser.navigate",
        &json!({"tabId": tab, "url": "example.com/docs"}),
    )
    .expect("navigate");
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.state.browser(pane).and_then(|b| b.url.as_deref()),
            Some("https://example.com/docs")
        );
    });

    assert_eq!(
        call(&view, cx, "browser.click", &json!({"tabId": tab})),
        Err("missing argument 'selector'".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "browser.click",
            &json!({"tabId": "nope", "selector": "a"})
        ),
        Err("browser tab not found nope".into())
    );
    assert_eq!(
        call(&view, cx, "browser.teleport", &json!({"tabId": tab})),
        Err("unknown command browser.teleport".into())
    );
    assert_eq!(
        call(
            &view,
            cx,
            "browser.navigate",
            &json!({"tabId": tab, "url": " "})
        ),
        Err("invalid url".into())
    );

    call(&view, cx, "browser.close", &json!({"tabId": tab})).expect("close");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(browser_pane(model).is_none()));
}

#[gpui::test]
fn opening_in_another_project_leaves_the_view_and_split_opens_beside(cx: &mut TestAppContext) {
    let (state, first, second, first_pane, _) = projects::two_projects();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| model.select_project(first, cx));
    cx.run_until_parked();

    let opened = call(
        &view,
        cx,
        "browser.open",
        &json!({"url": "https://example.com", "project": second.to_string()}),
    )
    .expect("open");
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, first);
        assert_eq!(model.active_pane(), Some(first_pane));
        let pane: PaneId = opened.as_str().expect("tab ID").parse().expect("pane");
        assert_eq!(model.state.pane_project(pane), Some(second));
    });
    let listed = call(
        &view,
        cx,
        "browser.list",
        &json!({"project": first.to_string()}),
    )
    .expect("list");
    assert_eq!(listed, json!([]));

    let split = call(&view, cx, "browser.open", &json!({"split": true})).expect("split");
    view.read_with(cx, |model, _| {
        let pane: PaneId = split.as_str().expect("tab ID").parse().expect("pane");
        assert_eq!(model.active_pane(), Some(pane));
        let tab = model.tab(model.active_tab().expect("tab")).expect("tab");
        assert_eq!(tab.panes.len(), 2);
        assert!(tab.panes.iter().any(|candidate| candidate.id == first_pane));
        assert!(matches!(
            model.state.browser(pane),
            Some(browser) if browser.url.is_none()
        ));
    });
}

#[gpui::test]
fn a_disabled_browser_opens_nothing_and_lists_nothing(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let (_, pane) = state
        .open_browser(home, muxy_app_core::browser::BrowserPane::default(), true)
        .expect("browser");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.browser.enabled = false;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, AppModel::new_browser_tab);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.browser_panes().len(), 1);
        assert!(model.browser_view(pane).is_none());
        assert_eq!(
            model.browser_unavailable(pane).as_deref(),
            Some("Built-in browser is disabled")
        );
    });
    assert_eq!(call(&view, cx, "browser.list", &json!({})), Ok(json!([])));
    assert_eq!(
        call(&view, cx, "browser.open", &json!({})),
        Err("the built-in browser is disabled".into())
    );
}

#[gpui::test]
fn urls_open_in_a_new_browser_tab_of_the_current_project(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.open_url_in_browser("https://example.com".into(), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let panes = model.browser_panes();
        assert_eq!(panes.len(), 1);
        assert!(matches!(
            &model.state.home().tabs[0].panes[0].content,
            PaneContent::Browser(browser) if browser.url.as_deref() == Some("https://example.com")
        ));
    });
}

fn saved_profile(model: &AppModel, pane: PaneId) -> Option<String> {
    model
        .state
        .browser(pane)
        .and_then(|browser| browser.profile.clone())
}

#[gpui::test]
fn tabs_keep_the_profile_they_opened_with_when_the_default_changes(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let opened = call(&view, cx, "browser.open", &json!({})).expect("open");
    let pane: PaneId = opened.as_str().expect("tab ID").parse().expect("pane");
    view.update(cx, |model, _| {
        let default = model
            .browser_profile(None)
            .expect("default")
            .id()
            .to_string();
        assert_eq!(saved_profile(model, pane), Some(default.clone()));
        model.add_browser_profile("Work").expect("profile");
        let work = model.browser_profile(Some("Work")).expect("work");
        model.settings.browser.default_profile = work.id().to_string();
        assert_eq!(saved_profile(model, pane), Some(default.clone()));
        assert_eq!(
            model
                .pane_profile(saved_profile(model, pane).as_deref())
                .expect("tab profile")
                .name(),
            "Default"
        );
    });
}

#[gpui::test]
fn deleting_a_profile_moves_its_tabs_only_once_it_is_gone(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.add_browser_profile("Work").expect("profile");
    });
    let opened = call(&view, cx, "browser.open", &json!({"profile": "Work"})).expect("open");
    let pane: PaneId = opened.as_str().expect("tab ID").parse().expect("pane");
    view.update(cx, |model, cx| {
        let work = saved_profile(model, pane).expect("work profile");
        let default = model
            .browser_profile(None)
            .expect("default")
            .id()
            .to_string();
        let missing = muxy_ui::browser::polybrowser::ProfileId::from_bytes([7; 16]).to_string();
        model
            .state
            .set_browser_profile(pane, Some(missing.clone()))
            .expect("assign");
        assert!(model.delete_browser_profile(&missing, cx).is_err());
        assert_eq!(saved_profile(model, pane), Some(missing));

        model
            .state
            .set_browser_profile(pane, Some(work.clone()))
            .expect("assign");
        model.delete_browser_profile(&work, cx).expect("delete");
        assert_eq!(saved_profile(model, pane), Some(default));
        assert!(model.browser_profile(Some(&work)).is_err());
    });
}

#[test]
fn page_scripts_are_parsed_to_tell_expressions_from_statements() -> std::result::Result<(), String>
{
    use muxy_app_core::browser::automation::{condition, script};
    use muxy_ui::javascript::is_async_function_body;
    let args = |value: Value| value.as_object().cloned().unwrap_or_default();
    let eval = |source: &str| {
        script(
            "eval",
            &args(json!({ "script": source })),
            is_async_function_body,
        )
    };
    assert_eq!(eval("'a;b'")?.as_deref(), Some("return (\n'a;b'\n);"));
    assert_eq!(
        eval("document.title;")?.as_deref(),
        Some("return (\ndocument.title\n);")
    );
    assert_eq!(
        eval("const a = 1;\nreturn a;")?.as_deref(),
        Some("const a = 1;\nreturn a;")
    );
    let wait = condition(
        "wait",
        &args(json!({ "function": "(() => { return true; })()" })),
        is_async_function_body,
    )?;
    assert!(
        wait.contains("return (\n(() => { return true; })()\n);"),
        "{wait}"
    );
    Ok(())
}

#[test]
fn extension_arguments_are_named_like_the_cli_names_them() {
    use crate::model::browsers::automation::extension_args;
    let typed = extension_args(
        "browser.type",
        &json!({"tabId": "t", "args": ["#q", "muxy", {"submit": true}]}),
    );
    assert_eq!(
        Value::Object(typed),
        json!({"tabId": "t", "selector": "#q", "text": "muxy", "submit": true})
    );
    let wait = extension_args(
        "browser.wait",
        &json!({"tabId": "t", "args": [{"text": "Done", "timeoutMs": 50}]}),
    );
    assert_eq!(
        Value::Object(wait),
        json!({"tabId": "t", "text": "Done", "timeoutMs": 50})
    );
    let press = extension_args(
        "browser.press",
        &json!({"tabId": "t", "args": ["Enter", null]}),
    );
    assert_eq!(Value::Object(press), json!({"tabId": "t", "key": "Enter"}));
    let page = extension_args("browser.open", &json!({"url": "a.dev", "split": true}));
    assert_eq!(Value::Object(page), json!({"url": "a.dev", "split": true}));
}
