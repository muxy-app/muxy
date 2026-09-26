#[cfg(target_os = "macos")]
#[allow(unsafe_code, clippy::expect_used)]
mod probe {
    use std::ptr;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::time::{Duration, Instant};

    use block2::RcBlock;
    use gpui::{
        AppContext, Application, AsyncApp, Context, IntoElement, Menu, MenuItem, OsAction, Render,
        Window, WindowOptions, div, point, px, size,
    };
    use muxy_core::shortcuts::{Defaults, ShortcutId};
    use muxy_ui::webview::{Event, NativeWebview, assets::Source};
    use muxy_ui::{shortcuts::Registry, text_input};
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
    use objc2_app_kit::{
        NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSPasteboard, NSPasteboardItem,
        NSPasteboardTypeString, NSView, NSWindow,
    };
    use objc2_foundation::{NSArray, NSError, NSPoint, NSString};
    use objc2_web_kit::WKWebView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    gpui::actions!(keyboard_probe, [Find, FindNext, FindPrevious]);

    static WINDOW: AtomicPtr<AnyObject> = AtomicPtr::new(ptr::null_mut());

    extern "C-unwind" fn key_window(_: &AnyObject, _: Sel) -> *mut AnyObject {
        WINDOW.load(Ordering::Relaxed)
    }

    extern "C-unwind" fn is_key(_: &AnyObject, _: Sel) -> Bool {
        Bool::YES
    }

    struct HiddenKeyWindow {
        app: Retained<NSApplication>,
        window: Retained<NSWindow>,
        app_class: &'static AnyClass,
        window_class: &'static AnyClass,
    }

    impl HiddenKeyWindow {
        fn new(window: Retained<NSWindow>) -> Self {
            let app =
                NSApplication::sharedApplication(MainThreadMarker::new().expect("main thread"));
            let app_class = app.class();
            let window_class = window.class();
            WINDOW.store(
                Retained::as_ptr(&window).cast_mut().cast(),
                Ordering::Relaxed,
            );
            unsafe {
                let mut class =
                    ClassBuilder::new(c"MuxyKeyboardProbeApp", app_class).expect("app class");
                class.add_method(
                    objc2::sel!(keyWindow),
                    key_window as extern "C-unwind" fn(_, _) -> _,
                );
                class.add_method(
                    objc2::sel!(mainWindow),
                    key_window as extern "C-unwind" fn(_, _) -> _,
                );
                objc2::ffi::object_setClass(
                    Retained::as_ptr(&app).cast_mut().cast(),
                    class.register(),
                );
                let mut class = ClassBuilder::new(c"MuxyKeyboardProbeWindow", window_class)
                    .expect("window class");
                class.add_method(
                    objc2::sel!(isKeyWindow),
                    is_key as extern "C-unwind" fn(_, _) -> _,
                );
                objc2::ffi::object_setClass(
                    Retained::as_ptr(&window).cast_mut().cast(),
                    class.register(),
                );
            }
            Self {
                app,
                window,
                app_class,
                window_class,
            }
        }

