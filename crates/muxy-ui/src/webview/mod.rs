pub mod assets;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr;

use async_channel::{Receiver, Sender};
use block2::{DynBlock, RcBlock};
use gpui::{Bounds, CursorStyle, Pixels, Rgba, Window};
use muxy_core::worker::WorkerPool;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send,
};
use objc2_app_kit::{
    NSApplication, NSColor, NSCursor, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType,
    NSImage, NSMenu, NSView, NSWindowOrderingMode,
};
use objc2_core_graphics::CGMutablePath;
use objc2_foundation::{
    NSComparisonResult, NSData, NSDictionary, NSError, NSJSONReadingOptions, NSJSONSerialization,
    NSJSONWritingOptions, NSNumber, NSObject, NSObjectNSKeyValueCoding, NSObjectProtocol, NSPoint,
    NSRect, NSSize, NSString, NSURL, NSURLRequest, NSURLResponse,
};
use objc2_quartz_core::CAShapeLayer;
use objc2_web_kit::{
    WKContentWorld, WKNavigation, WKNavigationAction, WKNavigationActionPolicy,
    WKNavigationDelegate, WKScriptMessage, WKScriptMessageHandlerWithReply, WKUIDelegate,
    WKURLSchemeHandler, WKURLSchemeTask, WKUserContentController, WKUserScript,
    WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration, WKWindowFeatures,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::tr;
use assets::Source;

type Reply = RcBlock<dyn Fn(*mut AnyObject, *mut NSString)>;

/// Premultiplied BGRA pixels of a page at the display's scale.
pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Snapshot({}x{})", self.width, self.height)
    }
}
type AssetResult = Result<(Vec<u8>, &'static str), String>;

#[derive(Debug)]
pub enum Event {
    Message {
        id: u64,
        generation: u64,
        json: String,
    },
    Navigated,
    Loaded,
    Failed(String),
    Escape,
    Shortcut(gpui::Keystroke),
    Snapshot {
        id: u64,
        generation: u64,
        image: Snapshot,
    },
    Asset {
        id: u64,
        result: AssetResult,
    },
}

struct State {
    source: Source,
    sender: Sender<Event>,
    worker: WorkerPool,
    sequence: Cell<u64>,
    generation: Cell<u64>,
    snapshot_id: Cell<u64>,
    focused: Cell<bool>,
    clicked: Cell<bool>,
    modal: Cell<bool>,
    shortcuts: RefCell<Vec<gpui::Keystroke>>,
    replies: RefCell<HashMap<u64, Reply>>,
    assets: RefCell<HashMap<u64, Retained<ProtocolObject<dyn WKURLSchemeTask>>>>,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebviewState")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl State {
    fn next(&self) -> u64 {
        let id = self.sequence.get() + 1;
        self.sequence.set(id);
        id
    }
    fn cancel_replies(&self) {
        for (_, reply) in self.replies.take() {
            reject(&reply, "document retired");
        }
    }
}

fn reject(reply: &DynBlock<dyn Fn(*mut AnyObject, *mut NSString)>, error: &str) {
    let error = NSString::from_str(error);
    reply.call((ptr::null_mut(), Retained::as_ptr(&error).cast_mut()));
}

fn own_url(url: &NSURL, owner: &str) -> bool {
    url.scheme()
        .is_some_and(|scheme| scheme.to_string() == "muxy-ext")
        && url.host().is_some_and(|host| host.to_string() == owner)
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "MuxyWebviewDelegate"]
    #[ivars = State]
    #[derive(Debug)]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl WKScriptMessageHandlerWithReply for Delegate {
        #[unsafe(method(userContentController:didReceiveScriptMessage:replyHandler:))]
        fn message(
            &self,
            _: &WKUserContentController,
            message: &WKScriptMessage,
            reply: &DynBlock<dyn Fn(*mut AnyObject, *mut NSString)>,
        ) {
            let state = self.ivars();
            let frame = unsafe { message.frameInfo() };
            if !unsafe { frame.isMainFrame() }
                || !unsafe { frame.request() }
                    .URL()
                    .is_some_and(|url| own_url(&url, &state.source.owner))
            {
                reject(reply, "message is not from the registered main document");
                return;
            }
            let body = unsafe { message.body() };
            let data = unsafe {
                NSJSONSerialization::dataWithJSONObject_options_error(
                    &body,
                    NSJSONWritingOptions::FragmentsAllowed,
                )
            };
            let Ok(data) = data else {
                reject(reply, "invalid JSON message");
                return;
            };
            if data.len() > 1024 * 1024 || state.replies.borrow().len() >= 128 {
                reject(reply, "webview request limit exceeded");
                return;
            }
            let json = String::from_utf8_lossy(&data.to_vec()).into_owned();
            let id = state.next();
            state.replies.borrow_mut().insert(id, reply.copy());
            if state
                .sender
                .try_send(Event::Message {
                    id,
                    generation: state.generation.get(),
                    json,
                })
                .is_err()
            {
                state.replies.borrow_mut().remove(&id);
                reject(reply, "webview request queue full");
            }
        }
    }

    unsafe impl WKNavigationDelegate for Delegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn policy(
            &self,
            _: &WKWebView,
            action: &WKNavigationAction,
            completion: &DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            let allowed = unsafe { action.request() }.URL().is_some_and(|url| {
                own_url(&url, &self.ivars().source.owner)
                    || url
                        .scheme()
                        .is_some_and(|scheme| scheme.to_string() == "about")
            });
            completion.call((if allowed {
                WKNavigationActionPolicy::Allow
            } else {
                WKNavigationActionPolicy::Cancel
            },));
        }
        #[unsafe(method(webView:didCommitNavigation:))]
        fn committed(&self, _: &WKWebView, _: Option<&WKNavigation>) {
            self.ivars().cancel_replies();
            self.ivars()
                .generation
                .set(self.ivars().generation.get() + 1);
            let _ = self.ivars().sender.try_send(Event::Navigated);
        }
        #[unsafe(method(webView:didFinishNavigation:))]
        fn loaded(&self, _: &WKWebView, _: Option<&WKNavigation>) {
            let _ = self.ivars().sender.try_send(Event::Loaded);
        }
        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn failed(&self, _: &WKWebView, _: Option<&WKNavigation>, error: &NSError) {
            let _ = self
                .ivars()
                .sender
                .try_send(Event::Failed(error.localizedDescription().to_string()));
        }
        #[unsafe(method(webViewWebContentProcessDidTerminate:))]
        fn terminated(&self, _: &WKWebView) {
            self.ivars().cancel_replies();
            let _ = self.ivars().sender.try_send(Event::Failed(
                tr!("Web content process terminated").to_string(),
            ));
        }
    }

    unsafe impl WKUIDelegate for Delegate {
        #[unsafe(method_id(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
        fn popup(
            &self,
            _: &WKWebView,
            _: &WKWebViewConfiguration,
            _: &WKNavigationAction,
            _: &WKWindowFeatures,
        ) -> Option<Retained<WKWebView>> {
            None
        }
    }

    unsafe impl WKURLSchemeHandler for Delegate {
        #[unsafe(method(webView:startURLSchemeTask:))]
        fn start(&self, _: &WKWebView, task: &ProtocolObject<dyn WKURLSchemeTask>) {
            let state = self.ivars();
            let Some(url) = (unsafe { task.request() })
                .URL()
                .filter(|url| own_url(url, &state.source.owner))
            else {
                fail_asset(task);
                return;
            };
            let Some(path) = url.path() else {
                fail_asset(task);
                return;
            };
            if state.assets.borrow().len() >= 32 {
                fail_asset(task);
                return;
            }
            let id = state.next();
            state.assets.borrow_mut().insert(id, Retained::from(task));
            let root = state.source.directory.clone();
            let relative = path.to_string();
            let sender = state.sender.clone();
            if state
                .worker
                .try_spawn(move || {
                    let result = assets::read(&root, &relative).map_err(|error| error.to_string());
                    let _ = sender.send_blocking(Event::Asset { id, result });
                })
                .is_err()
            {
                state.assets.borrow_mut().remove(&id);
                fail_asset(task);
            }
        }
        #[unsafe(method(webView:stopURLSchemeTask:))]
        fn stop(&self, _: &WKWebView, task: &ProtocolObject<dyn WKURLSchemeTask>) {
            self.ivars()
                .assets
                .borrow_mut()
                .retain(|_, current| !ptr::eq(&raw const **current, task));
        }
    }
);

