//! Where everything goes on screen. The worker sizes terminals from the same
//! geometry the screen is drawn with.

use std::ops::RangeInclusive;

use muxy_app_core::{Axis, Layout, PaneId};
use muxy_protocol::Size;
use ratatui::layout::Rect;

use crate::state::State;

pub(crate) const SIDEBAR_WIDTH: u16 = 26;
pub(crate) const SIDEBAR_WIDTHS: RangeInclusive<u16> = 18..=40;
/// Narrower screens give the sidebar's room to the panes.
const NARROW: u16 = 60;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Chrome {
    pub sidebar: Rect,
    pub tabs: Rect,
    /// Where panes go.
    pub body: Rect,
}

impl Chrome {
    /// Lays out `area` with a sidebar `sidebar` columns wide, if shown.
    pub(crate) fn new(area: Rect, sidebar: Option<u16>) -> Self {
        let width = match sidebar {
            Some(width) if area.width >= NARROW => width
                .clamp(*SIDEBAR_WIDTHS.start(), *SIDEBAR_WIDTHS.end())
                .min(area.width / 2),
            _ => 0,
        };
        let main = Rect::new(area.x + width, area.y, area.width - width, area.height);
        let tab_rows = u16::from(main.height > 1);
        Self {
            sidebar: Rect::new(area.x, area.y, width, area.height),
            tabs: Rect::new(main.x, main.y, main.width, tab_rows),
            body: Rect::new(
                main.x,
                main.y + tab_rows,
                main.width,
                main.height - tab_rows,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PaneArea {
    pub id: PaneId,
    pub outer: Rect,
    /// Where the terminal is drawn: inside the frame, if the pane has one.
    pub inner: Rect,
    pub framed: bool,
}

/// The panes of the current tab inside `body`. A lone pane has no frame.
pub(crate) fn panes(state: &State, body: Rect) -> Vec<PaneArea> {
    let Some(tab) = state.tab() else {
        return Vec::new();
    };
    let framed = tab.panes.len() > 1;
    let mut leaves = Vec::new();
    if tab.zoom {
        leaves.push((tab.focus, body));
    } else {
        divide(&tab.layout, body, &mut leaves, &mut Vec::new());
    }
    leaves
        .into_iter()
        .map(|(id, outer)| PaneArea {
            id,
            outer,
            inner: if framed { shrink(outer) } else { outer },
            framed,
        })
        .collect()
}

/// A border between two split panes that can be dragged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Divider {
    /// The cells that grab it: the facing borders of both sides.
    pub area: Rect,
    pub axis: Axis,
    /// The whole split.
    pub span: Rect,
    /// A pane on each side, which names the split even if the layout changes.
    pub first: PaneId,
    pub second: PaneId,
}

impl Divider {
    /// The ratio that puts the border under the pointer.
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn ratio(&self, column: u16, row: u16) -> f32 {
        let (position, start, length) = match self.axis {
            Axis::Horizontal => (column, self.span.x, self.span.width),
            Axis::Vertical => (row, self.span.y, self.span.height),
        };
        f32::from(position.saturating_add(1).saturating_sub(start)) / f32::from(length.max(1))
    }
}

pub(crate) fn dividers(state: &State, body: Rect) -> Vec<Divider> {
    let mut dividers = Vec::new();
    if let Some(tab) = state.tab().filter(|tab| !tab.zoom) {
        divide(&tab.layout, body, &mut Vec::new(), &mut dividers);
    }
    dividers
}

fn divide(
    layout: &Layout,
    area: Rect,
    leaves: &mut Vec<(PaneId, Rect)>,
    dividers: &mut Vec<Divider>,
) {
    match layout {
        Layout::Leaf(id) => leaves.push((*id, area)),
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let mut a = area;
            let mut b = area;
            let grab = match axis {
                Axis::Horizontal => {
                    a.width = split_extent(area.width, *ratio);
                    b.x += a.width;
                    b.width -= a.width;
                    Rect::new(b.x.saturating_sub(1).max(area.x), area.y, 2, area.height)
                }
                Axis::Vertical => {
                    a.height = split_extent(area.height, *ratio);
                    b.y += a.height;
                    b.height -= a.height;
                    Rect::new(area.x, b.y.saturating_sub(1).max(area.y), area.width, 2)
                }
            };
            if let (Some(first_pane), Some(second_pane)) =
                (first.leaves().first(), second.leaves().first())
            {
                dividers.push(Divider {
                    area: grab.intersection(area),
                    axis: *axis,
                    span: area,
                    first: *first_pane,
                    second: *second_pane,
                });
            }
            divide(first, a, leaves, dividers);
            divide(second, b, leaves, dividers);
        }
    }
}

fn split_extent(length: u16, ratio: f32) -> u16 {
    let target = f32::from(length) * ratio;
    let (mut lower, mut upper) = (0, length);
    // Find the nearest cell boundary within the original integer extent.
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if f32::from(middle) + 0.5 <= target {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    lower
}

fn shrink(area: Rect) -> Rect {
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

pub(crate) fn terminal_size(inner: Rect) -> Option<Size> {
    (inner.width > 0 && inner.height > 0).then(|| Size {
        cols: inner.width.min(muxy_protocol::MAX_COLS),
        rows: inner.height.min(muxy_protocol::MAX_ROWS),
    })
}
