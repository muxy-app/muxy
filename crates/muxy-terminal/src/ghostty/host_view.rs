use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;

use async_channel::{Receiver, Sender};
use ghostty_host::{
    ClipboardCompletionError, ClipboardRequestToken, ColorScheme, GhosttyApp, GhosttyConfig,
    GhosttySurface, KeyAction, KeyboardInput, Modifiers, MouseButton, MouseButtonState,
    MouseMomentum, MousePressureStage, MouseShape, MouseVisibility, ScrollMetadata, SurfaceError,
    SurfaceId, SurfaceOptions, SurfacePoint, SurfaceTextError,
};
use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAppearanceCustomization, NSAppearanceName, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSCursor, NSEvent, NSEventMask, NSEventModifierFlags, NSEventPhase, NSEventType,
    NSTextInputClient, NSTextInputContextKeyboardSelectionDidChangeNotification, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSViewBoundsDidChangeNotification, NSWindowOrderingMode,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSNotFound, NSNotification,
    NSNotificationCenter, NSPoint, NSRange, NSRangePointer, NSRect, NSSize, NSString, NSUInteger,
};
use thiserror::Error;

use super::scrollbar::NativeScrollbar;
use crate::backend::ShortcutGate;
use crate::scrollbar::ScrollbarMetrics;

const APPKIT_CAPS_LOCK: usize = 1 << 16;
const APPKIT_SHIFT: usize = 1 << 17;
const APPKIT_CONTROL: usize = 1 << 18;
const APPKIT_OPTION: usize = 1 << 19;
const APPKIT_COMMAND: usize = 1 << 20;

const APPKIT_RIGHT_SHIFT: usize = 0x04;
const APPKIT_RIGHT_CONTROL: usize = 0x2000;
const APPKIT_RIGHT_OPTION: usize = 0x40;
const APPKIT_RIGHT_COMMAND: usize = 0x10;

