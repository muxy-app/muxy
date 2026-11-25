use gpui::{Pixels, Point, Window};

use super::ScreenPoint;

pub(super) const CAN_PLACE_WINDOWS: bool = false;

pub(super) fn cursor() -> Option<ScreenPoint> {
    None
}

pub(super) fn local(_window: &Window, _point: ScreenPoint) -> Option<Point<Pixels>> {
    None
}

pub(super) fn window_number(_window: &Window) -> Option<isize> {
    None
}

pub(super) fn window_at(_point: ScreenPoint, _below: Option<isize>) -> Option<isize> {
    None
}

pub(super) fn move_window_to(_window: &Window, _point: ScreenPoint, _grab: Point<Pixels>) -> bool {
    false
}

pub(super) fn keep_on_screen(_window: &Window) {}

pub(super) fn set_window_alpha(_window: &Window, _alpha: f32) {}

pub(super) fn set_ignores_mouse(_window: &Window, _ignores: bool) {}
