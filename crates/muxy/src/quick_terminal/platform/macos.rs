use crate::quick_terminal::panel::{
    AccessibilityPreferences, PanelGeometryRequest, QuickTerminalConfiguration,
    resolve_panel_geometry,
};
use crate::quick_terminal::platform::SystemMutation;
use crate::quick_terminal::shortcut_service::{
    EventTapRecovery, MonitoringState, ShortcutBackend, ShortcutBackendFactory, event_tap_recovery,
};
use crate::quick_terminal::view::{AccessibilityNode, AccessibilityRole};
use crate::quick_terminal::{ShortcutCapture, ShortcutRecordingEvent};
use block2::RcBlock;
use gpui::{Window, px, size};
use muxy_core::quick_terminal::geometry::{Point, Rect};
use muxy_core::quick_terminal::{DoubleShiftDetector, DoubleShiftInput, QuickTerminalShortcut};
use muxy_core::shortcuts::{COMMAND, CONTROL, KeyCombo, OPTION, SHIFT};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, ProtocolObject, Sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityButtonRole, NSAccessibilityElement, NSAccessibilityGroupRole,
    NSAccessibilityPostNotification, NSAccessibilityStaticTextRole,
    NSAccessibilityValueChangedNotification, NSApplication, NSApplicationActivationOptions,
    NSApplicationDidChangeScreenParametersNotification, NSColor, NSEvent, NSEventMask,
    NSEventModifierFlags, NSEventType, NSRunningApplication, NSScreen, NSStatusWindowLevel,
    NSTextInputContextKeyboardSelectionDidChangeNotification, NSView, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask, NSWorkspace,
    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
};
use objc2_core_foundation::{
    CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes,
};
use objc2_core_graphics::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
    CGPreflightListenEventAccess, CGRequestListenEventAccess,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSNotification, NSNotificationCenter, NSObjectProtocol,
    NSOperationQueue, NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::{CACornerMask, CALayer, CATransaction, kCACornerCurveContinuous};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::OnceLock;

struct SystemObserver {
    center: Retained<NSNotificationCenter>,
    token: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Drop for SystemObserver {
    fn drop(&mut self) {
        unsafe {
            let _: () = objc2::msg_send![&*self.center, removeObserver: &*self.token];
        }
    }
}

pub struct SystemObservers {
    _observers: Vec<SystemObserver>,
}

impl SystemObservers {
    pub fn start(sender: async_channel::Sender<SystemMutation>) -> Result<Self, String> {
        let default_center = NSNotificationCenter::defaultCenter();
        let workspace_center = NSWorkspace::sharedWorkspace().notificationCenter();
        let observers = unsafe {
            vec![
                system_observer(
                    workspace_center,
                    NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification,
                    sender.clone(),
                    SystemMutation::Accessibility,
                ),
                system_observer(
                    default_center.clone(),
                    NSTextInputContextKeyboardSelectionDidChangeNotification,
                    sender.clone(),
                    SystemMutation::KeyboardLayout,
                ),
                system_observer(
                    default_center,
                    NSApplicationDidChangeScreenParametersNotification,
                    sender,
                    SystemMutation::Screens,
                ),
            ]
        };
        Ok(Self {
            _observers: observers,
        })
    }
}

fn system_observer(
    center: Retained<NSNotificationCenter>,
    name: &objc2_foundation::NSNotificationName,
    sender: async_channel::Sender<SystemMutation>,
    mutation: SystemMutation,
) -> SystemObserver {
    let block = RcBlock::new(move |_: NonNull<NSNotification>| {
        let _ = sender.try_send(mutation);
    });
    let queue = NSOperationQueue::mainQueue();
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(name), None, Some(&queue), &block)
    };
    SystemObserver { center, token }
}

pub struct MacShortcutBackendFactory {
    next_identifier: u32,
}

impl MacShortcutBackendFactory {
    pub fn new() -> Self {
        Self { next_identifier: 1 }
    }

    fn take_identifier(&mut self) -> u32 {
        let identifier = self.next_identifier;
        self.next_identifier = self.next_identifier.wrapping_add(1).max(1);
        identifier
    }
}

impl ShortcutBackendFactory for MacShortcutBackendFactory {
    fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>> {
        match shortcut {
            QuickTerminalShortcut::Unassigned => None,
            QuickTerminalShortcut::DoubleShift => Some(Box::new(DoubleShiftBackend::new())),
            QuickTerminalShortcut::KeyCombo { .. } => {
                shortcut.registration_identity().map(|identity| {
                    Box::new(CarbonHotKeyBackend::new(identity, self.take_identifier())) as Box<_>
                })
            }
        }
    }

    fn request_input_monitoring_access(&mut self) -> bool {
        CGRequestListenEventAccess()
    }
}

pub fn resolve_key(virtual_key_code: u16) -> Option<String> {
    if virtual_key_code > 127 {
        return None;
    }
    if let Some(name) = special_key_name(virtual_key_code) {
        return Some(name.to_owned());
    }
    let event = CGEvent::new_keyboard_event(None, virtual_key_code, true)?;
    let event = NSEvent::eventWithCGEvent(&event)?;
    let key = event.charactersIgnoringModifiers()?.to_string();
    let key = muxy_core::shortcuts::canonical_key(&key);
    (!key.is_empty()).then_some(key)
}

