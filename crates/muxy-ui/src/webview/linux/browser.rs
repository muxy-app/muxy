use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use async_channel::Receiver;
use gpui::{Bounds, CursorStyle, Keystroke, Pixels, Window};
use polybrowser::{Cookie, Engine, Error, FindOptions, Image, Profile, Result};

use super::{Events, NEXT, NativeWebview, Shared, page, post};
use crate::browser::{Event, Options};
use crate::webview::Pointer;

pub(super) type Done<T> = Box<dyn FnOnce(Result<T>) + Send>;

#[derive(Debug)]
pub struct NativeBrowser {
    page: NativeWebview,
    visible: Cell<bool>,
    focused: Cell<bool>,
    clicked: Cell<Option<Instant>>,
    inspector: Cell<bool>,
    navigation: Cell<u64>,
}

impl NativeBrowser {
    pub const DRAWN_BY_APP: bool = true;

    pub fn new(
        window: &Window,
        options: &Options<'_>,
    ) -> std::result::Result<(Self, Receiver<Event>), String> {
        let (browser, events) = Self::open(options)?;
        let bounds = Bounds::new(
            gpui::point(gpui::px(0.0), gpui::px(0.0)),
            window.viewport_size(),
        );
        browser.sync(bounds, bounds, 0.0, window.scale_factor());
        Ok((browser, events))
    }

    pub(super) fn open(
        options: &Options<'_>,
    ) -> std::result::Result<(Self, Receiver<Event>), String> {
        super::ffi::load()?;
        polybrowser::webkitgtk::probe().map_err(|error| error.to_string())?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let shared = Arc::new(Shared::default());
        let (sender, events) = async_channel::bounded(256);
        let output = Events::Browser(sender, Arc::default());
        let setup = page::Setup {
            id,
            url: options.url.unwrap_or("about:blank").to_owned(),
            source: None,
            browser: Some(page::browser::Setup {
                profile: options.profile.clone(),
                allows: options.allows,
            }),
            script: String::new(),
            transparent: false,
            events: output.clone(),
            shared: shared.clone(),
        };
        post(move |runtime| match runtime {
            Ok(runtime) => runtime.open(setup),
            Err(reason) => setup.events.browser(polybrowser::BrowserEvent::LoadFailed {
                url: setup.url,
                error: reason.to_owned(),
            }),
        });
        Ok((
            Self {
                page: NativeWebview {
                    id,
                    events: output,
                    shared,
                    modal: Cell::new(false),
                    passthrough: Cell::new(false),
                    placed: Cell::new(None),
                    shown: Cell::new(None),
                },
                visible: Cell::new(false),
                focused: Cell::new(false),
                clicked: Cell::new(None),
                inspector: Cell::new(false),
                navigation: Cell::new(0),
            },
            events,
        ))
    }

