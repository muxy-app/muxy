//! A native page shown over GPUI. The page sits in a container view that
//! follows its pane, hides the parts app content covers, and shares the
//! keyboard with Muxy's shortcuts. Extension pages and browser pages both
//! live in one.

use std::cell::{Cell, RefCell};
use std::ptr;
use std::rc::Rc;

use block2::RcBlock;
use gpui::{Bounds, CursorStyle, Pixels};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSCursor, NSEvent, NSEventMask, NSEventModifierFlags,
    NSEventType, NSImage, NSMenu, NSView, NSWindowOrderingMode,
};
use objc2_core_graphics::CGMutablePath;
use objc2_foundation::{
    NSComparisonResult, NSError, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::CAShapeLayer;
use objc2_web_kit::WKWebView;

use super::Snapshot;

/// What the page hands back to Muxy: keys it keeps, and clicks.
#[derive(Debug)]
pub(crate) enum Signal {
    Escape,
    Shortcut(gpui::Keystroke),
    Clicked,
}

/// How Muxy shows the page, as the key monitor sees it.
#[derive(Debug, Default)]
struct Focus {
    focused: Cell<bool>,
    clicked: Cell<bool>,
    /// When the user last clicked into the page.
    last_click: Cell<Option<std::time::Instant>>,
    modal: Cell<bool>,
    shortcuts: RefCell<Vec<gpui::Keystroke>>,
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
    /// Holds the page. Clicks on the parts app content covers pass through
    /// to Muxy, and its layer mask hides them.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "MuxyPageContainer"]
    #[ivars = HitTestRegions]
    #[derive(Debug)]
    struct Container;
    unsafe impl NSObjectProtocol for Container {}
    impl Container {
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
                .then(|| Retained::into_super(objc2::Message::retain(self)))
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

/// Native view state as last applied. Every frame syncs the view, and each
/// new layer mask costs a view-sized bitmap, so only changes are applied.
#[derive(Debug, Default)]
struct Applied {
    frame: Option<NSRect>,
    radius: Option<f64>,
    mask: Option<Mask>,
}

#[derive(Debug, PartialEq)]
struct Mask {
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    occlusions: Vec<Bounds<Pixels>>,
}

#[derive(Debug)]
pub(crate) struct Host {
    container: Retained<Container>,
    page: Option<Retained<WKWebView>>,
    grips: Retained<GripOverlay>,
    parent: Retained<NSView>,
    focus: Rc<Focus>,
    monitor: Option<Retained<AnyObject>>,
    visible: Cell<bool>,
    bounds: Cell<Bounds<Pixels>>,
    clip: Cell<Bounds<Pixels>>,
    occlusions: RefCell<Vec<Bounds<Pixels>>>,
    applied: RefCell<Applied>,
}

impl Host {
    /// An empty, hidden container in `parent`, GPUI's view.
    pub(crate) fn new(parent: Retained<NSView>, mtm: MainThreadMarker) -> Self {
        let allocated = Container::alloc(mtm).set_ivars(HitTestRegions::default());
        let container: Retained<Container> =
            unsafe { msg_send![super(allocated), initWithFrame: NSRect::ZERO] };
        container.setWantsLayer(true);
        container.setHidden(true);
        parent.addSubview(&container);
        let allocated = GripOverlay::alloc(mtm).set_ivars(RefCell::default());
        let grips: Retained<GripOverlay> =
            unsafe { msg_send![super(allocated), initWithFrame: parent.bounds()] };
        grips.setHidden(true);
        parent.addSubview_positioned_relativeTo(
            &grips,
            NSWindowOrderingMode::Above,
            Some(&container),
        );
        Self {
            container,
            page: None,
            grips,
            parent,
            focus: Rc::default(),
            monitor: None,
            visible: Cell::new(false),
            bounds: Cell::default(),
            clip: Cell::default(),
            occlusions: RefCell::default(),
            applied: RefCell::default(),
        }
    }

    /// The view a page is added to.
    pub(crate) fn container(&self) -> &NSView {
        &self.container
    }

    /// Starts routing keys for `page`, a view inside the container. Keys Muxy
    /// keeps, and clicks into the page, go to `on_signal`, which reports
    /// whether it took them.
    pub(crate) fn attach(
        &mut self,
        page: Retained<WKWebView>,
        on_signal: impl Fn(Signal) -> bool + 'static,
    ) {
        page.setFrame(self.container.bounds());
        page.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        self.monitor = monitor(&self.container, &page, &self.focus, Rc::new(on_signal));
        self.page = Some(page);
    }

    pub(crate) fn set_shortcuts(&self, modal: bool, shortcuts: Vec<gpui::Keystroke>) {
        self.focus.modal.set(modal);
        *self.focus.shortcuts.borrow_mut() = shortcuts;
    }

    /// Whether Muxy shows this page focused. Keys reach the page only while it
    /// is, or after a click into it until Muxy catches up. A page that takes
    /// the native focus itself, as when its script focuses an element, doesn't
    /// get them.
    pub(crate) fn set_focused(&self, focused: bool) {
        self.focus.focused.set(focused);
        if focused {
            self.focus.clicked.set(false);
        }
    }

    /// Whether the user clicked into the page within `elapsed`.
    pub(crate) fn clicked_within(&self, elapsed: std::time::Duration) -> bool {
        self.focus
            .last_click
            .get()
            .is_some_and(|clicked| clicked.elapsed() <= elapsed)
    }

    /// Sends clicks on the resize grips drawn over the page, in window
    /// coordinates, to Muxy and shows each grip's cursor there. The page's
    /// pixels don't change. Call after [`Self::occlude`].
    pub(crate) fn set_grips(&self, grips: &[(Bounds<Pixels>, CursorStyle)]) {
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

    pub(crate) fn set_mouse_passthrough(&self, enabled: bool) {
        self.container.ivars().passthrough_all.set(enabled);
    }

    pub(crate) fn sync(&self, bounds: Bounds<Pixels>, clip: Bounds<Pixels>, radius: f64) {
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
            self.container.setFrame(frame);
            applied.frame = Some(frame);
        }
        if applied.radius != Some(radius)
            && let Some(layer) = self.container.layer()
        {
            layer.setCornerRadius(radius);
            layer.setMasksToBounds(true);
            applied.radius = Some(radius);
        }
        drop(applied);
        self.apply_mask();
    }

    pub(crate) fn occlude(&self, occlusions: &[Bounds<Pixels>]) {
        if *self.occlusions.borrow() != occlusions {
            *self.occlusions.borrow_mut() = occlusions.to_vec();
        }
        self.apply_mask();
    }

    /// Adds `bounds` to what covers the page, for app content drawn after
    /// the frame's occlusions were applied, such as a dropdown.
    pub(crate) fn occlude_also(&self, bounds: Bounds<Pixels>) {
        let mut occlusions = self.occlusions.borrow().clone();
        if !occlusions.contains(&bounds) {
            occlusions.push(bounds);
            self.occlude(&occlusions);
        }
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
        *self.container.ivars().occluded.borrow_mut() = excluded
            .into_iter()
            .map(|region| self.local_rect(region))
            .collect();
        if let Some(layer) = self.container.layer() {
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
                if self.container.isFlipped() {
                    top
                } else {
                    f64::from(f32::from(bounds.size.height)) - top - height
                },
            ),
            NSSize::new(f64::from(f32::from(region.size.width)), height),
        )
    }

    pub(crate) fn is_visible(&self) -> bool {
        self.visible.get()
    }

    pub(crate) fn set_visible(&self, visible: bool) {
        if self.visible.replace(visible) == visible && self.container.isHidden() != visible {
            return;
        }
        // Hiding the focused page would leave the keyboard with the window.
        if !visible {
            self.blur();
        }
        self.container.setHidden(!visible);
        self.grips.setHidden(!visible);
    }

    pub(crate) fn focus(&self) {
        if self.visible.get()
            && let Some(page) = &self.page
            && let Some(window) = self.container.window()
        {
            window.makeFirstResponder(Some(page));
        }
    }

    pub(crate) fn blur(&self) {
        self.focus.clicked.set(false);
        if let Some(window) = self.container.window()
            && window.firstResponder().is_some_and(|responder| {
                responder
                    .downcast_ref::<NSView>()
                    .is_some_and(|view| view.isDescendantOf(&self.container))
            })
        {
            window.makeFirstResponder(Some(&self.parent));
        }
    }

    /// Takes a picture of the page as shown, at the display's scale.
    pub(crate) fn snapshot(&self, done: impl Fn(Snapshot) + 'static) {
        let Some(page) = &self.page else {
            return;
        };
        let scale = self
            .container
            .window()
            .map_or(1.0, |window| window.backingScaleFactor());
        let bounds = self.bounds.get();
        let (Some(width), Some(height)) = (
            pixel_dimension(bounds.size.width, scale),
            pixel_dimension(bounds.size.height, scale),
        ) else {
            return;
        };
        let completion = RcBlock::new(move |image: *mut NSImage, _: *mut NSError| {
            if let Some(bgra) = unsafe { image.as_ref() }
                .and_then(|image| crate::bitmap::render_bgra(image, width, height))
            {
                done(Snapshot {
                    width,
                    height,
                    bgra,
                });
            }
        });
        unsafe {
            page.takeSnapshotWithConfiguration_completionHandler(None, &completion);
        }
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.blur();
        if let Some(monitor) = self.monitor.take() {
            unsafe {
                NSEvent::removeMonitor(&monitor);
            }
        }
        self.grips.removeFromSuperview();
        self.container.removeFromSuperview();
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pixel_dimension(points: Pixels, scale: f64) -> Option<u32> {
    let pixels = f64::from(f32::from(points)) * scale;
    (pixels.is_finite() && (1.0..=16384.0).contains(&pixels)).then(|| pixels.round() as u32)
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
    container: &Retained<Container>,
    page: &Retained<WKWebView>,
    focus: &Rc<Focus>,
    on_signal: Rc<dyn Fn(Signal) -> bool>,
) -> Option<Retained<AnyObject>> {
    let container = container.clone();
    let page = page.clone();
    let focus = focus.clone();
    let monitor = RcBlock::new(move |event: ptr::NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        let event = event.as_ptr();
        if matches!(
            event_ref.r#type(),
            NSEventType::LeftMouseDown | NSEventType::RightMouseDown | NSEventType::OtherMouseDown
        ) {
            // Muxy hears of a click from the page itself, which can be late.
            let clicked = container
                .window()
                .filter(|window| event_ref.window(container.mtm()).as_ref() == Some(window))
                .and_then(|window| window.contentView())
                .and_then(|content| content.hitTest(event_ref.locationInWindow()))
                .is_some_and(|view| view.isDescendantOf(&container));
            focus.clicked.set(clicked);
            if clicked {
                focus.last_click.set(Some(std::time::Instant::now()));
                on_signal(Signal::Clicked);
            }
            return event;
        }
        if container.isHidden() {
            return event;
        }
        let Some(window) = container.window().filter(|window| {
            window.isKeyWindow() && event_ref.window(container.mtm()).as_ref() == Some(window)
        }) else {
            return event;
        };
        let page_responder = || {
            window.firstResponder().filter(|responder| {
                responder
                    .downcast_ref::<NSView>()
                    .is_some_and(|responder| responder.isDescendantOf(&container))
            })
        };
        // Keys follow Muxy's focus: pages take the native focus on their own,
        // even from each other, and a click on Muxy never takes it back.
        let focused = focus.focused.get() || focus.clicked.get();
        let holds_focus = page_responder().is_some();
        if holds_focus && !focused {
            if let Some(parent) = unsafe { container.superview() } {
                window.makeFirstResponder(Some(&parent));
            }
            return event;
        }
        if !holds_focus && focused && event_ref.r#type() == NSEventType::KeyDown {
            window.makeFirstResponder(Some(&page));
        }
        let Some(responder) = page_responder() else {
            return event;
        };
        if event_ref.r#type() == NSEventType::FlagsChanged {
            if let Some(parent) = unsafe { container.superview() } {
                parent.flagsChanged(event_ref);
            }
            return event;
        }
        if route_key_event(
            keystroke(event_ref),
            focus.modal.get(),
            &focus.shortcuts.borrow(),
            &*on_signal,
            || page_key_equivalent(&page, event_ref),
            || {
                let app = NSApplication::sharedApplication(container.mtm());
                let Some(key) = event_ref.charactersIgnoringModifiers() else {
                    return false;
                };
                app.mainMenu().is_some_and(|menu| {
                    native_menu_key_equivalent(&menu, &key, event_ref.modifierFlags(), &page)
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

/// Lets the web engine handle a command key, even in a page whose class
/// declines key equivalents, so the page's own shortcuts see it.
fn page_key_equivalent(page: &WKWebView, event: &NSEvent) -> bool {
    let handled: Bool =
        unsafe { msg_send![super(page, WKWebView::class()), performKeyEquivalent: event] };
    handled.as_bool()
}

fn native_menu_key_equivalent(
    menu: &NSMenu,
    key: &NSString,
    modifiers: NSEventModifierFlags,
    view: &NSView,
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
    on_signal: &dyn Fn(Signal) -> bool,
    page_key_equivalent: impl FnOnce() -> bool,
    menu_key_equivalent: impl FnOnce() -> bool,
    page_key_down: impl FnOnce(),
) -> bool {
    if keystroke.key == "escape" && modal {
        on_signal(Signal::Escape);
        return true;
    }
    if shortcuts
        .iter()
        .any(|key| key.key == keystroke.key && key.modifiers == keystroke.modifiers)
    {
        return on_signal(Signal::Shortcut(keystroke));
    }
    if keystroke.modifiers.platform {
        menu_key_equivalent() || page_key_equivalent()
    } else {
        page_key_down();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_shortcuts_try_native_menu_before_webkit() {
        let emitted = RefCell::new(0);
        let on_key = |_: Signal| {
            *emitted.borrow_mut() += 1;
            true
        };
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
                        &on_key,
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
                    assert_eq!(*emitted.borrow(), 0);
                }
            }
        }
    }

    #[test]
    fn kept_shortcuts_and_modal_escape_go_to_muxy() {
        let emitted = RefCell::new(Vec::new());
        let on_key = |key: Signal| {
            emitted.borrow_mut().push(format!("{key:?}"));
            true
        };
        let shortcut = gpui::Keystroke::parse("cmd-t").expect("binding");
        assert!(route_key_event(
            shortcut.clone(),
            false,
            std::slice::from_ref(&shortcut),
            &on_key,
            || panic!("kept shortcut must not reach the page"),
            || panic!("kept shortcut must not reach the menu"),
            || panic!("kept shortcut must not reach the page"),
        ));
        assert!(route_key_event(
            gpui::Keystroke::parse("escape").expect("binding"),
            true,
            &[],
            &on_key,
            || panic!("modal escape must not reach the page"),
            || panic!("modal escape must not reach the menu"),
            || panic!("modal escape must not reach the page"),
        ));
        assert_eq!(emitted.borrow().len(), 2);
    }
}
