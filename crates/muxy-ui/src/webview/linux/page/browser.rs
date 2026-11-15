use super::{
    Api, Callback, Cell, FALSE, HashMap, Page, Rc, RefCell, Runtime, TRUE, c_int, connect, cstring,
    ffi, gboolean, gpointer, guard, ptr, target, text, web_context,
};
use crate::webview::linux::browser::error;
use polybrowser::{BrowserEvent, Engine, FindOptions, Image, Profile, ProfileId, Result};
type Done<T> = Box<dyn FnOnce(Result<T>)>;

mod cookies;
pub(in crate::webview::linux) use cookies::{clear_site_data, import_cookies};

pub(in crate::webview::linux) struct Setup {
    pub(in crate::webview::linux) profile: Profile,
    pub(in crate::webview::linux) allows: fn(&str) -> bool,
}

pub(super) struct State {
    context: Rc<Context>,
    allows: fn(&str) -> bool,
    address: RefCell<String>,
    pending_url: RefCell<Option<String>>,
    pub(super) zoom: Cell<f64>,
    pub(super) native: RefCell<Option<polybrowser::webkitgtk::NativeView>>,
}

struct Context {
    native: gpointer,
    profile: Profile,
    pending: Cell<usize>,
    retiring: Cell<bool>,
    retired: RefCell<Option<Done<()>>>,
}

thread_local! {
    static CONTEXTS: RefCell<HashMap<ProfileId, std::rc::Weak<Context>>> = RefCell::default();
}

impl Context {
    fn get(runtime: &Runtime, profile: &Profile) -> Result<Rc<Self>> {
        CONTEXTS.with(|contexts| {
            let mut contexts = contexts.borrow_mut();
            contexts.retain(|_, context| context.strong_count() > 0);
            if let Some(context) = contexts.get(&profile.id()).and_then(std::rc::Weak::upgrade) {
                if context.retiring.get() {
                    return Err(error("browser profile is being deleted"));
                }
                return Ok(context);
            }
            let data = profile.engine_data_dir(Engine::WebKit);
            if let Some(data) = &data {
                std::fs::create_dir_all(data).map_err(|failure| error(&failure.to_string()))?;
            }
            let g = runtime.g;
            let native = unsafe {
                let context = match &data {
                    Some(data) => web_context(g, Some(data)),
                    None => g.webkit_web_context_new_ephemeral(),
                };
                g.webkit_web_context_set_sandbox_enabled(context, i32::from(runtime.sandboxed));
                if let Some(data) = data {
                    let cookies = cstring(&data.join("cookies.sqlite").to_string_lossy());
                    g.webkit_cookie_manager_set_persistent_storage(
                        g.webkit_web_context_get_cookie_manager(context),
                        cookies.as_ptr(),
                        1,
                    );
                }
                context
            };
            let context = Rc::new(Self {
                native,
                profile: profile.clone(),
                pending: Cell::new(0),
                retiring: Cell::new(false),
                retired: RefCell::default(),
            });
            contexts.insert(profile.id(), Rc::downgrade(&context));
            Ok(context)
        })
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        if let Some(g) = ffi::loaded() {
            unsafe {
                g.g_object_unref(self.native);
            }
        }
    }
}
impl State {
    pub(super) fn new(runtime: &Runtime, setup: &Setup) -> Result<Self> {
        Ok(Self {
            context: Context::get(runtime, &setup.profile)?,
            allows: setup.allows,
            address: RefCell::default(),
            pending_url: RefCell::default(),
            zoom: Cell::new(1.0),
            native: RefCell::default(),
        })
    }
    pub(super) fn cancel(&self, _g: &Api) {
        if let Some(native) = self.native.borrow().as_ref() {
            native.cancel();
        }
    }
    pub(super) fn context(&self) -> gpointer {
        self.context.native
    }

    fn accepts_load(&self, url: &str) -> bool {
        self.pending_url
            .borrow()
            .as_ref()
            .is_none_or(|pending| pending == url)
    }
}

