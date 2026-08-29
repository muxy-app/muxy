#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.x < self.origin.x + self.size.width
            && point.y >= self.origin.y
            && point.y < self.origin.y + self.size.height
    }
}

pub const PANEL_TOP_GAP: f64 = 12.0;

pub fn panel_frame(screen: Rect, visible: Rect, preferred: Size) -> Rect {
    if screen.size.width <= 0.0
        || screen.size.height <= 0.0
        || visible.size.width <= 0.0
        || visible.size.height <= PANEL_TOP_GAP
        || preferred.width <= 0.0
        || preferred.height <= 0.0
    {
        return Rect::default();
    }
    let size = Size {
        width: preferred.width.min(visible.size.width),
        height: preferred.height.min(visible.size.height - PANEL_TOP_GAP),
    };
    let centered_x = screen.origin.x + screen.size.width / 2.0 - size.width / 2.0;
    let minimum_x = visible.origin.x;
    let maximum_x = minimum_x.max(visible.origin.x + visible.size.width - size.width);
    Rect::new(
        centered_x.clamp(minimum_x, maximum_x),
        visible.origin.y + visible.size.height - PANEL_TOP_GAP - size.height,
        size.width,
        size.height,
    )
}

pub fn preferred_screen_index(
    mouse: Point,
    screens: &[Rect],
    key_window: Option<usize>,
    main_window: Option<usize>,
    main_screen: Option<usize>,
) -> Option<usize> {
    screens
        .iter()
        .position(|screen| screen.contains(mouse))
        .or_else(|| {
            [key_window, main_window, main_screen]
                .into_iter()
                .flatten()
                .find(|candidate| *candidate < screens.len())
        })
        .or_else(|| (!screens.is_empty()).then_some(0))
}

pub fn cutout_rect(
    screen: Rect,
    safe_area_top: f64,
    left_auxiliary_width: Option<f64>,
    right_auxiliary_width: Option<f64>,
) -> Option<Rect> {
    let left = left_auxiliary_width?;
    let right = right_auxiliary_width?;
    if safe_area_top <= 0.0 || screen.size.width <= 0.0 {
        return None;
    }
    let width = screen.size.width - left - right;
    (width > 0.0).then(|| {
        Rect::new(
            screen.origin.x + left,
            screen.origin.y + screen.size.height - safe_area_top,
            width,
            safe_area_top,
        )
    })
}

pub fn collapsed_rect(cutout: Rect, panel: Rect) -> Rect {
    Rect::new(
        cutout.origin.x - panel.origin.x,
        panel.size.height - cutout.size.height,
        cutout.size.width,
        cutout.size.height,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        Point, Rect, Size, collapsed_rect, cutout_rect, panel_frame, preferred_screen_index,
    };

    #[test]
    fn quick_terminal_geometry_centers_clamps_and_insets_from_the_visible_top() {
        let screen = Rect::new(100.0, 50.0, 1200.0, 900.0);
        let visible = Rect::new(140.0, 80.0, 1100.0, 820.0);
        assert_eq!(
            panel_frame(
                screen,
                visible,
                Size {
                    width: 720.0,
                    height: 430.0,
                },
            ),
            Rect::new(340.0, 458.0, 720.0, 430.0)
        );
        assert_eq!(
            panel_frame(
                screen,
                visible,
                Size {
                    width: 2000.0,
                    height: 1000.0,
                },
            ),
            Rect::new(140.0, 80.0, 1100.0, 808.0)
        );
        assert_eq!(
            panel_frame(
                Rect::default(),
                visible,
                Size {
                    width: 1.0,
                    height: 1.0
                }
            ),
            Rect::default()
        );
    }

    #[test]
    fn quick_terminal_geometry_prefers_pointer_then_window_and_display_fallbacks() {
        let screens = [
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(100.0, 0.0, 100.0, 100.0),
        ];
        assert_eq!(
            preferred_screen_index(Point { x: 150.0, y: 50.0 }, &screens, Some(0), None, None),
            Some(1)
        );
        assert_eq!(
            preferred_screen_index(
                Point { x: 300.0, y: 50.0 },
                &screens,
                Some(1),
                Some(0),
                None
            ),
            Some(1)
        );
        assert_eq!(
            preferred_screen_index(
                Point { x: 300.0, y: 50.0 },
                &screens,
                Some(8),
                Some(0),
                None
            ),
            Some(0)
        );
        assert_eq!(
            preferred_screen_index(Point::default(), &[], None, None, None),
            None
        );
    }

    #[test]
    fn quick_terminal_cutout_converts_to_panel_coordinates() {
        let screen = Rect::new(100.0, 50.0, 1000.0, 800.0);
        let cutout = cutout_rect(screen, 40.0, Some(420.0), Some(420.0)).unwrap();
        assert_eq!(cutout, Rect::new(520.0, 810.0, 160.0, 40.0));
        let panel = Rect::new(300.0, 500.0, 600.0, 350.0);
        assert_eq!(
            collapsed_rect(cutout, panel),
            Rect::new(220.0, 310.0, 160.0, 40.0)
        );
        assert!(cutout_rect(screen, 0.0, Some(1.0), Some(1.0)).is_none());
        assert!(cutout_rect(screen, 40.0, None, Some(1.0)).is_none());
        assert!(cutout_rect(screen, 40.0, Some(600.0), Some(600.0)).is_none());
    }
}