        async fn key(&self, text: &str, code: u16, shift: bool, cx: &AsyncApp) {
            let text = NSString::from_str(text);
            let mut flags = NSEventModifierFlags::Command;
            if shift {
                flags |= NSEventModifierFlags::Shift;
            }
            let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
                NSEventType::KeyDown, NSPoint::ZERO, flags, 0.0,
                self.window.windowNumber(), None, &text, &text, false, code,
            ).expect("key event");
            self.app.sendEvent(&event);
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
        }
    }

    impl Drop for HiddenKeyWindow {
        fn drop(&mut self) {
            unsafe {
                objc2::ffi::object_setClass(
                    Retained::as_ptr(&self.window).cast_mut().cast(),
                    self.window_class,
                );
                objc2::ffi::object_setClass(
                    Retained::as_ptr(&self.app).cast_mut().cast(),
                    self.app_class,
                );
            }
            WINDOW.store(ptr::null_mut(), Ordering::Relaxed);
        }
    }

    struct Clipboard {
        board: Retained<NSPasteboard>,
        saved: Vec<Retained<NSPasteboardItem>>,
        change_count: isize,
    }

    impl Clipboard {
        fn save() -> Self {
            let board = NSPasteboard::generalPasteboard();
            let saved = board
                .pasteboardItems()
                .into_iter()
                .flat_map(|items| items.to_vec())
                .map(|item| {
                    let copy = NSPasteboardItem::new();
                    for kind in &item.types() {
                        if let Some(data) = item.dataForType(&kind) {
                            assert!(copy.setData_forType(&data, &kind));
                        }
                    }
                    copy
                })
                .collect();
            let change_count = board.changeCount();
            Self {
                board,
                saved,
                change_count,
            }
        }

        fn write(&mut self, text: &str) {
            assert_eq!(
                self.board.changeCount(),
                self.change_count,
                "clipboard changed outside probe"
            );
            self.board.clearContents();
            assert!(
                self.board
                    .setString_forType(&NSString::from_str(text), unsafe {
                        NSPasteboardTypeString
                    })
            );
            self.change_count = self.board.changeCount();
        }

        fn expect_copied(&mut self, expected: &str) {
            let change_count = self.board.changeCount();
            let text = self.board.stringForType(unsafe { NSPasteboardTypeString });
            assert_eq!(
                text.as_ref().map(ToString::to_string).as_deref(),
                Some(expected)
            );
            assert_eq!(
                self.board.changeCount(),
                change_count,
                "clipboard changed outside probe"
            );
            self.change_count = change_count;
        }
    }

    impl Drop for Clipboard {
        fn drop(&mut self) {
            if self.board.changeCount() == self.change_count {
                self.board.clearContents();
                let saved = NSArray::from_retained_slice(&self.saved);
                let _: bool = unsafe { objc2::msg_send![&*self.board, writeObjects: &*saved] };
            }
        }
    }

    struct Probe {
        _page: Rc<NativeWebview>,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    async fn receive<T>(receiver: &async_channel::Receiver<T>, cx: &AsyncApp) -> T {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match receiver.try_recv() {
                Ok(value) => return value,
                Err(error) => assert!(
                    error.is_empty() && Instant::now() < deadline,
                    "native probe timed out: {error}"
                ),
            }
            cx.background_executor()
                .timer(Duration::from_millis(10))
                .await;
        }
    }

    async fn evaluate(page: &WKWebView, script: &str, cx: &AsyncApp) -> String {
        let (sender, receiver) = async_channel::bounded(1);
        let callback = RcBlock::new(move |value: *mut AnyObject, error: *mut NSError| {
            let result = unsafe {
                if let Some(error) = error.as_ref() {
                    Err(error.localizedDescription().to_string())
                } else if let Some(value) = value.as_ref() {
                    let text: Retained<NSString> = objc2::msg_send![value, description];
                    Ok(text.to_string())
                } else {
                    Ok("undefined".into())
                }
            };
            let _ = sender.try_send(result);
        });
        unsafe {
            page.evaluateJavaScript_completionHandler(&NSString::from_str(script), Some(&callback));
        }
        receive(&receiver, cx).await.expect("JavaScript result")
    }

    async fn check(window: &HiddenKeyWindow, page: &WKWebView, cx: &AsyncApp) {
        assert!(!window.window.isVisible());
        assert_eq!(evaluate(page, "selectProbe()", cx).await, "1");
        cx.background_executor()
            .timer(Duration::from_millis(200))
            .await;
        let mut clipboard = Clipboard::save();
        clipboard.write("sentinel");
        window.key("c", 8, false, cx).await;
        clipboard.expect_copied("native clipboard probe");
        clipboard.write("pasted probe");
        window.key("v", 9, false, cx).await;
        assert_eq!(evaluate(page, "probeText()", cx).await, "pasted probe");
        window.key("a", 0, false, cx).await;
        window.key("x", 7, false, cx).await;
        clipboard.expect_copied("pasted probe");
        assert_eq!(evaluate(page, "probeText()", cx).await, "");
        window.key("v", 9, false, cx).await;
        assert_eq!(evaluate(page, "probeText()", cx).await, "pasted probe");
        for (text, code, shift) in [
            ("f", 3, false),
            ("g", 5, false),
            ("G", 5, true),
            ("s", 1, false),
            ("z", 6, false),
            ("Z", 6, true),
        ] {
            window.key(text, code, shift, cx).await;
        }
        assert_eq!(
            evaluate(page, "probeEvents()", cx).await,
            "copy,paste,cut,paste,f,g,shift-g,s,z,shift-z"
        );
        let paste = window
            .app
            .mainMenu()
            .expect("main menu")
            .itemAtIndex(0)
            .expect("Edit")
            .submenu()
            .expect("Edit menu")
            .itemWithTitle(&NSString::from_str("Paste"))
            .expect("Paste");
        paste.setKeyEquivalent(&NSString::from_str("p"));
        paste.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
        );
        clipboard.write("remapped paste");
        window.key("a", 0, false, cx).await;
        window.key("P", 35, true, cx).await;
        assert_eq!(evaluate(page, "probeText()", cx).await, "remapped paste");
        assert!(!window.window.isVisible());
    }

    pub(super) fn run() {
        Application::new().run(|cx| {
            let mut registry = Registry::new(&Defaults);
            registry.register(ShortcutId::Copy, &text_input::Copy);
            registry.register(ShortcutId::Paste, &text_input::Paste);
            registry.register(ShortcutId::Find, &Find);
            registry.register(ShortcutId::FindNext, &FindNext);
            registry.register(ShortcutId::FindPrevious, &FindPrevious);
            text_input::register_shortcuts(&mut registry);
            cx.bind_keys(registry.into_bindings());
            cx.on_action(|_: &Find, _| {});
            cx.on_action(|_: &FindNext, _| {});
            cx.on_action(|_: &FindPrevious, _| {});
            cx.set_menus(vec![Menu {
                name: "Edit".into(),
                items: vec![
                    MenuItem::os_action("Cut", text_input::Cut, OsAction::Cut),
                    MenuItem::os_action("Copy", text_input::Copy, OsAction::Copy),
                    MenuItem::os_action("Paste", text_input::Paste, OsAction::Paste),
                    MenuItem::os_action("Select All", text_input::SelectAll, OsAction::SelectAll),
                    MenuItem::action("Find…", Find),
                    MenuItem::action("Find Next", FindNext),
                    MenuItem::action("Find Previous", FindPrevious),
                ],
            }]);
            cx.open_window(
                WindowOptions {
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                |window, cx| {
                    let RawWindowHandle::AppKit(handle) =
                        window.window_handle().expect("handle").as_raw()
                    else {
                        unreachable!()
                    };
                    let parent =
                        unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
                            .expect("parent");
                    let native = HiddenKeyWindow::new(parent.window().expect("native window"));
                    let source = Source {
                        owner: "keyboard-probe".into(),
                        directory: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("examples/fixtures"),
                        entry: "webview_keyboard.html".into(),
                    };
                    let (page, events) =
                        NativeWebview::new(window, source, "", gpui::rgb(0x00ff_ffff), false)
                            .expect("webview");
                    let page = Rc::new(page);
                    let bounds =
                        gpui::Bounds::new(point(px(0.0), px(0.0)), size(px(400.0), px(300.0)));
                    page.sync(bounds, bounds, gpui::rgb(0x00ff_ffff), 0.0);
                    page.set_visible(true);
                    page.focus();
                    let view = parent
                        .subviews()
                        .iter()
                        .find_map(|view| view.downcast::<WKWebView>().ok())
                        .expect("WKWebView");
                    let asset_page = page.clone();
                    let (loaded, ready) = async_channel::bounded(1);
                    cx.spawn(async move |_| {
                        while let Ok(event) = events.recv().await {
                            match event {
                                Event::Asset { id, result } => {
                                    asset_page.complete_asset(id, result);
                                }
                                Event::Loaded => {
                                    let _ = loaded.try_send(Ok(()));
                                }
                                Event::Failed(error) => {
                                    let _ = loaded.try_send(Err(error));
                                }
                                _ => (),
                            }
                        }
                    })
                    .detach();
                    cx.spawn(async move |cx| {
                        receive(&ready, cx).await.expect("page loaded");
                        check(&native, &view, cx).await;
                        drop(native);
                        cx.update(|cx| cx.quit()).expect("quit");
                    })
                    .detach();
                    cx.new(|_| Probe { _page: page })
                },
            )
            .expect("window");
        });
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    probe::run();
}