const GHOSTTY_SHIFT: u32 = 1 << 0;
const GHOSTTY_CONTROL: u32 = 1 << 1;
const GHOSTTY_ALT: u32 = 1 << 2;
const GHOSTTY_SUPER: u32 = 1 << 3;
const GHOSTTY_CAPS: u32 = 1 << 4;
const GHOSTTY_SHIFT_RIGHT: u32 = 1 << 6;
const GHOSTTY_CONTROL_RIGHT: u32 = 1 << 7;
const GHOSTTY_ALT_RIGHT: u32 = 1 << 8;
const GHOSTTY_SUPER_RIGHT: u32 = 1 << 9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostViewPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HostViewEvent {
    ContextMenu(HostViewPoint),
    Appearance(ColorScheme),
    AppShortcut,
    NavigateBack,
    NavigateForward,
}

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum HostViewError {
    #[error("a Ghostty surface is already attached to this host view")]
    AlreadyAttached,
    #[error("no Ghostty surface is attached to this host view")]
    NoSurface,
    #[error(transparent)]
    Surface(#[from] SurfaceError),
    #[error(transparent)]
    Clipboard(#[from] ClipboardCompletionError),
}

#[allow(dead_code)]
pub struct HostIvars {
    surface: RefCell<Option<GhosttySurface>>,
    app: RefCell<Option<GhosttyApp>>,
    events: Sender<HostViewEvent>,
    event_receiver: Receiver<HostViewEvent>,
    window_active: Cell<bool>,
    overlay_active: Cell<bool>,
    forwarded_keys: RefCell<HashSet<u16>>,
    forwarded_mouse_buttons: Cell<u16>,
    forwarded_right_mouse_press: Cell<bool>,
    marked_text: RefCell<String>,
    marked_range: Cell<NSRange>,
    selected_range: Cell<NSRange>,
    interpreting_key_event: Cell<bool>,
    key_text_accumulator: RefCell<Vec<String>>,
    command_selector_called: Cell<bool>,
    tracking_area: RefCell<Option<Retained<NSTrackingArea>>>,
    event_monitor: RefCell<Option<Retained<AnyObject>>>,
    observing_input_source: Cell<bool>,
    pointer_inside: Cell<bool>,
    requested_cursor_hidden: Cell<bool>,
    cursor_hidden: Cell<bool>,
    cursor: RefCell<Retained<NSCursor>>,
    color_scheme: Cell<ColorScheme>,
    shortcut_gate: RefCell<Option<Rc<ShortcutGate>>>,
    app_view: RefCell<Option<Retained<NSView>>>,
    scrollbar: RefCell<Option<NativeScrollbar>>,
}

impl Default for HostIvars {
    fn default() -> Self {
        let (events, event_receiver) = async_channel::unbounded();
        Self {
            surface: RefCell::new(None),
            app: RefCell::new(None),
            events,
            event_receiver,
            window_active: Cell::new(false),
            overlay_active: Cell::new(false),
            forwarded_keys: RefCell::new(HashSet::new()),
            forwarded_mouse_buttons: Cell::new(0),
            forwarded_right_mouse_press: Cell::new(false),
            marked_text: RefCell::new(String::new()),
            marked_range: Cell::new(not_found_range()),
            selected_range: Cell::new(empty_range()),
            interpreting_key_event: Cell::new(false),
            key_text_accumulator: RefCell::new(Vec::new()),
            command_selector_called: Cell::new(false),
            tracking_area: RefCell::new(None),
            event_monitor: RefCell::new(None),
            observing_input_source: Cell::new(false),
            pointer_inside: Cell::new(false),
            requested_cursor_hidden: Cell::new(false),
            cursor_hidden: Cell::new(false),
            cursor: RefCell::new(NSCursor::arrowCursor()),
            color_scheme: Cell::new(ColorScheme::Light),
            shortcut_gate: RefCell::new(None),
            app_view: RefCell::new(None),
            scrollbar: RefCell::new(None),
        }
    }
}

define_class!(
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "MuxyGhosttyHostView"]
    #[ivars = HostIvars]
    pub struct GhosttyHostView;

    impl GhosttyHostView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            !self.ivars().overlay_active.get()
        }

        #[unsafe(method(becomeFirstResponder))]
        fn become_first_responder(&self) -> bool {
            if self.ivars().overlay_active.get() {
                false
            } else {
                let accepted: bool = unsafe { msg_send![super(self), becomeFirstResponder] };
                if accepted {
                    self.sync_focus();
                }
                accepted
            }
        }

        #[unsafe(method(resignFirstResponder))]
        fn resign_first_responder(&self) -> bool {
            let accepted: bool = unsafe { msg_send![super(self), resignFirstResponder] };
            if accepted {
                self.sync_focus();
            }
            accepted
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            if !self.process_key_down(event) && !self.ivars().overlay_active.get() {
                let _: () = unsafe { msg_send![super(self), keyDown: event] };
            }
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            let was_forwarded = self
                .ivars()
                .forwarded_keys
                .borrow_mut()
                .remove(&event.keyCode());
            if !forwards_input_while_overlay(
                self.ivars().overlay_active.get(),
                true,
                was_forwarded,
            ) {
                return;
            }
            if !self.forward_physical_key(event, KeyAction::Release, None, false, Modifiers::NONE) {
                let _: () = unsafe { msg_send![super(self), keyUp: event] };
            }
        }

        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            if self.has_marked_text_internal() {
                return;
            }
            let keycode = event.keyCode();
            let action = if modifier_key_is_pressed(keycode, event.modifierFlags().0) {
                KeyAction::Press
            } else {
                KeyAction::Release
            };
            let was_forwarded = match action {
                KeyAction::Press | KeyAction::Repeat => false,
                KeyAction::Release => self.ivars().forwarded_keys.borrow_mut().remove(&keycode),
            };
            if !forwards_input_while_overlay(
                self.ivars().overlay_active.get(),
                matches!(action, KeyAction::Release),
                was_forwarded,
            ) {
                return;
            }
            if self.forward_physical_key(event, action, None, false, Modifiers::NONE)
                && matches!(action, KeyAction::Press)
            {
                self.ivars().forwarded_keys.borrow_mut().insert(keycode);
            }
        }

        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> bool {
            self.perform_key_equivalent_event(event)
        }

        #[unsafe(method(insertText:))]
        unsafe fn insert_text_legacy(&self, value: &AnyObject) {
            self.insert_text_value(value);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if self.ivars().overlay_active.get() {
                return;
            }
            if let Some(window) = self.window() {
                window.makeFirstResponder(Some(self));
            }
            self.mouse_button_event(event, MouseButtonState::Press, MouseButton::Left, false);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.mouse_button_event(event, MouseButtonState::Release, MouseButton::Left, false);
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.process_right_mouse_down(event);
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            self.process_right_mouse_up(event);
        }

        #[unsafe(method(otherMouseDown:))]
        fn other_mouse_down(&self, event: &NSEvent) {
            if let Some(button) = mouse_button(event.buttonNumber()) {
                self.mouse_button_event(event, MouseButtonState::Press, button, false);
            }
        }

        #[unsafe(method(otherMouseUp:))]
        fn other_mouse_up(&self, event: &NSEvent) {
            if let Some(button) = mouse_button(event.buttonNumber()) {
                self.mouse_button_event(event, MouseButtonState::Release, button, false);
            }
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.forward_mouse_position(event);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.forward_mouse_position(event);
        }

        #[unsafe(method(rightMouseDragged:))]
        fn right_mouse_dragged(&self, event: &NSEvent) {
            self.forward_mouse_position(event);
        }

        #[unsafe(method(otherMouseDragged:))]
        fn other_mouse_dragged(&self, event: &NSEvent) {
            self.forward_mouse_position(event);
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            self.set_pointer_inside(true);
            self.forward_mouse_position(event);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, event: &NSEvent) {
            self.forward_mouse_position(event);
            self.set_pointer_inside(false);
        }

        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self, _event: &NSEvent) {
            self.apply_cursor();
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            if self.ivars().overlay_active.get() {
                return;
            }
            let metadata = ScrollMetadata::new(
                event.hasPreciseScrollingDeltas(),
                mouse_momentum(event.momentumPhase()),
            );
            if let Some(surface) = self.ivars().surface.borrow().as_ref() {
                surface.send_mouse_scroll(event.scrollingDeltaX(), event.scrollingDeltaY(), metadata);
            }
            self.flash_scrollbar();
        }

        #[unsafe(method(pressureChangeWithEvent:))]
        fn pressure_change(&self, event: &NSEvent) {
            if self.ivars().overlay_active.get() {
                return;
            }
            let stage = mouse_pressure_stage(event.stage());
            if let Some(surface) = self.ivars().surface.borrow().as_ref() {
                surface.send_mouse_pressure(stage, f64::from(event.pressure()));
            }
        }

        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };
            self.install_tracking_area();
        }

        #[unsafe(method(setFrameSize:))]
        fn set_frame_size(&self, size: NSSize) {
            let _: () = unsafe { msg_send![super(self), setFrameSize: size] };
            self.sync_surface_geometry();
        }

        #[unsafe(method(viewDidChangeBackingProperties))]
        fn view_did_change_backing_properties(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidChangeBackingProperties] };
            self.sync_surface_geometry();
        }

        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn view_did_change_effective_appearance(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidChangeEffectiveAppearance] };
            self.sync_color_scheme(true);
        }

        #[unsafe(method(viewDidMoveToWindow))]
        fn view_did_move_to_window(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidMoveToWindow] };
            self.sync_input_source_observer();
            self.sync_surface_geometry();
            self.sync_color_scheme(false);
            self.sync_cursor_visibility();
            self.sync_focus();
        }

        #[unsafe(method(keyboardInputSourceChanged:))]
        fn keyboard_input_source_changed(&self, _notification: &NSNotification) {
            self.clear_marked_text(true);
            if let Some(context) = self.inputContext() {
                context.discardMarkedText();
                context.invalidateCharacterCoordinates();
            }
            if let Some(app) = self.ivars().app.borrow().as_ref() {
                app.keyboard_changed();
            }
        }

        #[unsafe(method(scrollbarBoundsDidChange:))]
        fn scrollbar_bounds_did_change(&self, _notification: &NSNotification) {
            let row = self
                .ivars()
                .scrollbar
                .borrow()
                .as_ref()
                .and_then(NativeScrollbar::live_row);
            if let Some(row) = row
                && let Some(surface) = self.ivars().surface.borrow().as_ref()
            {
                surface.perform_binding_action(&format!("scroll_to_row:{row}"));
            }
        }
    }

    unsafe impl NSObjectProtocol for GhosttyHostView {}

    #[allow(non_snake_case)]
    unsafe impl NSTextInputClient for GhosttyHostView {
        #[unsafe(method(insertText:replacementRange:))]
        unsafe fn insertText_replacementRange(
            &self,
            value: &AnyObject,
            _replacement_range: NSRange,
        ) {
            self.insert_text_value(value);
        }

        #[unsafe(method(doCommandBySelector:))]
        unsafe fn doCommandBySelector(&self, _selector: objc2::runtime::Sel) {
            self.ivars().command_selector_called.set(true);
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        unsafe fn setMarkedText_selectedRange_replacementRange(
            &self,
            value: &AnyObject,
            selected_range: NSRange,
            _replacement_range: NSRange,
        ) {
            self.set_marked_text_value(value, selected_range);
        }

        #[unsafe(method(unmarkText))]
        fn unmarkText(&self) {
            self.clear_marked_text(true);
        }

        #[unsafe(method(selectedRange))]
        fn selectedRange(&self) -> NSRange {
            self.ivars().selected_range.get()
        }

        #[unsafe(method(markedRange))]
        fn markedRange(&self) -> NSRange {
            self.ivars().marked_range.get()
        }

        #[unsafe(method(hasMarkedText))]
        fn hasMarkedText(&self) -> bool {
            self.has_marked_text_internal()
        }

        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        unsafe fn attributedSubstringForProposedRange_actualRange(
            &self,
            range: NSRange,
            actual_range: NSRangePointer,
        ) -> Option<Retained<NSAttributedString>> {
            unsafe { self.attributed_substring(range, actual_range) }
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn validAttributesForMarkedText(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            NSArray::from_slice(&[])
        }

        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        unsafe fn firstRectForCharacterRange_actualRange(
            &self,
            range: NSRange,
            actual_range: NSRangePointer,
        ) -> NSRect {
            if !actual_range.is_null() {
                unsafe { *actual_range = range };
            }
            self.ime_screen_rect()
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn characterIndexForPoint(&self, _point: NSPoint) -> NSUInteger {
            ns_not_found()
        }
    }
);

impl Drop for GhosttyHostView {
    fn drop(&mut self) {
        if let Some(monitor) = self.ivars().event_monitor.borrow_mut().take() {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        if self.ivars().cursor_hidden.replace(false) {
            NSCursor::unhide();
        }
        self.ivars().observing_input_source.set(false);
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(self) };
    }
}

#[allow(dead_code)]
impl GhosttyHostView {
    pub fn new(mtm: objc2_foundation::MainThreadMarker) -> Retained<Self> {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        let this = Self::alloc(mtm).set_ivars(HostIvars::default());
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setWantsLayer(true);
        this.install_scrollbar(mtm);
        this.sync_color_scheme(false);
        this
    }

    fn install_scrollbar(&self, mtm: objc2_foundation::MainThreadMarker) {
        let scrollbar = NativeScrollbar::new(mtm);
        let content_view = scrollbar.content_view();
        content_view.setPostsBoundsChangedNotifications(true);
        self.addSubview_positioned_relativeTo(scrollbar.view(), NSWindowOrderingMode::Above, None);
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                self,
                sel!(scrollbarBoundsDidChange:),
                Some(NSViewBoundsDidChangeNotification),
                Some(&content_view),
            )
        };
        scrollbar.layout(self.bounds());
        self.ivars().scrollbar.replace(Some(scrollbar));
    }

    pub fn attach_surface(
        &self,
        app: &GhosttyApp,
        mut options: SurfaceOptions,
    ) -> Result<SurfaceId, HostViewError> {
        if self.ivars().surface.borrow().is_some() {
            return Err(HostViewError::AlreadyAttached);
        }

        options.scale_factor = self.backing_scale();
        let view_pointer = NonNull::from(self).cast::<c_void>();
        let surface = unsafe { GhosttySurface::new(app, view_pointer, options) }?;
        let id = surface.id();
        app.set_color_scheme(self.color_scheme());
        surface.set_color_scheme(self.color_scheme());
        self.ivars().app.replace(Some(app.clone()));
        self.ivars().surface.replace(Some(surface));
        self.install_event_monitor();
        self.sync_surface_geometry();
        self.sync_focus();
        Ok(id)
    }

    pub fn event_receiver(&self) -> Receiver<HostViewEvent> {
        self.ivars().event_receiver.clone()
    }

    pub fn set_window_active(&self, active: bool) {
        self.ivars().window_active.set(active);
        self.sync_cursor_visibility();
        self.sync_focus();
    }

    pub fn set_overlay_active(&self, active: bool) {
        if self.ivars().overlay_active.replace(active) == active {
            return;
        }
        if active {
            self.clear_marked_text(true);
        }
        self.sync_cursor_visibility();
        self.sync_focus();
    }

    pub fn set_app_view(&self, view: Retained<NSView>) {
        *self.ivars().app_view.borrow_mut() = Some(view);
    }

    fn forward_app_shortcut(&self, event: &NSEvent) -> bool {
        if !self.is_app_shortcut(event) {
            return false;
        }
        let _ = self.ivars().events.try_send(HostViewEvent::AppShortcut);
        if let Some(view) = self.ivars().app_view.borrow().clone() {
            let _: () = unsafe { msg_send![&*view, keyDown: event] };
        }
        true
    }

    pub fn set_shortcut_gate(&self, gate: Rc<ShortcutGate>) {
        *self.ivars().shortcut_gate.borrow_mut() = Some(gate);
    }

    fn is_app_shortcut(&self, event: &NSEvent) -> bool {
        let gate = self.ivars().shortcut_gate.borrow();
        let Some(gate) = gate.as_ref() else {
            return false;
        };
        let modifiers = (event.modifierFlags().0
            & (APPKIT_SHIFT | APPKIT_CONTROL | APPKIT_OPTION | APPKIT_COMMAND))
            as u64;
        if modifiers == 0 {
            return false;
        }
        let characters = event
            .charactersIgnoringModifiers()
            .map(|characters| characters.to_string())
            .unwrap_or_default();
        gate.declines(&characters, event.keyCode(), modifiers)
    }

    pub fn restore_focus(&self) -> bool {
        if self.ivars().overlay_active.get() {
            return false;
        }
        self.window()
            .is_some_and(|window| window.makeFirstResponder(Some(self)))
    }

    pub fn binding_action(&self, action: &str) -> bool {
        self.ivars()
            .surface
            .borrow()
            .as_ref()
            .is_some_and(|surface| surface.perform_binding_action(action))
    }

    pub fn update_scrollbar(&self, metrics: ScrollbarMetrics) {
        let cell_height = self
            .ivars()
            .surface
            .borrow()
            .as_ref()
            .map(|surface| f64::from(surface.size().cell_height_px) / self.backing_scale())
            .unwrap_or(0.0);
        if let Some(scrollbar) = self.ivars().scrollbar.borrow().as_ref() {
            scrollbar.update(metrics, cell_height);
        }
    }

    fn flash_scrollbar(&self) {
        if let Some(scrollbar) = self.ivars().scrollbar.borrow().as_ref() {
            scrollbar.flash();
        }
    }

    fn handle_scrollbar_mouse_down(&self, event: &NSEvent) -> bool {
        self.ivars()
            .scrollbar
            .borrow()
            .as_ref()
            .is_some_and(|scrollbar| scrollbar.handle_mouse_down(event))
    }

    pub fn has_selection(&self) -> bool {
        self.ivars()
            .surface
            .borrow()
            .as_ref()
            .is_some_and(GhosttySurface::has_selection)
    }

    pub fn read_selection(&self) -> Result<Option<String>, SurfaceTextError> {
        self.ivars()
            .surface
            .borrow()
            .as_ref()
            .map_or(Ok(None), |surface| {
                surface
                    .read_selection()
                    .map(|selection| selection.map(|selection| selection.text))
            })
    }

    pub fn send_text(&self, text: &str) -> bool {
        let surface = self.ivars().surface.borrow();
        let Some(surface) = surface.as_ref() else {
            return false;
        };
        surface.send_text(text);
        true
    }

    pub fn send_bytes(&self, bytes: &[u8]) -> bool {
        let surface = self.ivars().surface.borrow();
        let Some(surface) = surface.as_ref() else {
            return false;
        };
        surface.send_input_raw(bytes);
        true
    }

    pub fn read_screen_text(&self, last_lines: usize) -> Option<String> {
        self.ivars()
            .surface
            .borrow()
            .as_ref()?
            .read_screen_text(last_lines)
    }

    pub fn foreground_pid(&self) -> Option<u64> {
        self.ivars().surface.borrow().as_ref()?.foreground_pid()
    }

    pub fn resolve_clipboard_request(
        &self,
        token: ClipboardRequestToken,
        contents: Option<&str>,
        confirmed: bool,
    ) -> Result<(), HostViewError> {
        let surface = self.ivars().surface.borrow();
        let surface = surface.as_ref().ok_or(HostViewError::NoSurface)?;
        match surface.complete_clipboard_request(&token, contents.unwrap_or(""), confirmed) {
            Ok(()) => Ok(()),
            Err(ClipboardCompletionError::TextContainsNul) => {
                surface.complete_clipboard_request(&token, "", true)?;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn apply_runtime_mouse_cursor(&self, shape: MouseShape) {
        self.ivars().cursor.replace(cursor_for_shape(shape));
        self.apply_cursor();
    }

    pub fn apply_runtime_mouse_visibility(&self, visibility: MouseVisibility) {
        self.ivars()
            .requested_cursor_hidden
            .set(matches!(visibility, MouseVisibility::Hidden));
        self.sync_cursor_visibility();
    }

    pub fn color_scheme(&self) -> ColorScheme {
        self.ivars().color_scheme.get()
    }

    pub fn request_close(&self) {
        if let Some(surface) = self.ivars().surface.borrow().as_ref() {
            surface.request_close();
        }
    }

    pub fn needs_confirm_quit(&self) -> bool {
        self.ivars()
            .surface
            .borrow()
            .as_ref()
            .is_some_and(GhosttySurface::needs_confirm_quit)
    }

    pub fn update_config(&self, config: &GhosttyConfig) {
        if let Some(surface) = self.ivars().surface.borrow().as_ref() {
            surface.update_config(config);
        }
    }

    pub fn set_pointer_inside(&self, inside: bool) {
        if self.ivars().pointer_inside.replace(inside) == inside {
            return;
        }
        if inside {
            self.apply_cursor();
        }
        self.sync_cursor_visibility();
    }

    pub fn mouse_captured(&self) -> bool {
        self.ivars()
            .surface
            .borrow()
            .as_ref()
            .is_some_and(GhosttySurface::mouse_captured)
    }

    fn surface_point(&self, x: f64, y: f64) -> SurfacePoint {
        let origin = self.frame().origin;
        SurfacePoint {
            x: x - origin.x,
            y: y - origin.y,
        }
    }

    pub fn forward_pointer_position(&self, x: f64, y: f64, modifiers: Modifiers) {
        if self.ivars().overlay_active.get() && self.ivars().forwarded_mouse_buttons.get() == 0 {
            return;
        }
        let point = self.surface_point(x, y);
        if let Some(scrollbar) = self.ivars().scrollbar.borrow().as_ref() {
            scrollbar.extend_reveal(point.x, self.bounds().size.width);
        }
        if let Some(surface) = self.ivars().surface.borrow().as_ref() {
            surface.set_mouse_position(point, modifiers);
        }
    }

    pub fn forward_pointer_button(
        &self,
        x: f64,
        y: f64,
        state: MouseButtonState,
        button: MouseButton,
        modifiers: Modifiers,
    ) -> bool {
        let mask = mouse_button_mask(button);
        let was_forwarded = self.ivars().forwarded_mouse_buttons.get() & mask != 0;
        if (state == MouseButtonState::Release && !was_forwarded)
            || (self.ivars().overlay_active.get() && state == MouseButtonState::Press)
        {
            return false;
        }

        let point = self.surface_point(x, y);
        let consumed = self
            .ivars()
            .surface
            .borrow()
            .as_ref()
            .is_some_and(|surface| {
                surface.set_mouse_position(point, modifiers);
                surface.send_mouse_button(state, button, modifiers)
            });
        match state {
            MouseButtonState::Press => self
                .ivars()
                .forwarded_mouse_buttons
                .set(self.ivars().forwarded_mouse_buttons.get() | mask),
            MouseButtonState::Release => self
                .ivars()
                .forwarded_mouse_buttons
                .set(self.ivars().forwarded_mouse_buttons.get() & !mask),
        }
        consumed
    }

    fn backing_scale(&self) -> f64 {
        self.window()
            .map(|window| window.backingScaleFactor())
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .unwrap_or(1.0)
    }

    fn sync_surface_geometry(&self) {
        let bounds = self.bounds();
        if let Some(scrollbar) = self.ivars().scrollbar.borrow().as_ref() {
            scrollbar.layout(bounds);
        }
        let surface = self.ivars().surface.borrow();
        let Some(surface) = surface.as_ref() else {
            return;
        };
        let scale = self.backing_scale();
        surface.set_content_scale(scale, scale);
        surface.set_size(
            physical_pixels(bounds.size.width, scale),
            physical_pixels(bounds.size.height, scale),
        );
        if let Some(scrollbar) = self.ivars().scrollbar.borrow().as_ref() {
            scrollbar.update_cell_height(f64::from(surface.size().cell_height_px) / scale);
        }
        if let Some(display_id) = self.display_id() {
            surface.set_display_id(display_id);
        }
    }

    fn display_id(&self) -> Option<u32> {
        let screen = self.window()?.screen()?;
        let description = screen.deviceDescription();
        let key = objc2_foundation::NSString::from_str("NSScreenNumber");
        let value = description.objectForKey(&key)?;
        let number = value.downcast_ref::<objc2_foundation::NSNumber>()?;
        Some(number.unsignedIntValue())
    }

    fn sync_focus(&self) {
        let focused = !self.ivars().overlay_active.get()
            && self.ivars().window_active.get()
            && self.is_terminal_first_responder();
        if let Some(app) = self.ivars().app.borrow().as_ref() {
            app.set_focus(focused);
        }
        if let Some(surface) = self.ivars().surface.borrow().as_ref() {
            surface.set_focus(focused);
        }
    }

    fn claims_key_event(&self, event: &NSEvent) -> bool {
        if self.ivars().overlay_active.get() || self.ivars().surface.borrow().is_none() {
            return false;
        }
        if is_reserved_app_shortcut(event) || self.is_app_shortcut(event) {
            return false;
        }
        let same_window = objc2_foundation::MainThreadMarker::new()
            .and_then(|mtm| event.window(mtm))
            .zip(self.window())
            .is_some_and(|(event_window, host)| {
                Retained::as_ptr(&event_window).cast::<()>() == Retained::as_ptr(&host).cast::<()>()
            });
        same_window && self.is_terminal_first_responder()
    }

    fn is_terminal_first_responder(&self) -> bool {
        self.window().is_some_and(|window| {
            window.firstResponder().is_some_and(|responder| {
                let responder = Retained::as_ptr(&responder).cast::<()>();
                let view = (self as *const Self).cast_mut().cast::<()>();
                let input_context = self
                    .inputContext()
                    .map(|context| Retained::as_ptr(&context).cast::<()>());
                responder == view || input_context == Some(responder)
            })
        })
    }

    fn perform_key_equivalent_event(&self, event: &NSEvent) -> bool {
        if self.ivars().overlay_active.get()
            || event.r#type() != NSEventType::KeyDown
            || is_reserved_app_shortcut(event)
            || self.is_app_shortcut(event)
            || !has_action_modifier(event.modifierFlags().0)
            || !self.is_terminal_first_responder()
        {
            return false;
        }
        let action = if event.isARepeat() {
            KeyAction::Repeat
        } else {
            KeyAction::Press
        };
        let input = self.keyboard_input(event, action, None, false, Modifiers::NONE);
        let is_binding = self
            .ivars()
            .surface
            .borrow()
            .as_ref()
            .is_some_and(|surface| surface.key_binding(&input).is_some());
        if !is_binding {
            return false;
        }
        if self.forward_input(&input) && matches!(action, KeyAction::Press) {
            self.ivars()
                .forwarded_keys
                .borrow_mut()
                .insert(event.keyCode());
        }
        true
    }

    fn process_key_down(&self, event: &NSEvent) -> bool {
        if self.ivars().overlay_active.get()
            || is_reserved_app_shortcut(event)
            || self.is_app_shortcut(event)
        {
            return false;
        }
        if self.ivars().surface.borrow().is_none() {
            return false;
        }
        let action = if event.isARepeat() {
            KeyAction::Repeat
        } else {
            KeyAction::Press
        };
        let flags = event.modifierFlags().0;
        if flags & APPKIT_CONTROL != 0
            && flags & (APPKIT_COMMAND | APPKIT_OPTION) == 0
            && !self.has_marked_text_internal()
        {
            let text = event
                .charactersIgnoringModifiers()
                .map(|text| text.to_string());
            let forwarded =
                self.forward_physical_key(event, action, text.as_deref(), false, Modifiers::NONE);
            self.track_forwarded_key(event, action, forwarded);
            return forwarded;
        }
        if flags & APPKIT_COMMAND != 0 {
            let forwarded = self.forward_physical_key(event, action, None, false, Modifiers::NONE);
            self.track_forwarded_key(event, action, forwarded);
            return forwarded;
        }

        let had_marked_text = self.has_marked_text_internal();
        self.ivars().interpreting_key_event.set(true);
        self.ivars().key_text_accumulator.borrow_mut().clear();
        self.ivars().command_selector_called.set(false);
        let option_as_alt = self.translated_option_as_alt(event);
        if option_as_alt {
            if let Some(synthetic) = event_without_option(event) {
                let events = NSArray::from_slice(&[&*synthetic]);
                self.interpretKeyEvents(&events);
            } else {
                let events = NSArray::from_slice(&[event]);
                self.interpretKeyEvents(&events);
            }
        } else {
            let events = NSArray::from_slice(&[event]);
            self.interpretKeyEvents(&events);
        }
        self.ivars().interpreting_key_event.set(false);
        self.sync_preedit(had_marked_text);

        let command_called = self.ivars().command_selector_called.get();
        let accumulated = std::mem::take(&mut *self.ivars().key_text_accumulator.borrow_mut());
        let consumed = if command_called {
            Modifiers::NONE
        } else {
            consumed_modifiers(flags, !option_as_alt)
        };
        let forwarded = if accumulated.is_empty() {
            let composing = self.has_marked_text_internal() || had_marked_text;
            let text = (!composing)
                .then(|| event.characters().map(|text| text.to_string()))
                .flatten();
            self.forward_physical_key(event, action, text.as_deref(), composing, consumed)
        } else {
            let mut forwarded = false;
            for text in accumulated {
                let input = self
                    .keyboard_input(event, action, None, false, consumed)
                    .with_text(&text);
                if let Ok(input) = input {
                    forwarded |= self.forward_input(&input);
                }
            }
            forwarded
        };
        self.track_forwarded_key(event, action, forwarded);
        forwarded
    }

    fn track_forwarded_key(&self, event: &NSEvent, action: KeyAction, forwarded: bool) {
        if forwarded && matches!(action, KeyAction::Press) {
            self.ivars()
                .forwarded_keys
                .borrow_mut()
                .insert(event.keyCode());
        }
    }

    fn forward_physical_key(
        &self,
        event: &NSEvent,
        action: KeyAction,
        text: Option<&str>,
        composing: bool,
        consumed_modifiers: Modifiers,
    ) -> bool {
        let input = self.keyboard_input(event, action, text, composing, consumed_modifiers);
        self.forward_input(&input)
    }

    fn keyboard_input(
        &self,
        event: &NSEvent,
        action: KeyAction,
        text: Option<&str>,
        composing: bool,
        consumed_modifiers: Modifiers,
    ) -> KeyboardInput {
        let unshifted_codepoint = unshifted_codepoint(event);
        KeyboardInput::new(
            action,
            u32::from(event.keyCode()),
            modifiers_from_raw_appkit_flags(event.modifierFlags().0),
            unshifted_codepoint,
            text,
        )
        .with_consumed_modifiers(consumed_modifiers)
        .with_composing(composing)
    }

    fn forward_input(&self, input: &KeyboardInput) -> bool {
        let surface = self.ivars().surface.borrow();
        let Some(surface) = surface.as_ref() else {
            return false;
        };
        surface.send_key(input);
        true
    }

    fn translated_option_as_alt(&self, event: &NSEvent) -> bool {
        if event.modifierFlags().0 & APPKIT_OPTION == 0 {
            return false;
        }
        let original = modifiers_from_raw_appkit_flags(event.modifierFlags().0);
        self.ivars()
            .surface
            .borrow()
            .as_ref()
            .is_some_and(|surface| {
                surface.key_translation_modifiers(original).raw() & GHOSTTY_ALT == 0
            })
    }

    fn insert_text_value(&self, value: &AnyObject) {
        let text = text_from_object(value);
        self.clear_marked_text(true);
        if text.is_empty() {
            return;
        }
        if self.ivars().interpreting_key_event.get() {
            self.ivars().key_text_accumulator.borrow_mut().push(text);
            return;
        }
        if self.ivars().overlay_active.get() {
            return;
        }
        let input =
            KeyboardInput::new(KeyAction::Press, 0, Modifiers::NONE, 0, None).with_text(&text);
        if let Ok(input) = input {
            self.forward_input(&input);
        }
    }

    fn set_marked_text_value(&self, value: &AnyObject, selected_range: NSRange) {
        let text = text_from_object(value);
        let length = text.encode_utf16().count();
        self.ivars().marked_text.replace(text);
        self.ivars().marked_range.set(if length == 0 {
            not_found_range()
        } else {
            NSRange::new(0, length)
        });
        self.ivars()
            .selected_range
            .set(clamp_range(selected_range, length));
        if !self.ivars().interpreting_key_event.get() {
            self.sync_preedit(true);
        }
    }

    fn clear_marked_text(&self, sync: bool) {
        if !self.has_marked_text_internal() {
            return;
        }
        self.ivars().marked_text.borrow_mut().clear();
        self.ivars().marked_range.set(not_found_range());
        self.ivars().selected_range.set(empty_range());
        if sync {
            self.sync_preedit(true);
        }
    }

    fn has_marked_text_internal(&self) -> bool {
        self.ivars().marked_range.get().location != ns_not_found()
    }

    unsafe fn attributed_substring(
        &self,
        range: NSRange,
        actual_range: NSRangePointer,
    ) -> Option<Retained<NSAttributedString>> {
        let marked_range = self.ivars().marked_range.get();
        if !self.has_marked_text_internal() {
            if !actual_range.is_null() {
                unsafe { *actual_range = empty_range() };
            }
            return (range == empty_range()).then(|| attributed_string(""));
        }
        let safe_range = intersect_ranges(range, marked_range)?;
        if !actual_range.is_null() {
            unsafe { *actual_range = safe_range };
        }
        let marked_text = NSString::from_str(&self.ivars().marked_text.borrow());
        let substring = marked_text.substringWithRange(safe_range);
        Some(NSAttributedString::initWithString(
            NSAttributedString::alloc(),
            &substring,
        ))
    }

    fn sync_preedit(&self, clear_if_needed: bool) {
        let surface = self.ivars().surface.borrow();
        let Some(surface) = surface.as_ref() else {
            return;
        };
        let marked_text = self.ivars().marked_text.borrow();
        if self.has_marked_text_internal() && !marked_text.is_empty() {
            surface.set_preedit(Some(&marked_text));
        } else if clear_if_needed {
            surface.set_preedit(None);
        }
    }

    fn ime_screen_rect(&self) -> NSRect {
        let ime = self
            .ivars()
            .surface
            .borrow()
            .as_ref()
            .map(GhosttySurface::ime_rect)
            .unwrap_or_default();
        let local_top = NSPoint::new(ime.x, self.bounds().size.height - ime.y);
        let window_point = self.convertPoint_toView(local_top, None);
        let screen_point = self.window().map_or(window_point, |window| {
            window.convertPointToScreen(window_point)
        });
        NSRect::new(
            NSPoint::new(screen_point.x, screen_point.y - ime.height),
            NSSize::new(ime.width, ime.height),
        )
    }

    fn mouse_button_event(
        &self,
        event: &NSEvent,
        state: MouseButtonState,
        button: MouseButton,
        allow_context_fallback: bool,
    ) -> bool {
        let mask = mouse_button_mask(button);
        let was_forwarded = self.ivars().forwarded_mouse_buttons.get() & mask != 0;
        if !forwards_input_while_overlay(
            self.ivars().overlay_active.get(),
            matches!(state, MouseButtonState::Release),
            was_forwarded,
        ) {
            return false;
        }
        let point = self.mouse_point(event);
        let modifiers = modifiers_from_raw_appkit_flags(event.modifierFlags().0);
        let Some(consumed) = self.ivars().surface.borrow().as_ref().map(|surface| {
            surface.set_mouse_position(
                SurfacePoint {
                    x: point.x,
                    y: point.y,
                },
                modifiers,
            );
            surface.send_mouse_button(state, button, modifiers)
        }) else {
            return false;
        };
        match state {
            MouseButtonState::Press => self
                .ivars()
                .forwarded_mouse_buttons
                .set(self.ivars().forwarded_mouse_buttons.get() | mask),
            MouseButtonState::Release => self
                .ivars()
                .forwarded_mouse_buttons
                .set(self.ivars().forwarded_mouse_buttons.get() & !mask),
        }
        if allow_context_fallback && !consumed {
            self.emit_context_menu(point);
        }
        consumed
    }

    fn process_right_mouse_down(&self, event: &NSEvent) {
        self.ivars().forwarded_right_mouse_press.set(false);
        if self.ivars().overlay_active.get() {
            return;
        }
        let point = self.mouse_point(event);
        let modifiers = modifiers_from_raw_appkit_flags(event.modifierFlags().0);
        let shift_held = event.modifierFlags().contains(NSEventModifierFlags::Shift);
        let captured = self
            .ivars()
            .surface
            .borrow()
            .as_ref()
            .is_some_and(|surface| {
                surface.set_mouse_position(
                    SurfacePoint {
                        x: point.x,
                        y: point.y,
                    },
                    modifiers,
                );
                surface.mouse_captured()
            });
        if !forwards_right_mouse_button(captured, shift_held) {
            self.emit_context_menu(point);
            return;
        }
        let consumed =
            self.mouse_button_event(event, MouseButtonState::Press, MouseButton::Right, false);
        if consumed {
            self.ivars().forwarded_right_mouse_press.set(true);
        } else {
            self.mouse_button_event(event, MouseButtonState::Release, MouseButton::Right, false);
            self.emit_context_menu(point);
        }
    }

    fn process_right_mouse_up(&self, event: &NSEvent) {
        let forwarded = self.ivars().forwarded_right_mouse_press.replace(false);
        if !forwarded {
            return;
        }
        self.mouse_button_event(event, MouseButtonState::Release, MouseButton::Right, false);
    }

    fn forward_mouse_position(&self, event: &NSEvent) {
        if self.ivars().overlay_active.get() {
            return;
        }
        let point = self.mouse_point(event);
        let modifiers = modifiers_from_raw_appkit_flags(event.modifierFlags().0);
        if let Some(surface) = self.ivars().surface.borrow().as_ref() {
            surface.set_mouse_position(
                SurfacePoint {
                    x: point.x,
                    y: point.y,
                },
                modifiers,
            );
        }
    }

    fn mouse_point(&self, event: &NSEvent) -> HostViewPoint {
        let local = self.convertPoint_fromView(event.locationInWindow(), None);
        HostViewPoint {
            x: local.x,
            y: self.bounds().size.height - local.y,
        }
    }

    fn covers_event(&self, event: &NSEvent) -> bool {
        if self.isHidden() {
            return false;
        }
        let local = self.convertPoint_fromView(event.locationInWindow(), None);
        let bounds = self.bounds();
        local.x >= bounds.origin.x
            && local.y >= bounds.origin.y
            && local.x < bounds.origin.x + bounds.size.width
            && local.y < bounds.origin.y + bounds.size.height
    }

    fn emit_context_menu(&self, point: HostViewPoint) {
        let _ = self
            .ivars()
            .events
            .try_send(HostViewEvent::ContextMenu(point));
    }

    fn install_event_monitor(&self) {
        if self.ivars().event_monitor.borrow().is_some() {
            return;
        }
        let weak = Weak::new(self);
        let block = block2::RcBlock::new(move |event_pointer: NonNull<NSEvent>| -> *mut NSEvent {
            let event = unsafe { event_pointer.as_ref() };
            if let Some(this) = weak.load() {
                if event.r#type() == NSEventType::KeyDown {
                    if this.claims_key_event(event) && this.process_key_down(event) {
                        return std::ptr::null_mut();
                    }
                    if this.forward_app_shortcut(event) {
                        return std::ptr::null_mut();
                    }
                    return event_pointer.as_ptr();
                }
                if this.process_monitored_event(event) {
                    return std::ptr::null_mut();
                }
            }
            event_pointer.as_ptr()
        });
        let mask = NSEventMask::KeyDown
            | NSEventMask::LeftMouseDown
            | NSEventMask::ScrollWheel
            | NSEventMask::Pressure
            | NSEventMask::OtherMouseDown
            | NSEventMask::OtherMouseUp
            | NSEventMask::OtherMouseDragged;
        let monitor =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
        self.ivars().event_monitor.replace(monitor);
    }

    fn process_monitored_event(&self, event: &NSEvent) -> bool {
        let Some(mtm) = objc2_foundation::MainThreadMarker::new() else {
            return false;
        };
        let same_window = event
            .window(mtm)
            .zip(self.window())
            .is_some_and(|(event, host)| {
                Retained::as_ptr(&event).cast::<()>() == Retained::as_ptr(&host).cast::<()>()
            });
        if !same_window || !self.covers_event(event) {
            return false;
        }
        match event.r#type() {
            NSEventType::LeftMouseDown => {
                if !self.ivars().overlay_active.get() && self.handle_scrollbar_mouse_down(event) {
                    return true;
                }
            }
            NSEventType::ScrollWheel => {
                if self.ivars().overlay_active.get() {
                    return false;
                }
                self.forward_mouse_position(event);
                let metadata = ScrollMetadata::new(
                    event.hasPreciseScrollingDeltas(),
                    mouse_momentum(event.momentumPhase()),
                );
                if let Some(surface) = self.ivars().surface.borrow().as_ref() {
                    surface.send_mouse_scroll(
                        event.scrollingDeltaX(),
                        event.scrollingDeltaY(),
                        metadata,
                    );
                }
                self.flash_scrollbar();
            }
            NSEventType::Pressure => {
                if self.ivars().overlay_active.get() {
                    return false;
                }
                let stage = mouse_pressure_stage(event.stage());
                if let Some(surface) = self.ivars().surface.borrow().as_ref() {
                    surface.send_mouse_pressure(stage, f64::from(event.pressure()));
                }
            }
            NSEventType::OtherMouseDown | NSEventType::OtherMouseUp
                if navigation_button_event(event.buttonNumber()).is_some() =>
            {
                if event.r#type() == NSEventType::OtherMouseDown
                    && let Some(navigation) = navigation_button_event(event.buttonNumber())
                {
                    let _ = self.ivars().events.try_send(navigation);
                }
                return true;
            }
            NSEventType::OtherMouseDown | NSEventType::OtherMouseUp
                if event.buttonNumber() >= 5 =>
            {
                if let Some(button) = mouse_button(event.buttonNumber()) {
                    let state = if event.r#type() == NSEventType::OtherMouseDown {
                        MouseButtonState::Press
                    } else {
                        MouseButtonState::Release
                    };
                    self.mouse_button_event(event, state, button, false);
                }
            }
            NSEventType::OtherMouseDragged if event.buttonNumber() >= 5 => {
                self.forward_mouse_position(event);
            }
            _ => {}
        }
        false
    }

    fn install_tracking_area(&self) {
        if let Some(existing) = self.ivars().tracking_area.borrow_mut().take() {
            self.removeTrackingArea(&existing);
        }
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::MouseMoved
            | NSTrackingAreaOptions::CursorUpdate
            | NSTrackingAreaOptions::ActiveInKeyWindow
            | NSTrackingAreaOptions::InVisibleRect
            | NSTrackingAreaOptions::EnabledDuringMouseDrag;
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                self.bounds(),
                options,
                Some(self),
                None,
            )
        };
        self.addTrackingArea(&area);
        self.ivars().tracking_area.replace(Some(area));
    }

    fn apply_cursor(&self) {
        if self.ivars().pointer_inside.get() && !self.ivars().overlay_active.get() {
            self.ivars().cursor.borrow().set();
        }
    }

    fn sync_cursor_visibility(&self) {
        let should_hide = self.ivars().requested_cursor_hidden.get()
            && self.ivars().pointer_inside.get()
            && self.ivars().window_active.get()
            && !self.ivars().overlay_active.get();
        if self.ivars().cursor_hidden.replace(should_hide) == should_hide {
            return;
        }
        if should_hide {
            NSCursor::hide();
        } else {
            NSCursor::unhide();
        }
    }

    fn sync_color_scheme(&self, emit: bool) {
        let (dark_aqua, aqua) = unsafe { (NSAppearanceNameDarkAqua, NSAppearanceNameAqua) };
        let names: Retained<NSArray<NSAppearanceName>> = NSArray::from_slice(&[dark_aqua, aqua]);
        let scheme = if self
            .effectiveAppearance()
            .bestMatchFromAppearancesWithNames(&names)
            .is_some_and(|name| name.to_string() == dark_aqua.to_string())
        {
            ColorScheme::Dark
        } else {
            ColorScheme::Light
        };
        let changed = self.ivars().color_scheme.replace(scheme) != scheme;
        if let Some(app) = self.ivars().app.borrow().as_ref() {
            app.set_color_scheme(self.color_scheme());
        }
        if let Some(surface) = self.ivars().surface.borrow().as_ref() {
            surface.set_color_scheme(self.color_scheme());
        }
        if emit && changed {
            let _ = self
                .ivars()
                .events
                .try_send(HostViewEvent::Appearance(scheme));
        }
    }

    fn sync_input_source_observer(&self) {
        let should_observe = self.window().is_some();
        if self.ivars().observing_input_source.replace(should_observe) == should_observe {
            return;
        }
        let center = NSNotificationCenter::defaultCenter();
        if should_observe {
            unsafe {
                center.addObserver_selector_name_object(
                    self,
                    sel!(keyboardInputSourceChanged:),
                    Some(NSTextInputContextKeyboardSelectionDidChangeNotification),
                    None,
                )
            };
        } else {
            unsafe {
                center.removeObserver_name_object(
                    self,
                    Some(NSTextInputContextKeyboardSelectionDidChangeNotification),
                    None,
                )
            };
        }
    }
}

