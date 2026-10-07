//! The tab bar of the current project.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{Target, Ui, clean, theme, truncate};
use crate::state::State;
use crate::worker::Shared;

const TITLE_WIDTH: usize = 24;
const NEW_TAB: &str = " + ";

pub(super) fn draw(frame: &mut Frame<'_>, shared: &Shared, state: &State, ui: &mut Ui, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    ui.hits.push((area, Target::TabBar));
    let mut left = area.x;
    if ui.chrome.sidebar.width == 0 {
        if !ui.sidebar_shown {
            let button = Rect::new(left, area.y, 3.min(area.width), 1);
            frame.render_widget(Paragraph::new(" » ").style(theme::muted()), button);
            ui.hits.push((button, Target::ShowSidebar));
            left = button.right();
        }
        let name = shared
            .catalog
            .as_ref()
            .and_then(|catalog| {
                catalog
                    .projects
                    .iter()
                    .find(|project| project.id == state.active)
            })
            .map_or("Home".into(), |project| clean(&project.name));
        let label = format!(" {} ", truncate(&name, 20));
        let width = u16::try_from(Span::raw(&label).width())
            .unwrap_or(0)
            .min(area.right() - left);
        frame.render_widget(
            Paragraph::new(label).style(theme::button()),
            Rect::new(left, area.y, width, 1),
        );
        left = (left + width + 1).min(area.right());
    }
    let mut right = area.right();
    if state.tab().is_some_and(|tab| tab.zoom) && right - left > 12 {
        let zoom = Rect::new(right - 6, area.y, 6, 1);
        frame.render_widget(Paragraph::new(" ZOOM ").style(theme::pill()), zoom);
        right = zoom.x.saturating_sub(1);
    }
    let existing = if shared.sessions_project == Some(state.active) {
        shared.sessions.len()
    } else {
        0
    };
    if existing > 0 {
        let label = format!(" ▤ {existing} ");
        let width = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
        if right - left > width + 12 {
            let badge = Rect::new(right - width, area.y, width, 1);
            frame.render_widget(Paragraph::new(label).style(theme::button()), badge);
            ui.hits.push((badge, Target::Existing));
            right = badge.x.saturating_sub(1);
        }
    }
    let tabs = labels(shared, state);
    let needed: u16 = tabs.iter().map(|(width, _)| width + 1).sum::<u16>() + 3;
    if let Some(message) = ui.message(shared) {
        let room = (right - left)
            .saturating_sub(needed)
            .max((right - left) * 2 / 3);
        let text = truncate(&message, usize::from(room.saturating_sub(1)));
        let width = u16::try_from(Span::raw(&text).width()).unwrap_or(0);
        frame.render_widget(
            Paragraph::new(text).style(theme::warning()),
            Rect::new(right - width, area.y, width, 1),
        );
        right = (right - width).saturating_sub(1).max(left);
    }
    strip(
        frame,
        state,
        ui,
        &tabs,
        Rect::new(left, area.y, right - left, 1),
    );
}

type Label<'a> = (u16, Vec<Span<'a>>);

fn labels<'a>(shared: &Shared, state: &State) -> Vec<Label<'a>> {
    let Some(project) = state.projects.get(&state.active) else {
        return Vec::new();
    };
    project
        .tabs
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let active = index == project.active;
            let base = if active { theme::pill() } else { theme::text() };
            let number = if active { base } else { theme::muted() };
            let title = match (&tab.title, tab.panes.get(&tab.focus)) {
                (Some(title), _) => clean(title),
                (None, Some(pane)) => super::title_of(shared, tab.focus, pane),
                (None, None) => "Terminal".into(),
            };
            let mut spans = vec![
                Span::styled(format!(" {} ", index + 1), number),
                Span::styled(truncate(&title, TITLE_WIDTH), base),
            ];
            if let Some((dot, style)) = theme::activity(super::tab_activity(shared, tab)) {
                spans.push(Span::styled(" ", base));
                spans.push(Span::styled(
                    dot,
                    base.patch(Style::new().fg(style.fg.unwrap_or_default())),
                ));
            }
            spans.push(Span::styled(" ", base));
            let width = spans.iter().map(Span::width).sum::<usize>();
            (u16::try_from(width).unwrap_or(u16::MAX), spans)
        })
        .collect()
}

fn strip(frame: &mut Frame<'_>, state: &State, ui: &mut Ui, tabs: &[Label<'_>], area: Rect) {
    if area.width == 0 {
        return;
    }
    let active = state
        .projects
        .get(&state.active)
        .map_or(0, |project| project.active);
    let room = area.width.saturating_sub(3);
    let fits = |start: usize| {
        let mut used = if start > 0 { 2 } else { 0 };
        let mut end = start;
        while end < tabs.len() {
            let next = used + tabs[end].0 + 1;
            let ellipsis = if end + 1 < tabs.len() { 2 } else { 0 };
            if next + ellipsis > room + 1 {
                break;
            }
            used = next;
            end += 1;
        }
        end
    };
    let mut start = 0;
    while start < active && fits(start) <= active {
        start += 1;
    }
    let end = fits(start).max((start + 1).min(tabs.len()));
    let mut x = area.x;
    if start > 0 {
        frame.render_widget(
            Paragraph::new("…").style(theme::muted()),
            Rect::new(x, area.y, 1, 1),
        );
        x = (x + 2).min(area.right());
    }
    let panes: Vec<_> = state
        .projects
        .get(&state.active)
        .map(|project| project.tabs.iter().map(|tab| tab.focus).collect())
        .unwrap_or_default();
    for ((width, spans), pane) in tabs.iter().zip(panes).take(end).skip(start) {
        let width = (*width).min(area.right().saturating_sub(x));
        let rect = Rect::new(x, area.y, width, 1);
        frame.render_widget(Paragraph::new(Line::from(spans.clone())), rect);
        ui.hits.push((rect, Target::Tab(pane)));
        x = (x + width + 1).min(area.right());
    }
    if end < tabs.len() && x < area.right() {
        frame.render_widget(
            Paragraph::new("…").style(theme::muted()),
            Rect::new(x, area.y, 1, 1),
        );
        x = (x + 2).min(area.right());
    }
    let button = Rect::new(x, area.y, 3.min(area.right() - x), 1);
    let style = if ui.hovers(button) {
        theme::key()
    } else {
        theme::muted()
    };
    frame.render_widget(Paragraph::new(NEW_TAB).style(style), button);
    ui.hits.push((button, Target::NewTab));
}