fn fail_asset(task: &ProtocolObject<dyn WKURLSchemeTask>) {
    let error = unsafe {
        NSError::errorWithDomain_code_userInfo(&NSString::from_str("MuxyWebviewAsset"), 1, None)
    };
    unsafe {
        task.didFailWithError(&error);
    }
}

#[derive(Debug, Default)]
struct HitTestRegions {
    occluded: RefCell<Vec<NSRect>>,
    passthrough_all: Cell<bool>,
}

impl HitTestRegions {
    fn excludes(&self, point: NSPoint) -> bool {
        self.passthrough_all.get()
            || self
                .occluded
                .borrow()
                .iter()
                .any(|rect| contains_point(*rect, point))
    }
}

#[allow(
    deprecated,
    reason = "GPUI shows these same cursors; their replacements need macOS 15"
)]
fn grip_cursor(style: CursorStyle) -> Option<Retained<NSCursor>> {
    match style {
        CursorStyle::ResizeLeftRight | CursorStyle::ResizeColumn => {
            Some(NSCursor::resizeLeftRightCursor())
        }
        CursorStyle::ResizeUpDown | CursorStyle::ResizeRow => Some(NSCursor::resizeUpDownCursor()),
        _ => None,
    }
}

fn contains_point(rect: NSRect, point: NSPoint) -> bool {
    point.x >= rect.origin.x
        && point.y >= rect.origin.y
        && point.x < rect.origin.x + rect.size.width
        && point.y < rect.origin.y + rect.size.height
}

