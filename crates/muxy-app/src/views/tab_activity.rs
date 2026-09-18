use gpui::prelude::FluentBuilder;
use gpui::{
    Animation, AnimationExt as _, AnyElement, Bounds, Hsla, IntoElement, ParentElement,
    PathBuilder, Pixels, SharedString, Styled, canvas, div, percentage, point, px, svg,
};
use muxy_app_core::{
    Tab,
    activity::{ActivityIndicator, indicator},
};
use muxy_protocol::{AgentProvider, ProgressState, TerminalProgress};

use crate::model::AppModel;
use gpui::InteractiveElement;

pub(super) fn glyph(tab: &Tab, model: &AppModel, size: Pixels, fallback: AnyElement) -> AnyElement {
    let mut progress = None;
    let mut completion = false;
    for pane in &tab.panes {
        if let muxy_app_core::PaneContent::Terminal {
            session: Some(session),
        } = pane.content
        {
            let agent = model
                .activity
                .snapshot
                .agents
                .iter()
                .find(|agent| agent.session == session);
            progress = progress.or(muxy_app_core::activity::effective_progress(
                agent.map(|agent| agent.state),
                model
                    .progress
                    .get(&session)
                    .and_then(|state| state.progress),
            ));
            completion |= model.completions.contains(&pane.id);
        }
    }
    let sessions: Vec<_> = tab
        .panes
        .iter()
        .filter_map(|pane| match pane.content {
            muxy_app_core::PaneContent::Terminal { session } => session,
            muxy_app_core::PaneContent::Settings => None,
        })
        .collect();
    let activity = indicator(&model.activity.snapshot, |session| {
        sessions.contains(&session)
    });
    completion |= activity == ActivityIndicator::Completed;
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
    let fallback = if !tab.pinned
        && let Some(provider) = provider
    {
        div()
            .debug_selector(move || format!("tab-provider-{id}-{provider:?}"))
            .flex()
            .child(provider_icon(provider, size, model))
            .into_any_element()
    } else {
        fallback
    };
    let dot = if activity == ActivityIndicator::Blocked {
        Some(model.theme.warning)
    } else if completion {
        Some(model.theme.accent)
    } else {
        None
    };
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size)
        .child(match progress {
            Some(progress) => div()
                .debug_selector(move || format!("tab-progress-{id}"))
                .flex()
                .child(progress_circle(id, progress, size, &model.theme))
                .into_any_element(),
            None => fallback,
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

fn progress_circle(
    id: muxy_app_core::TabId,
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

pub(super) fn activity_glyph(
    id: String,
    activity: ActivityIndicator,
    size: Pixels,
    model: &AppModel,
) -> AnyElement {
    if activity == ActivityIndicator::Working {
        return svg()
            .path("icons/progress-indeterminate.svg")
            .size(size)
            .text_color(model.theme.accent)
            .with_animation(
                SharedString::from(id),
                Animation::new(std::time::Duration::from_secs(1)).repeat(),
                |svg, delta| {
                    svg.with_transformation(gpui::Transformation::rotate(percentage(delta)))
                },
            )
            .into_any_element();
    }
    let color = match activity {
        ActivityIndicator::Blocked => model.theme.warning,
        ActivityIndicator::Completed => model.theme.accent,
        _ => return div().into_any_element(),
    };
    div()
        .size(model.metrics.scaled(8.0))
        .rounded_full()
        .bg(color)
        .into_any_element()
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
