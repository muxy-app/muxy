use gpui::{Pixels, Point, Window};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(not(target_os = "macos"))]
use unsupported as backend;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint {
    x: f64,
    y: f64,
}

pub fn cursor() -> Option<ScreenPoint> {
    backend::cursor()
}

pub fn local(window: &Window, point: ScreenPoint) -> Option<Point<Pixels>> {
    backend::local(window, point)
}

pub fn window_number(window: &Window) -> Option<isize> {
    backend::window_number(window)
}

pub fn window_at(point: ScreenPoint, below: Option<isize>) -> Option<isize> {
    backend::window_at(point, below)
}

pub fn move_window_to(window: &Window, point: ScreenPoint, grab: Point<Pixels>) -> bool {
    backend::move_window_to(window, point, grab)
}

pub fn keep_on_screen(window: &Window) {
    backend::keep_on_screen(window);
}

pub fn set_window_alpha(window: &Window, alpha: f32) {
    backend::set_window_alpha(window, alpha);
}

pub fn set_ignores_mouse(window: &Window, ignores: bool) {
    backend::set_ignores_mouse(window, ignores);
}

pub fn can_place_windows() -> bool {
    backend::CAN_PLACE_WINDOWS
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn flip(x: f64, y: f64, height: f64) -> Point<Pixels> {
    #[allow(clippy::cast_possible_truncation)]
    gpui::point(gpui::px(x as f32), gpui::px((height - y) as f32))
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn unflip(grab: Point<Pixels>, height: f64) -> (f64, f64) {
    (
        f64::from(f32::from(grab.x)),
        height - f64::from(f32::from(grab.y)),
    )
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn shifted(origin: ScreenPoint, from: ScreenPoint, to: ScreenPoint) -> ScreenPoint {
    ScreenPoint {
        x: origin.x + to.x - from.x,
        y: origin.y + to.y - from.y,
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn within(start: f64, length: f64, area_start: f64, area_length: f64) -> f64 {
    start.min(area_start + area_length - length).max(area_start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px};

    #[test]
    fn flip_measures_from_the_top_of_the_content() {
        assert_eq!(flip(10.0, 590.0, 600.0), point(px(10.0), px(10.0)));
        assert_eq!(flip(-5.0, 700.0, 600.0), point(px(-5.0), px(-100.0)));
        assert_eq!(flip(900.0, -50.0, 600.0), point(px(900.0), px(650.0)));
    }

    #[test]
    fn within_moves_a_span_back_inside_its_area_or_to_its_start() {
        assert!((within(1800.0, 900.0, 0.0, 2000.0) - 1100.0).abs() < f64::EPSILON);
        assert!((within(-300.0, 900.0, 0.0, 2000.0)).abs() < f64::EPSILON);
        assert!((within(400.0, 900.0, 0.0, 2000.0) - 400.0).abs() < f64::EPSILON);
        assert!((within(500.0, 2500.0, 100.0, 2000.0) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn unflip_undoes_flip() {
        let grab = point(px(42.0), px(17.5));
        let (x, y) = unflip(grab, 600.0);
        assert_eq!(flip(x, y, 600.0), grab);
    }

    #[test]
    fn shifted_moves_the_frame_by_the_pointer_offset() {
        let origin = ScreenPoint { x: 100.0, y: 200.0 };
        let from = ScreenPoint { x: 150.0, y: 700.0 };
        let to = ScreenPoint {
            x: -1200.0,
            y: 1500.0,
        };
        assert_eq!(
            shifted(origin, from, to),
            ScreenPoint {
                x: -1250.0,
                y: 1000.0
            }
        );
        assert_eq!(shifted(origin, from, from), origin);
    }
}