pub struct ShortcutRecorder {
    monitor: Option<Retained<AnyObject>>,
}

impl ShortcutRecorder {
    pub fn start(sender: async_channel::Sender<ShortcutRecordingEvent>) -> Result<Self, String> {
        let block = RcBlock::new(move |event_pointer: NonNull<NSEvent>| -> *mut NSEvent {
            let event = unsafe { event_pointer.as_ref() };
            let virtual_key_code = event.keyCode();
            if virtual_key_code == 53 {
                let _ = sender.try_send(ShortcutRecordingEvent::Cancelled);
                return std::ptr::null_mut();
            }
            let flags = NSEventModifierFlags(
                event.modifierFlags().0 & NSEventModifierFlags::DeviceIndependentFlagsMask.0,
            );
            let mut modifiers = 0;
            if flags.contains(NSEventModifierFlags::Control) {
                modifiers |= CONTROL;
            }
            if flags.contains(NSEventModifierFlags::Option) {
                modifiers |= OPTION;
            }
            if flags.contains(NSEventModifierFlags::Shift) {
                modifiers |= SHIFT;
            }
            if flags.contains(NSEventModifierFlags::Command) {
                modifiers |= COMMAND;
            }
            let Some(key) = resolve_key(virtual_key_code) else {
                let _ = sender.try_send(ShortcutRecordingEvent::Rejected(
                    "Unsupported physical key".to_owned(),
                ));
                return std::ptr::null_mut();
            };
            let combo = KeyCombo::new(&key, modifiers);
            if !combo.is_supported_shortcut() {
                let _ = sender.try_send(ShortcutRecordingEvent::Rejected(
                    "Use Command, Control, or Option with a supported key".to_owned(),
                ));
                return std::ptr::null_mut();
            }
            let _ = sender.try_send(ShortcutRecordingEvent::Captured(ShortcutCapture {
                combo,
                virtual_key_code,
            }));
            std::ptr::null_mut()
        });
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &block)
        }
        .ok_or_else(|| "failed to install Quick Terminal shortcut recorder".to_owned())?;
        Ok(Self {
            monitor: Some(monitor),
        })
    }
}

impl Drop for ShortcutRecorder {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    }
}

