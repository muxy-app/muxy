use gpui::prelude::FluentBuilder;
use gpui::{
    Animation, AnimationExt as _, AnyElement, AppContext, Bounds, Hsla, IntoElement, ParentElement,
    PathBuilder, Pixels, SharedString, StatefulInteractiveElement, Styled, canvas, div, percentage,
    point, px, svg,
};
use muxy_app_core::{
    PaneContent, ProjectId, Tab,
    activity::{ActivityIndicator, indicator},
    settings::AppLayout,
};
use muxy_protocol::{AgentProvider, ProgressState, SessionId, TerminalProgress};
use muxy_ui::components::Tooltip;

use crate::model::AppModel;
use gpui::InteractiveElement;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Status {
    None,
    Progress(TerminalProgress),
    Blocked,
    Unread(usize),
    Completed,
}

fn resolve(
    sessions: &[SessionId],
    completion: bool,
    count_unread: bool,
    model: &AppModel,
) -> Status {
    let snapshot = &model.activity.snapshot;
    let activity = indicator(snapshot, |session| sessions.contains(&session));
    if activity == ActivityIndicator::Blocked {
        return Status::Blocked;
    }
    let progress = sessions.iter().find_map(|session| {
        let agent = snapshot
            .agents
            .iter()
            .find(|agent| agent.session == *session);
        muxy_app_core::activity::effective_progress(
            agent.map(|agent| agent.state),
            model.progress.get(session).and_then(|state| state.progress),
        )
    });
    if let Some(progress) = progress {
        return Status::Progress(progress);
    }
    let unread = snapshot
        .events
        .iter()
        .filter(|event| !event.read && sessions.contains(&event.session))
        .count();
    if count_unread && unread > 0 {
        Status::Unread(unread)
    } else if completion || activity == ActivityIndicator::Completed {
        Status::Completed
    } else {
        Status::None
    }
}

pub(super) fn tab_status(tab: &Tab, model: &AppModel) -> Status {
    let sessions: Vec<_> = tab
        .panes
        .iter()
        .filter_map(|pane| match pane.content {
            PaneContent::Terminal { session } => session,
            PaneContent::Settings | PaneContent::Webview(_) => None,
        })
        .collect();
    resolve(
        &sessions,
        tab.panes
            .iter()
            .any(|pane| model.completions.contains(&pane.id)),
        false,
        model,
    )
}

pub(super) fn project_status(id: ProjectId, model: &AppModel) -> Status {
    let tabs_visible =
        model.appearance.layout == AppLayout::TabFocused && model.project_expanded(id);
    let children_hidden = if model.appearance.layout == AppLayout::TabFocused {
        !tabs_visible
    } else {
        !model.appearance.sidebar_expanded
    };
    let includes_project = |candidate| {
        candidate == id
            || (children_hidden
                && model
                    .state
                    .project(candidate)
                    .is_some_and(|project| project.parent_id == Some(id)))
    };
    let panes: Vec<_> = model
        .state
        .projects()
        .iter()
        .filter(|project| includes_project(project.id))
        .flat_map(|project| &project.tabs)
        .flat_map(|tab| &tab.panes)
        .collect();
    let visible_sessions: Vec<_> = panes
        .iter()
        .filter_map(|pane| match pane.content {
            PaneContent::Terminal { session } => session,
            PaneContent::Settings | PaneContent::Webview(_) => None,
        })
        .collect();
    let includes = |project, session| {
        includes_project(project) && (!tabs_visible || !visible_sessions.contains(&session))
    };
    let snapshot = &model.activity.snapshot;
    let mut sessions: Vec<_> = snapshot
        .agents
        .iter()
        .filter(|agent| includes(agent.project, agent.session))
        .map(|agent| agent.session)
        .chain(
            snapshot
                .events
                .iter()
                .filter(|event| includes(event.project, event.session))
                .map(|event| event.session),
        )
        .collect();
    if !tabs_visible {
        sessions.extend(visible_sessions);
    }
    let completion = !tabs_visible
        && panes
            .iter()
            .any(|pane| model.completions.contains(&pane.id));
    resolve(&sessions, completion, true, model)
}

pub(super) fn icon(tab: &Tab, model: &AppModel, size: Pixels, fallback: AnyElement) -> AnyElement {
    let provider = tab
        .displayed_pane(model.state.window().active_pane)
        .and_then(|pane| model.pane_session(pane.id))
        .and_then(|session| {
            model
                .activity
                .snapshot
                .agents
                .iter()
                .find(|agent| agent.session == session)
        })
        .map(|agent| agent.provider);
    let id = tab.id;
    if let Some(provider) = provider {
        div()
            .debug_selector(move || format!("tab-provider-{id}-{provider:?}"))
            .flex()
            .flex_none()
            .size(size)
            .child(provider_icon(provider, size, model))
            .into_any_element()
    } else {
        fallback
    }
}

pub(super) fn glyph(tab: &Tab, model: &AppModel, size: Pixels, fallback: AnyElement) -> AnyElement {
    let status = tab_status(tab, model);
    let fallback = if tab.pinned {
        fallback
    } else {
        icon(tab, model, size, fallback)
    };
    let dot = match status {
        Status::Blocked => Some(model.theme.warning),
        Status::Completed | Status::Unread(_) => Some(model.theme.accent),
        _ => None,
    };
    let id = tab.id;
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size)
        .child(if matches!(status, Status::Progress(_)) {
            status_glyph(format!("tab-{id}"), status, size, model)
        } else {
            fallback
        })
        .when_some(dot, |glyph, color| {
            glyph.child(
                div()
                    .debug_selector(move || format!("tab-completion-{id}"))
                    .absolute()
                    .top(model.metrics.scaled(-3.0))
                    .right(model.metrics.scaled(-3.0))
                    .size(model.metrics.scaled(6.0))
                    .rounded_full()
                    .bg(color),
            )
        })
        .into_any_element()
}

