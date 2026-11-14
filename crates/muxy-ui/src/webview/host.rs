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
use objc2::runtime::{AnyObject, Bool, Sel};
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
    shortcuts: RefCell<Rc<Vec<gpui::Keystroke>>>,
}

#[derive(Debug, Default)]
struct HitTestRegions {
    occluded: RefCell<Vec<NSRect>>,
    passthrough_all: Cell<bool>,
    /// Where the page takes clicks, from its top left corner. `None` takes
    /// them everywhere.
    interactive: RefCell<Option<Vec<NSRect>>>,
}

impl HitTestRegions {
    fn excludes(&self, point: NSPoint, page: NSPoint) -> bool {
        self.passthrough_all.get()
            || self
                .occluded
                .borrow()
                .iter()
                .any(|rect| contains_point(*rect, point))
            || self
                .interactive
                .borrow()
                .as_ref()
                .is_some_and(|regions| !regions.iter().any(|rect| contains_point(*rect, page)))
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
            let page = if self.isFlipped() { local } else { NSPoint::new(local.x, self.bounds().size.height - local.y) };
            if self.ivars().excludes(local, page) { None }
            else { unsafe { msg_send![super(self), hitTest: point] } }
        }

        // WebKit runs a key's command up the page's responders to learn
        // whether anything wanted it. GPUI's view takes every command, so
        // keys the page left alone would never reach the Edit menu. They
        // go on to the window instead.
        #[unsafe(method(doCommandBySelector:))]
        fn do_command_by_selector(&self, selector: Sel) {
            if let Some(window) = self.window() {
                let _: () = unsafe { msg_send![&*window, doCommandBySelector: selector] };
            }
        }

        // Typing the page left alone comes back here, and must not reach
        // whatever Muxy last focused.
        #[unsafe(method(keyDown:))]
        fn key_down(&self, _: &NSEvent) {}
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
    parent: RefCell<Retained<NSView>>,
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
            parent: RefCell::new(parent),
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

