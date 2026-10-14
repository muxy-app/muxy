//! `browser.*` requests from the CLI and extensions: opening and listing
//! tabs, and driving their pages.

use std::time::{Duration, Instant};

use gpui::{Context, Entity, Task, WeakEntity, Window};
use muxy_app_core::PaneId;
use muxy_app_core::browser::automation::{
    self, Args, DEFAULT_NAVIGATION_WAIT, DEFAULT_WAIT, MAX_WAIT, POLL_INTERVAL, flag,
    optional_string, string,
};
use muxy_app_core::browser::{is_blank, resolve_target, url_host};
use muxy_ui::browser::polybrowser::{Cookie, Timestamp};
use muxy_ui::javascript::is_async_function_body;
use serde_json::{Value, json};

use super::OpenBrowser;
use crate::model::AppModel;
use crate::views::browser::BrowserView;

type Reply = Task<Result<Value, String>>;

/// How long a navigation that a click just caused may take to start.
const NAVIGATION_START: Duration = Duration::from_millis(500);

impl AppModel {
    /// Carries out `browser.<verb>`.
    pub(crate) fn browser_call(
        &mut self,
        verb: &str,
        args: &Args,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Reply {
        let verb = verb.strip_prefix("browser.").unwrap_or(verb);
        if !self.browser_enabled() {
            return Task::ready(if verb == "list" {
                Ok(json!([]))
            } else {
                Err("the built-in browser is disabled".into())
            });
        }
        self.browser_request(verb, args, window, cx)
            .unwrap_or_else(|error| Task::ready(Err(error)))
    }

    fn browser_request(
        &mut self,
        verb: &str,
        args: &Args,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Reply, String> {
        let done = |value: Value| Ok(Task::ready(Ok(value)));
        match verb {
            "open" => done(self.browser_open(args, cx)?.to_string().into()),
            "list" => done(self.browser_list(args, cx)?),
            "navigate" => {
                let pane = self.browser_tab(args)?;
                let url = self
                    .settings
                    .browser
                    .resolve(string(args, "url")?)
                    .ok_or("invalid url")?;
                if let Some(view) = self.browser_view(pane) {
                    view.update(cx, |view, cx| view.load(&url, cx));
                } else {
                    self.state
                        .set_browser_url(pane, Some(url))
                        .map_err(|error| error.to_string())?;
                    self.save(cx);
                }
                done(Value::Null)
            }
            "close" => {
                let pane = self.browser_tab(args)?;
                self.close_pane(pane, cx);
                done(Value::Null)
            }
            "reload" | "back" | "forward" => {
                let view = self.browser_page(args, window, cx)?;
                view.update(cx, |view, cx| match verb {
                    "reload" => view.reload(cx),
                    "back" => view.go_back(cx),
                    _ => view.go_forward(cx),
                });
                done(Value::Null)
            }
            "screenshot" => {
                let view = self.browser_page(args, window, cx)?;
                Ok(screenshot(&view, cx))
            }
            "cookies.get" | "cookies.set" | "cookies.delete" | "cookies.clear" => {
                let view = self.browser_page(args, window, cx)?;
                cookies(verb, &view, args, cx)
            }
            "wait" | "waitFor" => {
                let condition = automation::condition(verb, args, is_async_function_body)?;
                let view = self.browser_page(args, window, cx)?;
                Ok(wait_until(
                    view.downgrade(),
                    condition,
                    automation::timeout(args, DEFAULT_WAIT),
                    cx,
                ))
            }
            "waitForNavigation" => {
                let view = self.browser_page(args, window, cx)?;
                Ok(wait_for_navigation(
                    view.downgrade(),
                    automation::timeout(args, DEFAULT_NAVIGATION_WAIT),
                    cx,
                ))
            }
            _ => {
                let script = automation::script(verb, args, is_async_function_body)?
                    .ok_or_else(|| format!("unknown command browser.{verb}"))?;
                let view = self.browser_page(args, window, cx)?;
                Ok(run_script(&view, &script, MAX_WAIT, cx))
            }
        }
    }

    fn browser_open(&mut self, args: &Args, cx: &mut Context<Self>) -> Result<PaneId, String> {
        let (project, worktree) = (
            optional_string(args, "project"),
            optional_string(args, "worktree"),
        );
        let target = resolve_target(&self.state, project, worktree)?;
        let url = match optional_string(args, "url") {
            Some(input) => Some(self.settings.browser.resolve(input).ok_or("invalid url")?),
            None => self.settings.browser.home_url(),
        };
        self.open_browser(
            OpenBrowser {
                url,
                profile: optional_string(args, "profile").map(Into::into),
                project: target,
                split: flag(args, "split"),
                select: project.is_none() && worktree.is_none(),
                focus_address: false,
            },
            cx,
        )
    }

    fn browser_list(&mut self, args: &Args, cx: &mut Context<Self>) -> Result<Value, String> {
        let (project, worktree) = (
            optional_string(args, "project"),
            optional_string(args, "worktree"),
        );
        let target = if project.is_some() || worktree.is_some() {
            Some(resolve_target(&self.state, project, worktree)?)
        } else {
            None
        };
        let panes: Vec<_> = self
            .browser_panes()
            .into_iter()
            .filter(|(project, ..)| target.is_none_or(|target| target == *project))
            .map(|(project, _, pane, browser)| (project, pane, browser.clone()))
            .collect();
        let mut tabs = Vec::new();
        for (project, pane, browser) in panes {
            let view = self.browser_view(pane).map(|view| view.read(cx));
            let url = view.map_or_else(
                || browser.url.clone().unwrap_or_default(),
                |view| view.page.url.clone(),
            );
            let title = self.browser_title(pane, cx).unwrap_or_default().to_owned();
            let profile = match view {
                Some(view) => view.profile.name().to_owned(),
                None => self
                    .pane_profile(browser.profile.as_deref())
                    .map(|profile| profile.name().to_owned())
                    .unwrap_or_default(),
            };
            tabs.push(json!({
                "tabId": pane.to_string(),
                "title": title,
                "url": if is_blank(&url) { String::new() } else { url },
                "profile": profile,
                "active": self.shown_pane_of(project) == Some(pane),
            }));
        }
        Ok(tabs.into())
    }

    fn browser_tab(&self, args: &Args) -> Result<PaneId, String> {
        let id = string(args, "tabId")?;
        self.state
            .browser_pane(id)
            .ok_or_else(|| format!("browser tab not found {id}"))
    }

    /// The live page of the tab in `args`, loaded hidden if it was never shown.
    fn browser_page(
        &mut self,
        args: &Args,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Entity<BrowserView>, String> {
        let pane = self.browser_tab(args)?;
        self.ensure_browser_view(pane, window, cx)
    }
}

/// Runs `body` as an async function in the page and parses what it
/// returned. A page that hasn't answered within `timeout` fails it.
fn run_script(
    view: &Entity<BrowserView>,
    body: &str,
    timeout: Duration,
    cx: &mut Context<AppModel>,
) -> Reply {
    let (sender, result) = async_channel::bounded::<Result<String, String>>(1);
    let answer = sender.clone();
    if let Err(error) = view
        .read(cx)
        .native
        .browser()
        .run_script(body, move |outcome| {
            let _ = answer.try_send(outcome.map_err(|error| error.to_string()));
        })
    {
        return Task::ready(Err(error.to_string()));
    }
    cx.spawn(async move |_, cx| {
        cx.background_executor().timer(timeout).await;
        let _ = sender.try_send(Err("the page did not answer in time".into()));
    })
    .detach();
    cx.spawn(async move |_, _| match result.recv().await {
        Ok(Ok(json)) => serde_json::from_str(&json).map_err(|error| error.to_string()),
        Ok(Err(error)) => Err(error),
        Err(_) => Err("the page closed before answering".into()),
    })
}

fn wait_until(
    view: WeakEntity<BrowserView>,
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
                .ok_or("browser tab closed")?;
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

/// Waits for the page to settle. A load that doesn't start soon counts as
/// none, so a click that navigates can be followed by this.
fn wait_for_navigation(
    view: WeakEntity<BrowserView>,
    timeout: Duration,
    cx: &mut Context<AppModel>,
) -> Reply {
    let started = Instant::now();
    let deadline = started + timeout;
    let loads = view.upgrade().map(|view| view.read(cx).page.loads);
    cx.spawn(async move |_, cx| {
        loop {
            let (loading, current_loads, url) = view
                .read_with(cx, |view, _| {
                    (view.page.loading, view.page.loads, view.page.url.clone())
                })
                .map_err(|_| "browser tab closed".to_owned())?;
            let waited_for_start =
                Some(current_loads) != loads || started.elapsed() >= NAVIGATION_START;
            if !loading && waited_for_start {
                return Ok(url.into());
            }
            if Instant::now() >= deadline {
                return Ok(Value::Null);
            }
            cx.background_executor().timer(POLL_INTERVAL).await;
        }
    })
}

fn screenshot(view: &Entity<BrowserView>, cx: &mut Context<AppModel>) -> Reply {
    let (sender, image) = async_channel::bounded(1);
    if let Err(error) = view.read(cx).native.browser().snapshot(move |outcome| {
        let _ = sender.try_send(outcome);
    }) {
        return Task::ready(Err(error.to_string()));
    }
    cx.spawn(async move |_, cx| {
        let image = image
            .recv()
            .await
            .map_err(|_| "the page closed before the screenshot".to_owned())?
            .map_err(|error| error.to_string())?;
        cx.background_executor()
            .spawn(async move {
                let pixels = image::RgbaImage::from_raw(image.width, image.height, image.rgba)
                    .ok_or("the screenshot has the wrong size")?;
                let mut png = std::io::Cursor::new(Vec::new());
                pixels
                    .write_to(&mut png, image::ImageFormat::Png)
                    .map_err(|error| error.to_string())?;
                Ok(json!({ "png": crate::model::extensions::base64(png.get_ref()) }))
            })
            .await
    })
}

fn cookies(
    verb: &str,
    view: &Entity<BrowserView>,
    args: &Args,
    cx: &mut Context<AppModel>,
) -> Result<Reply, String> {
    let (sender, outcome) = async_channel::bounded::<Result<Value, String>>(1);
    let browser = view.read(cx).native.browser();
    let started = match verb {
        "cookies.get" => browser.cookies(optional_string(args, "url"), move |cookies| {
            let _ = sender.try_send(
                cookies
                    .map(|cookies| cookies.iter().map(cookie_json).collect())
                    .map_err(|error| error.to_string()),
            );
        }),
        "cookies.set" => browser.set_cookie(cookie(args)?, move |result| {
            let _ = sender.try_send(
                result
                    .map(|()| Value::Null)
                    .map_err(|error| error.to_string()),
            );
        }),
        "cookies.delete" => browser.delete_cookies(
            string(args, "name")?,
            optional_string(args, "domain"),
            move |result| {
                let _ = sender.try_send(
                    result
                        .map(|_| Value::Null)
                        .map_err(|error| error.to_string()),
                );
            },
        ),
        _ => browser.clear_cookies(move |result| {
            let _ = sender.try_send(
                result
                    .map(|()| Value::Null)
                    .map_err(|error| error.to_string()),
            );
        }),
    };
    started.map_err(|error| error.to_string())?;
    Ok(cx.spawn(async move |_, _| {
        outcome
            .recv()
            .await
            .map_err(|_| "the page closed before answering".to_owned())?
    }))
}

fn cookie(args: &Args) -> Result<Cookie, String> {
    let domain = string(args, "domain")?.to_owned();
    if domain.is_empty() || url_host(&format!("https://{domain}")).is_none() {
        return Err("invalid cookie domain".into());
    }
    Ok(Cookie {
        name: string(args, "name")?.to_owned(),
        value: string(args, "value")?.to_owned(),
        domain,
        path: optional_string(args, "path").unwrap_or("/").to_owned(),
        secure: flag(args, "secure"),
        http_only: flag(args, "httpOnly"),
        expires: args
            .get("expires")
            .and_then(Value::as_u64)
            .map(|seconds| Timestamp::from(std::time::UNIX_EPOCH + Duration::from_secs(seconds))),
    })
}

fn cookie_json(cookie: &Cookie) -> Value {
    let expires = cookie.expires.and_then(|expires| {
        std::time::SystemTime::from(expires)
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|since| since.as_secs())
    });
    json!({
        "name": cookie.name,
        "value": cookie.value,
        "domain": cookie.domain,
        "path": cookie.path,
        "secure": cookie.secure,
        "httpOnly": cookie.http_only,
        "expires": expires,
    })
}

/// An extension's positional arguments, as `browser.<verb>` takes them.
pub(crate) fn extension_args(verb: &str, args: &Value) -> Args {
    let Some(list) = args.get("args").and_then(Value::as_array) else {
        return args.as_object().cloned().unwrap_or_default();
    };
    let mut named = Args::new();
    if let Some(tab) = args.get("tabId") {
        named.insert("tabId".into(), tab.clone());
    }
    let positions: &[&str] = match verb.strip_prefix("browser.").unwrap_or(verb) {
        "eval" => &["script"],
        "click" | "hover" | "scrollIntoView" | "getText" | "getValue" | "getCount" | "getHTML"
        | "snapshot" => &["selector"],
        "type" | "fill" => &["selector", "text", "options"],
        "press" => &["key", "selector"],
        "select" => &["selector", "value"],
        "setChecked" => &["selector", "checked"],
        "is" => &["property", "selector"],
        "waitFor" => &["selector", "options"],
        "wait" | "waitForNavigation" | "cookies.set" => &["options"],
        "getAttribute" => &["selector", "attribute"],
        "find" => &["kind", "value"],
        "storage.get" => &["key", "kind"],
        "storage.set" => &["key", "value", "kind"],
        "storage.clear" => &["kind"],
        "cookies.get" => &["url"],
        "cookies.delete" => &["name", "domain"],
        _ => &[],
    };
    for (name, value) in positions.iter().zip(list) {
        match (*name, value) {
            ("options", Value::Object(options)) => {
                named.extend(
                    options
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
            }
            ("options", _) => {}
            (name, value) if !value.is_null() => {
                named.insert(name.into(), value.clone());
            }
            _ => {}
        }
    }
    named
}
