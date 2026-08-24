use super::{Point, Rect};
use serde::{Deserialize, Serialize};

pub const DRAG_ACTIVATION_DISTANCE: f32 = 4.0;
pub const TOP_LEVEL_VERTICAL_TRANSITION: f32 = 24.0;
pub const DROP_ZONE_RATIO: f32 = 0.3;
pub const DROP_ZONE_SNAP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DropZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

impl DropZone {
    pub fn at(point: Point, bounds: Rect) -> Option<Self> {
        if !bounds.contains_with_snap(point, DROP_ZONE_SNAP) {
            return None;
        }
        let point = bounds.clamped_point(point);
        let horizontal_zone = bounds.width.max(0.0) * DROP_ZONE_RATIO;
        let vertical_zone = bounds.height.max(0.0) * DROP_ZONE_RATIO;
        if point.x <= bounds.x + horizontal_zone {
            Some(Self::Left)
        } else if point.x >= bounds.max_x() - horizontal_zone {
            Some(Self::Right)
        } else if point.y <= bounds.y + vertical_zone {
            Some(Self::Top)
        } else if point.y >= bounds.max_y() - vertical_zone {
            Some(Self::Bottom)
        } else {
            Some(Self::Center)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DragCoordinator {
    origin: Option<Point>,
    pointer: Option<Point>,
    active: bool,
}

impl Default for DragCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl DragCoordinator {
    pub const fn new() -> Self {
        Self {
            origin: None,
            pointer: None,
            active: false,
        }
    }

    pub fn begin(&mut self, origin: Point) {
        self.origin = Some(origin);
        self.pointer = Some(origin);
        self.active = false;
    }

    pub fn update(&mut self, pointer: Point) -> bool {
        let Some(origin) = self.origin else {
            return false;
        };
        self.pointer = Some(pointer);
        if !self.active && origin.distance_to(pointer) >= DRAG_ACTIVATION_DISTANCE {
            self.active = true;
        }
        self.active
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn top_level_transitioned(&self) -> bool {
        match (self.origin, self.pointer) {
            (Some(origin), Some(pointer)) => {
                (pointer.y - origin.y).abs() >= TOP_LEVEL_VERTICAL_TRANSITION
            }
            _ => false,
        }
    }

    pub fn drop_zone(&self, bounds: Rect) -> Option<DropZone> {
        self.active
            .then(|| self.pointer.and_then(|point| DropZone::at(point, bounds)))
            .flatten()
    }

    pub fn finish(&mut self) -> Option<Point> {
        if !self.active {
            self.cancel();
            return None;
        }
        let pointer = self.pointer;
        self.cancel();
        pointer
    }

    pub fn cancel(&mut self) {
        self.origin = None;
        self.pointer = None;
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_exact_activation_and_top_level_transition_distances() {
        let mut drag = DragCoordinator::new();
        drag.begin(Point::new(10.0, 10.0));
        assert!(!drag.update(Point::new(13.9, 10.0)));
        assert!(drag.update(Point::new(14.0, 10.0)));
        assert!(!drag.top_level_transitioned());
        drag.update(Point::new(14.0, 34.0));
        assert!(drag.top_level_transitioned());
    }

    #[test]
    fn resolves_thirty_percent_zones_x_before_y_with_eight_point_snap() {
        let bounds = Rect::new(100.0, 100.0, 100.0, 100.0);
        assert_eq!(
            DropZone::at(Point::new(95.0, 95.0), bounds),
            Some(DropZone::Left)
        );
        assert_eq!(
            DropZone::at(Point::new(125.0, 125.0), bounds),
            Some(DropZone::Left)
        );
        assert_eq!(
            DropZone::at(Point::new(175.0, 125.0), bounds),
            Some(DropZone::Right)
        );
        assert_eq!(
            DropZone::at(Point::new(150.0, 125.0), bounds),
            Some(DropZone::Top)
        );
        assert_eq!(
            DropZone::at(Point::new(150.0, 175.0), bounds),
            Some(DropZone::Bottom)
        );
        assert_eq!(
            DropZone::at(Point::new(150.0, 150.0), bounds),
            Some(DropZone::Center)
        );
        assert_eq!(DropZone::at(Point::new(91.9, 150.0), bounds), None);
    }
}
