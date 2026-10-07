//! Dialogs drawn over a dimmed screen: the project finder, existing
//! terminals, help, and the close confirmation.

use muxy_protocol::CatalogPage;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
};

use super::{Overlay, Target, Ui, clean, theme, truncate};
use crate::projects::{self, Entry};
use crate::worker::{Closing, Shared};

/// Keyboard help: a group name, then each key and what it does.
pub(crate) const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "Tabs",
        &[
            ("c", "new tab"),
            ("n / p", "next / previous tab"),
            ("1 … 9", "go to tab"),
        ],
    ),
    (
        "Panes",
        &[
            ("% or v", "split side by side"),
            ("\" or -", "split top and bottom"),
            ("arrows, h j k l", "move focus"),
            ("o", "next pane"),
            ("Ctrl/Alt + arrows", "resize"),
            ("r", "resize mode"),
            ("z", "zoom"),
            ("x", "close pane"),
        ],
    ),
    (
        "Projects",
        &[
            ("s", "find a project"),
            ("( / )", "previous / next project"),
            ("w", "existing terminals"),
            ("b", "show or hide the sidebar"),
        ],
    ),
    (
        "Scrollback",
        &[("[ or PgUp", "scroll mode"), ("y", "copy the selection")],
    ),
    (
        "Session",
        &[
            ("d", "detach; terminals keep running"),
            ("m", "mouse off, for the terminal's own selection"),
            ("Ctrl-B", "send Ctrl-B to the terminal"),
            ("?", "this help"),
        ],
    ),
    (
        "Mouse",
        &[
            ("click", "focus panes, pick tabs and projects"),
            ("right-click", "menu for a terminal or a tab"),
            ("drag a border", "resize panes or the sidebar"),
            ("wheel", "scroll back, or scroll the app"),
            ("drag", "select and copy text"),
            ("double-click", "select a word; triple: a line"),
            ("Shift-drag", "select even in apps that use the mouse"),
        ],
    ),
];

const NOTES: &[&str] = &[
    "Typing and paste always go to the focused terminal.",
    "Closing a pane ends its terminal only when nothing else shows it.",
    "The desktop app keeps its own layout; terminal UI windows share one.",
];

/// The projects matching `filter`, in list order.
pub(crate) fn project_choices<'a>(catalog: &'a CatalogPage, filter: &str) -> Vec<Entry<'a>> {
    let filter = filter.to_lowercase();
    projects::tree(catalog)
        .into_iter()
        .filter(|entry| {
            filter.is_empty()
                || entry.project.name.to_lowercase().contains(&filter)
                || projects::display_path(catalog, &entry.project.directory)
                    .to_lowercase()
                    .contains(&filter)
        })
        .collect()
}

/// Lines of help text: the intro, each group's title, keys, and blank line,
/// then the notes.
pub(crate) fn help_lines() -> usize {
    1 + HELP.iter().map(|(_, keys)| keys.len() + 2).sum::<usize>() + NOTES.len()
}

pub(super) fn draw(frame: &mut Frame<'_>, shared: &Shared, ui: &mut Ui) {
    if ui.overlay == Overlay::None {
        return;
    }
    let area = frame.area();
    // A menu leaves the screen as it is; dialogs dim what is behind them.
    if !matches!(ui.overlay, Overlay::Menu(_)) {
        let buffer = frame.buffer_mut();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buffer.cell_mut((x, y)) {
                    let style = cell.style();
                    cell.set_style(style.add_modifier(Modifier::DIM));
                }
            }
        }
    }
    ui.hits.push((area, Target::Backdrop));
    match ui.overlay.clone() {
        Overlay::None => {}
        Overlay::Projects { filter, selected } => projects(frame, shared, ui, &filter, selected),
        Overlay::Sessions(selected) => sessions(frame, shared, ui, selected),
        Overlay::Help(scroll) => help(frame, ui, scroll),
        Overlay::Confirm(closing) => confirm(frame, ui, closing),
        Overlay::Menu(menu) => super::menu::draw(frame, ui, &menu),
        Overlay::Rename { text, .. } => rename(frame, ui, &text),
    }
}

fn panel(frame: &mut Frame<'_>, ui: &mut Ui, size: (u16, u16), title: &str, border: Style) -> Rect {
    let area = frame.area();
    let width = size
        .0
        .min(area.width.saturating_sub(4))
        .max(area.width.min(8));
    let height = size
        .1
        .min(area.height.saturating_sub(2))
        .max(area.height.min(3));
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    let block = Block::bordered().border_style(border).title(Line::styled(
        format!(" {title} "),
        border.add_modifier(Modifier::BOLD),
    ));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    ui.hits.push((rect, Target::Panel));
    inner
}