fn special_key_name(virtual_key_code: u16) -> Option<&'static str> {
    match virtual_key_code {
        36 | 76 => Some("return"),
        48 => Some("tab"),
        49 => Some("space"),
        123 => Some("leftarrow"),
        124 => Some("rightarrow"),
        125 => Some("downarrow"),
        126 => Some("uparrow"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PanelProperties {
    pub borderless: bool,
    pub nonactivating: bool,
    pub status_level: bool,
    pub joins_all_spaces: bool,
    pub full_screen_auxiliary: bool,
    pub ignores_cycle: bool,
    pub floating: bool,
    pub visible_on_deactivate: bool,
    pub movable: bool,
    pub transparent: bool,
    pub key_capable: bool,
    pub main_capable: bool,
}

impl PanelProperties {
    pub fn satisfies_contract(self) -> bool {
        self.borderless
            && self.nonactivating
            && self.status_level
            && self.joins_all_spaces
            && self.full_screen_auxiliary
            && self.ignores_cycle
            && self.floating
            && self.visible_on_deactivate
            && !self.movable
            && self.transparent
            && self.key_capable
            && !self.main_capable
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PanelTelemetry {
    pub screen_index: usize,
    pub screen_name: String,
    pub frame: Rect,
    pub collapsed_cutout: Option<Rect>,
    pub active_space_intent: bool,
}

struct FocusSnapshot {
    window: Option<Retained<NSWindow>>,
    application: Option<Retained<NSRunningApplication>>,
}

pub struct PanelAdapter {
    window: Retained<NSWindow>,
    original_window_class: &'static AnyClass,
    view: Retained<NSView>,
    mask_host: Retained<NSView>,
    reveal_mask: Retained<CALayer>,
    target_frame: Option<NSRect>,
    collapsed_mask_frame: Option<NSRect>,
    collapsed_mask_radius: f64,
    telemetry: Option<PanelTelemetry>,
    focus_snapshot: Option<FocusSnapshot>,
    accessibility_children: Vec<Retained<AnyObject>>,
}

impl PanelAdapter {
    pub fn configure(window: &Window) -> Result<Self, String> {
        MainThreadMarker::new().ok_or_else(|| {
            "Quick Terminal panel must be configured on the main thread".to_owned()
        })?;
        let handle = HasWindowHandle::window_handle(window)
            .map_err(|error| format!("GPUI did not provide a native panel handle: {error}"))?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("GPUI Quick Terminal window is not backed by AppKit".to_owned());
        };
        let view =
            unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<objc2_app_kit::NSView>()) }
                .ok_or_else(|| "GPUI Quick Terminal view is unavailable".to_owned())?;
        let native_window = view
            .window()
            .ok_or_else(|| "GPUI Quick Terminal view is detached".to_owned())?;
        let original_window_class = native_window.class();
        let class = panel_class(original_window_class)?;
        native_window.setStyleMask(NSWindowStyleMask::NonactivatingPanel);
        native_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        native_window.setHidesOnDeactivate(false);
        native_window.setMovable(false);
        native_window.setOpaque(false);
        native_window.setBackgroundColor(Some(&NSColor::clearColor()));
        native_window.setHasShadow(true);
        native_window.setExcludedFromWindowsMenu(true);
        let _: () = unsafe { objc2::msg_send![&native_window, setFloatingPanel: true] };
        native_window.setLevel(NSStatusWindowLevel);
        unsafe {
            AnyObject::set_class(&native_window, class);
        }
        let mask_host = unsafe { view.superview() }
            .ok_or_else(|| "GPUI Quick Terminal content view is detached".to_owned())?;
        mask_host.setWantsLayer(true);
        let layer = mask_host.layer().ok_or_else(|| {
            "GPUI Quick Terminal content view has no Core Animation layer".to_owned()
        })?;
        let reveal_mask = CALayer::layer();
        reveal_mask.setBackgroundColor(Some(&NSColor::whiteColor().CGColor()));
        reveal_mask.setCornerCurve(unsafe { kCACornerCurveContinuous });
        reveal_mask.setMaskedCorners(all_mask_corners());
        set_mask_frame(
            &reveal_mask,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            14.0,
            false,
            std::time::Duration::ZERO,
        );
        unsafe {
            layer.setMask(Some(&reveal_mask));
        }
        let adapter = Self {
            window: native_window,
            original_window_class,
            view,
            mask_host,
            reveal_mask,
            target_frame: None,
            collapsed_mask_frame: None,
            collapsed_mask_radius: 14.0,
            telemetry: None,
            focus_snapshot: None,
            accessibility_children: Vec::new(),
        };
        let properties = adapter.properties();
        if !properties.satisfies_contract() {
            return Err(format!(
                "GPUI panel property readback did not satisfy the contract: {properties:?}"
            ));
        }
        Ok(adapter)
    }

    pub fn prepare(
        &mut self,
        configuration: QuickTerminalConfiguration,
        window: &mut Window,
    ) -> Result<PanelTelemetry, String> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| "Quick Terminal geometry requires the main thread".to_owned())?;
        let screens = NSScreen::screens(mtm);
        let frames = screens
            .iter()
            .map(|screen| rect_from_ns(screen.frame()))
            .collect::<Vec<_>>();
        let visible_frames = screens
            .iter()
            .map(|screen| rect_from_ns(screen.visibleFrame()))
            .collect::<Vec<_>>();
        let application = NSApplication::sharedApplication(mtm);
        let key_window = application
            .keyWindow()
            .and_then(|window| window.screen())
            .and_then(|screen| screen_index(&screens, &screen));
        let main_window = application
            .mainWindow()
            .and_then(|window| window.screen())
            .and_then(|screen| screen_index(&screens, &screen));
        let main_screen =
            NSScreen::mainScreen(mtm).and_then(|screen| screen_index(&screens, &screen));
        let mouse = NSEvent::mouseLocation();
        let selected_index = muxy_core::quick_terminal::geometry::preferred_screen_index(
            Point {
                x: mouse.x,
                y: mouse.y,
            },
            &frames,
            key_window,
            main_window,
            main_screen,
        )
        .ok_or_else(|| "no display is available for Quick Terminal".to_owned())?;
        if selected_index >= screens.len() {
            return Err("selected Quick Terminal display disappeared".to_owned());
        }
        let selected = screens.objectAtIndex(selected_index);
        let insets = selected.safeAreaInsets();
        let left = selected.auxiliaryTopLeftArea().size.width;
        let right = selected.auxiliaryTopRightArea().size.width;
        let (_, frame, collapsed_cutout) = resolve_panel_geometry(PanelGeometryRequest {
            mouse: Point {
                x: mouse.x,
                y: mouse.y,
            },
            screens: &frames,
            visible_frames: &visible_frames,
            key_window: Some(selected_index),
            main_window: None,
            main_screen: None,
            preferred_size: configuration.preferred_size(),
            safe_area_top: insets.top,
            auxiliary_widths: (insets.top > 0.0).then_some((left, right)),
        })
        .ok_or_else(|| "failed to resolve Quick Terminal geometry".to_owned())?;
        let (collapsed, collapsed_radius) = collapsed_cutout.map_or_else(
            || {
                let width = frame.size.width.min(180.0);
                (
                    Rect::new(
                        (frame.size.width - width) / 2.0,
                        (frame.size.height - 34.0).max(0.0),
                        width,
                        frame.size.height.min(34.0),
                    ),
                    14.0,
                )
            },
            |collapsed| {
                let radius = 12.0_f64.min(collapsed.size.height / 2.0);
                (collapsed, radius)
            },
        );
        let target_frame = rect_to_ns(frame);
        self.target_frame = Some(target_frame);
        self.collapsed_mask_frame = Some(rect_to_ns(collapsed));
        self.collapsed_mask_radius = collapsed_radius;
        let target_size = size(px(frame.size.width as f32), px(frame.size.height as f32));
        if window.viewport_size() != target_size {
            window.resize(target_size);
        }
        self.apply_target_origin();
        if !self.window.isVisible() {
            set_mask_frame(
                &self.reveal_mask,
                rect_to_ns(collapsed),
                collapsed_radius,
                false,
                std::time::Duration::ZERO,
            );
        }
        let telemetry = PanelTelemetry {
            screen_index: selected_index,
            screen_name: selected.localizedName().to_string(),
            frame,
            collapsed_cutout,
            active_space_intent: true,
        };
        self.telemetry = Some(telemetry.clone());
        Ok(telemetry)
    }

    pub fn apply_target_origin(&self) {
        if let Some(frame) = self.target_frame {
            self.window.setFrameOrigin(frame.origin);
        }
    }

    pub fn show(&mut self, duration: std::time::Duration) {
        if crate::quick_terminal::panel::capture_focus(
            self.focus_snapshot.is_some(),
            self.window.isKeyWindow(),
        ) {
            self.focus_snapshot = Some(FocusSnapshot::capture(&self.window));
        }
        if !self.window.isVisible()
            && let Some(frame) = self.collapsed_mask_frame
        {
            set_mask_frame(
                &self.reveal_mask,
                frame,
                self.collapsed_mask_radius,
                false,
                std::time::Duration::ZERO,
            );
            CATransaction::flush();
        }
        self.window.orderFrontRegardless();
        self.window.makeKeyAndOrderFront(None);
        self.mask_host.layoutSubtreeIfNeeded();
        self.mask_host.displayIfNeeded();
        CATransaction::flush();
        if self.target_frame.is_some() {
            self.reveal_mask.setMaskedCorners(all_mask_corners());
            set_mask_frame(
                &self.reveal_mask,
                self.mask_host.bounds(),
                20.0,
                true,
                duration,
            );
            self.window.invalidateShadow();
        }
    }

    pub fn begin_hide(&self, duration: std::time::Duration) {
        if let Some(frame) = self.collapsed_mask_frame {
            set_mask_frame(
                &self.reveal_mask,
                frame,
                self.collapsed_mask_radius,
                false,
                duration,
            );
        }
    }

    pub fn finish_hide(&mut self, restores_focus: bool) {
        let should_restore =
            crate::quick_terminal::panel::restore_focus(restores_focus, self.window.isKeyWindow());
        self.window.orderOut(None);
        self.reveal_mask.setMaskedCorners(all_mask_corners());
        if should_restore {
            if let Some(snapshot) = self.focus_snapshot.take() {
                snapshot.restore();
            }
        } else {
            self.focus_snapshot = None;
        }
    }

    pub fn telemetry(&self) -> Option<&PanelTelemetry> {
        self.telemetry.as_ref()
    }

    pub fn is_key(&self) -> bool {
        self.window.isKeyWindow()
    }

    pub fn native_frame(&self) -> Rect {
        rect_from_ns(self.window.frame())
    }

    pub fn install_accessibility(&mut self, nodes: &[AccessibilityNode]) {
        let frame = self.window.frame();
        let children = nodes
            .iter()
            .map(|node| {
                let label = NSString::from_str(&node.label);
                let role = unsafe {
                    match node.role {
                        AccessibilityRole::Group => NSAccessibilityGroupRole,
                        AccessibilityRole::Status => NSAccessibilityStaticTextRole,
                        AccessibilityRole::Button => NSAccessibilityButtonRole,
                    }
                };
                let element = unsafe {
                    NSAccessibilityElement::accessibilityElementWithRole_frame_label_parent(
                        role,
                        frame,
                        Some(&label),
                        Some(&self.view),
                    )
                };
                if !node.value.is_empty() {
                    let value = NSString::from_str(&node.value);
                    let _: () =
                        unsafe { objc2::msg_send![&element, setAccessibilityValue: &*value] };
                }
                if node.announces_changes {
                    unsafe {
                        NSAccessibilityPostNotification(
                            &element,
                            NSAccessibilityValueChangedNotification,
                        )
                    };
                }
                element
            })
            .collect::<Vec<_>>();
        let array = NSArray::from_retained_slice(&children);
        unsafe {
            self.view.setAccessibilityChildren(Some(&array));
        }
        self.accessibility_children = children;
    }

    pub fn is_visible(&self) -> bool {
        self.window.isVisible()
    }

    pub fn properties(&self) -> PanelProperties {
        let style = self.window.styleMask();
        let behavior = self.window.collectionBehavior();
        let floating: bool = unsafe { objc2::msg_send![&self.window, isFloatingPanel] };
        PanelProperties {
            borderless: !style.intersects(
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Resizable,
            ),
            nonactivating: style.contains(NSWindowStyleMask::NonactivatingPanel),
            status_level: self.window.level() == NSStatusWindowLevel,
            joins_all_spaces: behavior.contains(NSWindowCollectionBehavior::CanJoinAllSpaces),
            full_screen_auxiliary: behavior
                .contains(NSWindowCollectionBehavior::FullScreenAuxiliary),
            ignores_cycle: behavior.contains(NSWindowCollectionBehavior::IgnoresCycle),
            floating,
            visible_on_deactivate: !self.window.hidesOnDeactivate(),
            movable: self.window.isMovable(),
            transparent: !self.window.isOpaque(),
            key_capable: self.window.canBecomeKeyWindow(),
            main_capable: self.window.canBecomeMainWindow(),
        }
    }
}

