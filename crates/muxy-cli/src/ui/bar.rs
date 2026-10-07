//! The mode bar: which keys apply after Ctrl-B, while scrolling, or while
//! resizing. It covers the bottom row of the panes and is hidden otherwise.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use super::{Mode, Ui, theme};
use crate::state::State;
use crate::worker::Shared;

const PREFIX: &[(&str, &str)] = &[
    ("c", "tab"),
    ("%", "split"),
    ("\"", "stack"),
    ("arrows", "focus"),
    ("z", "zoom"),
    ("x", "close"),
    ("[", "scroll"),
    ("s", "projects"),
    ("b", "sidebar"),
    ("d", "detach"),
    ("?", "help"),
];
const SCROLL: &[(&str, &str)] = &[
    ("↑↓", "line"),
    ("pgup/pgdn", "page"),
    ("g/G", "top/bottom"),
    ("y", "copy"),
    ("q", "exit"),
];
const RESIZE: &[(&str, &str)] = &[("arrows", "resize"), ("esc", "done")];

pub(super) fn draw(frame: &mut Frame<'_>, shared: &Shared, state: &State, ui: &Ui, body: Rect) {
    let (name, style, keys) = match ui.mode {
        Mode::Normal => return,
        Mode::Prefix => (" PREFIX ", theme::pill(), PREFIX),
        Mode::Scroll => (" SCROLL ", theme::pill(), SCROLL),
        Mode::Resize => (" RESIZE ", theme::resize_pill(), RESIZE),
    };
    if body.height == 0 || body.width == 0 {
        return;
    }
    let area = Rect::new(body.x, body.bottom() - 1, body.width, 1);
    frame.render_widget(Clear, area);
    let position = (ui.mode == Mode::Scroll)
        .then(|| {
            let tab = state.tab()?;
            let view = shared.views.get(&tab.focus)?;
            let height = super::layout::panes(state, body)
                .iter()
                .find(|area| area.id == tab.focus)
                .map_or(0, |area| usize::from(area.inner.height));
            Some(match view.scroll.total(height) {
                Some(total) => format!("{}/{total} ", view.scroll.offset()),
                None if view.scroll.is_active() => "loading… ".into(),
                None => "bottom ".into(),
            })
        })
        .flatten()
        .unwrap_or_default();
    let mut spans = vec![Span::styled(name, style)];
    let mut used = Span::raw(name).width() + position.chars().count();
    for (key, label) in keys {
        let width = key.chars().count() + label.chars().count() + 3;
        if used + width > usize::from(area.width) {
            break;
        }
        used += width;
        spans.push(Span::raw("  "));
        spans.push(Span::styled(*key, theme::key()));
        spans.push(Span::styled(format!(" {label}"), theme::muted()));
    }
    let padding = usize::from(area.width).saturating_sub(used);
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(
        position,
        Style::default().patch(theme::muted()),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
