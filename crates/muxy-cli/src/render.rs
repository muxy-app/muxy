use muxy_app_core::{Axis, Layout, PaneId};
use muxy_client::RunGrid;
use muxy_protocol::{Color as TerminalColor, Size, Style as TerminalStyle, Underline};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::state::State;
use crate::tui::Overlay;
use crate::worker::{Shared, hosting_session};

pub(crate) fn regions(state: &State, area: Rect) -> Vec<(PaneId, Rect)> {
    let body = Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(2),
    );
    let Some(tab) = state.tab() else {
        return Vec::new();
    };
    if tab.zoom {
        return vec![(tab.focus, body)];
    }
    let mut regions = Vec::new();
    divide(&tab.layout, body, &mut regions);
    regions
}

fn divide(layout: &Layout, area: Rect, regions: &mut Vec<(PaneId, Rect)>) {
    match layout {
        Layout::Leaf(id) => regions.push((*id, area)),
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let mut a = area;
            let mut b = area;
            match axis {
                Axis::Horizontal => {
                    a.width = split_extent(area.width, *ratio);
                    b.x += a.width;
                    b.width -= a.width;
                }
                Axis::Vertical => {
                    a.height = split_extent(area.height, *ratio);
                    b.y += a.height;
                    b.height -= a.height;
                }
            }
            divide(first, a, regions);
            divide(second, b, regions);
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

pub(crate) fn terminal_size(area: Rect) -> Option<Size> {
    (area.width > 2 && area.height > 2).then(|| Size {
        cols: area.width.saturating_sub(2).min(muxy_protocol::MAX_COLS),
        rows: area.height.saturating_sub(2).min(muxy_protocol::MAX_ROWS),
    })
}

pub(crate) fn draw(frame: &mut Frame<'_>, shared: &Shared, prefix: bool, overlay: &Overlay) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }
    let Some(state) = &shared.state else {
        frame.render_widget(Paragraph::new(clean(&shared.message)), area);
        return;
    };
    draw_header(frame, shared, state);
    for (id, rect) in regions(state, area) {
        let focus = state.tab().is_some_and(|tab| tab.focus == id);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::new().fg(if focus { Color::Cyan } else { Color::DarkGray }));
        let inner = block.inner(rect);
        frame.render_widget(block, rect);
        if let Some(view) = shared.views.get(&id) {
            grid(frame.buffer_mut(), inner, &view.grid);
            if view.ended {
                frame.render_widget(
                    Paragraph::new(" Ended — Ctrl-B x closes this pane ")
                        .style(Style::new().fg(Color::DarkGray)),
                    Rect::new(inner.x, inner.y, inner.width, inner.height.min(1)),
                );
            } else if focus
                && matches!(overlay, Overlay::None)
                && view.grid.cursor.visible
                && view.grid.cursor.col < inner.width
                && view.grid.cursor.row < inner.height
            {
                frame.set_cursor_position((
                    inner.x + view.grid.cursor.col,
                    inner.y + view.grid.cursor.row,
                ));
            }
        } else {
            let pane = state.tab().and_then(|tab| tab.panes.get(&id));
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
            frame.render_widget(Paragraph::new(clean(text)), inner);
        }
    }
    if state.tab().is_none() && area.height > 2 {
        frame.render_widget(
            Paragraph::new(
                " No tabs. Ctrl-B c opens a terminal; Ctrl-B w picks an existing terminal.",
            ),
            Rect::new(area.x, area.y + 1, area.width, area.height - 2),
        );
    }
    let status = if prefix {
        "Ctrl-B  c new · % / \" split · arrows focus · x close · d detach · ? help".into()
    } else if shared.message.is_empty() {
        "Ctrl-B ? help  ·  Ctrl-B s projects  ·  Ctrl-B d detach".into()
    } else {
        clean(&shared.message)
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::new().fg(Color::DarkGray)),
        Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
    );
    draw_overlay(frame, shared, overlay);
}