impl Drop for PanelAdapter {
    fn drop(&mut self) {
        unsafe {
            AnyObject::set_class(&self.window, self.original_window_class);
        }
    }
}

impl FocusSnapshot {
    fn capture(panel: &NSWindow) -> Self {
        let mtm = MainThreadMarker::new().expect("focus capture requires the main thread");
        let application = NSApplication::sharedApplication(mtm);
        let window = application
            .keyWindow()
            .filter(|window| !std::ptr::eq(&**window, panel));
        Self {
            window,
            application: NSWorkspace::sharedWorkspace().frontmostApplication(),
        }
    }

    fn restore(self) {
        let Some(application) = self
            .application
            .filter(|application| !application.isTerminated())
        else {
            return;
        };
        if application.processIdentifier() == std::process::id() as i32 {
            if let Some(window) = self.window.filter(|window| window.isVisible()) {
                window.makeKeyAndOrderFront(None);
            }
        } else {
            application.activateWithOptions(NSApplicationActivationOptions::empty());
        }
    }
}

pub fn accessibility_preferences() -> AccessibilityPreferences {
    let workspace = NSWorkspace::sharedWorkspace();
    AccessibilityPreferences {
        reduce_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        reduce_transparency: workspace.accessibilityDisplayShouldReduceTransparency(),
        increase_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
    }
}

