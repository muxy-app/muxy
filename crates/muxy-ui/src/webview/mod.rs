pub mod assets;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr;

use async_channel::{Receiver, Sender};
use block2::{DynBlock, RcBlock};
use gpui::{Bounds, Pixels, Rgba, Window};
use muxy_core::worker::WorkerPool;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSColor, NSEvent, NSEventMask, NSEventModifierFlags, NSImage, NSMenu, NSView,
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
            let _ = self
                .ivars()
                .sender
                .try_send(Event::Failed("Web content process terminated".into()));
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
    passthrough_left: Cell<f64>,
    passthrough_all: Cell<bool>,
}

impl HitTestRegions {
    fn excludes(&self, point: NSPoint, bounds: NSRect) -> bool {
        if self.passthrough_all.get() {
            return true;
        }
        let grip = NSRect::new(
            bounds.origin,
            NSSize::new(
                self.passthrough_left.get().min(bounds.size.width),
                bounds.size.height,
            ),
        );
        contains_point(grip, point)
            || self
                .occluded
                .borrow()
                .iter()
                .any(|rect| contains_point(*rect, point))
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
            if self.ivars().excludes(local, self.bounds()) { None }
            else { unsafe { msg_send![super(self), hitTest: point] } }
        }
    }
);

#[derive(Debug)]
pub struct NativeWebview {
    view: Retained<MaskedWebview>,
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
        let monitor = monitor(&view, &delegate, sender);
        let url = NSURL::URLWithString(&NSString::from_str(&url)).ok_or("invalid entry URL")?;
        unsafe {
            view.loadRequest(&NSURLRequest::requestWithURL(&url));
        }
        Ok((
            Self {
                view,
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

    pub fn set_mouse_passthrough_left(&self, width: Pixels) {
        self.view
            .ivars()
            .passthrough_left
            .set(f64::from(f32::from(width)));
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
        self.view.setHidden(!visible);
        if !visible {
            self.blur();
        }
    }

    pub fn focus(&self) {
        if self.visible.get()
            && let Some(window) = self.view.window()
        {
            window.makeFirstResponder(Some(&self.view));
        }
    }

    pub fn blur(&self) {
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
        self.view.removeFromSuperview();
    }
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
        if !monitor_view.isHidden()
            && monitor_view.window().is_some_and(|window| {
                window.isKeyWindow()
                    && window.firstResponder().is_some_and(|responder| {
                        let view: &NSView = &monitor_view;
                        ptr::eq(&raw const *responder, &raw const **view)
                            || responder
                                .downcast_ref::<NSView>()
                                .is_some_and(|responder| responder.isDescendantOf(view))
                    })
            })
            && route_key_event(
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
            )
        {
            return ptr::null_mut();
        }
        event
    });
    unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &monitor) }
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
    keystroke.modifiers.platform && (menu_key_equivalent() || page_key_equivalent())
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
    use gpui::{point, px, size};
    use objc2_app_kit::NSBitmapImageRep;

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

    #[test]
    fn native_editing_shortcuts_do_not_depend_on_webkit_resending_the_event() {
        let (sender, receiver) = async_channel::bounded(8);
        for binding in ["cmd-x", "cmd-c", "cmd-v", "cmd-a"] {
            let menu_deliveries = Cell::new(0);
            assert!(route_key_event(
                gpui::Keystroke::parse(binding).expect("binding"),
                false,
                &[],
                &sender,
                || panic!("WebKit must not consume a native editing command"),
                || {
                    menu_deliveries.set(menu_deliveries.get() + 1);
                    true
                },
            ));
            assert_eq!(menu_deliveries.get(), 1);
            assert!(receiver.is_empty());
        }
    }

    #[test]
    fn control_and_plain_keys_keep_native_dispatch() {
        let (sender, receiver) = async_channel::bounded(8);
        for binding in ["ctrl-f", "ctrl-g", "alt-f", "f", "left", "escape"] {
            assert!(!route_key_event(
                gpui::Keystroke::parse(binding).expect("binding"),
                false,
                &[],
                &sender,
                || panic!("non-command key must use native dispatch"),
                || panic!("non-command key must not be forwarded to the menu"),
            ));
            assert!(receiver.is_empty());
        }
    }

    #[test]
    fn app_shortcuts_and_modal_escape_take_priority_over_page_delivery() {
        let (sender, receiver) = async_channel::bounded(1);
        for binding in ["cmd-w", "cmd-t", "cmd-f", "cmd-v", "ctrl-f"] {
            let key = gpui::Keystroke::parse(binding).expect("binding");
            assert!(route_key_event(
                key.clone(),
                false,
                std::slice::from_ref(&key),
                &sender,
                || panic!("app shortcut must not reach the page"),
                || panic!("app shortcut must not reach the menu"),
            ));
            assert!(matches!(receiver.try_recv(), Ok(Event::Shortcut(actual)) if actual == key));
        }
        assert!(route_key_event(
            gpui::Keystroke::parse("escape").expect("escape"),
            true,
            &[],
            &sender,
            || panic!("modal escape must not reach the page"),
            || panic!("modal escape must not reach the menu"),
        ));
        let key = gpui::Keystroke::parse("cmd-w").expect("close shortcut");
        assert!(!route_key_event(
            key.clone(),
            false,
            &[key],
            &sender,
            || panic!("a full app queue must not redirect shortcuts to the page"),
            || panic!("a full app queue must not redirect shortcuts to the menu"),
        ));
        assert!(matches!(receiver.try_recv(), Ok(Event::Escape)));
        assert!(receiver.is_empty());
    }

    #[test]
    fn snapshots_pack_rows_in_bgra_order() {
        let rgba = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 10, 20, 30, 255,
        ];
        let source = image::RgbaImage::from_raw(2, 2, rgba).expect("source pixels");
        let mut png = std::io::Cursor::new(Vec::new());
        source
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("PNG");
        let bitmap =
            NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png.get_ref())).expect("bitmap");
        let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(2.0, 2.0));
        image.addRepresentation(&bitmap);
        let bgra = snapshot_rows(&image, 2, 2).expect("rows");
        assert_eq!(
            bgra,
            vec![
                0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 30, 20, 10, 255
            ]
        );
        assert_eq!(pixel_dimension(px(100.5), 2.0), Some(201));
        assert_eq!(pixel_dimension(px(0.0), 2.0), None);
    }

    #[test]
    fn native_function_keys_and_shifted_symbols_match_gpui_bindings() {
        for number in 1..=24_u32 {
            let character = char::from_u32(0xf704 + number - 1).expect("function key");
            let actual =
                native_keystroke(0, &character.to_string(), NSEventModifierFlags::Function);
            let expected = gpui::Keystroke::parse(&format!("f{number}")).expect("binding");
            assert_eq!(actual.key, expected.key);
            assert_eq!(actual.modifiers, expected.modifiers);
        }
        for (code, characters, flags, binding) in [
            (76, "\u{3}", NSEventModifierFlags::Command, "cmd-enter"),
            (
                24,
                "+",
                NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
                "cmd-+",
            ),
            (
                13,
                "W",
                NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
                "cmd-shift-w",
            ),
            (
                48,
                "\u{19}",
                NSEventModifierFlags::Control | NSEventModifierFlags::Shift,
                "ctrl-shift-tab",
            ),
            (
                124,
                "\u{f703}",
                NSEventModifierFlags::Command
                    | NSEventModifierFlags::Option
                    | NSEventModifierFlags::Function,
                "cmd-alt-right",
            ),
        ] {
            let actual = native_keystroke(code, characters, flags);
            let expected = gpui::Keystroke::parse(binding).expect("binding");
            assert_eq!(actual.key, expected.key);
            assert_eq!(actual.modifiers, expected.modifiers);
        }
    }

    #[test]
    fn native_background_preserves_dark_and_light_theme_colors() {
        for rgba in [0x1e1e_24ff, 0xf5f3_efff, 0x2835_4280] {
            let background = gpui::rgba(rgba);
            let color = background_color(background);
            for (actual, expected) in [
                (color.redComponent(), background.r),
                (color.greenComponent(), background.g),
                (color.blueComponent(), background.b),
                (color.alphaComponent(), 1.0),
            ] {
                assert!((actual - f64::from(expected)).abs() < 0.000_001);
            }
        }
    }

    #[test]
    fn asset_responses_parse_content_type_and_utf8_separately() {
        let url = NSURL::URLWithString(&NSString::from_str("muxy-ext://foundation/index.html"))
            .expect("asset URL");
        let bytes = "<p>Right edge → café</p>".as_bytes();
        let length = isize::try_from(bytes.len()).expect("asset length");
        for (content_type, mime, encoding) in [
            ("text/html; charset=utf-8", "text/html", Some("utf-8")),
            ("text/css; charset=utf-8", "text/css", Some("utf-8")),
            (
                "application/javascript; charset=utf-8",
                "application/javascript",
                Some("utf-8"),
            ),
            (
                "application/json; charset=utf-8",
                "application/json",
                Some("utf-8"),
            ),
            ("image/svg+xml", "image/svg+xml", None),
            ("image/png", "image/png", None),
            ("font/woff2", "font/woff2", None),
            ("application/wasm", "application/wasm", None),
            ("application/octet-stream", "application/octet-stream", None),
        ] {
            let response = asset_response(&url, content_type, length).expect("asset response");
            assert_eq!(response.MIMEType().expect("MIME type").to_string(), mime);
            assert_eq!(
                response.textEncodingName().map(|name| name.to_string()),
                encoding.map(str::to_owned)
            );
            assert_eq!(
                response.expectedContentLength(),
                i64::try_from(bytes.len()).expect("byte length")
            );
            assert_eq!(response.URL().expect("response URL"), url);
        }
    }

    #[test]
    fn asset_responses_report_success_without_caching() {
        let url = NSURL::URLWithString(&NSString::from_str("muxy-ext://foundation/modal.html"))
            .expect("asset URL");
        let response =
            asset_response(&url, "text/html; charset=utf-8", 42).expect("asset response");
        let response = response
            .downcast_ref::<objc2_foundation::NSHTTPURLResponse>()
            .expect("HTTP response matching Swift main");
        assert_eq!(response.statusCode(), 200);
        for (header, value) in [
            ("Content-Type", "text/html; charset=utf-8"),
            ("Content-Length", "42"),
            ("Cache-Control", "no-store"),
        ] {
            assert_eq!(
                response
                    .valueForHTTPHeaderField(&NSString::from_str(header))
                    .expect("header")
                    .to_string(),
                value
            );
        }
    }

    #[test]
    fn resize_passthrough_tracks_live_bounds_and_restores_content_hit_testing() {
        let regions = HitTestRegions::default();
        regions.passthrough_left.set(9.0);
        let occlusion = NSRect::new(NSPoint::new(100.0, 100.0), NSSize::new(40.0, 40.0));
        regions.occluded.borrow_mut().push(occlusion);
        for (width, height) in [(360.0, 300.0), (520.0, 700.0), (240.0, 500.0)] {
            let bounds = NSRect::new(NSPoint::ZERO, NSSize::new(width, height));
            let content = NSPoint::new(width - 1.0, height - 1.0);
            assert!(!regions.excludes(content, bounds));
            regions.passthrough_all.set(true);
            assert!(regions.excludes(content, bounds));
            assert!(regions.excludes(NSPoint::new(0.0, 0.0), bounds));
            assert_eq!(*regions.occluded.borrow(), vec![occlusion]);
            regions.passthrough_all.set(false);
            assert!(!regions.excludes(content, bounds));
            assert!(regions.excludes(NSPoint::new(8.0, height - 1.0), bounds));
            assert!(!regions.excludes(NSPoint::new(9.0, height - 1.0), bounds));
            assert!(regions.excludes(NSPoint::new(110.0, 110.0), bounds));
        }
    }

    #[test]
    fn resize_mouse_passthrough_preserves_webview_pixels_and_other_hit_regions() {
        let bounds = NSRect::new(NSPoint::new(12.0, 20.0), NSSize::new(360.0, 300.0));
        let regions = HitTestRegions::default();
        let edge = NSPoint::new(12.0, 100.0);
        assert!(!regions.excludes(edge, bounds));
        for width in [9.0, 14.0] {
            regions.passthrough_left.set(width);
            assert!(regions.excludes(edge, bounds));
            assert!(regions.excludes(NSPoint::new(12.0 + width - 0.5, 100.0), bounds));
            assert!(!regions.excludes(NSPoint::new(12.0 + width, 100.0), bounds));
            assert!(!regions.excludes(NSPoint::new(11.0, 100.0), bounds));
            assert!(!regions.excludes(NSPoint::new(12.0, 19.0), bounds));
            assert!(!regions.excludes(NSPoint::new(12.0, 320.0), bounds));
        }
        let canvas = Bounds::new(point(px(0.0), px(0.0)), size(px(360.0), px(300.0)));
        assert_eq!(
            composition_regions(canvas, canvas, &[]),
            (vec![canvas], vec![])
        );
        regions.occluded.borrow_mut().push(NSRect::new(
            NSPoint::new(100.0, 100.0),
            NSSize::new(40.0, 40.0),
        ));
        assert!(regions.excludes(NSPoint::new(110.0, 110.0), bounds));
        assert!(regions.excludes(edge, bounds));
        regions.passthrough_left.set(0.0);
        assert!(!regions.excludes(edge, bounds));
        assert!(regions.excludes(NSPoint::new(110.0, 110.0), bounds));
        regions.occluded.borrow_mut().clear();
        assert!(!regions.excludes(NSPoint::new(110.0, 110.0), bounds));
    }

    #[test]
    fn native_pixels_and_hit_testing_stay_inside_the_ancestor_clip() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(360.0), px(300.0)));
        let clip = Bounds::new(point(px(161.0), px(0.0)), size(px(199.0), px(300.0)));
        let overlay = Bounds::new(point(px(200.0), px(200.0)), size(px(200.0), px(100.0)));
        let (visible, excluded) = composition_regions(bounds, clip, &[overlay]);
        assert!(
            visible
                .iter()
                .all(|region| region.left() >= clip.left() && region.right() <= clip.right())
        );
        assert!(
            excluded
                .iter()
                .any(|region| region.left() == px(0.0) && region.right() == px(161.0))
        );
        assert!(
            visible
                .iter()
                .all(|region| region.bottom() <= px(200.0) || region.right() <= px(200.0))
        );
        let hidden = Bounds::new(point(px(500.0), px(0.0)), size(px(100.0), px(100.0)));
        assert!(composition_regions(bounds, hidden, &[]).0.is_empty());
        assert_eq!(composition_regions(bounds, hidden, &[]).1, vec![bounds]);
    }

    #[test]
    fn overlapping_nonmodal_occlusions_never_restore_a_hole() {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(100.0)));
        let right = Bounds::new(point(px(70.0), px(0.0)), size(px(40.0), px(100.0)));
        let bottom = Bounds::new(point(px(0.0), px(70.0)), size(px(100.0), px(40.0)));
        let regions: Vec<_> = subtract(bounds, right)
            .into_iter()
            .flat_map(|region| subtract(region, bottom))
            .collect();
        assert_eq!(
            regions,
            vec![Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(70.0), px(70.0))
            )]
        );
        assert!(subtract(bounds, bounds).is_empty());
        assert_eq!(
            subtract(
                bounds,
                Bounds::new(point(px(200.0), px(200.0)), size(px(20.0), px(20.0)))
            ),
            vec![bounds]
        );
    }
}
