use std::cell::RefCell;

use super::*;
use serde_json::{Value, json};

/// Sends a `browser.*` request as the CLI does and waits for the reply.
fn call(
    view: &Entity<AppModel>,
    cx: &mut VisualTestContext,
    verb: &str,
    args: &Value,
) -> Result<Value, String> {
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