define_class!(
    #[unsafe(super(WKWebView))]
    #[thread_kind = MainThreadOnly]
    #[name = "MuxyMaskedWebview"]
    #[ivars = HitTestRegions]
    #[derive(Debug)]
    struct MaskedWebview;
    unsafe impl NSObjectProtocol for MaskedWebview {}
    impl MaskedWebview {
        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> Option<Retained<NSView>> {
            let local = self.convertPoint_fromView(point, unsafe { self.superview() }.as_deref());
            if self.ivars().excludes(local) { None }
            else { unsafe { msg_send![super(self), hitTest: point] } }
        }
    }
);

define_class!(
    /// Covers the resize grips drawn over a page, in the parent's flipped
    /// coordinates. It takes the grips' clicks and hands them to the parent,
    /// and its cursor rects keep WebKit from changing the cursor there.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "MuxyWebviewGrips"]
    #[ivars = RefCell<Vec<(NSRect, CursorStyle)>>]
    #[derive(Debug)]
    struct GripOverlay;
    unsafe impl NSObjectProtocol for GripOverlay {}
    impl GripOverlay {
        #[unsafe(method(isFlipped))]
        fn flipped(&self) -> bool { true }

        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> Option<Retained<NSView>> {
            let local = self.convertPoint_fromView(point, unsafe { self.superview() }.as_deref());
            self.ivars()
                .borrow()
                .iter()
                .any(|(grip, _)| contains_point(*grip, local))
                .then(|| Retained::into_super(self.retain()))
        }

        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            for (grip, style) in self.ivars().borrow().iter() {
                if let Some(cursor) = grip_cursor(*style) {
                    self.addCursorRect_cursor(*grip, &cursor);
                }
            }
        }

        // A view that leaves a click unhandled passes it to the page below,
        // so each one is handed to the parent explicitly.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) { self.forward(|parent| parent.mouseDown(event)); }
        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) { self.forward(|parent| parent.mouseDragged(event)); }
        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) { self.forward(|parent| parent.mouseUp(event)); }
        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) { self.forward(|parent| parent.rightMouseDown(event)); }
        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) { self.forward(|parent| parent.rightMouseDragged(event)); }
        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) { self.forward(|parent| parent.rightMouseUp(event)); }
        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) { self.forward(|parent| parent.otherMouseDown(event)); }
        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) { self.forward(|parent| parent.otherMouseDragged(event)); }
        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) { self.forward(|parent| parent.otherMouseUp(event)); }
        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) { self.forward(|parent| parent.scrollWheel(event)); }
    }
);

impl GripOverlay {
    fn forward(&self, deliver: impl FnOnce(&NSView)) {
        if let Some(parent) = unsafe { self.superview() } {
            deliver(&parent);
        }
    }
}

#[derive(Debug)]
pub struct NativeWebview {
    view: Retained<MaskedWebview>,
    grips: Retained<GripOverlay>,
    parent: Retained<NSView>,
    delegate: Retained<Delegate>,
    monitor: Option<Retained<AnyObject>>,
    visible: Cell<bool>,
    bounds: Cell<Bounds<Pixels>>,
    clip: Cell<Bounds<Pixels>>,
    occlusions: RefCell<Vec<Bounds<Pixels>>>,
    applied: RefCell<Applied>,
}

/// Native view state as last applied. Every frame syncs the view, and each
/// new layer mask costs a view-sized bitmap, so only changes are applied.
#[derive(Debug, Default)]
struct Applied {
    frame: Option<NSRect>,
    background: Option<Rgba>,
    radius: Option<f64>,
    mask: Option<Mask>,
}

#[derive(Debug, PartialEq)]
struct Mask {
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    occlusions: Vec<Bounds<Pixels>>,
}

