use gpui::{Bounds, Pixels, Point, point, px};
use muxy_app_core::{Direction, Layout, PaneId};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DropTarget {
    Swap(PaneId),
    /// Beside `pane`, or beside the enclosing row or column `level` picks.
    Dock {
        pane: PaneId,
        edge: Direction,
        level: usize,
    },
}

impl DropTarget {
    pub(super) fn apply(&self, layout: &Layout, source: PaneId) -> Option<Layout> {
        match self {
            Self::Swap(target) => layout.moved(source, *target, None),
            Self::Dock { pane, edge, level } => layout.docked(source, *pane, *edge, *level),
        }
    }
}

const EDGES: [Direction; 4] = [
    Direction::Left,
    Direction::Right,
    Direction::Up,
    Direction::Down,
];

/// Share of a pane, from each edge, that docks instead of swapping.
const EDGE_ZONE: f32 = 0.3;

/// Widest band each enclosing row or column gets, nearest the shared edge.
const BAND: Pixels = px(16.0);

fn distances(position: Point<Pixels>, bounds: Bounds<Pixels>) -> [Pixels; 4] {
    [
        position.x - bounds.left(),
        bounds.right() - position.x,
        position.y - bounds.top(),
        bounds.bottom() - position.y,
    ]
}

pub(super) fn within(position: Point<Pixels>, bounds: Bounds<Pixels>) -> bool {
    bounds.size.width > px(0.0)
        && bounds.size.height > px(0.0)
        && bounds.dilate(px(8.0)).contains(&position)
}

pub(super) fn target_at(
    layout: &Layout,
    bounds: Bounds<Pixels>,
    scale: f32,
    position: Point<Pixels>,
) -> Option<DropTarget> {
    if !within(position, bounds) {
        return None;
    }
    let position = point(
        position.x.clamp(bounds.left(), bounds.right()),
        position.y.clamp(bounds.top(), bounds.bottom()),
    );
    let (pane, bounds) = pane_at(layout, bounds, scale, position);
    let distance = distances(position, bounds);
    let zones = [
        bounds.size.width * EDGE_ZONE,
        bounds.size.width * EDGE_ZONE,
        bounds.size.height * EDGE_ZONE,
        bounds.size.height * EDGE_ZONE,
    ];
    // Where edge zones overlap, the outermost row or column wins, so outer
    // edges and the gaps between columns stay reliable where dividers cross.
    let target = (0..4)
        .filter(|&index| distance[index] < zones[index])
        .filter_map(|index| {
            let edge = EDGES[index];
            let depths = layout.dock_depths(pane, edge);
            let level = level_at(depths.len(), distance[index], zones[index]);
            let depth = *depths.get(level)?;
            Some((
                DropTarget::Dock { pane, edge, level },
                depth,
                distance[index],
            ))
        })
        .min_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| f32::from(a.2).total_cmp(&f32::from(b.2)))
        });
    Some(target.map_or(DropTarget::Swap(pane), |(target, ..)| target))
}

/// The drop level `distance` into an edge zone picks: bands for enclosing
/// rows and columns sit nearest the edge, outermost first, and the pane itself
/// takes the rest of the zone.
fn level_at(levels: usize, distance: Pixels, zone: Pixels) -> usize {
    let band = BAND.min(zone / f32::from(u8::try_from(levels).unwrap_or(u8::MAX)));
    let mut level = levels.saturating_sub(1);
    let mut reach = band;
    while level > 0 && distance >= reach {
        level -= 1;
        reach += band;
    }
    level
}

/// The pane under `position`; divider gaps belong to the nearer side.
fn pane_at(
    mut node: &Layout,
    mut bounds: Bounds<Pixels>,
    scale: f32,
    position: Point<Pixels>,
) -> (PaneId, Bounds<Pixels>) {
    loop {
        match node {
            Layout::Leaf(pane) => return (*pane, bounds),
            Layout::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let [a, _, b] = super::super::split_bounds(bounds, *axis, *ratio, scale);
                (node, bounds) = if outside_distance(position, a) <= outside_distance(position, b) {
                    (first, a)
                } else {
                    (second, b)
                };
            }
        }
    }
}

fn outside_distance(position: Point<Pixels>, bounds: Bounds<Pixels>) -> Pixels {
    distances(position, bounds)
        .into_iter()
        .map(|distance| (-distance).max(px(0.0)))
        .fold(px(0.0), |sum, distance| sum + distance)
}