impl Page {
    pub(in crate::webview::linux) fn install_browser(&self, runtime: &Runtime) -> Result<()> {
        let g = runtime.g;
        if let Some(browser) = &self.browser {
            match unsafe { polybrowser::webkitgtk::NativeView::from_raw(self.view) } {
                Ok(native) => {
                    browser.native.replace(Some(native));
                }
                Err(failure) => return Err(failure),
            }
        }
        unsafe {
            let script = g.webkit_user_script_new_for_world(
                cstring(&guard(&self.token)).as_ptr(),
                ffi::INJECT_TOP_FRAME,
                ffi::INJECT_AT_DOCUMENT_START,
                c"muxy-browser".as_ptr(),
                ptr::null(),
                ptr::null(),
            );
            g.webkit_user_content_manager_add_script(self.manager, script);
            g.webkit_user_script_unref(script);
            for signal in [
                c"notify::uri",
                c"notify::title",
                c"notify::estimated-load-progress",
            ] {
                connect(
                    g,
                    self.view,
                    signal,
                    callback!(on_changed(gpointer, gpointer, gpointer)),
                    usize::try_from(self.id).unwrap_or(0),
                );
            }
            connect(
                g,
                self.view,
                c"close",
                callback!(on_close(gpointer, gpointer)),
                usize::try_from(self.id).unwrap_or(0),
            );
            let settings = g.webkit_web_view_get_settings(self.view);
            g.webkit_settings_set_enable_developer_extras(settings, TRUE);
        }
        Ok(())
    }

    pub(in crate::webview::linux) fn browser_url(&self, g: &Api) -> String {
        unsafe { text(g.webkit_web_view_get_uri(self.view)).unwrap_or_default() }
    }

    fn sync_browser(&self, g: &Api) {
        let Some(browser) = &self.browser else {
            return;
        };
        let url = self.browser_url(g);
        if !browser.accepts_load(&url) {
            return;
        }
        if !url.is_empty() && *browser.address.borrow() != url {
            browser.address.replace(url.clone());
            self.events.browser(BrowserEvent::UrlChanged { url });
        }
        unsafe {
            self.events.browser(BrowserEvent::TitleChanged {
                title: text(g.webkit_web_view_get_title(self.view)).unwrap_or_default(),
            });
            self.events.browser(BrowserEvent::ProgressChanged {
                progress: g.webkit_web_view_get_estimated_load_progress(self.view),
            });
            self.events.browser(BrowserEvent::NavigationStateChanged {
                can_go_back: g.webkit_web_view_can_go_back(self.view) != FALSE,
                can_go_forward: g.webkit_web_view_can_go_forward(self.view) != FALSE,
            });
        }
    }

    pub(in crate::webview::linux) fn browser_load(&self, g: &Api, event: c_int) {
        let Some(browser) = &self.browser else {
            return;
        };
        let url = self.browser_url(g);
        if !browser.accepts_load(&url) {
            return;
        }
        if event == ffi::LOAD_STARTED {
            browser.pending_url.take();
            self.events.browser(BrowserEvent::LoadStarted { url });
        }
        self.sync_browser(g);
        if event == ffi::LOAD_FINISHED {
            let url = self.browser_url(g);
            let title = unsafe { text(g.webkit_web_view_get_title(self.view)) };
            if let Ok(history) = browser.context.profile.history() {
                let _ = history.record_visit(&url, title.as_deref());
            }
            self.events.browser(BrowserEvent::LoadFinished { url });
            let events = self.events.clone();
            self.browser_script_with(g, "const icon = [...document.querySelectorAll('link[rel~=icon]')].at(-1)?.href; return icon || new URL('/favicon.ico', location.href).href;", Box::new(move |result| {
                if let Ok(json) = result && let Ok(url) = serde_json::from_str::<String>(&json) {
                    events.browser(BrowserEvent::FaviconChanged { url });
                }
            }));
        }
    }

