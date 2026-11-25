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
use objc2_core_graphics::{CGColor, CGMutablePath, CGPath};
use objc2_foundation::{
    NSComparisonResult, NSError, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::{CALayer, CAShapeLayer, CATransaction};
use objc2_web_kit::WKWebView;

use super::{Cover, Snapshot};

#[derive(Debug)]
pub(crate) enum Signal {
    Escape,
    Shortcut(gpui::Keystroke),
    Clicked,
}

#[derive(Debug, Default)]
struct Focus {
    focused: Cell<bool>,
    clicked: Cell<bool>,
    last_click: Cell<Option<std::time::Instant>>,
    modal: Cell<bool>,
    shortcuts: RefCell<Rc<Vec<gpui::Keystroke>>>,
}

#[derive(Debug, Default)]
struct HitTestRegions {
    mask: RefCell<Option<Retained<CAShapeLayer>>>,
    pointer_exclusions: RefCell<Vec<NSRect>>,
    passthrough_all: Cell<bool>,
    interactive: RefCell<Option<Vec<NSRect>>>,
}

impl HitTestRegions {
    fn excludes(&self, point: NSPoint, page: NSPoint) -> bool {
        self.passthrough_all.get()
            || self.mask.borrow().as_ref().is_some_and(|mask| {
                mask.path().is_none_or(|path| !unsafe {
                    CGPath::contains_point(Some(&path), ptr::null(), point, false)
                })
            })
            || self
                .pointer_exclusions
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

        #[unsafe(method(doCommandBySelector:))]
        fn do_command_by_selector(&self, selector: Sel) {
            if let Some(window) = self.window() {
                let _: () = unsafe { msg_send![&*window, doCommandBySelector: selector] };
            }
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, _: &NSEvent) {}
    }
);

define_class!(
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

#[derive(Debug, Default)]
struct Applied {
    frame: Option<NSRect>,
    radius: Option<f64>,
    mask: Option<Mask>,
    shadows: Vec<Retained<CALayer>>,
}

#[derive(Debug)]
struct Mask {
    bounds: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    occlusions: Vec<Cover>,
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
    occlusions: RefCell<Vec<Cover>>,
    applied: RefCell<Applied>,
}

impl Host {
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

    pub(crate) fn container(&self) -> &NSView {
        &self.container
    }

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

    pub(crate) fn set_focused(&self, focused: bool) {
        self.focus.focused.set(focused);
        if focused {
            self.focus.clicked.set(false);
        }
    }

    pub(crate) fn clicked_within(&self, elapsed: std::time::Duration) -> bool {
        self.focus
            .last_click
            .get()
            .is_some_and(|clicked| clicked.elapsed() <= elapsed)
    }

    pub(crate) fn take_click(&self) -> bool {
        super::rules::take_click(&self.focus.last_click)
    }

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
        let mut applied = self.applied.borrow_mut();
        for shadow in applied.shadows.drain(..) {
            shadow.removeFromSuperlayer();
        }
        *applied = Applied::default();
    }

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

    pub(crate) fn occlude(&self, occlusions: &[Cover], window: &mut gpui::Window) {
        if *self.occlusions.borrow() != occlusions {
            *self.occlusions.borrow_mut() = occlusions.to_vec();
            if self.visible.get() {
                window.synchronize_native_layers();
            }
        }
        self.apply_mask();
    }

    pub(crate) fn occlude_also(&self, bounds: Bounds<Pixels>) {
        let mut occlusions = self.occlusions.borrow_mut();
        let cover = Cover::from(bounds);
        if !occlusions.contains(&cover) {
            occlusions.push(cover);
            drop(occlusions);
            self.apply_mask();
        }
    }

    fn apply_mask(&self) {
        let bounds = self.bounds.get();
        let clip = bounds.intersect(&self.clip.get());
        let occlusions = self.occlusions.borrow();
        let mut applied = self.applied.borrow_mut();
        if applied.mask.as_ref().is_some_and(|mask| {
            mask.bounds == bounds && mask.clip == clip && mask.occlusions == *occlusions
        }) {
            return;
        }
        let Some(layer) = self.container.layer() else {
            return;
        };
        applied.mask = Some(Mask {
            bounds,
            clip,
            occlusions: occlusions.clone(),
        });
        let mut cuts = occlusions
            .iter()
            .filter(|cover| !cover.pointer_only && clip.intersects(&cover.bounds))
            .peekable();
        let mask = (clip != bounds || cuts.peek().is_some()).then(|| {
            let cuts = cuts.map(|cover| (self.local_rect(cover.bounds), f64::from(cover.radius)));
            let mask = visible_mask(self.local_rect(clip), cuts);
            mask.setContentsScale(layer.contentsScale());
            mask
        });
        let actions_disabled = CATransaction::disableActions();
        CATransaction::setDisableActions(true);
        unsafe { layer.setMask(mask.as_deref().map(AsRef::as_ref)) };
        let regions = self.container.ivars();
        *regions.mask.borrow_mut() = mask;
        *regions.pointer_exclusions.borrow_mut() = occlusions
            .iter()
            .filter(|cover| cover.pointer_only && clip.intersects(&cover.bounds))
            .map(|cover| self.local_rect(cover.bounds))
            .collect();
        self.sync_shadows(&layer, &mut applied.shadows, clip, &occlusions);
        CATransaction::setDisableActions(actions_disabled);
    }

    fn sync_shadows(
        &self,
        layer: &CALayer,
        layers: &mut Vec<Retained<CALayer>>,
        clip: Bounds<Pixels>,
        covers: &[Cover],
    ) {
        let mut count = 0;
        for cover in covers {
            for shadow in &cover.shadows {
                let shape = (cover.bounds + shadow.offset).dilate(shadow.spread_radius);
                if !clip.intersects(&shape.dilate(shadow.blur_radius * 3.0)) {
                    continue;
                }
                if count == layers.len() {
                    let shadow_layer = CALayer::new();
                    shadow_layer.setZPosition(1.0);
                    shadow_layer.setShadowOffset(NSSize::ZERO);
                    layer.addSublayer(&shadow_layer);
                    layers.push(shadow_layer);
                }
                let shadow_layer = &layers[count];
                shadow_layer.setFrame(layer.bounds());
                let radius = f64::from(cover.radius);
                let path = unsafe {
                    CGPath::with_rounded_rect(self.local_rect(shape), radius, radius, ptr::null())
                };
                shadow_layer.setShadowPath(Some(&path));
                let color: gpui::Rgba = shadow.color.into();
                let rgb = CGColor::new_srgb(color.r.into(), color.g.into(), color.b.into(), 1.0);
                shadow_layer.setShadowColor(Some(&rgb));
                shadow_layer.setShadowOpacity(color.a);
                shadow_layer.setShadowRadius(f64::from(shadow.blur_radius));
                count += 1;
            }
        }
        for layer in layers.drain(count..) {
            layer.removeFromSuperlayer();
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

fn visible_grips(
    bounds: Bounds<Pixels>,
    occlusions: &[Cover],
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
                        .flat_map(|region| subtract(region, occlusion.bounds))
                        .collect()
                })
                .into_iter()
                .map(|region| (region, *cursor))
        })
        .collect()
}

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

