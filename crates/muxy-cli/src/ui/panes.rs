//! The panes of the current tab: terminal content, frames, scrollback, and
//! selection.

use muxy_client::RunGrid;
use muxy_protocol::{Color as TerminalColor, Run, Style as TerminalStyle, Underline};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

use super::layout::{self, PaneArea};
use super::{Mode, Overlay, Target, Ui, clean, theme, truncate};
use crate::selection::{self, Point, Selection};
use crate::state::State;
use crate::worker::{Shared, View, hosting_session};

pub(super) fn draw(frame: &mut Frame<'_>, shared: &Shared, state: &State, ui: &mut Ui, body: Rect) {
    let Some(tab) = state.tab() else {
        empty(frame, ui, body);
        return;
    };
    for area in layout::panes(state, body) {
        let focused = tab.focus == area.id;
        let view = shared.views.get(&area.id);
        ui.hits.push((area.outer, Target::Pane(area.id)));
        ui.hits.push((area.inner, Target::Content(area.id)));
        if area.framed {
            frame_pane(frame, shared, state, &area, focused, view);
        }
        let Some(view) = view else {
            placeholder(frame, shared, state, &area);
            continue;
        };
        let selected = ui
            .selection
            .filter(|selection| selection.pane == area.id && !selection.is_empty())
            .map(|selection| selection.range(view.scroll.grid(&view.grid)));
        content(frame.buffer_mut(), area.inner, view, selected);
        if view.scroll.is_active() {
            scroll_position(frame, &area, view, focused);
        }
        if view.ended {
            frame.render_widget(
                Paragraph::new(" Ended — Ctrl-B x closes this pane ").style(theme::muted()),
                Rect::new(
                    area.inner.x,
                    area.inner.y,
                    area.inner.width,
                    area.inner.height.min(1),
                ),
            );
        } else if focused
            && ui.overlay == Overlay::None
            && matches!(ui.mode, Mode::Normal | Mode::Prefix)
            && !view.scroll.is_active()
            && view.grid.cursor.visible
            && view.grid.cursor.col < area.inner.width
            && view.grid.cursor.row < area.inner.height
        {
            frame.set_cursor_position((
                area.inner.x + view.grid.cursor.col,
                area.inner.y + view.grid.cursor.row,
            ));
        }
    }
    for divider in layout::dividers(state, body) {
        ui.hits.push((divider.area, Target::Divider(divider)));
    }
}

fn frame_pane(
    frame: &mut Frame<'_>,
    shared: &Shared,
    state: &State,
    area: &PaneArea,
    focused: bool,
    view: Option<&View>,
) {
    let border = if focused {
        theme::accent()
    } else {
        theme::line()
    };
    let mut title = vec![Span::styled(" ", border)];
    if let Some((dot, style)) = view
        .map(|view| super::session_activity(shared, view.session))
        .and_then(theme::activity)
    {
        title.push(Span::styled(format!("{dot} "), style));
    }
    let label = super::pane_title(shared, state, area.id);
    let width = usize::from(area.outer.width.saturating_sub(8));
    title.push(Span::styled(
        truncate(&label, width),
        if focused {
            theme::key()
        } else {
            theme::muted()
        },
    ));
    title.push(Span::styled(" ", border));
    frame.render_widget(
        Block::bordered()
            .border_style(border)
            .title(Line::from(title)),
        area.outer,
    );
}

fn placeholder(frame: &mut Frame<'_>, shared: &Shared, state: &State, area: &PaneArea) {
    let pane = state.tab().and_then(|tab| tab.panes.get(&area.id));
    let host = shared
        .catalog
        .as_ref()
        .and_then(|catalog| hosting_session(catalog.server));
    let text = if pane.is_some_and(|pane| pane.session.is_some() && pane.session == host) {
        "Hosting terminal excluded — Ctrl-B c opens another tab"
    } else {
        pane.and_then(|pane| pane.error.as_deref())
            .unwrap_or("Connecting…")
    };
    frame.render_widget(
        Paragraph::new(clean(text)).style(theme::muted()),
        area.inner,
    );
}

fn empty(frame: &mut Frame<'_>, ui: &mut Ui, body: Rect) {
    if body.height < 2 {
        frame.render_widget(Paragraph::new("No tabs. Ctrl-B c opens a terminal."), body);
        return;
    }
    let top = body.y + body.height.saturating_sub(4) / 2;
    let lines = [
        Line::styled("No tabs.", theme::strong()),
        Line::from(vec![
            Span::styled("Ctrl-B c", theme::key()),
            Span::styled(" opens a terminal · ", theme::muted()),
            Span::styled("Ctrl-B w", theme::key()),
            Span::styled(" picks an existing terminal", theme::muted()),
        ]),
    ];
    for (offset, line) in (0..).zip(lines) {
        frame.render_widget(
            Paragraph::new(line).alignment(Alignment::Center),
            Rect::new(body.x, top + offset, body.width, 1),
        );
    }
    let label = " + New terminal ";
    let width = u16::try_from(label.len()).unwrap_or(0).min(body.width);
    let button = Rect::new(body.x + (body.width - width) / 2, top + 3, width, 1).intersection(body);
    frame.render_widget(Paragraph::new(label).style(theme::pill()), button);
    ui.hits.push((button, Target::NewTab));
}