fn text_from_object(value: &AnyObject) -> String {
    if let Some(value) = value.downcast_ref::<NSString>() {
        value.to_string()
    } else if let Some(value) = value.downcast_ref::<NSAttributedString>() {
        value.string().to_string()
    } else {
        String::new()
    }
}

fn attributed_string(value: &str) -> Retained<NSAttributedString> {
    let value = NSString::from_str(value);
    NSAttributedString::initWithString(NSAttributedString::alloc(), &value)
}

fn modifiers_from_raw_appkit_flags(flags: usize) -> Modifiers {
    Modifiers::from_raw(ghostty_modifier_mask(flags))
}

fn unshifted_codepoint(event: &NSEvent) -> u32 {
    unshifted_ascii_for_keycode(event.keyCode())
        .or_else(|| {
            event
                .charactersIgnoringModifiers()
                .and_then(|characters| characters.to_string().chars().next())
        })
        .map_or(0, u32::from)
}

fn unshifted_ascii_for_keycode(keycode: u16) -> Option<char> {
    Some(match keycode {
        0 => 'a',
        1 => 's',
        2 => 'd',
        3 => 'f',
        4 => 'h',
        5 => 'g',
        6 => 'z',
        7 => 'x',
        8 => 'c',
        9 => 'v',
        11 => 'b',
        12 => 'q',
        13 => 'w',
        14 => 'e',
        15 => 'r',
        16 => 'y',
        17 => 't',
        18 => '1',
        19 => '2',
        20 => '3',
        21 => '4',
        22 => '6',
        23 => '5',
        24 => '=',
        25 => '9',
        26 => '7',
        27 => '-',
        28 => '8',
        29 => '0',
        30 => ']',
        31 => 'o',
        32 => 'u',
        33 => '[',
        34 => 'i',
        35 => 'p',
        37 => 'l',
        38 => 'j',
        39 => '\'',
        40 => 'k',
        41 => ';',
        42 => '\\',
        43 => ',',
        44 => '/',
        45 => 'n',
        46 => 'm',
        47 => '.',
        49 => ' ',
        50 => '`',
        65 => '.',
        67 => '*',
        69 => '+',
        75 => '/',
        78 => '-',
        81 => '=',
        82 => '0',
        83 => '1',
        84 => '2',
        85 => '3',
        86 => '4',
        87 => '5',
        88 => '6',
        89 => '7',
        91 => '8',
        92 => '9',
        _ => return None,
    })
}