fn visible_mask(
    clip: NSRect,
    covers: impl Iterator<Item = (NSRect, f64)>,
) -> Retained<CAShapeLayer> {
    let mask = CAShapeLayer::new();
    if clip.size.width <= 0.0 || clip.size.height <= 0.0 {
        return mask;
    }
    let page = unsafe { CGPath::with_rect(clip, ptr::null()) };
    let cuts = CGMutablePath::new();
    for (rect, radius) in covers {
        if radius == 0.0 {
            unsafe { CGMutablePath::add_rect(Some(&cuts), ptr::null(), rect) };
        } else {
            unsafe {
                CGMutablePath::add_rounded_rect(Some(&cuts), ptr::null(), rect, radius, radius);
            }
        }
    }
    let visible = CGPath::new_copy_by_subtracting_path(Some(&page), Some(&cuts), false);
    mask.setPath(visible.as_deref());
    mask
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
    let shortcuts = focus.shortcuts.borrow().clone();
    let kept = shortcuts
        .iter()
        .any(|key| key.key == keystroke.key && key.modifiers == keystroke.modifiers);
    if kept {
        return on_signal(Signal::Shortcut(keystroke));
    }
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
    fn a_live_page_keeps_its_rounded_cutouts_and_respects_overlapping_covers() {
        let rect = |x, y, w, h| NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
        let page = rect(0.0, 0.0, 100.0, 80.0);
        let covers = [
            (rect(-20.0, -10.0, 90.0, 70.0), 16.0),
            (rect(50.0, 10.0, 55.0, 25.0), 0.0),
        ];
        let regions = HitTestRegions::default();
        *regions.mask.borrow_mut() = Some(visible_mask(page, covers.into_iter()));
        let hits_page = |x, y| {
            let point = NSPoint::new(x, y);
            !regions.excludes(point, point)
        };
        assert!(hits_page(69.0, 59.0), "page at the rounded corner");
        assert!(!hits_page(64.0, 54.0), "inside the sidebar curve");
        assert!(!hits_page(60.0, 24.0), "where the two covers overlap");
        assert!(!hits_page(85.0, 22.0), "inside only the second cover");
        assert!(hits_page(85.0, 50.0), "uncovered page");
        assert!(!hits_page(105.0, 50.0), "outside the content clip");
        *regions.mask.borrow_mut() = Some(visible_mask(page, std::iter::empty()));
        assert!(hits_page(60.0, 24.0), "page restored when the covers leave");
        assert!(!hits_page(105.0, 50.0), "the content clip still applies");
        assert!(hits_page(2.0, 40.0));
        regions
            .pointer_exclusions
            .borrow_mut()
            .push(rect(0.0, 0.0, 8.0, 80.0));
        assert!(
            !hits_page(2.0, 40.0),
            "the edge can receive input over the visible page"
        );
        assert!(hits_page(9.0, 40.0));
    }

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