    pub(in crate::webview::linux) fn browser_policy(
        &self,
        runtime: &Runtime,
        decision: gpointer,
        kind: c_int,
    ) -> gboolean {
        let g = runtime.g;
        unsafe {
            let allowed = if kind == ffi::POLICY_NAVIGATION_ACTION || kind == 1 {
                let action = g.webkit_navigation_policy_decision_get_navigation_action(decision);
                let request = g.webkit_navigation_action_get_request(action);
                let url = text(g.webkit_uri_request_get_uri(request)).unwrap_or_default();
                let allowed = self
                    .browser
                    .as_ref()
                    .is_some_and(|browser| (browser.allows)(&url));
                if kind == 1 && allowed {
                    self.events
                        .browser(BrowserEvent::NewWindowRequested { url: url.clone() });
                }
                if !allowed && g.webkit_navigation_action_is_user_gesture(action) != FALSE {
                    self.events
                        .browser(BrowserEvent::ExternalLinkRequested { url });
                }
                kind == ffi::POLICY_NAVIGATION_ACTION && allowed
            } else {
                kind == ffi::POLICY_RESPONSE
                    && g.webkit_response_policy_decision_is_mime_type_supported(decision) != FALSE
            };
            if allowed {
                g.webkit_policy_decision_use(decision);
            } else {
                g.webkit_policy_decision_ignore(decision);
            }
        }
        TRUE
    }

    pub(in crate::webview::linux) fn browser_navigate(&self, runtime: &Runtime, url: &str) {
        if let Some(browser) = &self.browser
            && (browser.allows)(url)
        {
            let requested =
                gpui::http_client::Url::parse(url).map_or_else(|_| url.to_owned(), Into::into);
            let same_document = self.committed.get()
                && browser.pending_url.borrow().is_none()
                && requested.split('#').next() == self.browser_url(runtime.g).split('#').next();
            browser
                .pending_url
                .replace((!same_document).then_some(requested));
            unsafe {
                runtime
                    .g
                    .webkit_web_view_load_uri(self.view, cstring(url).as_ptr());
            }
        }
    }
    pub(in crate::webview::linux) fn browser_action(&self, runtime: &Runtime, action: &str) {
        let g = runtime.g;
        if matches!(action, "reload" | "stop" | "back" | "forward")
            && let Some(browser) = &self.browser
        {
            browser.pending_url.take();
        }
        unsafe {
            match action {
                "reload" => g.webkit_web_view_reload(self.view),
                "stop" => g.webkit_web_view_stop_loading(self.view),
                "back" => g.webkit_web_view_go_back(self.view),
                "forward" => g.webkit_web_view_go_forward(self.view),
                "inspect" => {
                    g.webkit_web_inspector_show(g.webkit_web_view_get_inspector(self.view));
                }
                "close-inspector" => {
                    g.webkit_web_inspector_close(g.webkit_web_view_get_inspector(self.view));
                }
                "clear-find" => self.evaluate(runtime, "window.getSelection()?.removeAllRanges()"),
                _ => (),
            }
        }
    }
    pub(in crate::webview::linux) fn browser_zoom(&self, runtime: &Runtime, zoom: f64) {
        if let Some(browser) = &self.browser {
            browser.zoom.set(zoom);
        }
        unsafe {
            runtime.g.webkit_web_view_set_zoom_level(
                self.view,
                self.scale.get() / self.unit.get() * zoom,
            );
        }
    }
    pub(in crate::webview::linux) fn browser_script(
        &self,
        runtime: &Runtime,
        body: &str,
        done: Done<String>,
    ) {
        self.browser_script_with(runtime.g, body, done);
    }
    fn browser_script_with(&self, _g: &Api, body: &str, done: Done<String>) {
        self.with_native(done, |native, done| native.run_script(body, done));
    }
    pub(in crate::webview::linux) fn browser_find(
        &self,
        _runtime: &Runtime,
        query: &str,
        options: FindOptions,
        done: Done<bool>,
    ) {
        self.with_native(done, |native, done| native.find(query, options, done));
    }
    pub(in crate::webview::linux) fn browser_snapshot(
        &self,
        _runtime: &Runtime,
        done: Done<Image>,
    ) {
        let id = self.id;
        self.with_native(
            Box::new(move |result| {
                if let Some(runtime) = super::runtime()
                    && let Some(page) = runtime.page(id)
                {
                    unsafe {
                        if page.visible.get() {
                            runtime.g.gtk_widget_show(page.view);
                        } else {
                            runtime.g.gtk_widget_hide(page.view);
                        }
                    }
                }
                done(result);
            }),
            polybrowser::webkitgtk::NativeView::snapshot,
        );
    }
    fn with_native<T: 'static>(
        &self,
        done: Done<T>,
        call: impl FnOnce(&polybrowser::webkitgtk::NativeView, Done<T>) -> Result<()>,
    ) {
        let Some(browser) = &self.browser else {
            done(Err(error("not a browser page")));
            return;
        };
        let native = browser.native.borrow();
        let Some(native) = native.as_ref() else {
            done(Err(error("browser engine unavailable")));
            return;
        };
        complete(done, |done| call(native, done));
    }
}
fn complete<T: 'static>(done: Done<T>, call: impl FnOnce(Done<T>) -> Result<()>) {
    let done = Rc::new(RefCell::new(Some(done)));
    let callback = done.clone();
    let result = call(Box::new(move |result| {
        if let Some(done) = callback.borrow_mut().take() {
            done(result);
        }
    }));
    if let Err(failure) = result
        && let Some(done) = done.borrow_mut().take()
    {
        done(Err(failure));
    }
}

