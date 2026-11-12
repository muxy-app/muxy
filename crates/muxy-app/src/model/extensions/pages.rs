//! What `muxy extension` does with an extension's live pages: listing them,
//! opening its surfaces, running its commands, sending it events, and driving
//! a page the way `muxy browser` drives a tab.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use gpui::{Context, Entity, Task, WeakEntity, Window};
use muxy_app_core::browser::automation::{
    self, Args, DEFAULT_WAIT, MAX_WAIT, POLL_INTERVAL, string,
};
use muxy_app_core::extensions::Extension;
use muxy_app_core::webview::WebviewDescriptor;
use muxy_ui::javascript::is_async_function_body;
use serde_json::{Value, json};

use super::AppModel;
use crate::views::webview::automation::{Picture, png};
use crate::views::webview::{SurfaceKind, Webview};

type Reply = Task<Result<Value, String>>;

/// How long an opened page has to finish loading.
const OPEN_WAIT: Duration = Duration::from_secs(15);
/// How long a page has to hand over a picture of itself.
const CAPTURE_WAIT: Duration = Duration::from_secs(15);

/// Whether `verb` is one of the page commands, which need the window.
pub(crate) fn is_page_verb(verb: &str) -> bool {
    matches!(
        verb,
        "extension.pages" | "extension.open" | "extension.run" | "extension.emit"
    ) || verb.starts_with("extension.page.")
}

fn kind_name(kind: SurfaceKind) -> &'static str {
    match kind {
        SurfaceKind::Modal => "modal",
        kind => kind.name(),
    }
}

fn page_id(view: &Entity<Webview>) -> String {
    view.entity_id().as_u64().to_string()
}

fn page_entry(view: &Entity<Webview>, cx: &gpui::App) -> Value {
    let page = view.read(cx);
    json!({
        "pageId": page_id(view),
        "extension": page.source.owner,
        "kind": kind_name(page.kind),
        "id": page.instance,
        "instance": page.panel_instance(),
        "title": page.title,
        "loaded": page.loaded(),
        "visible": page.shown(),
        "error": page.error(),
    })
}