fn ghostty_modifier_mask(flags: usize) -> u32 {
    let mut modifiers = 0;
    for (appkit, ghostty) in [
        (APPKIT_SHIFT, GHOSTTY_SHIFT),
        (APPKIT_CONTROL, GHOSTTY_CONTROL),
        (APPKIT_OPTION, GHOSTTY_ALT),
        (APPKIT_COMMAND, GHOSTTY_SUPER),
        (APPKIT_CAPS_LOCK, GHOSTTY_CAPS),
        (APPKIT_RIGHT_SHIFT, GHOSTTY_SHIFT_RIGHT),
        (APPKIT_RIGHT_CONTROL, GHOSTTY_CONTROL_RIGHT),
        (APPKIT_RIGHT_OPTION, GHOSTTY_ALT_RIGHT),
        (APPKIT_RIGHT_COMMAND, GHOSTTY_SUPER_RIGHT),
    ] {
        if flags & appkit != 0 {
            modifiers |= ghostty;
        }
    }
    modifiers
}

fn consumed_modifiers(flags: usize, consume_option: bool) -> Modifiers {
    let mut modifiers = 0;
    if flags & APPKIT_SHIFT != 0 {
        modifiers |= GHOSTTY_SHIFT;
    }
    if consume_option && flags & APPKIT_OPTION != 0 {
        modifiers |= GHOSTTY_ALT;
    }
    Modifiers::from_raw(modifiers)
}