unsafe extern "C" fn on_changed(_view: gpointer, _property: gpointer, data: gpointer) {
    if let Some((runtime, page)) = target(data) {
        page.sync_browser(runtime.g);
    }
}
unsafe extern "C" fn on_close(_view: gpointer, data: gpointer) {
    if let Some((_, page)) = target(data) {
        page.events.browser(BrowserEvent::CloseRequested);
    }
}

impl Runtime {
    pub(in crate::webview::linux) fn close_profile(&self, profile: &Profile, done: Done<()>) {
        let ids: Vec<_> = self
            .pages
            .borrow()
            .values()
            .filter(|page| {
                page.browser
                    .as_ref()
                    .is_some_and(|browser| browser.context.profile.id() == profile.id())
            })
            .map(|page| page.id)
            .collect();
        let context = match Context::get(self, profile) {
            Ok(context) => context,
            Err(failure) => {
                done(Err(failure));
                return;
            }
        };
        for id in ids {
            self.close(id);
        }
        context.retiring.set(true);
        context.retired.replace(Some(done));
        context.finish_retirement();
    }
}

impl Context {
    fn operation<T: 'static>(
        self: &Rc<Self>,
        done: Done<T>,
        call: impl FnOnce(Done<T>) -> Result<()>,
    ) {
        if self.retiring.get() {
            done(Err(error("browser profile is being deleted")));
            return;
        }
        if self.pending.get() >= 128 {
            done(Err(error("too many pending profile operations")));
            return;
        }
        self.pending.set(self.pending.get() + 1);
        let context = self.clone();
        complete(
            Box::new(move |result| {
                context.pending.set(context.pending.get() - 1);
                done(result);
                context.finish_retirement();
            }),
            call,
        );
    }
    fn finish_retirement(self: &Rc<Self>) {
        if self.pending.get() != 0 {
            return;
        }
        let Some(done) = self.retired.take() else {
            return;
        };
        let data = match unsafe { polybrowser::webkitgtk::SiteData::from_raw(self.native) } {
            Ok(data) => data,
            Err(failure) => {
                done(Err(failure));
                return;
            }
        };
        let context = self.clone();
        complete(
            Box::new(move |result| {
                drop(context);
                done(result);
            }),
            |done| data.clear(done),
        );
    }
}
