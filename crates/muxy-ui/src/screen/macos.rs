use gpui::{Pixels, Point, Window};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEvent, NSScreen, NSWindow};
use objc2_foundation::NSPoint;

use super::{ScreenPoint, flip, shifted, unflip, within};
use crate::appkit::ns_window;

pub(super) const CAN_PLACE_WINDOWS: bool = true;

#[allow(clippy::unnecessary_wraps)]
pub(super) fn cursor() -> Option<ScreenPoint> {
    let location = NSEvent::mouseLocation();
    Some(ScreenPoint {
        x: location.x,
        y: location.y,
    })
}

/// GPUI measures mouse positions from the top of the content view, flipping
/// the event's location in the window.
pub(super) fn local(window: &Window, point: ScreenPoint) -> Option<Point<Pixels>> {
    let window = ns_window(window).ok()?;
    let height = window.contentView()?.frame().size.height;
    let location = window.convertPointFromScreen(NSPoint::new(point.x, point.y));
    Some(flip(location.x, location.y, height))
}

pub(super) fn window_number(window: &Window) -> Option<isize> {
    Some(ns_window(window).ok()?.windowNumber())
}

pub(super) fn window_at(point: ScreenPoint, below: Option<isize>) -> Option<isize> {
    let mtm = MainThreadMarker::new()?;
    let number = NSWindow::windowNumberAtPoint_belowWindowWithWindowNumber(
        NSPoint::new(point.x, point.y),
        below.unwrap_or(0),
        mtm,
    );
    if number <= 0 {
        return None;
    }
    NSApplication::sharedApplication(mtm)
        .windowWithWindowNumber(number)
        .map(|_| number)
}

pub(super) fn move_window_to(window: &Window, point: ScreenPoint, grab: Point<Pixels>) -> bool {
    let Ok(window) = ns_window(window) else {
        return false;
    };
    let Some(content) = window.contentView() else {
        return false;
    };
    let (x, y) = unflip(grab, content.frame().size.height);
    let from = window.convertPointToScreen(NSPoint::new(x, y));
    let origin = window.frame().origin;
    let origin = shifted(
        ScreenPoint {
            x: origin.x,
            y: origin.y,
        },
        ScreenPoint {
            x: from.x,
            y: from.y,
        },
        point,
    );
    window.setFrameOrigin(NSPoint::new(origin.x, origin.y));
    true
}

pub(super) fn keep_on_screen(window: &Window) {
    let Ok(window) = ns_window(window) else {
        return;
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(screen) = window.screen().or_else(|| NSScreen::mainScreen(mtm)) else {
        return;
    };
    let area = screen.visibleFrame();
    let frame = window.frame();
    let x = within(
        frame.origin.x,
        frame.size.width,
        area.origin.x,
        area.size.width,
    );
    let y = within(
        frame.origin.y,
        frame.size.height,
        area.origin.y,
        area.size.height,
    );
    if (x - frame.origin.x).abs() > f64::EPSILON || (y - frame.origin.y).abs() > f64::EPSILON {
        window.setFrameOrigin(NSPoint::new(x, y));
    }
}

pub(super) fn set_window_alpha(window: &Window, alpha: f32) {
    if let Ok(window) = ns_window(window) {
        window.setAlphaValue(f64::from(alpha.clamp(0.0, 1.0)));
    }
}

pub(super) fn set_ignores_mouse(window: &Window, ignores: bool) {
    if let Ok(window) = ns_window(window) {
        window.setIgnoresMouseEvents(ignores);
    }
}
