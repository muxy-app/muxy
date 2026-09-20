//! Indeterminate progress drawn by Core Animation. The turning arc is a layer
//! animation, so it costs the app no frames.

use std::cell::Cell;
use std::ptr;

use gpui::{Bounds, Pixels, Rgba, Window};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::NSView;
use objc2_core_graphics::{CGColor, CGPath};
use objc2_foundation::{NSNumber, NSPoint, NSRect, NSSize, ns_string};
use objc2_quartz_core::{
    CABasicAnimation, CAMediaTiming, CAShapeLayer, CATransaction, kCALineCapRound,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

const LINE_FRACTION: f64 = 1.0 / 8.0;
const ARC_FRACTION: f64 = 0.28;
const TRACK_ALPHA: f32 = 0.25;

pub struct NativeSpinner {
    view: Retained<NSView>,
    parent: Retained<NSView>,
    track: Retained<CAShapeLayer>,
    arc: Retained<CAShapeLayer>,
    frame: Cell<Option<NSRect>>,
    color: Cell<Option<Rgba>>,
    visible: Cell<bool>,
}

impl std::fmt::Debug for NativeSpinner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeSpinner")
            .field("visible", &self.visible.get())
            .finish_non_exhaustive()
    }
}

impl NativeSpinner {
    pub fn new(window: &Window) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("spinner must be created on the main thread")?;
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window)
            .map_err(|error| error.to_string())?
            .as_raw()
        else {
            return Err("expected AppKit window".into());
        };
        let parent = unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
            .ok_or("missing native view")?;
        let view = NSView::initWithFrame(NSView::alloc(mtm), NSRect::ZERO);
        view.setWantsLayer(true);
        view.setHidden(true);
        let layer = view.layer().ok_or("spinner view has no layer")?;
        let track = CAShapeLayer::new();
        let arc = CAShapeLayer::new();
        for shape in [&track, &arc] {
            shape.setFillColor(None);
            layer.addSublayer(shape);
        }
        unsafe { arc.setLineCap(kCALineCapRound) };
        arc.setStrokeEnd(ARC_FRACTION);
        parent.addSubview(&view);
        Ok(Self {
            view,
            parent,
            track,
            arc,
            frame: Cell::new(None),
            color: Cell::new(None),
            visible: Cell::new(false),
        })
    }

    pub fn show(&self, bounds: Bounds<Pixels>, color: Rgba, visible: bool) {
        let frame = self.frame_in_parent(bounds);
        if self.frame.get() != Some(frame) {
            self.view.setFrame(frame);
            self.layout(frame.size);
            self.frame.set(Some(frame));
        }
        if self.color.get() != Some(color) {
            let track = Rgba {
                a: color.a * TRACK_ALPHA,
                ..color
            };
            stroke(&self.track, track);
            stroke(&self.arc, color);
            self.color.set(Some(color));
        }
        if self.visible.replace(visible) != visible {
            self.view.setHidden(!visible);
            if visible {
                self.turn();
            } else {
                self.arc.removeAllAnimations();
            }
        }
    }

    fn frame_in_parent(&self, bounds: Bounds<Pixels>) -> NSRect {
        let top = f64::from(f32::from(bounds.top()));
        let height = f64::from(f32::from(bounds.size.height));
        let y = if self.parent.isFlipped() {
            top
        } else {
            self.parent.bounds().size.height - top - height
        };
        NSRect::new(
            NSPoint::new(f64::from(f32::from(bounds.left())), y),
            NSSize::new(f64::from(f32::from(bounds.size.width)), height),
        )
    }

    fn layout(&self, size: NSSize) {
        let diameter = size.width.min(size.height);
        let line = (diameter * LINE_FRACTION).max(1.0);
        let inset = NSRect::new(
            NSPoint::new(
                f64::midpoint(size.width - diameter, line),
                f64::midpoint(size.height - diameter, line),
            ),
            NSSize::new(diameter - line, diameter - line),
        );
        let path = unsafe { CGPath::with_ellipse_in_rect(inset, ptr::null()) };
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        for shape in [&self.track, &self.arc] {
            shape.setFrame(NSRect::new(NSPoint::ZERO, size));
            shape.setPath(Some(&path));
            shape.setLineWidth(line);
        }
        CATransaction::commit();
    }

    fn turn(&self) {
        let animation =
            CABasicAnimation::animationWithKeyPath(Some(ns_string!("transform.rotation.z")));
        let from = NSNumber::new_f64(0.0);
        let to = NSNumber::new_f64(-std::f64::consts::TAU);
        unsafe {
            animation.setFromValue(Some(&from));
            animation.setToValue(Some(&to));
        }
        animation.setDuration(1.0);
        animation.setRepeatCount(f32::INFINITY);
        self.arc
            .addAnimation_forKey(&animation, Some(ns_string!("turn")));
    }
}

impl Drop for NativeSpinner {
    fn drop(&mut self) {
        self.view.removeFromSuperview();
    }
}

fn stroke(shape: &CAShapeLayer, color: Rgba) {
    let color = CGColor::new_srgb(
        f64::from(color.r),
        f64::from(color.g),
        f64::from(color.b),
        f64::from(color.a),
    );
    shape.setStrokeColor(Some(&color));
}