fn screen_index(screens: &NSArray<NSScreen>, candidate: &NSScreen) -> Option<usize> {
    screens
        .iter()
        .position(|screen| std::ptr::eq(&*screen, candidate))
}

fn rect_from_ns(rect: NSRect) -> Rect {
    Rect::new(
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    )
}

fn rect_to_ns(rect: Rect) -> NSRect {
    NSRect::new(
        NSPoint::new(rect.origin.x, rect.origin.y),
        NSSize::new(rect.size.width, rect.size.height),
    )
}

fn all_mask_corners() -> CACornerMask {
    CACornerMask::LayerMinXMinYCorner
        | CACornerMask::LayerMaxXMinYCorner
        | CACornerMask::LayerMinXMaxYCorner
        | CACornerMask::LayerMaxXMaxYCorner
}

fn set_mask_frame(
    layer: &CALayer,
    frame: NSRect,
    corner_radius: f64,
    visible: bool,
    duration: std::time::Duration,
) {
    CATransaction::begin();
    CATransaction::setDisableActions(duration.is_zero());
    CATransaction::setAnimationDuration(duration.as_secs_f64());
    layer.setFrame(frame);
    layer.setCornerRadius(corner_radius);
    layer.setOpacity(if visible { 1.0 } else { 0.0 });
    CATransaction::commit();
}

fn panel_class(superclass: &AnyClass) -> Result<&'static AnyClass, String> {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    if let Some(class) = CLASS.get() {
        return Ok(class);
    }
    if let Some(class) = AnyClass::get(c"MuxyQuickTerminalGPUIPanel") {
        let _ = CLASS.set(class);
        return Ok(class);
    }
    let mut builder = ClassBuilder::new(c"MuxyQuickTerminalGPUIPanel", superclass)
        .ok_or_else(|| "failed to allocate the Quick Terminal panel subclass".to_owned())?;
    unsafe {
        builder.add_method::<AnyObject, _>(
            objc2::sel!(canBecomeMainWindow),
            quick_terminal_panel_cannot_become_main as extern "C-unwind" fn(_, _) -> _,
        );
    }
    let class = builder.register();
    let _ = CLASS.set(class);
    Ok(class)
}

extern "C-unwind" fn quick_terminal_panel_cannot_become_main(
    _object: &AnyObject,
    _selector: Sel,
) -> Bool {
    Bool::NO
}

struct DoubleShiftBackend {
    detector: DoubleShiftDetector,
    caps_lock_enabled: Option<bool>,
    local_monitor: Option<Retained<AnyObject>>,
    event_tap: Option<CFRetained<CFMachPort>>,
    event_tap_source: Option<CFRetained<CFRunLoopSource>>,
    event_tap_run_loop: Option<CFRetained<CFRunLoop>>,
    trigger: Option<Rc<dyn Fn()>>,
}

impl DoubleShiftBackend {
    fn new() -> Self {
        Self {
            detector: DoubleShiftDetector::default(),
            caps_lock_enabled: None,
            local_monitor: None,
            event_tap: None,
            event_tap_source: None,
            event_tap_run_loop: None,
            trigger: None,
        }
    }

