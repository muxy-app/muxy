//! A web page in a pane: a polybrowser view inside the same native host as
//! extension pages, so it follows its pane, hides under app content, and
//! shares the keyboard with Muxy's shortcuts.

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;

use async_channel::{Receiver, Sender};
use gpui::{Bounds, CursorStyle, Pixels, Window};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSView, NSWorkspace};
use objc2_foundation::{NSString, NSURL};
use objc2_web_kit::WKWebView;
use polybrowser::{Browser, NewWindowPolicy, Rect};
use raw_window_handle::{
    AppKitWindowHandle, HandleError, HasWindowHandle, RawWindowHandle, WindowHandle,
};

use super::{ENGINE, Event, Options};
use crate::webview::host::{Host, Signal};

pub struct NativeBrowser {
    browser: Browser,
    host: Host,
    sender: Sender<Event>,
    snapshot_id: Cell<u64>,
}

impl std::fmt::Debug for NativeBrowser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NativeBrowser")
            .field("browser", &self.browser)
            .finish_non_exhaustive()
    }
}

impl NativeBrowser {
    pub const DRAWN_BY_APP: bool = false;

    #[allow(clippy::unused_self)]
    pub fn is_current_navigation(&self, navigation: u64) -> bool {
        navigation == 0
    }
    pub fn capture(
        &self,
        done: impl FnOnce(polybrowser::Result<polybrowser::Image>) + 'static,
    ) -> polybrowser::Result<()> {
        self.browser.snapshot(done)
    }
    #[allow(clippy::unused_self)]
    pub fn accept_frame(&self, _id: u64, _generation: u64) -> bool {
        false
    }

    #[allow(clippy::unused_self)]
    pub fn pointer(&self, _event: crate::webview::Pointer) {}
    #[allow(clippy::unused_self)]
    pub fn key(
        &self,
        _key: &gpui::Keystroke,
        _pressed: bool,
        _clipboard: impl FnOnce() -> Option<String>,
    ) {
    }
    #[allow(clippy::unused_self)]
    pub fn choose(&self, _option: Option<usize>) {}

    /// A hidden page in `window`; it shows once synced and made visible.
    /// Until then it takes the window's size, so a page loaded for automation
    /// before its pane is ever shown can lay out and be pictured.
    pub fn new(window: &Window, options: &Options<'_>) -> Result<(Self, Receiver<Event>), String> {
        let mtm = MainThreadMarker::new().ok_or("browser must be created on the main thread")?;
        let parent = crate::appkit::gpui_view(window)?;
        let mut host = Host::new(parent.clone(), mtm);
        host.container().setFrame(parent.bounds());
        let (sender, receiver) = async_channel::unbounded();
        let events = sender.clone();
        let allows = options.allows;
        let mut builder = Browser::builder()
            .engine(ENGINE)
            .profile(options.profile)
            .devtools(true)
            .new_window_policy(NewWindowPolicy::Block)
            .on_navigation(allows)
            .on_event(move |event| {
                let _ = events.try_send(Event::Page {
                    navigation: 0,
                    event,
                });
            });
        if let Some(url) = options.url {
            builder = builder.url(url);
        }
        let browser = builder
            .build(&Container(host.container()), Rect::default())
            .map_err(|error| error.to_string())?;
        let page = host
            .container()
            .subviews()
            .lastObject()
            .and_then(|view| view.downcast::<WKWebView>().ok())
            .ok_or("the browser view is missing")?;
        let keys = sender.clone();
        host.attach(page, move |signal| {
            keys.try_send(match signal {
                Signal::Escape => Event::Escape,
                Signal::Shortcut(keystroke) => Event::Shortcut(keystroke),
                Signal::Clicked => Event::Clicked,
            })
            .is_ok()
        });
        Ok((
            Self {
                browser,
                host,
                sender,
                snapshot_id: Cell::new(0),
            },
            receiver,
        ))
    }

