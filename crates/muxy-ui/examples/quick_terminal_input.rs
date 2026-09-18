#[cfg(target_os = "macos")]
#[allow(unsafe_code, clippy::expect_used)]
mod probe {
    use gpui::{
        AppContext, Application, Context, Focusable, InteractiveElement, IntoElement,
        ParentElement, Render, Styled, Window, WindowKind, WindowOptions, div, px,
    };
    use muxy_ui::quick_terminal::{
        panel::QuickTerminalConfiguration, platform::macos::PanelAdapter,
    };
    use muxy_ui::text_input::{InputStyle, TextInput};
    use objc2::rc::Retained;
    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType, NSView, NSWindow};
    use objc2_foundation::{NSPoint, NSString};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Duration;

    struct Probe {
        input: gpui::Entity<TextInput>,
        controls: Rc<Cell<usize>>,
        panel: PanelAdapter,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .on_key_down(cx.listener(|probe, event: &gpui::KeyDownEvent, _, cx| {
                    if event.keystroke.modifiers.control && event.keystroke.key == "l" {
                        probe.controls.set(probe.controls.get() + 1);
                        cx.stop_propagation();
                    }
                }))
                .child(self.input.clone())
        }
    }

    fn key(window: &NSWindow, text: &str, modifiers: NSEventModifierFlags, code: u16) {
        let text = NSString::from_str(text);
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown, NSPoint::ZERO, modifiers, 0.0,
            window.windowNumber(), None, &text, &text, false, code,
        ).expect("key event");
        window.sendEvent(&event);
    }

    pub(super) fn run() {
        Application::new().run(|cx| {
            cx.open_window(
                WindowOptions {
                    kind: WindowKind::PopUp,
                    titlebar: None,
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
                    let native_view =
                        unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
                            .expect("view");
                    let native = native_view.window().expect("window");
                    let mut panel = PanelAdapter::configure(window).expect("configure");
                    panel
                        .prepare(QuickTerminalConfiguration::default(), window)
                        .expect("prepare");
                    let color = gpui::rgb(0x00ff_ffff).into();
                    let input = cx.new(|cx| {
                        TextInput::new(
                            InputStyle {
                                text: color,
                                placeholder: color,
                                ghost: color,
                                cursor: color,
                                selection: color,
                                scrollbar: color,
                                font_size: px(14.0),
                                line_height: px(20.0),
                            },
                            cx,
                        )
                    });
                    input.read(cx).focus_handle(cx).focus(window);
                    panel.show(Duration::ZERO);
                    let controls = Rc::new(Cell::new(0));
                    let probe = cx.new(|_| Probe {
                        input,
                        controls,
                        panel,
                    });
                    let check = probe.clone();
                    cx.spawn(async move |cx| {
                        for round in 1..=2 {
                            cx.background_executor()
                                .timer(Duration::from_millis(300))
                                .await;
                            let responder = native.firstResponder().expect("first responder");
                            assert!(std::ptr::eq(
                                &raw const *responder,
                                &raw const **native_view
                            ));
                            key(&native, "a", NSEventModifierFlags::empty(), 0);
                            key(&native, "B", NSEventModifierFlags::Shift, 11);
                            key(&native, "l", NSEventModifierFlags::Control, 37);
                            check
                                .update(cx, |probe, cx| {
                                    assert_eq!(probe.input.read(cx).text(), "aB".repeat(round));
                                    assert_eq!(probe.controls.get(), round);
                                    if round == 1 {
                                        probe.panel.begin_hide(Duration::ZERO);
                                        probe.panel.finish_hide(false);
                                        probe.panel.show(Duration::ZERO);
                                    }
                                })
                                .expect("verify input");
                        }
                        cx.update(|cx| cx.quit()).expect("quit");
                    })
                    .detach();
                    probe
                },
            )
            .expect("open");
        });
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    probe::run();
}
