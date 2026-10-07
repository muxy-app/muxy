//! The sidebar: projects, the agents at work, and the server's state.

use muxy_app_core::activity::ActivityIndicator;
use muxy_protocol::{AgentActivity, CatalogPage, ProjectId};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{Target, Ui, clean, theme, truncate};
use crate::projects;
use crate::state::State;
use crate::worker::Shared;

/// Rows above the project list: its title and a blank line.
const HEADER_ROWS: u16 = 2;

pub(super) fn draw(frame: &mut Frame<'_>, shared: &Shared, state: &State, ui: &mut Ui, area: Rect) {
    if area.width < 2 || area.height == 0 {
        return;
    }
    let edge = Rect::new(area.right() - 1, area.y, 1, area.height);
    for y in edge.y..edge.bottom() {
        frame.render_widget(
            Paragraph::new("│").style(theme::line()),
            Rect::new(edge.x, y, 1, 1),
        );
    }
    ui.hits.push((edge, Target::SidebarEdge));
    let inner = Rect::new(area.x, area.y, area.width - 1, area.height);
    let footer_rows = u16::from(inner.height >= 3);
    let footer = Rect::new(
        inner.x,
        inner.bottom() - footer_rows,
        inner.width,
        footer_rows,
    );
    let rest = Rect::new(inner.x, inner.y, inner.width, inner.height - footer_rows);
    let agents = agents(shared);
    let agent_rows = if agents.is_empty() {
        0
    } else {
        u16::try_from(3 + 2 * agents.len())
            .unwrap_or(u16::MAX)
            .min(rest.height / 2)
    };
    let list = Rect::new(rest.x, rest.y, rest.width, rest.height - agent_rows);
    let Some(catalog) = &shared.catalog else {
        return;
    };
    project_list(frame, shared, state, catalog, ui, list);
    agent_list(
        frame,
        state,
        catalog,
        ui,
        &agents,
        Rect::new(rest.x, list.bottom(), rest.width, agent_rows),
    );
    if footer_rows > 0 {
        footer_row(frame, shared, ui, footer);
    }
}

struct Row<'a> {
    project: ProjectId,
    lines: Vec<Line<'a>>,
}

fn project_list(
    frame: &mut Frame<'_>,
    shared: &Shared,
    state: &State,
    catalog: &CatalogPage,
    ui: &mut Ui,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            " projects",
            theme::muted().patch(theme::strong()),
        )])),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let list = Rect::new(
        area.x,
        area.y + HEADER_ROWS.min(area.height),
        area.width,
        area.height.saturating_sub(HEADER_ROWS),
    );
    if list.height == 0 {
        return;
    }
    let rows = project_rows(shared, state, catalog, usize::from(list.width));
    let total: usize = rows.iter().map(|row| row.lines.len()).sum();
    let height = usize::from(list.height);
    if ui.revealed != Some(state.active) {
        ui.revealed = Some(state.active);
        reveal(ui, &rows, state.active, height);
    }
    ui.sidebar_scroll = ui.sidebar_scroll.min(total.saturating_sub(height));
    ui.hits.push((list, Target::SidebarList));
    let mut line = 0;
    for row in rows {
        let first = line;
        line += row.lines.len();
        if line <= ui.sidebar_scroll || first >= ui.sidebar_scroll + height {
            continue;
        }
        let active = row.project == state.active;
        let mut top = None;
        for (offset, text) in row.lines.into_iter().enumerate() {
            let Some(y) = (first + offset)
                .checked_sub(ui.sidebar_scroll)
                .filter(|y| *y < height)
                .and_then(|y| u16::try_from(y).ok())
            else {
                continue;
            };
            let y = list.y + y;
            top.get_or_insert(y);
            let stripe = if active { "▌" } else { " " };
            let hovered = !active && ui.hovers(Rect::new(list.x, y, list.width, 1));
            let mut spans = vec![Span::styled(stripe, theme::accent())];
            spans.extend(text.spans.into_iter().map(|span| {
                if hovered && offset == 0 && span.style == theme::text() {
                    span.style(theme::accent())
                } else {
                    span
                }
            }));
            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(list.x, y, list.width, 1),
            );
        }
        if let Some(top) = top {
            let rows = u16::try_from(line - first).unwrap_or(u16::MAX);
            let visible = Rect::new(list.x, top, list.width, rows).intersection(list);
            ui.hits.push((visible, Target::Project(row.project)));
        }
    }
    scrollbar(frame, list, ui.sidebar_scroll, total);
}