    fn receive_local_event(&mut self, event: &NSEvent) {
        let flags = NSEventModifierFlags(
            event.modifierFlags().0 & NSEventModifierFlags::DeviceIndependentFlagsMask.0,
        );
        let shift_pressed = flags.contains(NSEventModifierFlags::Shift);
        let input = match event.r#type() {
            NSEventType::FlagsChanged => DoubleShiftInput::ModifierChange {
                shift_pressed,
                other_modifier_pressed: self.other_modifier_pressed(
                    flags.intersects(
                        NSEventModifierFlags::Control
                            | NSEventModifierFlags::Option
                            | NSEventModifierFlags::Command
                            | NSEventModifierFlags::Function,
                    ),
                    flags.contains(NSEventModifierFlags::CapsLock),
                ),
                timestamp: event.timestamp(),
            },
            NSEventType::KeyDown => DoubleShiftInput::KeyDown {
                shift_pressed,
                timestamp: event.timestamp(),
            },
            NSEventType::LeftMouseDown
            | NSEventType::RightMouseDown
            | NSEventType::OtherMouseDown => DoubleShiftInput::PointerDown {
                shift_pressed,
                timestamp: event.timestamp(),
            },
            _ => return,
        };
        self.process(input);
    }

    fn receive_global_event(&mut self, event_type: CGEventType, event: &CGEvent) {
        if event_type == CGEventType::TapDisabledByTimeout
            || event_type == CGEventType::TapDisabledByUserInput
        {
            self.recover_disabled_event_tap();
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if NSApplication::sharedApplication(mtm).isActive() {
            return;
        }
        let flags = CGEvent::flags(Some(event));
        let shift_pressed = flags.contains(CGEventFlags::MaskShift);
        let timestamp = CGEvent::timestamp(Some(event)) as f64 / 1_000_000_000.0;
        let input = match event_type {
            CGEventType::FlagsChanged => DoubleShiftInput::ModifierChange {
                shift_pressed,
                other_modifier_pressed: self.other_modifier_pressed(
                    flags.intersects(
                        CGEventFlags::MaskControl
                            | CGEventFlags::MaskAlternate
                            | CGEventFlags::MaskCommand
                            | CGEventFlags::MaskSecondaryFn,
                    ),
                    flags.contains(CGEventFlags::MaskAlphaShift),
                ),
                timestamp,
            },
            CGEventType::KeyDown => DoubleShiftInput::KeyDown {
                shift_pressed,
                timestamp,
            },
            CGEventType::LeftMouseDown
            | CGEventType::RightMouseDown
            | CGEventType::OtherMouseDown => DoubleShiftInput::PointerDown {
                shift_pressed,
                timestamp,
            },
            _ => return,
        };
        self.process(input);
    }

    fn other_modifier_pressed(&mut self, conventional: bool, caps_lock_enabled: bool) -> bool {
        let caps_lock_changed = self
            .caps_lock_enabled
            .is_some_and(|previous| previous != caps_lock_enabled);
        self.caps_lock_enabled = Some(caps_lock_enabled);
        conventional || caps_lock_changed
    }

    fn process(&mut self, input: DoubleShiftInput) {
        if self.detector.process(input)
            && let Some(trigger) = &self.trigger
        {
            trigger();
        }
    }

    fn enable_event_tap_if_authorized(&mut self) -> bool {
        if !CGPreflightListenEventAccess() {
            self.remove_event_tap();
            return false;
        }
        if let Some(event_tap) = &self.event_tap {
            CGEvent::tap_enable(event_tap, true);
            if CGEvent::tap_is_enabled(event_tap) {
                return true;
            }
            self.remove_event_tap();
        }
        let mask = event_mask(&[
            CGEventType::FlagsChanged,
            CGEventType::KeyDown,
            CGEventType::LeftMouseDown,
            CGEventType::RightMouseDown,
            CGEventType::OtherMouseDown,
        ]);
        let pointer = std::ptr::from_mut(self).cast::<c_void>();
        let Some(event_tap) = (unsafe {
            CGEvent::tap_create(
                CGEventTapLocation::SessionEventTap,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::ListenOnly,
                mask,
                Some(double_shift_event_tap_callback),
                pointer,
            )
        }) else {
            return false;
        };
        let Some(source) = CFMachPort::new_run_loop_source(None, Some(&event_tap), 0) else {
            return false;
        };
        let Some(run_loop) = CFRunLoop::main() else {
            return false;
        };
        let common_modes = unsafe { kCFRunLoopCommonModes };
        run_loop.add_source(Some(&source), common_modes);
        CGEvent::tap_enable(&event_tap, true);
        if !CGEvent::tap_is_enabled(&event_tap) {
            run_loop.remove_source(Some(&source), common_modes);
            event_tap.invalidate();
            return false;
        }
        self.event_tap = Some(event_tap);
        self.event_tap_source = Some(source);
        self.event_tap_run_loop = Some(run_loop);
        true
    }

    fn recover_disabled_event_tap(&mut self) {
        let authorized = CGPreflightListenEventAccess();
        let reenabled = if authorized {
            self.event_tap.as_ref().is_some_and(|event_tap| {
                CGEvent::tap_enable(event_tap, true);
                CGEvent::tap_is_enabled(event_tap)
            })
        } else {
            false
        };
        if event_tap_recovery(authorized, reenabled) == EventTapRecovery::Downgrade {
            self.remove_event_tap();
        }
    }

    fn remove_event_tap(&mut self) {
        if let (Some(run_loop), Some(source)) = (&self.event_tap_run_loop, &self.event_tap_source) {
            run_loop.remove_source(Some(source), unsafe { kCFRunLoopCommonModes });
        }
        if let Some(event_tap) = self.event_tap.take() {
            event_tap.invalidate();
        }
        self.event_tap_source = None;
        self.event_tap_run_loop = None;
    }
}