    pub(crate) fn set_shortcuts(&self, modal: bool, shortcuts: Rc<Vec<gpui::Keystroke>>) {
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
        let frame = self.parent().bounds();
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

    /// Where the page takes clicks, from its top left corner; elsewhere
    /// they reach what is under it. `None` takes them everywhere.
    pub(crate) fn set_hit_regions(&self, regions: Option<&[Bounds<Pixels>]>) {
        let regions = regions.map(|regions| {
            regions
                .iter()
                .map(|region| {
                    NSRect::new(
                        NSPoint::new(
                            f64::from(f32::from(region.origin.x)),
                            f64::from(f32::from(region.origin.y)),
                        ),
                        NSSize::new(
                            f64::from(f32::from(region.size.width)),
                            f64::from(f32::from(region.size.height)),
                        ),
                    )
                })
                .collect()
        });
        *self.container.ivars().interactive.borrow_mut() = regions;
    }

    /// Puts these pages above every other native view in the window, the
    /// last one in front. Views already in place are left alone, so calling
    /// this every frame costs nothing, and the front page moves only when
    /// something else covers it: raising a page that was just clicked moves
    /// the pages that go under it instead, so the click isn't interrupted.
    pub(crate) fn order_front(hosts: &[&Self]) {
        let mut remaining = hosts.to_vec();
        while let Some(first) = remaining.first() {
            let parent = first.parent();
            let (group, rest): (Vec<_>, Vec<_>) = remaining.into_iter().partition(|host| {
                ptr::eq(
                    Retained::as_ptr(&host.parent.borrow()),
                    Retained::as_ptr(&parent),
                )
            });
            Self::order_front_in_parent(&group);
            remaining = rest;
        }
    }

    /// [`Self::order_front`] for pages that share one parent.
    fn order_front_in_parent(hosts: &[&Self]) {
        let Some(front) = hosts.last() else {
            return;
        };
        let ours: Vec<*const NSView> = hosts
            .iter()
            .flat_map(|host| [host.container_ptr(), host.grips_ptr()])
            .collect();
        let views = front.subview_order();
        let at = |views: &[*const NSView], view: *const NSView| {
            views.iter().position(|entry| ptr::eq(*entry, view))
        };
        let covered = at(&views, front.container_ptr()).is_none_or(|index| {
            views[index + 1..]
                .iter()
                .any(|view| !ours.iter().any(|entry| ptr::eq(*entry, *view)))
                || at(&views, front.grips_ptr()) != Some(index + 1)
        });
        if covered {
            front.place(NSWindowOrderingMode::Above, None);
        }
        for pair in hosts.windows(2).rev() {
            let (below, above) = (pair[0], pair[1]);
            let views = front.subview_order();
            let in_place = match (
                at(&views, below.container_ptr()),
                at(&views, above.container_ptr()),
            ) {
                (Some(container), Some(anchor)) => {
                    container + 2 == anchor && at(&views, below.grips_ptr()) == Some(container + 1)
                }
                _ => false,
            };
            if !in_place {
                below.place(NSWindowOrderingMode::Below, Some(&above.container));
            }
        }
    }

    fn parent(&self) -> Retained<NSView> {
        self.parent.borrow().clone()
    }

    /// Moves the page into `parent`, another window's GPUI view, keeping it
    /// loaded. It stays hidden there until the next [`Self::sync`] and
    /// [`Self::set_visible`] place it in that window, and the keyboard
    /// returns to it with the next [`Self::focus`] or key Muxy routes to it.
    pub(crate) fn move_to(&self, parent: Retained<NSView>) {
        if ptr::eq(
            Retained::as_ptr(&self.parent.borrow()),
            Retained::as_ptr(&parent),
        ) {
            return;
        }
        self.blur();
        self.container.setHidden(true);
        self.grips.setHidden(true);
        self.grips.removeFromSuperview();
        self.container.removeFromSuperview();
        parent.addSubview(&self.container);
        parent.addSubview_positioned_relativeTo(
            &self.grips,
            NSWindowOrderingMode::Above,
            Some(&self.container),
        );
        self.grips.setFrame(parent.bounds());
        self.grips.ivars().borrow_mut().clear();
        if let Some(window) = parent.window() {
            window.invalidateCursorRectsForView(&self.grips);
        }
        *self.parent.borrow_mut() = parent;
        *self.applied.borrow_mut() = Applied::default();
    }

    /// The parent's subviews, back to front, as they are now.
    fn subview_order(&self) -> Vec<*const NSView> {
        let subviews = self.parent().subviews();
        (0..subviews.count())
            .map(|index| Retained::as_ptr(&subviews.objectAtIndex(index)))
            .collect()
    }

    fn container_ptr(&self) -> *const NSView {
        Retained::as_ptr(&self.container).cast()
    }

    fn grips_ptr(&self) -> *const NSView {
        Retained::as_ptr(&self.grips).cast()
    }

    /// Moves the page and its grips next to `relative`, or to the front,
    /// keeping the keyboard on the page if it had it.
    fn place(&self, ordering: NSWindowOrderingMode, relative: Option<&NSView>) {
        let focused = self.page.as_ref().is_some_and(|page| {
            self.container.window().is_some_and(|window| {
                window.firstResponder().is_some_and(|responder| {
                    responder
                        .downcast_ref::<NSView>()
                        .is_some_and(|view| view.isDescendantOf(page))
                })
            })
        });
        let parent = self.parent();
        parent.addSubview_positioned_relativeTo(&self.container, ordering, relative);
        parent.addSubview_positioned_relativeTo(
            &self.grips,
            NSWindowOrderingMode::Above,
            Some(&self.container),
        );
        if focused {
            self.focus();
        }
    }

    pub(crate) fn sync(&self, bounds: Bounds<Pixels>, clip: Bounds<Pixels>, radius: f64) {
        self.bounds.set(bounds);
        self.clip.set(clip);
        let frame = crate::appkit::frame_in(&self.parent(), bounds);
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
            window.makeFirstResponder(Some(&*self.parent()));
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

/// What GPUI reads from a key event, without its text: a kept shortcut no
/// binding takes must not type through `dispatch_keystroke`.
fn keystroke(event: &NSEvent) -> Option<gpui::Keystroke> {
    let event = ptr::from_ref(event).cast_mut().cast();
    gpui::keystroke_from_native_event(event).map(|keystroke| gpui::Keystroke {
        key_char: None,
        ..keystroke
    })
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
        let Some(keystroke) = keystroke(event_ref) else {
            return event;
        };
        if route_key_event(
            keystroke,
            &focus,
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
    focus: &Focus,
    on_signal: &dyn Fn(Signal) -> bool,
    page_key_equivalent: impl FnOnce() -> bool,
    menu_key_equivalent: impl FnOnce() -> bool,
    page_key_down: impl FnOnce(),
) -> bool {
    if keystroke.key == "escape" && focus.modal.get() {
        on_signal(Signal::Escape);
        return true;
    }
    // Routing can redraw Muxy before it returns, as when WebKit hands a key
    // back to GPUI, and a redraw sets new shortcuts. So the list is cloned out
    // and never borrowed across a call back into Muxy.
    let shortcuts = focus.shortcuts.borrow().clone();
    let kept = shortcuts
        .iter()
        .any(|key| key.key == keystroke.key && key.modifiers == keystroke.modifiers);
    if kept {
        return on_signal(Signal::Shortcut(keystroke));
    }
    // The page sees a command key first, as in Safari, so editors keep
    // their own select all and undo. WebKit sends back keys the page
    // leaves alone, and those then reach the Edit menu.
    if keystroke.modifiers.platform {
        page_key_equivalent() || menu_key_equivalent()
    } else {
        page_key_down();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_shortcuts_try_webkit_before_native_menu() {
        let emitted = RefCell::new(0);
        let on_key = |_: Signal| {
            *emitted.borrow_mut() += 1;
            true
        };
        for modal in [false, true] {
            let focus = Focus::default();
            focus.modal.set(modal);
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
                        &focus,
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
                        if page_handled {
                            vec!["page"]
                        } else {
                            vec!["page", "menu"]
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
        let focus = Focus::default();
        *focus.shortcuts.borrow_mut() = Rc::new(vec![shortcut.clone()]);
        assert!(route_key_event(
            shortcut,
            &focus,
            &on_key,
            || panic!("kept shortcut must not reach the page"),
            || panic!("kept shortcut must not reach the menu"),
            || panic!("kept shortcut must not reach the page"),
        ));
        focus.modal.set(true);
        assert!(route_key_event(
            gpui::Keystroke::parse("escape").expect("binding"),
            &focus,
            &on_key,
            || panic!("modal escape must not reach the page"),
            || panic!("modal escape must not reach the menu"),
            || panic!("modal escape must not reach the page"),
        ));
        assert_eq!(emitted.borrow().len(), 2);
    }

    #[test]
    fn shortcuts_can_change_while_a_key_is_routed() {
        let shortcut = gpui::Keystroke::parse("cmd-t").expect("binding");
        let focus = Focus::default();
        let redraws = Cell::new(0);
        let redraw = || {
            *focus.shortcuts.borrow_mut() = Rc::new(vec![shortcut.clone()]);
            redraws.set(redraws.get() + 1);
        };
        let on_key = |_: Signal| {
            redraw();
            true
        };
        for (binding, handled, routed_redraws) in
            [("j", true, 1), ("cmd-j", false, 2), ("cmd-t", true, 1)]
        {
            *focus.shortcuts.borrow_mut() = Rc::new(vec![shortcut.clone()]);
            redraws.set(0);
            let result = route_key_event(
                gpui::Keystroke::parse(binding).expect("binding"),
                &focus,
                &on_key,
                || {
                    redraw();
                    false
                },
                || {
                    redraw();
                    false
                },
                redraw,
            );
            assert_eq!(result, handled, "{binding}");
            assert_eq!(redraws.get(), routed_redraws, "{binding}");
        }
    }
}