impl NativeWebview {
    pub fn new(
        window: &Window,
        source: Source,
        script: &str,
        background: Rgba,
        transparent_background: bool,
    ) -> Result<(Self, Receiver<Event>), String> {
        let url = source.entry_url().map_err(|error| error.to_string())?;
        let mtm = MainThreadMarker::new().ok_or("webview must be created on the main thread")?;
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window)
            .map_err(|error| error.to_string())?
            .as_raw()
        else {
            return Err("expected AppKit window".into());
        };
        let parent = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
            .ok_or("missing native view")?;
        let (sender, receiver) = async_channel::bounded(256);
        let delegate = Delegate::alloc(mtm).set_ivars(State {
            source,
            sender: sender.clone(),
            worker: WorkerPool::new("webview-assets", 2, 32).map_err(|error| error.to_string())?,
            sequence: Cell::new(0),
            generation: Cell::new(0),
            snapshot_id: Cell::new(0),
            focused: Cell::new(false),
            clicked: Cell::new(false),
            modal: Cell::new(false),
            shortcuts: RefCell::default(),
            replies: RefCell::default(),
            assets: RefCell::default(),
        });
        let delegate: Retained<Delegate> = unsafe { msg_send![super(delegate), init] };
        let configuration = unsafe { WKWebViewConfiguration::new(mtm) };
        let controller = unsafe { configuration.userContentController() };
        unsafe {
            configuration.setURLSchemeHandler_forURLScheme(
                Some(ProtocolObject::from_ref(&*delegate)),
                &NSString::from_str("muxy-ext"),
            );
            controller.addScriptMessageHandlerWithReply_contentWorld_name(
                ProtocolObject::from_ref(&*delegate),
                &WKContentWorld::pageWorld(mtm),
                &NSString::from_str("muxy"),
            );
        }
        install_script(&controller, script, mtm);
        let allocated = MaskedWebview::alloc(mtm).set_ivars(HitTestRegions::default());
        let view: Retained<MaskedWebview> = unsafe {
            msg_send![super(allocated), initWithFrame: NSRect::ZERO, configuration: &*configuration]
        };
        unsafe {
            view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            view.setUIDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        }
        view.setWantsLayer(true);
        view.setHidden(true);
        unsafe {
            view.setUnderPageBackgroundColor(Some(&background_color(background)));
            if transparent_background {
                view.setValue_forKey(
                    Some(&NSNumber::new_bool(false)),
                    &NSString::from_str("drawsBackground"),
                );
            }
        }
        parent.addSubview(&view);
        let allocated = GripOverlay::alloc(mtm).set_ivars(RefCell::default());
        let grips: Retained<GripOverlay> =
            unsafe { msg_send![super(allocated), initWithFrame: parent.bounds()] };
        grips.setHidden(true);
        parent.addSubview_positioned_relativeTo(&grips, NSWindowOrderingMode::Above, Some(&view));
        let monitor = monitor(&view, &delegate, sender);
        let url = NSURL::URLWithString(&NSString::from_str(&url)).ok_or("invalid entry URL")?;
        unsafe {
            view.loadRequest(&NSURLRequest::requestWithURL(&url));
        }
        Ok((
            Self {
                view,
                grips,
                parent,
                delegate,
                monitor,
                visible: Cell::new(false),
                bounds: Cell::default(),
                clip: Cell::default(),
                occlusions: RefCell::default(),
                applied: RefCell::default(),
            },
            receiver,
        ))
    }

    pub fn set_shortcuts(&self, modal: bool, shortcuts: Vec<gpui::Keystroke>) {
        self.delegate.ivars().modal.set(modal);
        *self.delegate.ivars().shortcuts.borrow_mut() = shortcuts;
    }

    /// Whether Muxy shows this page focused. Keys reach the page only while it
    /// is, or after a click into it until Muxy catches up. A page that takes
    /// the native focus itself, as when its script focuses an element, doesn't
    /// get them.
    pub fn set_focused(&self, focused: bool) {
        let state = self.delegate.ivars();
        state.focused.set(focused);
        if focused {
            state.clicked.set(false);
        }
    }

    /// Sends clicks on the resize grips drawn over the page, in window
    /// coordinates, to Muxy and shows each grip's cursor there. The page's
    /// pixels don't change. Call after [`Self::occlude`].
    pub fn set_grips(&self, grips: &[(Bounds<Pixels>, CursorStyle)]) {
        let frame = self.parent.bounds();
        let regions: Vec<_> = visible_grips(self.bounds.get(), &self.occlusions.borrow(), grips)
            .into_iter()
            .map(|(grip, cursor)| (flipped_rect(grip), cursor))
            .collect();
        let moved = self.grips.frame() != frame;
        if moved {
            self.grips.setFrame(frame);
        }
        if moved || *self.grips.ivars().borrow() != regions {
            *self.grips.ivars().borrow_mut() = regions;
            if let Some(window) = self.grips.window() {
                window.invalidateCursorRectsForView(&self.grips);
            }
        }
    }

    pub fn set_mouse_passthrough(&self, enabled: bool) {
        self.view.ivars().passthrough_all.set(enabled);
    }

    pub fn generation(&self) -> u64 {
        self.delegate.ivars().generation.get()
    }

    pub fn install_script(&self, script: &str) {
        let controller = unsafe { self.view.configuration().userContentController() };
        install_script(&controller, script, self.view.mtm());
    }

    pub fn evaluate(&self, script: &str) {
        unsafe {
            self.view
                .evaluateJavaScript_completionHandler(&NSString::from_str(script), None);
        }
    }

    pub fn reply(&self, id: u64, generation: u64, json: &str) {
        if self.generation() != generation {
            return;
        }
        let Some(reply) = self.delegate.ivars().replies.borrow_mut().remove(&id) else {
            return;
        };
        let data = NSData::with_bytes(json.as_bytes());
        match NSJSONSerialization::JSONObjectWithData_options_error(
            &data,
            NSJSONReadingOptions::FragmentsAllowed,
        ) {
            Ok(value) => reply.call((Retained::as_ptr(&value).cast_mut(), ptr::null_mut())),
            Err(_) => reject(&reply, "invalid host response"),
        }
    }

    pub fn complete_asset(&self, id: u64, result: AssetResult) {
        let Some(task) = self.delegate.ivars().assets.borrow_mut().remove(&id) else {
            return;
        };
        let Ok((bytes, mime)) = result else {
            fail_asset(&task);
            return;
        };
        let Some(url) = (unsafe { task.request() }).URL() else {
            fail_asset(&task);
            return;
        };
        let Ok(length) = isize::try_from(bytes.len()) else {
            fail_asset(&task);
            return;
        };
        let Some(response) = asset_response(&url, mime, length) else {
            fail_asset(&task);
            return;
        };
        unsafe {
            task.didReceiveResponse(&response);
            task.didReceiveData(&NSData::with_bytes(&bytes));
            task.didFinish();
        }
    }

    pub fn sync(
        &self,
        bounds: Bounds<Pixels>,
        clip: Bounds<Pixels>,
        background: Rgba,
        radius: f64,
    ) {
        self.bounds.set(bounds);
        self.clip.set(clip);
        let top = f64::from(f32::from(bounds.top()));
        let height = f64::from(f32::from(bounds.size.height));
        let y = if self.parent.isFlipped() {
            top
        } else {
            self.parent.bounds().size.height - top - height
        };
        let frame = NSRect::new(
            NSPoint::new(f64::from(f32::from(bounds.left())), y),
            NSSize::new(f64::from(f32::from(bounds.size.width)), height),
        );
        let mut applied = self.applied.borrow_mut();
        if applied.frame != Some(frame) {
            self.view.setFrame(frame);
            applied.frame = Some(frame);
        }
        if applied.background != Some(background) {
            unsafe {
                self.view
                    .setUnderPageBackgroundColor(Some(&background_color(background)));
            }
            applied.background = Some(background);
        }
        if applied.radius != Some(radius)
            && let Some(layer) = self.view.layer()
        {
            layer.setCornerRadius(radius);
            layer.setMasksToBounds(true);
            applied.radius = Some(radius);
        }
        drop(applied);
        self.apply_mask();
    }

    pub fn occlude(&self, occlusions: &[Bounds<Pixels>]) {
        if *self.occlusions.borrow() != occlusions {
            *self.occlusions.borrow_mut() = occlusions.to_vec();
        }
        self.apply_mask();
    }

    fn apply_mask(&self) {
        let bounds = self.bounds.get();
        let clip = bounds.intersect(&self.clip.get());
        let occlusions = self.occlusions.borrow();
        let mask = Mask {
            bounds,
            clip,
            occlusions: occlusions.clone(),
        };
        if self.applied.borrow().mask.as_ref() == Some(&mask) {
            return;
        }
        self.applied.borrow_mut().mask = Some(mask);
        let (regions, excluded) = composition_regions(bounds, clip, &occlusions);
        *self.view.ivars().occluded.borrow_mut() = excluded
            .into_iter()
            .map(|region| self.local_rect(region))
            .collect();
        if let Some(layer) = self.view.layer() {
            if occlusions.is_empty() && clip == bounds {
                unsafe {
                    layer.setMask(None);
                }
                return;
            }
            let path = CGMutablePath::new();
            for region in regions {
                unsafe {
                    CGMutablePath::add_rect(Some(&path), ptr::null(), self.local_rect(region));
                }
            }
            let mask = CAShapeLayer::new();
            mask.setPath(Some(&path));
            unsafe {
                layer.setMask(Some(&mask));
            }
        }
    }

    fn local_rect(&self, region: Bounds<Pixels>) -> NSRect {
        let bounds = self.bounds.get();
        let height = f64::from(f32::from(region.size.height));
        let top = f64::from(f32::from(region.top() - bounds.top()));
        NSRect::new(
            NSPoint::new(
                f64::from(f32::from(region.left() - bounds.left())),
                if self.view.isFlipped() {
                    top
                } else {
                    f64::from(f32::from(bounds.size.height)) - top - height
                },
            ),
            NSSize::new(f64::from(f32::from(region.size.width)), height),
        )
    }

    pub fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) == visible && self.view.isHidden() != visible {
            return;
        }
        // Hiding the focused page would leave the keyboard with the window.
        if !visible {
            self.blur();
        }
        self.view.setHidden(!visible);
        self.grips.setHidden(!visible);
    }

    pub fn focus(&self) {
        if self.visible.get()
            && let Some(window) = self.view.window()
        {
            window.makeFirstResponder(Some(&self.view));
        }
    }

    pub fn blur(&self) {
        self.delegate.ivars().clicked.set(false);
        if let Some(window) = self.view.window()
            && window.firstResponder().is_some_and(|responder| {
                responder
                    .downcast_ref::<NSView>()
                    .is_some_and(|view| view.isDescendantOf(&self.view))
            })
        {
            window.makeFirstResponder(Some(&self.parent));
        }
    }

    pub fn snapshot(&self) {
        let scale = self
            .view
            .window()
            .map_or(1.0, |window| window.backingScaleFactor());
        let bounds = self.bounds.get();
        let (Some(width), Some(height)) = (
            pixel_dimension(bounds.size.width, scale),
            pixel_dimension(bounds.size.height, scale),
        ) else {
            return;
        };
        let delegate = self.delegate.clone();
        let state = delegate.ivars();
        let id = state.next();
        let generation = state.generation.get();
        state.snapshot_id.set(id);
        let completion = RcBlock::new(move |image: *mut NSImage, _: *mut NSError| {
            let state = delegate.ivars();
            if state.snapshot_id.get() != id || state.generation.get() != generation {
                return;
            }
            let Some(bgra) =
                unsafe { image.as_ref() }.and_then(|image| snapshot_rows(image, width, height))
            else {
                return;
            };
            let _ = state.sender.try_send(Event::Snapshot {
                id,
                generation,
                image: Snapshot {
                    width,
                    height,
                    bgra,
                },
            });
        });
        unsafe {
            self.view
                .takeSnapshotWithConfiguration_completionHandler(None, &completion);
        }
    }

    pub fn is_current_snapshot(&self, id: u64, generation: u64) -> bool {
        let state = self.delegate.ivars();
        state.snapshot_id.get() == id && state.generation.get() == generation
    }
}

