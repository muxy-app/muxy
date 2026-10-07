//! Text selected with the mouse. Rows count up from the last content row,
//! so history paged in above a selection never moves it.

use muxy_app_core::PaneId;
use muxy_client::RunGrid;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Point {
    /// Rows above the last content row.
    pub back: usize,
    pub col: u16,
}

impl Point {
    fn before(self, other: Self) -> bool {
        (other.back, self.col) < (self.back, other.col)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Unit {
    Cell,
    Word,
    Line,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Selection {
    pub pane: PaneId,
    pub anchor: Point,
    pub head: Point,
    pub unit: Unit,
}

impl Selection {
    /// The first and last selected cells, widened to whole words or lines.
    pub(crate) fn range(&self, grid: &RunGrid) -> (Point, Point) {
        let (mut start, mut end) = if self.head.before(self.anchor) {
            (self.head, self.anchor)
        } else {
            (self.anchor, self.head)
        };
        match self.unit {
            Unit::Cell => {}
            Unit::Word => {
                start.col = word_edge(grid, start, false);
                end.col = word_edge(grid, end, true);
            }
            Unit::Line => {
                start.col = 0;
                end.col = grid.size.cols.saturating_sub(1);
            }
        }
        (start, end)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.unit == Unit::Cell && self.anchor == self.head
    }

    pub(crate) fn contains(range: (Point, Point), point: Point) -> bool {
        let (start, end) = range;
        !point.before(start) && !end.before(point)
    }

    pub(crate) fn text(&self, grid: &RunGrid) -> String {
        let (start, end) = self.range(grid);
        let count = content_rows(grid);
        let mut lines = Vec::new();
        for back in (end.back..=start.back).rev() {
            let Some(cells) = count
                .checked_sub(back + 1)
                .and_then(|index| row_cells(grid, index))
            else {
                continue;
            };
            let first = if back == start.back { start.col } else { 0 };
            let last = if back == end.back { end.col } else { u16::MAX };
            let line: String = cells
                .iter()
                .enumerate()
                .filter(|(col, _)| (usize::from(first)..=usize::from(last)).contains(col))
                .map(|(_, cell)| cell.as_str())
                .collect();
            lines.push(line.trim_end().to_owned());
        }
        lines.join("\n")
    }
}

pub(crate) fn content_rows(grid: &RunGrid) -> usize {
    grid.history.len() + grid.rows.len()
}

/// Each column's text as drawn; a wide character leaves the next column empty.
fn row_cells(grid: &RunGrid, index: usize) -> Option<Vec<String>> {
    let runs = grid.content_row(index)?;
    let area = Rect::new(0, 0, grid.size.cols.max(1), 1);
    let mut buffer = Buffer::empty(area);
    crate::ui::panes::row(&mut buffer, area, 0, runs);
    let mut cells = Vec::with_capacity(usize::from(area.width));
    while cells.len() < usize::from(area.width) {
        let x = u16::try_from(cells.len()).unwrap_or(u16::MAX);
        let symbol = buffer
            .cell((x, 0))
            .map(|cell| cell.symbol().to_owned())
            .unwrap_or_default();
        let width = Span::raw(&symbol).width().max(1);
        cells.push(symbol);
        cells.extend(std::iter::repeat_n(String::new(), width - 1));
    }
    cells.truncate(usize::from(area.width));
    Some(cells)
}

fn word_edge(grid: &RunGrid, point: Point, forward: bool) -> u16 {
    let Some(cells) = content_rows(grid)
        .checked_sub(point.back + 1)
        .and_then(|index| row_cells(grid, index))
    else {
        return point.col;
    };
    let word = |col: u16| {
        cells
            .get(usize::from(col))
            .is_some_and(|cell| !cell.is_empty() && cell.chars().all(is_word))
    };
    let mut col = point.col.min(
        u16::try_from(cells.len())
            .unwrap_or(u16::MAX)
            .saturating_sub(1),
    );
    if !word(col) {
        return col;
    }
    if forward {
        while col + 1 < u16::try_from(cells.len()).unwrap_or(u16::MAX) && word(col + 1) {
            col += 1;
        }
    } else {
        while col > 0 && word(col - 1) {
            col -= 1;
        }
    }
    col
}

fn is_word(character: char) -> bool {
    !character.is_whitespace() && !"()[]{}<>'\"`,;│".contains(character)
}