/// Each project's lines: its name, then its folder unless it is a worktree.
fn project_rows<'a>(
    shared: &Shared,
    state: &State,
    catalog: &CatalogPage,
    width: usize,
) -> Vec<Row<'a>> {
    projects::tree(catalog)
        .into_iter()
        .map(|entry| {
            let project = entry.project;
            let open = state
                .projects
                .get(&project.id)
                .is_some_and(|layout| !layout.tabs.is_empty());
            let name_style = if project.id == state.active {
                theme::strong()
            } else {
                theme::text()
            };
            let activity = theme::activity(super::project_activity(shared, project.id));
            let reserved = if activity.is_some() { 3 } else { 1 };
            let mut first = if entry.worktree {
                vec![Span::styled(
                    if entry.last { "  └─ " } else { "  ├─ " },
                    theme::muted(),
                )]
            } else {
                vec![
                    Span::styled(if open { "●" } else { "○" }, theme::project(&project.color)),
                    Span::raw(" "),
                ]
            };
            let used: usize = first.iter().map(Span::width).sum::<usize>() + 1;
            first.push(Span::styled(
                truncate(&clean(&project.name), width.saturating_sub(used + reserved)),
                name_style,
            ));
            if let Some((dot, style)) = activity {
                let filled: usize = first.iter().map(Span::width).sum::<usize>() + 1;
                first.push(Span::raw(" ".repeat(width.saturating_sub(filled + 2))));
                first.push(Span::styled(dot, style));
            }
            let mut lines = vec![Line::from(first)];
            if !entry.worktree {
                lines.push(Line::styled(
                    format!(
                        "  {}",
                        truncate(
                            &projects::display_path(catalog, &project.directory),
                            width.saturating_sub(4)
                        )
                    ),
                    theme::muted(),
                ));
            }
            Row {
                project: project.id,
                lines,
            }
        })
        .collect()
}