fn snapshot_rows(image: &NSImage, width: u32, height: u32) -> Option<Vec<u8>> {
    crate::bitmap::render_bgra(image, width, height)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pixel_dimension(points: Pixels, scale: f64) -> Option<u32> {
    let pixels = f64::from(f32::from(points)) * scale;
    (pixels.is_finite() && (1.0..=16384.0).contains(&pixels)).then(|| pixels.round() as u32)
}

fn install_script(controller: &WKUserContentController, source: &str, mtm: MainThreadMarker) {
    unsafe {
        controller.removeAllUserScripts();
        controller.addUserScript(
            &WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
                WKUserScript::alloc(mtm),
                &NSString::from_str(source),
                WKUserScriptInjectionTime::AtDocumentStart,
                true,
            ),
        );
    }
}

impl Drop for NativeWebview {
    fn drop(&mut self) {
        self.blur();
        if let Some(monitor) = self.monitor.take() {
            unsafe {
                NSEvent::removeMonitor(&monitor);
            }
        }
        self.delegate.ivars().cancel_replies();
        self.delegate.ivars().sender.close();
        unsafe {
            self.view.stopLoading();
            self.view.setNavigationDelegate(None);
            self.view.setUIDelegate(None);
            self.view
                .configuration()
                .userContentController()
                .removeAllScriptMessageHandlers();
            self.view
                .configuration()
                .userContentController()
                .removeAllUserScripts();
        }
        self.delegate.ivars().assets.borrow_mut().clear();
        self.grips.removeFromSuperview();
        self.view.removeFromSuperview();
    }
}