/// Shows how far back a pane is scrolled: a badge in its corner and a
/// scrollbar on its right edge.
fn scroll_position(frame: &mut Frame<'_>, area: &PaneArea, view: &View, focused: bool) {
    let height = usize::from(area.inner.height);
    let offset = view.scroll.offset();
    let badge = match view.scroll.total(height) {
        Some(total) => format!(" ↑ {offset}/{total} "),
        None => " ↑ loading… ".into(),
    };
    let width = u16::try_from(badge.chars().count())
        .unwrap_or(u16::MAX)
        .min(area.inner.width);
    let row = if area.framed {
        area.outer.y
    } else {
        area.inner.y
    };
    let right = if area.framed {
        area.outer.right().saturating_sub(2)
    } else {
        area.inner.right()
    };
    frame.render_widget(
        Paragraph::new(badge).style(if focused {
            theme::pill()
        } else {
            theme::button()
        }),
        Rect::new(right.saturating_sub(width), row, width, 1),
    );
    let Some(total) = view.scroll.total(height).filter(|total| *total > 0) else {
        return;
    };
    let track = area.inner.height.saturating_sub(1);
    let column = if area.framed {
        area.outer.right().saturating_sub(1)
    } else {
        area.inner.right().saturating_sub(1)
    };
    let content = total + height;
    let thumb = (height * usize::from(track) / content).max(1);
    let travel = usize::from(track).saturating_sub(thumb);
    let top = (total - offset.min(total)) * travel / total;
    let buffer = frame.buffer_mut();
    for (index, y) in (area.inner.y + 1..area.inner.y + 1 + track).enumerate() {
        if let Some(cell) = buffer.cell_mut((column, y)) {
            if (top..top + thumb).contains(&index) {
                cell.set_symbol("┃").set_style(theme::accent());
            } else if area.framed {
                cell.set_symbol("│").set_style(theme::line());
            }
        }
    }
}

fn content(buffer: &mut Buffer, inner: Rect, view: &View, selected: Option<(Point, Point)>) {
    let grid = view.scroll.grid(&view.grid);
    let count = selection::content_rows(grid);
    let start = view.scroll.start(&view.grid, usize::from(inner.height));
    for y in 0..inner.height {
        let index = start + usize::from(y);
        row(
            buffer,
            inner,
            inner.y + y,
            grid.content_row(index).unwrap_or_default(),
        );
        let Some(range) = selected else {
            continue;
        };
        let Some(back) = count.checked_sub(index + 1) else {
            continue;
        };
        for col in 0..inner.width {
            if Selection::contains(range, Point { back, col })
                && let Some(cell) = buffer.cell_mut((inner.x + col, inner.y + y))
            {
                let style = cell.style();
                cell.set_style(style.add_modifier(Modifier::REVERSED));
            }
        }
    }
}

/// Draws one terminal row at `y`, clipped to `area`'s columns.
pub(crate) fn row(buffer: &mut Buffer, area: Rect, y: u16, runs: &[Run]) {
    for x in area.x..area.right() {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.reset();
        }
    }
    let mut x = area.x;
    for run in runs {
        if x >= area.right() {
            break;
        }
        let width = run.width.min(area.right() - x);
        let style = style(run.style);
        let safe = !run.text.chars().any(char::is_control)
            && Span::raw(&run.text).width() == usize::from(run.width);
        if safe && (width == run.width || run.text.is_ascii()) {
            buffer.set_stringn(x, y, &run.text, usize::from(width), style);
        } else {
            buffer.set_stringn(
                x,
                y,
                "?".repeat(usize::from(width)),
                usize::from(width),
                style,
            );
        }
        x = x.saturating_add(run.width);
    }
}

fn style(source: TerminalStyle) -> Style {
    let mut style = Style::new()
        .fg(color(source.fg))
        .bg(color(source.bg))
        .underline_color(color(source.underline_color));
    for (enabled, modifier) in [
        (source.bold, Modifier::BOLD),
        (source.italic, Modifier::ITALIC),
        (source.faint, Modifier::DIM),
        (source.inverse, Modifier::REVERSED),
        (source.invisible, Modifier::HIDDEN),
        (source.strikethrough, Modifier::CROSSED_OUT),
        (source.underline != Underline::None, Modifier::UNDERLINED),
    ] {
        if enabled {
            style = style.add_modifier(modifier);
        }
    }
    style
}

fn color(source: TerminalColor) -> Color {
    match source {
        TerminalColor::Default => Color::Reset,
        TerminalColor::Indexed(value) => Color::Indexed(value),
        TerminalColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// The content row and column under a screen cell of `view`'s pane.
pub(crate) fn point(view: &View, inner: Rect, column: u16, row: u16) -> Point {
    let grid: &RunGrid = view.scroll.grid(&view.grid);
    let y = row
        .saturating_sub(inner.y)
        .min(inner.height.saturating_sub(1));
    let index = view.scroll.start(&view.grid, usize::from(inner.height)) + usize::from(y);
    Point {
        back: selection::content_rows(grid).saturating_sub(index + 1),
        col: column
            .saturating_sub(inner.x)
            .min(inner.width.saturating_sub(1)),
    }
}
