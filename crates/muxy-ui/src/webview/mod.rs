pub mod assets;
pub(crate) mod host;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr;

use async_channel::{Receiver, Sender};
use block2::{DynBlock, RcBlock};
use gpui::{Bounds, CursorStyle, Pixels, Rgba, Window};
use muxy_core::worker::WorkerPool;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSColor, NSView};
use objc2_foundation::{
    NSData, NSDictionary, NSError, NSJSONReadingOptions, NSJSONSerialization, NSJSONWritingOptions,
    NSNumber, NSObject, NSObjectNSKeyValueCoding, NSObjectProtocol, NSRect, NSString, NSURL,
    NSURLRequest, NSURLResponse,
};
use objc2_web_kit::{
    WKContentWorld, WKNavigation, WKNavigationAction, WKNavigationActionPolicy,
    WKNavigationDelegate, WKScriptMessage, WKScriptMessageHandlerWithReply, WKUIDelegate,
    WKURLSchemeHandler, WKURLSchemeTask, WKUserContentController, WKUserScript,
    WKUserScriptInjectionTime, WKWebView, WKWebViewConfiguration, WKWindowFeatures,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::tr;
use assets::Source;
use host::{Host, Signal};

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

#[derive(Debug)]
pub struct NativeWebview {
    host: Host,
    view: Retained<WKWebView>,
    delegate: Retained<Delegate>,
    background: Cell<Option<Rgba>>,
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
        let parent = parent_view(window)?;
        let (sender, receiver) = async_channel::bounded(256);
        let delegate = Delegate::alloc(mtm).set_ivars(State {
            source,
            sender: sender.clone(),
            worker: WorkerPool::new("webview-assets", 2, 32).map_err(|error| error.to_string())?,
            sequence: Cell::new(0),
            generation: Cell::new(0),
            snapshot_id: Cell::new(0),
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
        let view = unsafe {
            WKWebView::initWithFrame_configuration(
                WKWebView::alloc(mtm),
                NSRect::ZERO,
                &configuration,
            )
        };
        unsafe {
            view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            view.setUIDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        }
        view.setWantsLayer(true);
        unsafe {
            view.setUnderPageBackgroundColor(Some(&background_color(background)));
            if transparent_background {
                view.setValue_forKey(
                    Some(&NSNumber::new_bool(false)),
                    &NSString::from_str("drawsBackground"),
                );
            }
        }
        let mut host = Host::new(parent, mtm);
        host.container().addSubview(&view);
        host.attach(view.clone(), move |signal| match signal {
            Signal::Escape => sender.try_send(Event::Escape).is_ok(),
            Signal::Shortcut(keystroke) => sender.try_send(Event::Shortcut(keystroke)).is_ok(),
            Signal::Clicked => false,
        });
        let url = NSURL::URLWithString(&NSString::from_str(&url)).ok_or("invalid entry URL")?;
        unsafe {
            view.loadRequest(&NSURLRequest::requestWithURL(&url));
        }
        Ok((
            Self {
                host,
                view,
                delegate,
                background: Cell::new(Some(background)),
            },
            receiver,
        ))
    }

    pub fn set_shortcuts(&self, modal: bool, shortcuts: Vec<gpui::Keystroke>) {
        self.host.set_shortcuts(modal, shortcuts);
    }

    /// Whether Muxy shows this page focused. Keys reach the page only while it
    /// is, or after a click into it until Muxy catches up. A page that takes
    /// the native focus itself, as when its script focuses an element, doesn't
    /// get them.
    pub fn set_focused(&self, focused: bool) {
        self.host.set_focused(focused);
    }

    /// Sends clicks on the resize grips drawn over the page, in window
    /// coordinates, to Muxy and shows each grip's cursor there. The page's
    /// pixels don't change. Call after [`Self::occlude`].
    pub fn set_grips(&self, grips: &[(Bounds<Pixels>, CursorStyle)]) {
        self.host.set_grips(grips);
    }

    pub fn set_mouse_passthrough(&self, enabled: bool) {
        self.host.set_mouse_passthrough(enabled);
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
        if self.background.replace(Some(background)) != Some(background) {
            unsafe {
                self.view
                    .setUnderPageBackgroundColor(Some(&background_color(background)));
            }
        }
        self.host.sync(bounds, clip, radius);
    }

    pub fn occlude(&self, occlusions: &[Bounds<Pixels>]) {
        self.host.occlude(occlusions);
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

    pub fn snapshot(&self) {
        let delegate = self.delegate.clone();
        let state = delegate.ivars();
        let id = state.next();
        let generation = state.generation.get();
        state.snapshot_id.set(id);
        self.host.snapshot(move |image| {
            let state = delegate.ivars();
            if state.snapshot_id.get() == id && state.generation.get() == generation {
                let _ = state.sender.try_send(Event::Snapshot {
                    id,
                    generation,
                    image,
                });
            }
        });
    }

    pub fn is_current_snapshot(&self, id: u64, generation: u64) -> bool {
        let state = self.delegate.ivars();
        state.snapshot_id.get() == id && state.generation.get() == generation
    }
}

/// The native view GPUI draws into, which pages are added to.
pub(crate) fn parent_view(window: &Window) -> Result<Retained<NSView>, String> {
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window)
        .map_err(|error| error.to_string())?
        .as_raw()
    else {
        return Err("expected AppKit window".into());
    };
    unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
        .ok_or_else(|| "missing native view".into())
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
        self.host.blur();
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