/// Scrolls the list just enough to show `project`.
fn reveal(ui: &mut Ui, rows: &[Row<'_>], project: ProjectId, height: usize) {
    let mut top = 0;
    for row in rows {
        if row.project == project {
            if top < ui.sidebar_scroll {
                ui.sidebar_scroll = top;
            } else if top + row.lines.len() > ui.sidebar_scroll + height {
                ui.sidebar_scroll = (top + row.lines.len()).saturating_sub(height);
            }
            return;
        }
        top += row.lines.len();
    }
}

fn scrollbar(frame: &mut Frame<'_>, list: Rect, offset: usize, total: usize) {
    let height = usize::from(list.height);
    if total <= height || list.width == 0 {
        return;
    }
    let thumb = (height * height / total).max(1);
    let top = offset * (height - thumb) / (total - height).max(1);
    let x = list.right() - 1;
    for (index, y) in (list.y..list.bottom()).enumerate() {
        let style = if (top..top + thumb).contains(&index) {
            theme::accent()
        } else {
            theme::line()
        };
        frame.render_widget(Paragraph::new("▕").style(style), Rect::new(x, y, 1, 1));
    }
}

/// Agents on the server, those needing attention first.
fn agents(shared: &Shared) -> Vec<(AgentActivity, ActivityIndicator)> {
    let mut agents: Vec<_> = shared
        .activity
        .agents
        .iter()
        .map(|agent| {
            (
                agent.clone(),
                super::session_activity(shared, agent.session),
            )
        })
        .collect();
    agents.sort_by_key(|(_, indicator)| std::cmp::Reverse(*indicator));
    agents
}

fn agent_list(
    frame: &mut Frame<'_>,
    state: &State,
    catalog: &CatalogPage,
    ui: &mut Ui,
    agents: &[(AgentActivity, ActivityIndicator)],
    area: Rect,
) {
    if area.height < 3 {
        return;
    }
    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(area.width))).style(theme::line()),
        Rect::new(area.x, area.y, area.width, 1),
    );
    let count = agents.len().to_string();
    let title_width = usize::from(area.width).saturating_sub(count.len() + 1);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{:<title_width$}", " agents"),
                theme::muted().patch(theme::strong()),
            ),
            Span::styled(count, theme::muted()),
        ])),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );
    let focused = state
        .tab()
        .and_then(|tab| tab.panes.get(&tab.focus))
        .and_then(|pane| pane.session);
    let width = usize::from(area.width);
    let mut y = area.y + 3;
    for (index, (agent, indicator)) in agents.iter().enumerate() {
        if y + 1 >= area.bottom() {
            if index < agents.len() {
                frame.render_widget(
                    Paragraph::new(format!("   +{} more", agents.len() - index))
                        .style(theme::muted()),
                    Rect::new(area.x, area.bottom() - 1, area.width, 1),
                );
            }
            break;
        }
        let project = catalog
            .projects
            .iter()
            .find(|project| project.id == agent.project)
            .map_or("Unknown project".into(), |project| clean(&project.name));
        let tab = state.projects.get(&agent.project).and_then(|layout| {
            layout.tabs.iter().position(|tab| {
                tab.panes
                    .values()
                    .any(|pane| pane.session == Some(agent.session))
            })
        });
        let current = focused == Some(agent.session);
        let (dot, dot_style) = theme::activity(*indicator).unwrap_or(("·", theme::muted()));
        let suffix = tab.map_or(String::new(), |tab| format!(" · {}", tab + 1));
        let mut first = vec![
            Span::styled(if current { "▌" } else { " " }, theme::accent()),
            Span::styled(dot, dot_style),
            Span::raw(" "),
            Span::styled(
                truncate(&project, width.saturating_sub(4 + suffix.len())),
                if current {
                    theme::strong()
                } else {
                    theme::text()
                },
            ),
        ];
        first.push(Span::styled(suffix, theme::muted()));
        frame.render_widget(
            Paragraph::new(Line::from(first)),
            Rect::new(area.x, y, area.width, 1),
        );
        let second = format!(
            "{}   {} · {}",
            if current { "▌" } else { " " },
            agent.provider.name(),
            theme::activity_label(*indicator)
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(second.chars().take(1).collect::<String>(), theme::accent()),
                Span::styled(
                    truncate(
                        &second.chars().skip(1).collect::<String>(),
                        width.saturating_sub(1),
                    ),
                    theme::muted(),
                ),
            ])),
            Rect::new(area.x, y + 1, area.width, 1),
        );
        ui.hits.push((
            Rect::new(area.x, y, area.width, 2),
            Target::Agent(agent.session, agent.project),
        ));
        y += 2;
    }
}

fn footer_row(frame: &mut Frame<'_>, shared: &Shared, ui: &mut Ui, area: Rect) {
    let online = shared
        .client
        .as_ref()
        .is_some_and(muxy_client::Client::is_connected);
    let server = truncate(
        &clean(&shared.server),
        usize::from(area.width).saturating_sub(8),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ● ", theme::connected(online)),
            Span::styled(server, theme::muted()),
        ])),
        area,
    );
    if area.width >= 8 {
        let help = Rect::new(area.right() - 4, area.y, 1, 1);
        let hide = Rect::new(area.right() - 2, area.y, 1, 1);
        frame.render_widget(Paragraph::new("?").style(theme::key()), help);
        frame.render_widget(
            Paragraph::new("«").style(Style::default().patch(theme::muted())),
            hide,
        );
        ui.hits.push((help, Target::Help));
        ui.hits.push((hide, Target::HideSidebar));
    }
}