    pub fn browser(&self) -> &Self {
        self
    }
    pub fn set_shortcuts(&self, modal: bool, shortcuts: Vec<Keystroke>) {
        self.page.set_shortcuts(modal, shortcuts);
    }
    pub fn set_focused(&self, focused: bool) {
        self.focused.set(focused);
    }
    pub fn set_grips(&self, grips: &[(Bounds<Pixels>, CursorStyle)]) {
        self.page.set_grips(grips);
    }
    pub fn set_mouse_passthrough(&self, enabled: bool) {
        self.page.set_mouse_passthrough(enabled);
    }
    pub fn sync(&self, bounds: Bounds<Pixels>, clip: Bounds<Pixels>, radius: f64, scale: f32) {
        self.page
            .sync(bounds, clip, gpui::Rgba::default(), radius, scale);
    }
    pub fn occlude(&self, occlusions: &[Bounds<Pixels>]) {
        self.page.occlude(occlusions);
    }
    #[allow(clippy::unused_self)]
    pub fn occlude_also(&self, _bounds: Bounds<Pixels>) {}
    pub fn clicked_within(&self, elapsed: Duration) -> bool {
        self.clicked
            .get()
            .is_some_and(|click| click.elapsed() <= elapsed)
    }
    pub fn is_visible(&self) -> bool {
        self.visible.get()
    }
    pub fn set_visible(&self, visible: bool) {
        self.visible.set(visible);
        self.page.set_visible(visible);
    }
    pub fn focus(&self) {
        self.page.focus();
    }
    pub fn blur(&self) {
        self.page.blur();
    }
    pub fn snapshot(&self) -> u64 {
        self.page.snapshot();
        self.page.shared.snapshot.load(Ordering::Acquire)
    }
    pub fn is_current_snapshot(&self, id: u64) -> bool {
        self.page.shared.snapshot.load(Ordering::Acquire) == id
    }
    pub fn accept_frame(&self, id: u64, generation: u64) -> bool {
        self.page.accept_snapshot(id, generation)
    }
    pub fn pointer(&self, event: Pointer) {
        if !self.visible.get() || self.page.passthrough.get() {
            return;
        }
        if matches!(event, Pointer::Pressed { .. }) {
            self.clicked.set(Some(Instant::now()));
            self.focus();
            if let Events::Browser(sender, _) = &self.page.events {
                let _ = sender.try_send(Event::Clicked);
            }
        }
        self.page.pointer(event);
    }
    pub fn key(&self, key: &Keystroke, pressed: bool, clipboard: impl FnOnce() -> Option<String>) {
        if self.visible.get() && self.focused.get() {
            self.page.key(key, pressed, clipboard);
        }
    }
    pub fn choose(&self, option: Option<usize>) {
        self.page.choose(option);
    }