/// The parts of the grips over a page at `bounds` that nothing covers.
fn visible_grips(
    bounds: Bounds<Pixels>,
    occlusions: &[Bounds<Pixels>],
    grips: &[(Bounds<Pixels>, CursorStyle)],
) -> Vec<(Bounds<Pixels>, CursorStyle)> {
    grips
        .iter()
        .filter(|(grip, _)| grip.intersects(&bounds))
        .flat_map(|(grip, cursor)| {
            occlusions
                .iter()
                .fold(vec![*grip], |regions, occlusion| {
                    regions
                        .into_iter()
                        .flat_map(|region| subtract(region, *occlusion))
                        .collect()
                })
                .into_iter()
                .map(|region| (region, *cursor))
        })
        .collect()
}

/// `bounds` in a flipped view that fills the parent.
fn flipped_rect(bounds: Bounds<Pixels>) -> NSRect {
    NSRect::new(
        NSPoint::new(
            f64::from(f32::from(bounds.left())),
            f64::from(f32::from(bounds.top())),
        ),
        NSSize::new(
            f64::from(f32::from(bounds.size.width)),
            f64::from(f32::from(bounds.size.height)),
        ),
    )
}

fn composition_regions(
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    occlusions: &[Bounds<Pixels>],
) -> (Vec<Bounds<Pixels>>, Vec<Bounds<Pixels>>) {
    let clip = bounds.intersect(&clip);
    let mut regions = if clip.size.width > gpui::px(0.0) && clip.size.height > gpui::px(0.0) {
        vec![clip]
    } else {
        Vec::new()
    };
    let mut excluded = subtract(bounds, clip);
    for occlusion in occlusions {
        let clipped = bounds.intersect(occlusion);
        if clipped.size.width <= gpui::px(0.0) || clipped.size.height <= gpui::px(0.0) {
            continue;
        }
        excluded.push(clipped);
        regions = regions
            .into_iter()
            .flat_map(|region| subtract(region, *occlusion))
            .collect();
    }
    (regions, excluded)
}