fn modifier_key_is_pressed(keycode: u16, flags: usize) -> bool {
    match keycode {
        56 | 60 => flags & APPKIT_SHIFT != 0,
        58 | 61 => flags & APPKIT_OPTION != 0,
        59 | 62 => flags & APPKIT_CONTROL != 0,
        54 | 55 => flags & APPKIT_COMMAND != 0,
        57 => flags & APPKIT_CAPS_LOCK != 0,
        _ => false,
    }
}

fn has_action_modifier(flags: usize) -> bool {
    flags & (APPKIT_COMMAND | APPKIT_CONTROL | APPKIT_OPTION) != 0
}

fn is_reserved_app_shortcut(event: &NSEvent) -> bool {
    let flags =
        event.modifierFlags().0 & (APPKIT_SHIFT | APPKIT_CONTROL | APPKIT_OPTION | APPKIT_COMMAND);
    if flags != APPKIT_COMMAND {
        return false;
    }
    event
        .charactersIgnoringModifiers()
        .map(|characters| characters.to_string().to_ascii_lowercase())
        .is_some_and(|key| matches!(key.as_str(), "q" | "h" | "m" | ","))
}

fn event_without_option(event: &NSEvent) -> Option<Retained<NSEvent>> {
    let characters = event
        .charactersIgnoringModifiers()
        .unwrap_or_else(|| NSString::from_str(""));
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        event.r#type(),
        event.locationInWindow(),
        NSEventModifierFlags(event.modifierFlags().0 & !APPKIT_OPTION),
        event.timestamp(),
        event.windowNumber(),
        None,
        &characters,
        &characters,
        event.isARepeat(),
        event.keyCode(),
    )
}