impl ShortcutBackend for DoubleShiftBackend {
    fn start(&mut self, trigger: Rc<dyn Fn()>) -> Result<(), String> {
        if self.local_monitor.is_some() {
            return Ok(());
        }
        self.trigger = Some(trigger);
        let pointer = std::ptr::from_mut(self);
        let block = RcBlock::new(move |event_pointer: NonNull<NSEvent>| -> *mut NSEvent {
            let event = unsafe { event_pointer.as_ref() };
            if let Some(backend) = unsafe { pointer.as_mut() } {
                backend.receive_local_event(event);
            }
            event_pointer.as_ptr()
        });
        let mask = NSEventMask::FlagsChanged
            | NSEventMask::KeyDown
            | NSEventMask::LeftMouseDown
            | NSEventMask::RightMouseDown
            | NSEventMask::OtherMouseDown;
        let monitor =
            unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
        let Some(monitor) = monitor else {
            self.trigger = None;
            return Err("failed to install local double-Shift monitor".to_owned());
        };
        self.local_monitor = Some(monitor);
        self.enable_event_tap_if_authorized();
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(monitor) = self.local_monitor.take() {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
        self.remove_event_tap();
        self.trigger = None;
        self.detector.reset();
        self.caps_lock_enabled = None;
    }

    fn monitoring_state(&self) -> MonitoringState {
        if self.event_tap.is_some() {
            MonitoringState::SystemWide
        } else if self.local_monitor.is_some() {
            MonitoringState::LocalOnly
        } else {
            MonitoringState::Stopped
        }
    }

    fn refresh_system_wide_monitoring(&mut self) -> bool {
        self.enable_event_tap_if_authorized()
    }
}

impl Drop for DoubleShiftBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe extern "C-unwind" fn double_shift_event_tap_callback(
    _proxy: objc2_core_graphics::CGEventTapProxy,
    event_type: CGEventType,
    event: NonNull<CGEvent>,
    user_info: *mut c_void,
) -> *mut CGEvent {
    if let Some(backend) = NonNull::new(user_info.cast::<DoubleShiftBackend>()) {
        unsafe { backend.as_ptr().as_mut() }
            .expect("non-null double-Shift backend")
            .receive_global_event(event_type, unsafe { event.as_ref() });
    }
    event.as_ptr()
}

fn event_mask(types: &[CGEventType]) -> u64 {
    types
        .iter()
        .fold(0, |mask, event_type| mask | (1_u64 << event_type.0))
}

struct CarbonHotKeyBackend {
    identity: muxy_core::quick_terminal::RegistrationIdentity,
    identifier: u32,
    event_handler: EventHandlerRef,
    hot_key: EventHotKeyRef,
    trigger: Option<Rc<dyn Fn()>>,
}

impl CarbonHotKeyBackend {
    fn new(identity: muxy_core::quick_terminal::RegistrationIdentity, identifier: u32) -> Self {
        Self {
            identity,
            identifier,
            event_handler: std::ptr::null_mut(),
            hot_key: std::ptr::null_mut(),
            trigger: None,
        }
    }

    fn handle(&self, signature: u32, identifier: u32) -> i32 {
        if signature != CARBON_SIGNATURE || identifier != self.identifier {
            return EVENT_NOT_HANDLED;
        }
        if let Some(trigger) = &self.trigger {
            trigger();
        }
        0
    }
}

impl ShortcutBackend for CarbonHotKeyBackend {
    fn start(&mut self, trigger: Rc<dyn Fn()>) -> Result<(), String> {
        if !self.hot_key.is_null() {
            return Ok(());
        }
        let event_type = EventTypeSpec {
            event_class: fourcc(*b"keyb"),
            event_kind: 5,
        };
        let target = unsafe { get_application_event_target() };
        let pointer = std::ptr::from_mut(self).cast::<c_void>();
        let mut event_handler = std::ptr::null_mut();
        let status = unsafe {
            install_event_handler(
                target,
                Some(carbon_event_handler),
                1,
                &event_type,
                pointer,
                &mut event_handler,
            )
        };
        if status != 0 {
            return Err(format!("failed to install Carbon event handler ({status})"));
        }
        let hot_key_id = EventHotKeyId {
            signature: CARBON_SIGNATURE,
            id: self.identifier,
        };
        let mut hot_key = std::ptr::null_mut();
        let status = unsafe {
            register_event_hot_key(
                u32::from(self.identity.virtual_key_code),
                carbon_modifiers(self.identity.modifiers),
                hot_key_id,
                target,
                0,
                &mut hot_key,
            )
        };
        if status != 0 {
            unsafe { remove_event_handler(event_handler) };
            return Err(format!("failed to register Carbon hotkey ({status})"));
        }
        self.event_handler = event_handler;
        self.hot_key = hot_key;
        self.trigger = Some(trigger);
        Ok(())
    }

    fn stop(&mut self) {
        if !self.hot_key.is_null() {
            unsafe { unregister_event_hot_key(self.hot_key) };
        }
        if !self.event_handler.is_null() {
            unsafe { remove_event_handler(self.event_handler) };
        }
        self.hot_key = std::ptr::null_mut();
        self.event_handler = std::ptr::null_mut();
        self.trigger = None;
    }

