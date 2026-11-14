use gpui::{Bounds, Pixels, Window};
use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub(crate) fn gpui_view(window: &Window) -> Result<Retained<NSView>, String> {
    let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window)
        .map_err(|error| error.to_string())?
        .as_raw()
    else {
        return Err("expected AppKit window".into());
    };
    unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
        .ok_or_else(|| "missing native view".into())
}

pub(crate) fn ns_window(window: &Window) -> Result<Retained<NSWindow>, String> {
    gpui_view(window)?
        .window()
        .ok_or_else(|| "native window is detached".into())
}

pub(crate) fn frame_in(parent: &NSView, bounds: Bounds<Pixels>) -> NSRect {
    let top = f64::from(f32::from(bounds.top()));
    let height = f64::from(f32::from(bounds.size.height));
    let y = if parent.isFlipped() {
        top
    } else {
        parent.bounds().size.height - top - height
    };
    NSRect::new(
        NSPoint::new(f64::from(f32::from(bounds.left())), y),
        NSSize::new(f64::from(f32::from(bounds.size.width)), height),
    )
}