fn mouse_button(number: isize) -> Option<MouseButton> {
    match number {
        0 => Some(MouseButton::Left),
        1 => Some(MouseButton::Right),
        2 => Some(MouseButton::Middle),
        3 => Some(MouseButton::Four),
        4 => Some(MouseButton::Five),
        5 => Some(MouseButton::Six),
        6 => Some(MouseButton::Seven),
        7 => Some(MouseButton::Eight),
        8 => Some(MouseButton::Nine),
        9 => Some(MouseButton::Ten),
        10 => Some(MouseButton::Eleven),
        _ => None,
    }
}

fn navigation_button_event(number: isize) -> Option<HostViewEvent> {
    match number {
        3 => Some(HostViewEvent::NavigateBack),
        4 => Some(HostViewEvent::NavigateForward),
        _ => None,
    }
}

fn mouse_button_mask(button: MouseButton) -> u16 {
    let index = match button {
        MouseButton::Unknown => return 0,
        MouseButton::Left => 1,
        MouseButton::Right => 2,
        MouseButton::Middle => 3,
        MouseButton::Four => 4,
        MouseButton::Five => 5,
        MouseButton::Six => 6,
        MouseButton::Seven => 7,
        MouseButton::Eight => 8,
        MouseButton::Nine => 9,
        MouseButton::Ten => 10,
        MouseButton::Eleven => 11,
    };
    1 << index
}