fn draw_header(frame: &mut Frame<'_>, shared: &Shared, state: &State) {
    let area = frame.area();
    let project_name = shared
        .catalog
        .as_ref()
        .and_then(|catalog| {
            catalog
                .projects
                .iter()
                .find(|project| project.id == state.active)
        })
        .map_or("Home", |project| project.name.as_str());
    let mut labels = vec![Span::styled(
        format!(" {} ", clean(project_name)),
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )];
    if let Some(project) = state.projects.get(&state.active) {
        for (index, tab) in project.tabs.iter().enumerate() {
            let title = shared
                .views
                .get(&tab.focus)
                .map_or("Terminal", |view| view.title.as_str());
            let style = if index == project.active {
                Style::new().add_modifier(Modifier::REVERSED)
            } else {
                Style::new()
            };
            labels.push(Span::styled(
                format!(
                    " {index}:{} ",
                    clean(title).chars().take(24).collect::<String>()
                ),
                style,
            ));
        }
    }
    let count = if shared.sessions_project == Some(state.active) {
        shared.sessions.len()
    } else {
        0
    };
    let existing = match count {
        0 => String::new(),
        _ => format!(" [▤ {count}] ^B w "),
    };
    let indicator_width = u16::try_from(existing.chars().count())
        .unwrap_or(u16::MAX)
        .min(area.width / 2);
    frame.render_widget(
        Paragraph::new(Line::from(labels)),
        Rect::new(
            area.x,
            area.y,
            area.width.saturating_sub(indicator_width),
            1,
        ),
    );
    frame.render_widget(
        Paragraph::new(existing).style(Style::new().fg(Color::Cyan)),
        Rect::new(
            area.right().saturating_sub(indicator_width),
            area.y,
            indicator_width,
            1,
        ),
    );
}

pub(crate) fn grid(buffer: &mut Buffer, area: Rect, grid: &RunGrid) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.reset();
            }
        }
    }
    for (index, row) in grid.rows.iter().take(usize::from(area.height)).enumerate() {
        let y = area.y + u16::try_from(index).unwrap_or(0);
        let mut x = area.x;
        for run in row {
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

pub(crate) fn clean(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn draw_overlay(frame: &mut Frame<'_>, shared: &Shared, overlay: &Overlay) {
    let (title, lines, selected) = match overlay {
        Overlay::None => return,
        Overlay::Help => (
            "Keyboard help",
            vec![
                "Ctrl-B, then: c new tab; n/p next/previous; 0–9 select tab".into(),
                "% side-by-side split; \" top-and-bottom split".into(),
                "Arrows focus; Ctrl + arrows resize; z zoom".into(),
                "x close pane; d detach; s projects; w existing terminals".into(),
                "Ctrl-B Ctrl-B sends Ctrl-B. Escape cancels.".into(),
                "Ctrl-C, Ctrl-D, typing and paste go to the focused terminal.".into(),
                "Detach keeps terminals running; close ends only an unshared terminal.".into(),
                "Desktop and CLI keep separate layouts. CLI instances share one layout.".into(),
                "Unlike tmux: no mouse, copy mode, scrollback keys, or custom bindings.".into(),
                "Graphics are omitted; unsupported text styles degrade to basic styles.".into(),
                "Escape / Enter closes help".into(),
            ],
            None,
        ),
        Overlay::Confirm(_) => (
            "Close terminal?",
            vec![
                "A foreground program may still be running.".into(),
                "Closing the last pane ends the terminal and discards its output.".into(),
                "Other open panes keep it running.".into(),
                "y / Enter: close     n / Escape: cancel".into(),
            ],
            None,
        ),
        Overlay::Projects(index) => (
            "Projects — arrows to choose, Enter to open",
            shared
                .catalog
                .as_ref()
                .map(|catalog| {
                    catalog
                        .projects
                        .iter()
                        .map(|project| clean(&project.name))
                        .collect()
                })
                .unwrap_or_default(),
            Some(*index),
        ),
        Overlay::Sessions(index) => (
            "Existing terminals — arrows to choose, Enter to open",
            session_lines(shared),
            Some(*index),
        ),
    };
    let area = frame.area();
    let width = area.width.saturating_sub(2).min(90);
    let height = area
        .height
        .saturating_sub(2)
        .min(u16::try_from(lines.len() + 2).unwrap_or(u16::MAX).max(3));
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let block = Block::bordered().title(title);
    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);
    let first = selected
        .unwrap_or(0)
        .saturating_sub(usize::from(inner.height).saturating_sub(1));
    let lines: Vec<Line<'_>> = lines
        .into_iter()
        .enumerate()
        .skip(first)
        .map(|(index, line)| {
            Line::styled(
                line,
                if Some(index) == selected {
                    Style::new().add_modifier(Modifier::REVERSED)
                } else {
                    Style::new()
                },
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn session_lines(shared: &Shared) -> Vec<String> {
    if shared.sessions.is_empty() {
        return vec!["No other terminals in this project".into()];
    }
    shared
        .sessions
        .iter()
        .map(|session| {
            format!(
                "{}  {}  {}",
                session.info.id.get(),
                session
                    .owner
                    .map_or_else(|| "No owner".to_owned(), |owner| format!("Owner: {owner}")),
                clean(&String::from_utf8_lossy(&session.info.directory.0))
            )
        })
        .collect()
}