pub(super) fn status_glyph(
    id: String,
    status: Status,
    size: Pixels,
    model: &AppModel,
) -> AnyElement {
    let theme = &model.theme;
    let (kind, tooltip, glyph) = match status {
        Status::None => return div().into_any_element(),
        Status::Progress(progress) => {
            let tooltip = match progress.state {
                ProgressState::Error => "Work reported an error.",
                ProgressState::Paused => "Work is paused.",
                ProgressState::Running | ProgressState::Indeterminate => "Work is in progress.",
            };
            (
                "progress",
                tooltip.to_owned(),
                progress_circle(&id, progress, size, theme),
            )
        }
        Status::Blocked | Status::Completed => {
            let blocked = status == Status::Blocked;
            let color = if blocked { theme.warning } else { theme.accent };
            let tooltip = if blocked {
                "An agent is waiting for your attention."
            } else {
                "Work finished and is ready to review."
            };
            (
                if blocked { "blocked" } else { "completion" },
                tooltip.to_owned(),
                div()
                    .size(
                        model
                            .metrics
                            .scaled(if id.starts_with("tab-") { 7.0 } else { 8.0 }),
                    )
                    .rounded_full()
                    .bg(color)
                    .into_any_element(),
            )
        }
        Status::Unread(count) => (
            "unread",
            format!(
                "{count} unread notification{}",
                if count == 1 { "" } else { "s" }
            ),
            div()
                .size(model.metrics.scaled(8.0))
                .rounded_full()
                .bg(theme.accent)
                .into_any_element(),
        ),
    };
    let selector = if let Some(tab) = id.strip_prefix("tab-") {
        format!("tab-{kind}-{tab}")
    } else {
        format!("{id}-{kind}")
    };
    let background = theme.raised();
    let foreground = theme.fg;
    let border = theme.border;
    div()
        .id(SharedString::from(id))
        .debug_selector(move || selector.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(size)
        .h(size)
        .tooltip(move |_, cx| {
            cx.new(|_| Tooltip::new(tooltip.clone(), background, foreground, border))
                .into()
        })
        .child(glyph)
        .into_any_element()
}

fn progress_circle(
    id: &str,
    progress: TerminalProgress,
    size: Pixels,
    theme: &muxy_ui::theme::Theme,
) -> AnyElement {
    let color = match progress.state {
        ProgressState::Error => theme.danger,
        ProgressState::Paused => theme.warning,
        ProgressState::Running | ProgressState::Indeterminate => theme.accent,
    };
    if progress.state == ProgressState::Indeterminate {
        return svg()
            .path("icons/progress-indeterminate.svg")
            .size(size)
            .text_color(color)
            .with_animation(
                SharedString::from(format!("terminal-progress-{id}")),
                Animation::new(std::time::Duration::from_secs(1)).repeat(),
                |svg, delta| {
                    svg.with_transformation(gpui::Transformation::rotate(percentage(delta)))
                },
            )
            .into_any_element();
    }
    progress_ring(
        size,
        f32::from(progress.percent.unwrap_or(100)) / 100.0,
        color,
    )
}

fn progress_ring(size: Pixels, fraction: f32, color: Hsla) -> AnyElement {
    let line_width = px((f32::from(size) / 8.0).max(1.0));
    canvas(
        move |bounds, _, _| {
            (
                ring_path(bounds, 1.0, line_width),
                ring_path(bounds, fraction.max(0.001), line_width),
            )
        },
        move |_, paths, window, _| {
            if let Some(path) = paths.0 {
                window.paint_path(path, color.opacity(0.25));
            }
            if let Some(path) = paths.1 {
                window.paint_path(path, color);
            }
        },
    )
    .size(size)
    .into_any_element()
}

fn ring_path(
    bounds: Bounds<Pixels>,
    fraction: f32,
    line_width: Pixels,
) -> Option<gpui::Path<Pixels>> {
    let center = bounds.center();
    let radius = (bounds.size.width.min(bounds.size.height) - line_width) / 2.0;
    let mut builder = PathBuilder::stroke(line_width);
    for step in 0..=48_u16 {
        let angle = -std::f32::consts::FRAC_PI_2
            + std::f32::consts::TAU * fraction.clamp(0.0, 1.0) * f32::from(step) / 48.0;
        let point = point(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        if step == 0 {
            builder.move_to(point);
        } else {
            builder.line_to(point);
        }
    }
    builder.build().ok()
}

fn provider_icon(provider: AgentProvider, size: Pixels, model: &AppModel) -> AnyElement {
    let name = match provider {
        AgentProvider::Claude => "claude",
        AgentProvider::Codex => "codex",
        AgentProvider::OpenCode => "opencode",
        AgentProvider::Cursor => "cursor",
        AgentProvider::Copilot => "copilot",
        AgentProvider::Droid => "factory",
        AgentProvider::Pi => "pi",
        AgentProvider::Grok => "grok",
        AgentProvider::Kiro => "kiro",
        AgentProvider::Xal => "xal",
        AgentProvider::Antigravity => "antigravity",
    };
    let path = SharedString::from(format!("icons/provider-{name}.svg"));
    if provider == AgentProvider::Claude {
        gpui::img(path).size(size).into_any_element()
    } else {
        svg()
            .path(path)
            .size(size)
            .text_color(model.theme.fg)
            .into_any_element()
    }
}