fn mouse_pressure_stage(stage: isize) -> MousePressureStage {
    match stage {
        1 => MousePressureStage::Normal,
        2 => MousePressureStage::Deep,
        _ => MousePressureStage::None,
    }
}

fn mouse_momentum(phase: NSEventPhase) -> MouseMomentum {
    if phase.contains(NSEventPhase::Began) {
        MouseMomentum::Began
    } else if phase.contains(NSEventPhase::Stationary) {
        MouseMomentum::Stationary
    } else if phase.contains(NSEventPhase::Changed) {
        MouseMomentum::Changed
    } else if phase.contains(NSEventPhase::Ended) {
        MouseMomentum::Ended
    } else if phase.contains(NSEventPhase::Cancelled) {
        MouseMomentum::Cancelled
    } else if phase.contains(NSEventPhase::MayBegin) {
        MouseMomentum::MayBegin
    } else {
        MouseMomentum::None
    }
}

fn forwards_right_mouse_button(captured: bool, shift_held: bool) -> bool {
    captured && !shift_held
}

fn forwards_input_while_overlay(overlay_active: bool, release: bool, was_forwarded: bool) -> bool {
    (!release && !overlay_active) || (release && was_forwarded)
}

fn empty_range() -> NSRange {
    NSRange::new(0, 0)
}

