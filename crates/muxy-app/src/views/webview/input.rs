//! Pointer input for a page the app draws itself.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    Bounds, CursorStyle, DispatchPhase, Hitbox, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, ScrollWheelEvent, Window,
};
use muxy_ui::webview::Pointer;

/// What the pointer is doing to a page, kept from one frame to the next.
#[derive(Default)]
pub(crate) struct PointerState {
    held: Cell<Option<MouseButton>>,
    over: Cell<bool>,
}

impl PointerState {
    /// Passes the pointer to `native` while it is over `hitbox`, and for as
    /// long as a button pressed there is held: a drag that leaves the page,
    /// as when selecting text, still ends in it.
    pub(crate) fn forward(
        self: Rc<Self>,
        native: impl Fn(Pointer) + Clone + 'static,
        bounds: Bounds<Pixels>,
        hitbox: Hitbox,
        cursor: CursorStyle,
        window: &mut Window,
    ) {
        window.set_cursor_style(cursor, &hitbox);
        window.on_mouse_event({
            let (state, native, hitbox) = (self.clone(), native.clone(), hitbox.clone());
            move |event: &MouseDownEvent, phase, window, _| {
                if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                    state.held.set(Some(event.button));
                    native(Pointer::Pressed {
                        position: event.position - bounds.origin,
                        button: event.button,
                        modifiers: event.modifiers,
                    });
                }
            }
        });
        window.on_mouse_event({
            let (state, native, hitbox) = (self.clone(), native.clone(), hitbox.clone());
            move |event: &MouseUpEvent, phase, window, _| {
                let held = state.held.get() == Some(event.button);
                if phase == DispatchPhase::Bubble && (held || hitbox.is_hovered(window)) {
                    if held {
                        state.held.set(None);
                    }
                    native(Pointer::Released {
                        position: event.position - bounds.origin,
                        button: event.button,
                        modifiers: event.modifiers,
                    });
                }
            }
        });
        window.on_mouse_event({
            let (state, native, hitbox) = (self, native.clone(), hitbox.clone());
            move |event: &MouseMoveEvent, phase, window, _| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                if state.held.get().is_some() || hitbox.is_hovered(window) {
                    state.over.set(true);
                    native(Pointer::Moved {
                        position: event.position - bounds.origin,
                        pressed: event.pressed_button,
                        modifiers: event.modifiers,
                    });
                } else if state.over.replace(false) {
                    native(Pointer::Left);
                }
            }
        });
        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                let line_height =
                    window.line_height() / muxy_ui::theme::Metrics::of_window(window).multiplier();
                native(Pointer::Scrolled {
                    position: event.position - bounds.origin,
                    delta: event.delta.pixel_delta(line_height),
                    modifiers: event.modifiers,
                });
            }
        });
    }
}