fn subtract(region: Bounds<Pixels>, cut: Bounds<Pixels>) -> Vec<Bounds<Pixels>> {
    use gpui::{point, px, size};
    let cut = region.intersect(&cut);
    if cut.size.width <= px(0.0) || cut.size.height <= px(0.0) {
        return vec![region];
    }
    [
        Bounds::new(
            region.origin,
            size(region.size.width, cut.top() - region.top()),
        ),
        Bounds::new(
            point(region.left(), cut.bottom()),
            size(region.size.width, region.bottom() - cut.bottom()),
        ),
        Bounds::new(
            point(region.left(), cut.top()),
            size(cut.left() - region.left(), cut.size.height),
        ),
        Bounds::new(
            point(cut.right(), cut.top()),
            size(region.right() - cut.right(), cut.size.height),
        ),
    ]
    .into_iter()
    .filter(|rect| rect.size.width > px(0.0) && rect.size.height > px(0.0))
    .collect()
}

fn keystroke(event: &NSEvent) -> gpui::Keystroke {
    native_keystroke(
        event.keyCode(),
        &event
            .charactersIgnoringModifiers()
            .map_or_else(String::new, |text| text.to_string()),
        event.modifierFlags(),
    )
}

fn native_keystroke(code: u16, characters: &str, flags: NSEventModifierFlags) -> gpui::Keystroke {
    let mut shift = flags.contains(NSEventModifierFlags::Shift);
    let key = match code {
        36 | 76 => "enter".into(),
        48 => "tab".into(),
        49 => "space".into(),
        51 => "backspace".into(),
        53 => "escape".into(),
        114 => "insert".into(),
        115 => "home".into(),
        116 => "pageup".into(),
        117 => "delete".into(),
        119 => "end".into(),
        121 => "pagedown".into(),
        123 => "left".into(),
        124 => "right".into(),
        125 => "down".into(),
        126 => "up".into(),
        _ => match characters.chars().next() {
            Some(character) if (0xf704..=0xf726).contains(&u32::from(character)) => {
                format!("f{}", u32::from(character) - 0xf704 + 1)
            }
            _ => {
                if shift
                    && !characters
                        .chars()
                        .all(|character| character.is_ascii_alphabetic())
                {
                    shift = false;
                }
                characters.to_lowercase()
            }
        },
    };
    gpui::Keystroke {
        key,
        key_char: None,
        modifiers: gpui::Modifiers {
            control: flags.contains(NSEventModifierFlags::Control),
            alt: flags.contains(NSEventModifierFlags::Option),
            shift,
            platform: flags.contains(NSEventModifierFlags::Command),
            function: flags.contains(NSEventModifierFlags::Function)
                && characters
                    .chars()
                    .next()
                    .is_none_or(|character| !(0xf700..=0xf747).contains(&u32::from(character))),
        },
    }
}

fn monitor(
    view: &Retained<MaskedWebview>,
    delegate: &Retained<Delegate>,
    sender: Sender<Event>,
) -> Option<Retained<AnyObject>> {
    let monitor_view = view.clone();
    let monitor_delegate = delegate.clone();
    let monitor = RcBlock::new(move |event: ptr::NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        let event = event.as_ptr();
        if matches!(
            event_ref.r#type(),
            NSEventType::LeftMouseDown | NSEventType::RightMouseDown | NSEventType::OtherMouseDown
        ) {
            // Muxy hears of a click from the page itself, which can be late.
            let clicked = monitor_view
                .window()
                .filter(|window| event_ref.window(monitor_view.mtm()).as_ref() == Some(window))
                .and_then(|window| window.contentView())
                .and_then(|content| content.hitTest(event_ref.locationInWindow()))
                .is_some_and(|view| view.isDescendantOf(&monitor_view));
            monitor_delegate.ivars().clicked.set(clicked);
            return event;
        }
        if monitor_view.isHidden() {
            return event;
        }
        let Some(window) = monitor_view.window().filter(|window| {
            window.isKeyWindow() && event_ref.window(monitor_view.mtm()).as_ref() == Some(window)
        }) else {
            return event;
        };
        let page_responder = || {
            window.firstResponder().filter(|responder| {
                responder
                    .downcast_ref::<NSView>()
                    .is_some_and(|responder| responder.isDescendantOf(&monitor_view))
            })
        };
        // Keys follow Muxy's focus: pages take the native focus on their own,
        // even from each other, and a click on Muxy never takes it back.
        let state = monitor_delegate.ivars();
        let focused = state.focused.get() || state.clicked.get();
        let holds_focus = page_responder().is_some();
        if holds_focus && !focused {
            if let Some(parent) = unsafe { monitor_view.superview() } {
                window.makeFirstResponder(Some(&parent));
            }
            return event;
        }
        if !holds_focus && focused && event_ref.r#type() == NSEventType::KeyDown {
            window.makeFirstResponder(Some(&monitor_view));
        }
        let Some(responder) = page_responder() else {
            return event;
        };
        if event_ref.r#type() == NSEventType::FlagsChanged {
            if let Some(parent) = unsafe { monitor_view.superview() } {
                parent.flagsChanged(event_ref);
            }
            return event;
        }
        if route_key_event(
            keystroke(event_ref),
            monitor_delegate.ivars().modal.get(),
            &monitor_delegate.ivars().shortcuts.borrow(),
            &sender,
            || monitor_view.performKeyEquivalent(event_ref),
            || {
                let app = NSApplication::sharedApplication(monitor_view.mtm());
                let Some(key) = event_ref.charactersIgnoringModifiers() else {
                    return false;
                };
                app.mainMenu().is_some_and(|menu| {
                    native_menu_key_equivalent(
                        &menu,
                        &key,
                        event_ref.modifierFlags(),
                        &monitor_view,
                    )
                })
            },
            || responder.keyDown(event_ref),
        ) {
            return ptr::null_mut();
        }
        event
    });
    unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::KeyDown
                | NSEventMask::FlagsChanged
                | NSEventMask::LeftMouseDown
                | NSEventMask::RightMouseDown
                | NSEventMask::OtherMouseDown,
            &monitor,
        )
    }
}