    fn monitoring_state(&self) -> MonitoringState {
        if self.hot_key.is_null() {
            MonitoringState::Stopped
        } else {
            MonitoringState::CarbonHotKey
        }
    }
}

impl Drop for CarbonHotKeyBackend {
    fn drop(&mut self) {
        self.stop();
    }
}

fn carbon_modifiers(modifiers: u64) -> u32 {
    let mut carbon = 0;
    if modifiers & COMMAND != 0 {
        carbon |= 1 << 8;
    }
    if modifiers & SHIFT != 0 {
        carbon |= 1 << 9;
    }
    if modifiers & OPTION != 0 {
        carbon |= 1 << 11;
    }
    if modifiers & CONTROL != 0 {
        carbon |= 1 << 12;
    }
    carbon
}

const fn fourcc(bytes: [u8; 4]) -> u32 {
    u32::from_be_bytes(bytes)
}

const CARBON_SIGNATURE: u32 = fourcc(*b"MXNT");
const EVENT_NOT_HANDLED: i32 = -9874;

#[repr(C)]
struct OpaqueEventTarget {
    _private: [u8; 0],
}

#[repr(C)]
struct OpaqueEventHandler {
    _private: [u8; 0],
}

#[repr(C)]
struct OpaqueEventHandlerCall {
    _private: [u8; 0],
}

#[repr(C)]
struct OpaqueEvent {
    _private: [u8; 0],
}

#[repr(C)]
struct OpaqueEventHotKey {
    _private: [u8; 0],
}

type EventTargetRef = *mut OpaqueEventTarget;
type EventHandlerRef = *mut OpaqueEventHandler;
type EventHandlerCallRef = *mut OpaqueEventHandlerCall;
type EventRef = *mut OpaqueEvent;
type EventHotKeyRef = *mut OpaqueEventHotKey;
type EventHandler = unsafe extern "C-unwind" fn(EventHandlerCallRef, EventRef, *mut c_void) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct EventHotKeyId {
    signature: u32,
    id: u32,
}

unsafe extern "C-unwind" fn carbon_event_handler(
    _call: EventHandlerCallRef,
    event: EventRef,
    user_data: *mut c_void,
) -> i32 {
    let Some(mut backend) = NonNull::new(user_data.cast::<CarbonHotKeyBackend>()) else {
        return EVENT_NOT_HANDLED;
    };
    if event.is_null() {
        return EVENT_NOT_HANDLED;
    }
    let mut hot_key_id = EventHotKeyId {
        signature: 0,
        id: 0,
    };
    let status = unsafe {
        get_event_parameter(
            event,
            fourcc(*b"----"),
            fourcc(*b"hkid"),
            std::ptr::null_mut(),
            std::mem::size_of::<EventHotKeyId>(),
            std::ptr::null_mut(),
            std::ptr::from_mut(&mut hot_key_id).cast::<c_void>(),
        )
    };
    if status != 0 {
        return EVENT_NOT_HANDLED;
    }
    unsafe { backend.as_mut() }.handle(hot_key_id.signature, hot_key_id.id)
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C-unwind" {
    #[link_name = "GetApplicationEventTarget"]
    fn get_application_event_target() -> EventTargetRef;
    #[link_name = "InstallEventHandler"]
    fn install_event_handler(
        target: EventTargetRef,
        handler: Option<EventHandler>,
        event_type_count: usize,
        event_types: *const EventTypeSpec,
        user_data: *mut c_void,
        event_handler: *mut EventHandlerRef,
    ) -> i32;
    #[link_name = "RemoveEventHandler"]
    fn remove_event_handler(event_handler: EventHandlerRef) -> i32;
    #[link_name = "GetEventParameter"]
    fn get_event_parameter(
        event: EventRef,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: usize,
        actual_size: *mut usize,
        data: *mut c_void,
    ) -> i32;
    #[link_name = "RegisterEventHotKey"]
    fn register_event_hot_key(
        key_code: u32,
        modifiers: u32,
        identifier: EventHotKeyId,
        target: EventTargetRef,
        options: u32,
        hot_key: *mut EventHotKeyRef,
    ) -> i32;
    #[link_name = "UnregisterEventHotKey"]
    fn unregister_event_hot_key(hot_key: EventHotKeyRef) -> i32;
}

#[cfg(test)]
mod tests {
    use super::{MacShortcutBackendFactory, carbon_modifiers};
    use muxy_core::shortcuts::{COMMAND, CONTROL, OPTION, SHIFT};

    #[test]
    fn quick_terminal_shortcut_factory_owns_carbon_identifiers() {
        let mut first = MacShortcutBackendFactory::new();
        let mut second = MacShortcutBackendFactory::new();
        assert_eq!(first.take_identifier(), 1);
        assert_eq!(first.take_identifier(), 2);
        assert_eq!(second.take_identifier(), 1);
    }

    #[test]
    fn quick_terminal_shortcut_carbon_modifier_mapping_is_complete() {
        assert_eq!(
            carbon_modifiers(COMMAND | SHIFT | CONTROL | OPTION),
            (1 << 8) | (1 << 9) | (1 << 11) | (1 << 12)
        );
    }
}