fn list(frame: &mut Frame<'_>, ui: &mut Ui, area: Rect, items: Vec<Line<'_>>, selected: usize) {
    let height = usize::from(area.height);
    if height == 0 {
        return;
    }
    let first = selected.saturating_sub(height - 1);
    for (offset, (index, item)) in items
        .into_iter()
        .enumerate()
        .skip(first)
        .take(height)
        .enumerate()
    {
        let rect = Rect::new(
            area.x,
            area.y + u16::try_from(offset).unwrap_or(0),
            area.width,
            1,
        );
        frame.render_widget(Paragraph::new(item), rect);
        if index == selected {
            frame.buffer_mut().set_style(rect, theme::pill());
        }
        ui.hits.push((rect, Target::Item(index)));
    }
}

fn hint(frame: &mut Frame<'_>, inner: Rect, text: &str) {
    if inner.height > 0 {
        frame.render_widget(
            Paragraph::new(format!(" {text}")).style(theme::muted()),
            Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
        );
    }
}

fn projects(frame: &mut Frame<'_>, shared: &Shared, ui: &mut Ui, filter: &str, selected: usize) {
    let Some(catalog) = &shared.catalog else {
        return;
    };
    let choices = project_choices(catalog, filter);
    let rows = u16::try_from(choices.len().clamp(1, 16)).unwrap_or(16);
    let inner = panel(frame, ui, (64, rows + 5), "Projects", theme::accent());
    if inner.height < 3 {
        return;
    }
    let query = if filter.is_empty() {
        Line::from(vec![
            Span::styled(" / ", theme::key()),
            Span::styled("type to filter", theme::muted()),
        ])
    } else {
        Line::from(vec![
            Span::styled(" / ", theme::key()),
            Span::raw(clean(filter)),
            Span::styled(" ", Style::new().add_modifier(Modifier::REVERSED)),
        ])
    };
    frame.render_widget(
        Paragraph::new(query),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(inner.width))).style(theme::line()),
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
    );
    let body = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 3);
    hint(frame, inner, "↑↓ choose · enter open · esc close");
    if choices.is_empty() {
        frame.render_widget(
            Paragraph::new(" No matching projects").style(theme::muted()),
            body,
        );
        return;
    }
    let width = usize::from(body.width);
    let items = choices
        .iter()
        .map(|entry| {
            let name = clean(&entry.project.name);
            let lead = if entry.worktree {
                Span::styled(
                    if entry.last {
                        "   └─ "
                    } else {
                        "   ├─ "
                    },
                    theme::muted(),
                )
            } else {
                Span::styled(" ● ", theme::project(&entry.project.color))
            };
            let path = projects::display_path(catalog, &entry.project.directory);
            let used = lead.width() + Span::raw(&name).width() + 2;
            Line::from(vec![
                lead,
                Span::raw(name),
                Span::raw("  "),
                Span::styled(
                    truncate(&path, width.saturating_sub(used + 1)),
                    theme::muted(),
                ),
            ])
        })
        .collect();
    list(frame, ui, body, items, selected.min(choices.len() - 1));
}

fn sessions(frame: &mut Frame<'_>, shared: &Shared, ui: &mut Ui, selected: usize) {
    let rows = u16::try_from(shared.sessions.len().clamp(1, 16)).unwrap_or(16);
    let inner = panel(
        frame,
        ui,
        (80, rows + 3),
        "Existing terminals",
        theme::accent(),
    );
    if inner.height < 2 {
        return;
    }
    let body = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    hint(frame, inner, "↑↓ choose · enter open · esc close");
    if shared.sessions.is_empty() {
        frame.render_widget(
            Paragraph::new(" No other terminals in this project").style(theme::muted()),
            body,
        );
        return;
    }
    let items = shared
        .sessions
        .iter()
        .map(|session| {
            Line::from(vec![
                Span::styled(format!(" {} ", session.info.id.get()), theme::strong()),
                Span::styled(
                    session.owner.map_or_else(
                        || " No owner ".to_owned(),
                        |owner| format!(" Owner: {owner} "),
                    ),
                    theme::muted(),
                ),
                Span::raw(format!(
                    " {}",
                    clean(&String::from_utf8_lossy(&session.info.directory.0))
                )),
            ])
        })
        .collect();
    list(
        frame,
        ui,
        body,
        items,
        selected.min(shared.sessions.len() - 1),
    );
}

