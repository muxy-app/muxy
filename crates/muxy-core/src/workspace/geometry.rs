use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    pub const fn axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Top | Self::Bottom => Axis::Vertical,
        }
    }

    pub const fn is_before(self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(self, other: Self) -> f32 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn max_x(self) -> f32 {
        self.x + self.width.max(0.0)
    }

    pub fn max_y(self) -> f32 {
        self.y + self.height.max(0.0)
    }

    pub fn mid_x(self) -> f32 {
        self.x + self.width.max(0.0) / 2.0
    }

    pub fn mid_y(self) -> f32 {
        self.y + self.height.max(0.0) / 2.0
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x && point.x <= self.max_x() && point.y >= self.y && point.y <= self.max_y()
    }

    pub fn contains_with_snap(self, point: Point, snap: f32) -> bool {
        let snap = finite_non_negative(snap);
        point.x >= self.x - snap
            && point.x <= self.max_x() + snap
            && point.y >= self.y - snap
            && point.y <= self.max_y() + snap
    }

    pub fn clamped_point(self, point: Point) -> Point {
        Point::new(
            point.x.clamp(self.x, self.max_x()),
            point.y.clamp(self.y, self.max_y()),
        )
    }

    pub fn split(self, axis: Axis, ratio: f32) -> (Self, Self) {
        let ratio = ratio.clamp(0.0, 1.0);
        match axis {
            Axis::Horizontal => {
                let first_width = self.width.max(0.0) * ratio;
                (
                    Self::new(self.x, self.y, first_width, self.height),
                    Self::new(
                        self.x + first_width,
                        self.y,
                        self.width.max(0.0) - first_width,
                        self.height,
                    ),
                )
            }
            Axis::Vertical => {
                let first_height = self.height.max(0.0) * ratio;
                (
                    Self::new(self.x, self.y, self.width, first_height),
                    Self::new(
                        self.x,
                        self.y + first_height,
                        self.width,
                        self.height.max(0.0) - first_height,
                    ),
                )
            }
        }
    }
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_rectangles_on_both_axes() {
        let rect = Rect::new(10.0, 20.0, 300.0, 200.0);
        let (left, right) = rect.split(Axis::Horizontal, 0.25);
        assert_eq!(left, Rect::new(10.0, 20.0, 75.0, 200.0));
        assert_eq!(right, Rect::new(85.0, 20.0, 225.0, 200.0));

        let (top, bottom) = rect.split(Axis::Vertical, 0.5);
        assert_eq!(top, Rect::new(10.0, 20.0, 300.0, 100.0));
        assert_eq!(bottom, Rect::new(10.0, 120.0, 300.0, 100.0));
    }
}