fn native_menu_key_equivalent(
    menu: &NSMenu,
    key: &NSString,
    modifiers: NSEventModifierFlags,
    view: &MaskedWebview,
) -> bool {
    let mask = NSEventModifierFlags::Command
        | NSEventModifierFlags::Control
        | NSEventModifierFlags::Option
        | NSEventModifierFlags::Shift;
    for item in &menu.itemArray() {
        if let Some(submenu) = item.submenu() {
            if native_menu_key_equivalent(&submenu, key, modifiers, view) {
                return true;
            }
        } else if item.keyEquivalentModifierMask() & mask == modifiers & mask
            && let Some(action) = item.action()
            && view.respondsToSelector(action)
            && item.keyEquivalent().caseInsensitiveCompare(key) == NSComparisonResult::Same
        {
            return unsafe {
                NSApplication::sharedApplication(view.mtm()).sendAction_to_from(
                    action,
                    Some(view),
                    Some(&item),
                )
            };
        }
    }
    false
}

fn route_key_event(
    keystroke: gpui::Keystroke,
    modal: bool,
    shortcuts: &[gpui::Keystroke],
    sender: &Sender<Event>,
    page_key_equivalent: impl FnOnce() -> bool,
    menu_key_equivalent: impl FnOnce() -> bool,
    page_key_down: impl FnOnce(),
) -> bool {
    if keystroke.key == "escape" && modal {
        let _ = sender.try_send(Event::Escape);
        return true;
    }
    if shortcuts
        .iter()
        .any(|key| key.key == keystroke.key && key.modifiers == keystroke.modifiers)
    {
        return sender.try_send(Event::Shortcut(keystroke)).is_ok();
    }
    if keystroke.modifiers.platform {
        menu_key_equivalent() || page_key_equivalent()
    } else {
        page_key_down();
        true
    }
}

fn background_color(background: Rgba) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(
        f64::from(background.r),
        f64::from(background.g),
        f64::from(background.b),
        1.0,
    )
}

fn asset_response(
    url: &NSURL,
    content_type: &str,
    length: isize,
) -> Option<Retained<NSURLResponse>> {
    let headers = NSDictionary::from_slices(
        &[
            &*NSString::from_str("Content-Type"),
            &*NSString::from_str("Content-Length"),
            &*NSString::from_str("Cache-Control"),
        ],
        &[
            &*NSString::from_str(content_type),
            &*NSString::from_str(&length.to_string()),
            &*NSString::from_str("no-store"),
        ],
    );
    objc2_foundation::NSHTTPURLResponse::initWithURL_statusCode_HTTPVersion_headerFields(
        objc2_foundation::NSHTTPURLResponse::alloc(),
        url,
        200,
        Some(&NSString::from_str("HTTP/1.1")),
        Some(&headers),
    )
    .map(Retained::into_super)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_shortcuts_try_native_menu_before_webkit() {
        let (sender, receiver) = async_channel::bounded(8);
        for modal in [false, true] {
            for binding in [
                "cmd-f",
                "cmd-g",
                "cmd-shift-g",
                "cmd-s",
                "cmd-x",
                "cmd-c",
                "cmd-v",
                "cmd-a",
                "cmd-z",
                "cmd-shift-z",
                "cmd-alt-shift-v",
            ] {
                for (page_handled, menu_handled) in
                    [(false, false), (false, true), (true, false), (true, true)]
                {
                    let deliveries = RefCell::new(Vec::new());
                    let handled = route_key_event(
                        gpui::Keystroke::parse(binding).expect("binding"),
                        modal,
                        &[],
                        &sender,
                        || {
                            deliveries.borrow_mut().push("page");
                            page_handled
                        },
                        || {
                            deliveries.borrow_mut().push("menu");
                            menu_handled
                        },
                        || panic!("command key must not be delivered as a plain key down"),
                    );
                    assert_eq!(handled, page_handled || menu_handled);
                    assert_eq!(
                        *deliveries.borrow(),
                        if menu_handled {
                            vec!["menu"]
                        } else {
                            vec!["menu", "page"]
                        },
                    );
                    assert!(receiver.is_empty());
                }
            }
        }
    }
}