fn ns_not_found() -> usize {
    NSNotFound as usize
}

fn not_found_range() -> NSRange {
    NSRange::new(ns_not_found(), 0)
}

fn clamp_range(range: NSRange, utf16_length: usize) -> NSRange {
    if range.location == ns_not_found() {
        return empty_range();
    }
    let location = range.location.min(utf16_length);
    NSRange::new(
        location,
        range.length.min(utf16_length.saturating_sub(location)),
    )
}

fn intersect_ranges(left: NSRange, right: NSRange) -> Option<NSRange> {
    if left.location == ns_not_found() || right.location == ns_not_found() {
        return None;
    }
    let start = left.location.max(right.location);
    let end = left
        .location
        .saturating_add(left.length)
        .min(right.location.saturating_add(right.length));
    (start <= end).then(|| NSRange::new(start, end - start))
}

fn cursor_for_shape(shape: MouseShape) -> Retained<NSCursor> {
    match shape {
        MouseShape::Default | MouseShape::Help | MouseShape::Progress | MouseShape::Wait => {
            NSCursor::arrowCursor()
        }
        MouseShape::ContextMenu => NSCursor::contextualMenuCursor(),
        MouseShape::Pointer => NSCursor::pointingHandCursor(),
        MouseShape::Cell | MouseShape::Crosshair => NSCursor::crosshairCursor(),
        MouseShape::Text => NSCursor::IBeamCursor(),
        MouseShape::VerticalText => NSCursor::IBeamCursorForVerticalLayout(),
        MouseShape::Alias => NSCursor::dragLinkCursor(),
        MouseShape::Copy => NSCursor::dragCopyCursor(),
        MouseShape::Move | MouseShape::Grab | MouseShape::AllScroll => NSCursor::openHandCursor(),
        MouseShape::NoDrop | MouseShape::NotAllowed => NSCursor::operationNotAllowedCursor(),
        MouseShape::Grabbing => NSCursor::closedHandCursor(),
        MouseShape::ColumnResize
        | MouseShape::EastResize
        | MouseShape::WestResize
        | MouseShape::EastWestResize => NSCursor::columnResizeCursor(),
        MouseShape::RowResize
        | MouseShape::NorthResize
        | MouseShape::SouthResize
        | MouseShape::NorthSouthResize => NSCursor::rowResizeCursor(),
        MouseShape::NorthEastResize
        | MouseShape::NorthWestResize
        | MouseShape::SouthEastResize
        | MouseShape::SouthWestResize
        | MouseShape::NorthEastSouthWestResize
        | MouseShape::NorthWestSouthEastResize => NSCursor::crosshairCursor(),
        MouseShape::ZoomIn => NSCursor::zoomInCursor(),
        MouseShape::ZoomOut => NSCursor::zoomOutCursor(),
    }
}

fn physical_pixels(points: f64, scale: f64) -> u32 {
    let pixels = (points.max(0.0) * scale).round();
    if !pixels.is_finite() || pixels <= 0.0 {
        0
    } else if pixels >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        pixels as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_pixel_conversion_scales_rounds_and_clamps() {
        assert_eq!(physical_pixels(100.25, 2.0), 201);
        assert_eq!(physical_pixels(-1.0, 2.0), 0);
        assert_eq!(physical_pixels(f64::INFINITY, 2.0), 0);
    }

    #[test]
    fn modifier_conversion_preserves_lock_and_right_side_bits() {
        let flags = APPKIT_SHIFT
            | APPKIT_OPTION
            | APPKIT_CAPS_LOCK
            | APPKIT_RIGHT_SHIFT
            | APPKIT_RIGHT_OPTION;
        assert_eq!(
            ghostty_modifier_mask(flags),
            GHOSTTY_SHIFT | GHOSTTY_ALT | GHOSTTY_CAPS | GHOSTTY_SHIFT_RIGHT | GHOSTTY_ALT_RIGHT
        );
    }

    #[test]
    fn unshifted_ascii_uses_physical_key_positions() {
        assert_eq!(unshifted_ascii_for_keycode(18), Some('1'));
        assert_eq!(unshifted_ascii_for_keycode(24), Some('='));
        assert_eq!(unshifted_ascii_for_keycode(83), Some('1'));
        assert_eq!(unshifted_ascii_for_keycode(127), None);
    }

    #[test]
    fn modifier_transition_uses_physical_key_and_current_flags() {
        assert!(modifier_key_is_pressed(60, APPKIT_SHIFT));
        assert!(!modifier_key_is_pressed(60, 0));
        assert!(modifier_key_is_pressed(57, APPKIT_CAPS_LOCK));
        assert!(!modifier_key_is_pressed(1, APPKIT_COMMAND));
    }

    #[test]
    fn marked_ranges_clamp_and_intersect_without_overflow() {
        assert_eq!(clamp_range(NSRange::new(3, 9), 5), NSRange::new(3, 2));
        assert_eq!(clamp_range(not_found_range(), 5), empty_range());
        assert_eq!(
            intersect_ranges(NSRange::new(2, usize::MAX), NSRange::new(4, 3)),
            Some(NSRange::new(4, 3))
        );
        assert_eq!(
            intersect_ranges(NSRange::new(0, 1), NSRange::new(2, 1)),
            None
        );
    }

    #[test]
    fn appkit_button_numbers_map_through_button_eleven() {
        assert_eq!(mouse_button(0), Some(MouseButton::Left));
        assert_eq!(mouse_button(2), Some(MouseButton::Middle));
        assert_eq!(mouse_button(10), Some(MouseButton::Eleven));
        assert_eq!(mouse_button(11), None);
        assert_eq!(mouse_button(-1), None);
    }

    #[test]
    fn appkit_navigation_buttons_are_application_events() {
        assert_eq!(
            navigation_button_event(3),
            Some(HostViewEvent::NavigateBack)
        );
        assert_eq!(
            navigation_button_event(4),
            Some(HostViewEvent::NavigateForward)
        );
        assert_eq!(navigation_button_event(2), None);
        assert_eq!(navigation_button_event(5), None);
    }

    #[test]
    fn pressure_stages_clamp_unknown_values_to_none() {
        assert_eq!(mouse_pressure_stage(0), MousePressureStage::None);
        assert_eq!(mouse_pressure_stage(1), MousePressureStage::Normal);
        assert_eq!(mouse_pressure_stage(2), MousePressureStage::Deep);
        assert_eq!(mouse_pressure_stage(3), MousePressureStage::None);
    }

    #[test]
    fn momentum_phases_preserve_each_appkit_value() {
        assert_eq!(mouse_momentum(NSEventPhase::Began), MouseMomentum::Began);
        assert_eq!(
            mouse_momentum(NSEventPhase::Stationary),
            MouseMomentum::Stationary
        );
        assert_eq!(
            mouse_momentum(NSEventPhase::Changed),
            MouseMomentum::Changed
        );
        assert_eq!(mouse_momentum(NSEventPhase::Ended), MouseMomentum::Ended);
        assert_eq!(
            mouse_momentum(NSEventPhase::Cancelled),
            MouseMomentum::Cancelled
        );
        assert_eq!(
            mouse_momentum(NSEventPhase::MayBegin),
            MouseMomentum::MayBegin
        );
        assert_eq!(mouse_momentum(NSEventPhase::None), MouseMomentum::None);
    }

    #[test]
    fn captured_right_click_honors_shift_escape() {
        assert!(forwards_right_mouse_button(true, false));
        assert!(!forwards_right_mouse_button(true, true));
        assert!(!forwards_right_mouse_button(false, false));
    }

    #[test]
    fn overlays_suppress_new_input_but_preserve_matching_releases() {
        assert!(forwards_input_while_overlay(false, false, false));
        assert!(!forwards_input_while_overlay(false, true, false));
        assert!(!forwards_input_while_overlay(true, false, false));
        assert!(!forwards_input_while_overlay(true, true, false));
        assert!(forwards_input_while_overlay(true, true, true));
    }
}