fn help(frame: &mut Frame<'_>, ui: &mut Ui, scroll: usize) {
    let lines = help_lines();
    let inner = panel(
        frame,
        ui,
        (72, u16::try_from(lines + 3).unwrap_or(u16::MAX)),
        "Keyboard help",
        theme::accent(),
    );
    if inner.height < 2 {
        return;
    }
    let mut text = vec![Line::from(vec![
        Span::styled(" Press ", theme::muted()),
        Span::styled("Ctrl-B", theme::key()),
        Span::styled(", then a key:", theme::muted()),
    ])];
    for (group, keys) in HELP {
        text.push(Line::styled(format!(" {group}"), theme::key()));
        for (key, label) in *keys {
            text.push(Line::from(vec![
                Span::styled(format!("   {key:<19}"), theme::strong()),
                Span::raw(*label),
            ]));
        }
        text.push(Line::default());
    }
    text.extend(
        NOTES
            .iter()
            .map(|note| Line::styled(format!(" {note}"), theme::muted())),
    );
    let body = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
    let first = scroll.min(text.len().saturating_sub(usize::from(body.height)));
    // Keep the offset to what is shown, so scrolling back responds at once.
    ui.overlay = Overlay::Help(first);
    frame.render_widget(
        Paragraph::new(text.into_iter().skip(first).collect::<Vec<_>>()),
        body,
    );
    hint(frame, inner, "↑↓ scroll · esc close");
}

fn confirm(frame: &mut Frame<'_>, ui: &mut Ui, closing: Closing) {
    let (title, lines) = match closing {
        Closing::Pane(_) => (
            "Close terminal?",
            [
                "A foreground program may still be running.",
                "Closing the last pane ends the terminal and discards its output.",
                "Other open panes keep it running.",
            ],
        ),
        Closing::Tab(_) => (
            "Close tab?",
            [
                "A program may still be running in this tab.",
                "Closing it ends its terminals and discards their output.",
                "Terminals that other panes show keep running.",
            ],
        ),
    };
    let inner = panel(frame, ui, (70, 7), title, theme::danger());
    for (offset, line) in (0..).zip(lines) {
        if offset < inner.height {
            frame.render_widget(
                Paragraph::new(format!(" {line}")),
                Rect::new(inner.x, inner.y + offset, inner.width, 1),
            );
        }
    }
    if inner.height < 5 {
        return;
    }
    let y = inner.bottom() - 1;
    let close = " ↵ close ";
    let cancel = " esc cancel ";
    let close_rect = Rect::new(inner.x + 1, y, 9.min(inner.width.saturating_sub(1)), 1);
    let cancel_rect = Rect::new(close_rect.right() + 2, y, 12, 1).intersection(inner);
    frame.render_widget(
        Paragraph::new(close).style(theme::danger_pill()),
        close_rect,
    );
    frame.render_widget(Paragraph::new(cancel).style(theme::button()), cancel_rect);
    ui.hits.push((close_rect, Target::Confirm(true)));
    ui.hits.push((cancel_rect, Target::Confirm(false)));
}

fn rename(frame: &mut Frame<'_>, ui: &mut Ui, text: &str) {
    let inner = panel(frame, ui, (56, 7), "Rename tab", theme::accent());
    if inner.height < 5 {
        return;
    }
    let field = Rect::new(inner.x + 1, inner.y + 1, inner.width.saturating_sub(2), 1);
    let shown = truncate(&clean(text), usize::from(field.width.saturating_sub(1)));
    let padding = usize::from(field.width).saturating_sub(Span::raw(&shown).width() + 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(shown),
            Span::styled(" ", Style::new().add_modifier(Modifier::REVERSED)),
            Span::raw(" ".repeat(padding)),
        ]))
        .style(Style::new().add_modifier(Modifier::UNDERLINED)),
        field,
    );
    frame.render_widget(
        Paragraph::new(" Leave it empty to show the terminal's title.").style(theme::muted()),
        Rect::new(inner.x, inner.y + 2, inner.width, 1),
    );
    let y = inner.bottom() - 1;
    let save = Rect::new(inner.x + 1, y, 8.min(inner.width.saturating_sub(1)), 1);
    let cancel = Rect::new(save.right() + 2, y, 12, 1).intersection(inner);
    frame.render_widget(Paragraph::new(" ↵ save ").style(theme::pill()), save);
    frame.render_widget(
        Paragraph::new(" esc cancel ").style(theme::button()),
        cancel,
    );
    ui.hits.push((save, Target::Confirm(true)));
    ui.hits.push((cancel, Target::Confirm(false)));
}