    fn command(
        &self,
        job: impl FnOnce(&page::Runtime, &page::Page) + Send + 'static,
    ) -> Result<()> {
        if let super::Thread::Failed(reason) = &*super::THREAD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            return Err(error(reason));
        }
        self.page.with(move |runtime, page| job(runtime, page));
        Ok(())
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "Load failures are delivered as events"
    )]
    pub fn load_url(&self, url: &str) -> Result<()> {
        let url = url.to_owned();
        let navigation = self.navigation.get().wrapping_add(1);
        self.navigation.set(navigation);
        let events = self.page.events.clone();
        let id = self.page.id;
        post(move |runtime| {
            events.begin_navigation(navigation);
            let result = runtime.and_then(|runtime| {
                let page = runtime.page(id).ok_or("browser page is unavailable")?;
                page.browser_navigate(runtime, &url);
                Ok(())
            });
            if let Err(error) = result {
                events.browser(polybrowser::BrowserEvent::LoadFailed {
                    url,
                    error: error.to_owned(),
                });
            }
        });
        Ok(())
    }

    pub fn is_current_navigation(&self, navigation: u64) -> bool {
        navigation == self.navigation.get()
    }
    pub fn reload(&self) -> Result<()> {
        self.command(|runtime, page| page.browser_action(runtime, "reload"))
    }
    pub fn stop(&self) -> Result<()> {
        self.command(|runtime, page| page.browser_action(runtime, "stop"))
    }
    pub fn go_back(&self) -> Result<()> {
        self.command(|runtime, page| page.browser_action(runtime, "back"))
    }
    pub fn go_forward(&self) -> Result<()> {
        self.command(|runtime, page| page.browser_action(runtime, "forward"))
    }
    pub fn set_zoom(&self, zoom: f64) -> Result<()> {
        self.command(move |runtime, page| page.browser_zoom(runtime, zoom))
    }
    pub fn run_script(
        &self,
        body: &str,
        done: impl FnOnce(Result<String>) + Send + 'static,
    ) -> Result<()> {
        let body = body.to_owned();
        self.request(done, move |runtime, page, done| {
            page.browser_script(runtime, &body, done);
        })
    }
    pub fn capture(&self, done: impl FnOnce(Result<Image>) + Send + 'static) -> Result<()> {
        self.request(done, |runtime, page, done| {
            page.browser_snapshot(runtime, done);
        })
    }
    pub fn find(
        &self,
        query: &str,
        options: FindOptions,
        done: impl FnOnce(Result<bool>) + Send + 'static,
    ) -> Result<()> {
        let query = query.to_owned();
        self.request(done, move |runtime, page, done| {
            page.browser_find(runtime, &query, options, done);
        })
    }
    pub fn clear_find(&self) -> Result<()> {
        self.command(|runtime, page| page.browser_action(runtime, "clear-find"))
    }
    pub fn is_devtools_open(&self) -> bool {
        self.inspector.get()
    }
    pub fn open_devtools(&self) -> Result<()> {
        self.inspector.set(true);
        self.command(|runtime, page| page.browser_action(runtime, "inspect"))
    }
    pub fn close_devtools(&self) -> Result<()> {
        self.inspector.set(false);
        self.command(|runtime, page| page.browser_action(runtime, "close-inspector"))
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "Matches the synchronous browser backend API"
    )]
    fn request<T: 'static>(
        &self,
        done: impl FnOnce(Result<T>) + Send + 'static,
        job: impl FnOnce(&page::Runtime, &page::Page, Done<T>) + Send + 'static,
    ) -> Result<()> {
        let id = self.page.id;
        post(move |runtime| match runtime {
            Ok(runtime) => match runtime.page(id) {
                Some(page) => job(runtime, &page, Box::new(done)),
                None => done(Err(error("browser page closed"))),
            },
            Err(reason) => done(Err(error(reason))),
        });
        Ok(())
    }

    pub fn cookies(
        &self,
        url: Option<&str>,
        done: impl FnOnce(Result<Vec<Cookie>>) + Send + 'static,
    ) -> Result<()> {
        let url = url.map(str::to_owned);
        self.request(done, move |runtime, page, done| {
            page.browser_cookies(runtime, url.as_deref(), done);
        })
    }
    pub fn set_cookie(
        &self,
        cookie: Cookie,
        done: impl FnOnce(Result<()>) + Send + 'static,
    ) -> Result<()> {
        self.request(done, move |runtime, page, done| {
            page.browser_set_cookie(runtime, &cookie, done);
        })
    }
    pub fn delete_cookies(
        &self,
        name: &str,
        domain: Option<&str>,
        done: impl FnOnce(Result<usize>) + Send + 'static,
    ) -> Result<()> {
        let (name, domain) = (name.to_owned(), domain.map(str::to_owned));
        self.request(done, move |runtime, page, done| {
            page.browser_delete_cookies(runtime, &name, domain.as_deref(), done);
        })
    }
    pub fn clear_cookies(&self, done: impl FnOnce(Result<()>) + Send + 'static) -> Result<()> {
        self.request(done, |runtime, page, done| {
            page.browser_clear_cookies(runtime, done);
        })
    }
}

mod desktop;

pub fn installed_browsers() -> Vec<(String, PathBuf)> {
    desktop::installed_browsers()
}

pub fn clear_site_data(
    _engine: Engine,
    profile: &Profile,
    done: impl FnOnce(Result<()>) + Send + 'static,
) -> Result<()> {
    let profile = profile.clone();
    post(move |runtime| match runtime {
        Ok(runtime) => page::browser::clear_site_data(runtime, &profile, Box::new(done)),
        Err(reason) => done(Err(error(reason))),
    });
    Ok(())
}

pub fn import_cookies(
    _engine: Engine,
    profile: &Profile,
    cookies: Vec<Cookie>,
    done: impl FnOnce(Result<usize>) + Send + 'static,
) -> Result<()> {
    let profile = profile.clone();
    post(move |runtime| match runtime {
        Ok(runtime) => page::browser::import_cookies(runtime, &profile, cookies, Box::new(done)),
        Err(reason) => done(Err(error(reason))),
    });
    Ok(())
}

pub(super) fn error(message: &str) -> Error {
    Error::backend(Engine::WebKit, message)
}

pub fn close_profile(profile: &Profile, done: impl FnOnce(Result<()>) + Send + 'static) {
    let profile = profile.clone();
    post(move |runtime| match runtime {
        Ok(runtime) => runtime.close_profile(&profile, Box::new(done)),
        Err(_) => done(Ok(())),
    });
}