impl AppModel {
    /// A `muxy extension` page command from the app socket.
    pub(crate) fn extension_page_cli_call(
        &mut self,
        verb: &str,
        args: &Args,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Reply {
        self.extension_cli_page_request(verb, args, window, cx)
            .unwrap_or_else(|error| Task::ready(Err(error)))
    }

    fn extension_cli_page_request(
        &mut self,
        verb: &str,
        args: &Args,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Reply, String> {
        if !self.extensions_loaded() {
            let refresh = self.refresh_installed_extensions_task(cx);
            let (verb, args) = (verb.to_owned(), args.clone());
            return Ok(cx.spawn_in(window, async move |model, cx| {
                refresh.await?;
                model
                    .update_in(cx, |model, window, cx| {
                        model.extension_page_cli_call(&verb, &args, window, cx)
                    })
                    .map_err(|error| error.to_string())?
                    .await
            }));
        }
        let done = |value: Value| Ok(Task::ready(Ok(value)));
        match verb {
            "extension.pages" => {
                let name = string(args, "name")?;
                self.enabled_extension(name)?;
                done(self.pages_of(name, cx).into())
            }
            "extension.open" => self.open_extension_surface(args, window, cx),
            "extension.run" => {
                let name = string(args, "name")?;
                let command = string(args, "command")?;
                let extension = self.enabled_extension(name)?;
                if extension.manifest.command(command).is_none()
                    && !self.runtime_command_event(name, &format!("command.{command}"))
                {
                    return Err(format!(
                        "{name} has no command '{command}'; its commands are listed in its package.json"
                    ));
                }
                self.run_extension_command(name, command, window, cx);
                done(Value::Null)
            }
            "extension.emit" => {
                let name = string(args, "name")?;
                let event = string(args, "event")?;
                let extension = self.enabled_extension(name)?;
                if !extension.allows_event(event) && !self.runtime_command_event(name, event) {
                    return Err(format!(
                        "{name} doesn't list '{event}' in its manifest's events, so it would not receive it"
                    ));
                }
                let payload = match args.get("payload") {
                    None | Some(Value::Null) => json!({}),
                    Some(payload @ Value::Object(_)) => payload.clone(),
                    Some(_) => return Err("the payload must be a JSON object".into()),
                };
                self.emit_extension_event(Some(name), event, &payload, cx);
                done(Value::Null)
            }
            _ => {
                let action = verb.strip_prefix("extension.page.").unwrap_or(verb);
                let view = self.extension_page(args)?;
                page_action(action, args, &view, cx)
            }
        }
    }

    fn enabled_extension(&self, name: &str) -> Result<Extension, String> {
        if let Some(extension) = self.extensions.registry.enabled(name) {
            return Ok(extension.clone());
        }
        if self.extensions.registry.extensions.contains_key(name) {
            return Err(format!(
                "{name} is turned off. Run muxy extension enable {name}."
            ));
        }
        Err(format!("{name} isn't loaded. Run muxy extension list."))
    }

    fn pages_of(&self, name: &str, cx: &gpui::App) -> Vec<Value> {
        self.extension_views()
            .into_iter()
            .filter(|view| view.read(cx).source.owner == name)
            .map(|view| page_entry(view, cx))
            .collect()
    }

    fn extension_page(&self, args: &Args) -> Result<Entity<Webview>, String> {
        let id = string(args, "pageId")?;
        self.extension_views()
            .into_iter()
            .find(|view| page_id(view) == id)
            .cloned()
            .ok_or_else(|| format!("no extension page {id}; run muxy extension pages <name>"))
    }

    /// Opens one of an extension's surfaces and answers with its page once
    /// that has loaded.
    fn open_extension_surface(
        &mut self,
        args: &Args,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Reply, String> {
        let name = string(args, "name")?.to_owned();
        let surface = string(args, "surface")?;
        let id = string(args, "id")?.to_owned();
        let data = args.get("data").cloned().unwrap_or(Value::Null);
        let extension = self.enabled_extension(&name)?;
        let before: HashSet<String> = self.extension_views().into_iter().map(page_id).collect();
        let (kind, instance) = match surface {
            "panel" => {
                if extension.manifest.panel(&id).is_none() {
                    return Err(format!("{name} has no panel '{id}'"));
                }
                self.panel_operation(
                    &name,
                    "panels.open",
                    &json!({ "panelID": id, "data": data }),
                    window,
                    cx,
                )?;
                (SurfaceKind::Panel, Some(id))
            }
            "tab" => {
                let pane = self.open_webview_tab(
                    WebviewDescriptor {
                        owner: name.clone(),
                        kind: id.clone(),
                        data,
                    },
                    false,
                    cx,
                )?;
                (SurfaceKind::Tab, Some(pane.to_string()))
            }
            "modal" => {
                self.open_extension_modal(
                    &extension,
                    &json!({ "entry": id, "data": data }),
                    window,
                    cx,
                )?;
                (SurfaceKind::Modal, None)
            }
            other => {
                return Err(format!(
                    "can't open '{other}'; use panel:<id>, tab:<type> or modal:<entry>"
                ));
            }
        };
        cx.notify();
        let deadline = Instant::now() + OPEN_WAIT;
        Ok(cx.spawn(async move |model, cx| {
            loop {
                let found = model
                    .read_with(cx, |model, cx| {
                        model
                            .extension_views()
                            .into_iter()
                            .filter(|view| {
                                let page = view.read(cx);
                                page.source.owner == name
                                    && page.kind == kind
                                    && instance
                                        .as_ref()
                                        .is_none_or(|instance| page.instance == *instance)
                            })
                            .max_by_key(|view| !before.contains(&page_id(view)))
                            .map(|view| {
                                let page = view.read(cx);
                                (
                                    page.loaded(),
                                    page.error().map(str::to_owned),
                                    page_entry(view, cx),
                                )
                            })
                    })
                    .map_err(|_| "Muxy is closing".to_owned())?;
                match found {
                    Some((_, Some(error), _)) => return Err(error),
                    Some((true, None, entry)) => return Ok(entry),
                    _ => {}
                }
                if Instant::now() >= deadline {
                    return Err(format!(
                        "the page didn't finish loading within {} seconds; check muxy extension logs {name}",
                        OPEN_WAIT.as_secs()
                    ));
                }
                cx.background_executor().timer(POLL_INTERVAL).await;
            }
        }))
    }
}

/// What `muxy extension page <action>` does to one page.
fn page_action(
    action: &str,
    args: &Args,
    view: &Entity<Webview>,
    cx: &mut Context<AppModel>,
) -> Result<Reply, String> {
    match action {
        "screenshot" => Ok(screenshot(view, cx)),
        "wait" | "waitFor" => {
            let condition = automation::condition(action, args, is_async_function_body)?;
            Ok(wait_until(
                view.downgrade(),
                condition,
                automation::timeout(args, DEFAULT_WAIT),
                cx,
            ))
        }
        _ => {
            let script = automation::script(action, args, is_async_function_body)?
                .ok_or_else(|| format!("unknown page command {action}"))?;
            Ok(run_script(view, &script, MAX_WAIT, cx))
        }
    }
}

/// Answers with what `receiver` gets, or `late` once `timeout` passes.
fn within<T: 'static>(
    receiver: async_channel::Receiver<Result<T, String>>,
    timeout: Duration,
    late: &str,
    cx: &mut Context<AppModel>,
) -> Task<Result<T, String>> {
    let (sender, answer) = async_channel::bounded(1);
    let forward = sender.clone();
    cx.spawn(async move |_, _| {
        let outcome = receiver
            .recv()
            .await
            .unwrap_or_else(|_| Err("the page closed before answering".into()));
        let _ = forward.try_send(outcome);
    })
    .detach();
    let late = late.to_owned();
    cx.spawn(async move |_, cx| {
        cx.background_executor().timer(timeout).await;
        let _ = sender.try_send(Err(late));
    })
    .detach();
    cx.spawn(async move |_, _| {
        answer
            .recv()
            .await
            .unwrap_or_else(|_| Err("Muxy is closing".into()))
    })
}

fn run_script(
    view: &Entity<Webview>,
    body: &str,
    timeout: Duration,
    cx: &mut Context<AppModel>,
) -> Reply {
    let (token, result) = view.update(cx, |view, _| view.run_page_script(body));
    let page = view.downgrade();
    cx.spawn(async move |_, cx| {
        cx.background_executor().timer(timeout).await;
        let _ = page.update(cx, |page, _| page.forget_page_script(&token));
    })
    .detach();
    within(result, timeout, "the page did not answer in time", cx)
}

fn wait_until(
    view: WeakEntity<Webview>,
    condition: String,
    timeout: Duration,
    cx: &mut Context<AppModel>,
) -> Reply {
    let deadline = Instant::now() + timeout;
    cx.spawn(async move |model, cx| {
        loop {
            let check = model
                .update(cx, |_, cx| {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    view.upgrade()
                        .map(|view| run_script(&view, &condition, remaining.max(POLL_INTERVAL), cx))
                })
                .map_err(|_| "Muxy is closing".to_owned())?
                .ok_or("the page closed")?;
            if check.await == Ok(Value::Bool(true)) {
                return Ok(true.into());
            }
            if Instant::now() >= deadline {
                return Ok(false.into());
            }
            cx.background_executor().timer(POLL_INTERVAL).await;
        }
    })
}

fn screenshot(view: &Entity<Webview>, cx: &mut Context<AppModel>) -> Reply {
    let picture = view.update(cx, |view, cx| {
        let picture = view.capture();
        cx.notify();
        picture
    });
    let picture = within(
        picture,
        CAPTURE_WAIT,
        "the page did not hand over a picture in time; show it first with muxy extension open",
        cx,
    );
    cx.spawn(async move |_, cx| {
        let picture: Picture = picture.await?;
        cx.background_executor()
            .spawn(async move {
                let png = png(picture)?;
                Ok(json!({ "png": muxy_core::base64::encode(&png) }))
            })
            .await
    })
}