    /// Moves the page into `window`, keeping it loaded with its history, for
    /// a pane that is now drawn there. It shows again once synced and made
    /// visible in that window, and takes the keyboard back on
    /// [`Self::focus`].
    pub fn move_to_window(&self, window: &Window) -> Result<(), String> {
        self.host.move_to(crate::appkit::gpui_view(window)?);
        Ok(())
    }

    /// The page itself: navigation, scripts, cookies, find, and more.
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    pub fn set_shortcuts(&self, modal: bool, shortcuts: Rc<Vec<gpui::Keystroke>>) {
        self.host.set_shortcuts(modal, shortcuts);
    }

    pub fn set_focused(&self, focused: bool) {
        self.host.set_focused(focused);
    }

    pub fn set_grips(&self, grips: &[(Bounds<Pixels>, CursorStyle)]) {
        self.host.set_grips(grips);
    }

    pub fn set_mouse_passthrough(&self, enabled: bool) {
        self.host.set_mouse_passthrough(enabled);
    }

    pub fn sync(&self, bounds: Bounds<Pixels>, clip: Bounds<Pixels>, radius: f64, _scale: f32) {
        self.host.sync(bounds, clip, radius);
    }

    pub fn occlude(&self, occlusions: &[Bounds<Pixels>]) {
        self.host.occlude(occlusions);
    }

    /// Hides the page under `bounds` too, for app content drawn after the
    /// frame's occlusions were applied.
    pub fn occlude_also(&self, bounds: Bounds<Pixels>) {
        self.host.occlude_also(bounds);
    }

    /// Whether the user clicked into the page within `elapsed`, as opposed
    /// to the page acting on its own.
    pub fn clicked_within(&self, elapsed: std::time::Duration) -> bool {
        self.host.clicked_within(elapsed)
    }

    pub fn is_visible(&self) -> bool {
        self.host.is_visible()
    }

    pub fn set_visible(&self, visible: bool) {
        self.host.set_visible(visible);
    }

    pub fn focus(&self) {
        self.host.focus();
    }

    pub fn blur(&self) {
        self.host.blur();
    }

    /// Takes a picture of the page as shown, delivered as
    /// [`Event::Snapshot`] with the returned ID. A newer request replaces it.
    pub fn snapshot(&self) -> u64 {
        let id = self.snapshot_id.get() + 1;
        self.snapshot_id.set(id);
        let sender = self.sender.clone();
        self.host.snapshot(move |image| {
            let _ = sender.try_send(Event::Snapshot { id, image });
        });
        id
    }

    pub fn is_current_snapshot(&self, id: u64) -> bool {
        self.snapshot_id.get() == id
    }
}

impl Drop for NativeBrowser {
    fn drop(&mut self) {
        self.host.blur();
        self.sender.close();
    }
}

/// The host's container, as the window polybrowser builds its view in. It
/// outlives the handle, which only builds the view.
struct Container<'a>(&'a NSView);

impl HasWindowHandle for Container<'_> {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let view = NonNull::from(self.0).cast::<c_void>();
        let raw = RawWindowHandle::AppKit(AppKitWindowHandle::new(view));
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

/// Apps that open web pages, by name and location, as the system lists them.
pub fn installed_browsers() -> Vec<(String, std::path::PathBuf)> {
    let Some(page) = NSURL::URLWithString(&NSString::from_str("https://example.com")) else {
        return Vec::new();
    };
    let mut found: Vec<_> = NSWorkspace::sharedWorkspace()
        .URLsForApplicationsToOpenURL(&page)
        .iter()
        .filter_map(|app| {
            let path = std::path::PathBuf::from(app.path()?.to_string());
            let name = path.file_stem()?.to_string_lossy().into_owned();
            Some((name, path))
        })
        .collect();
    found.sort_by_key(|(name, _)| name.to_lowercase());
    found.dedup_by(|a, b| a.0 == b.0);
    found
}
