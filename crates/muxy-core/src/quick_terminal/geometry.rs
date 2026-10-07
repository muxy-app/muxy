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

pub fn panel_frame(screen: Rect, visible: Rect, preferred: Size) -> Rect {
    if screen.size.width <= 0.0
        || screen.size.height <= 0.0
        || visible.size.width <= 0.0
        || visible.size.height <= 0.0
        || preferred.width <= 0.0
        || preferred.height <= 0.0
    {
        return Rect::default();
    }
    let size = Size {
        width: preferred.width.min(visible.size.width),
        height: preferred.height.min(visible.size.height),
    };
    let centered_x = screen.origin.x + screen.size.width / 2.0 - size.width / 2.0;
    let minimum_x = visible.origin.x;
    let maximum_x = minimum_x.max(visible.origin.x + visible.size.width - size.width);
    Rect::new(
        centered_x.clamp(minimum_x, maximum_x),
        visible.origin.y + visible.size.height - size.height,
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
