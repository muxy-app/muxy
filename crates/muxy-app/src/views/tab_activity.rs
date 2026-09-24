use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Bounds, Hsla, IntoElement, ParentElement, PathBuilder, Pixels,
    SharedString, StatefulInteractiveElement, Styled, canvas, div, point, px, svg,
};
use muxy_app_core::{
    PaneContent, ProjectId, Tab,
    activity::{ActivityIndicator, indicator},
    settings::AppLayout,
};
use muxy_protocol::{AgentProvider, ProgressState, SessionId, TerminalProgress};
use muxy_ui::components::Tooltip;
use muxy_ui::spinner::NativeSpinner;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

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
    let expanded = match model.appearance.layout {
        AppLayout::TabFocused => model.project_expanded(id),
        AppLayout::ProjectFocused => {
            model.appearance.sidebar_expanded
                && model.expanded_worktrees.contains(&id)
                && model
                    .state
                    .project(id)
                    .is_some_and(|project| model.has_worktrees(project))
        }
    };
    if expanded {
        return Status::None;
    }
    let include_children = model.appearance.layout == AppLayout::ProjectFocused;
    project_scope_status(id, include_children, model)
}

pub(super) fn worktree_status(id: ProjectId, model: &AppModel) -> Status {
    project_scope_status(id, false, model)
}

fn project_scope_status(id: ProjectId, include_children: bool, model: &AppModel) -> Status {
    let panes: Vec<_> = model
        .state
        .projects()
        .iter()
        .filter(|project| project.id == id || (include_children && project.parent_id == Some(id)))
        .flat_map(|project| &project.tabs)
        .flat_map(|tab| &tab.panes)
        .collect();
    let sessions: Vec<_> = panes
        .iter()
        .filter_map(|pane| match pane.content {
            PaneContent::Terminal { session } => session,
            PaneContent::Settings | PaneContent::Webview(_) => None,
        })
        .collect();
    let completion = panes
        .iter()
        .any(|pane| model.completions.contains(&pane.id));
    resolve(
        &sessions,
        completion,
        model.appearance.worktree_show_unread,
        model,
    )
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

#[derive(Clone, Default)]
pub(crate) struct Spinners(Rc<RefCell<Registry>>);

#[derive(Default)]
struct Registry {
    blocked: bool,
    natives: Vec<Weak<Native>>,
}

struct Native {
    spinner: Option<NativeSpinner>,
    in_bounds: Cell<bool>,
    visible: Cell<bool>,
}

impl Native {
    fn set_blocked(&self, blocked: bool) {
        let visible = !blocked && self.in_bounds.get();
        self.visible.set(visible);
        if let Some(spinner) = &self.spinner {
            spinner.set_visible(visible);
        }
    }
}

impl Spinners {
    pub(crate) fn set_blocked(&self, blocked: bool) {
        let mut registry = self.0.borrow_mut();
        let changed = registry.blocked != blocked;
        registry.blocked = blocked;
        registry.natives.retain(|native| {
            let Some(native) = native.upgrade() else {
                return false;
            };
            if changed {
                native.set_blocked(blocked);
            }
            true
        });
    }

    fn glyph(&self, id: String, size: Pixels, color: Hsla) -> AnyElement {
        let registry = self.0.clone();
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                window.with_global_id(SharedString::from(id).into(), |id, window| {
                    window.with_element_state(id, |state: Option<Rc<Native>>, window| {
                        let mut registry = registry.borrow_mut();
                        let native = state.unwrap_or_else(|| {
                            let native = Rc::new(Native {
                                spinner: if cfg!(test) {
                                    None
                                } else {
                                    NativeSpinner::new(window).ok()
                                },
                                in_bounds: Cell::new(false),
                                visible: Cell::new(false),
                            });
                            registry.natives.retain(|native| native.strong_count() > 0);
                            registry.natives.push(Rc::downgrade(&native));
                            native
                        });
                        let mask = window.content_mask().bounds;
                        native.in_bounds.set(
                            mask.contains(&bounds.origin) && mask.contains(&bounds.bottom_right()),
                        );
                        let visible = !registry.blocked && native.in_bounds.get();
                        native.visible.set(visible);
                        if let Some(spinner) = &native.spinner {
                            spinner.show(bounds, color.into(), visible, window.scale_factor());
                        }
                        ((), native)
                    });
                });
            },
        )
        .size(size)
        .into_any_element()
    }
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
                progress_circle(&id, progress, size, model),
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
    let theme_background = theme.bg;
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
            cx.new(|_| {
                Tooltip::new(
                    tooltip.clone(),
                    background,
                    foreground,
                    border,
                    theme_background,
                )
            })
            .into()
        })
        .child(glyph)
        .into_any_element()
}

fn progress_circle(
    id: &str,
    progress: TerminalProgress,
    size: Pixels,
    model: &AppModel,
) -> AnyElement {
    let theme = &model.theme;
    let color = match progress.state {
        ProgressState::Error => theme.danger,
        ProgressState::Paused => theme.warning,
        ProgressState::Running | ProgressState::Indeterminate => theme.accent,
    };
    if progress.state == ProgressState::Indeterminate {
        return model.spinners.glyph(id.to_owned(), size, color);
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AnyView, Context, Entity, Render, TestAppContext, Window};

    struct Glyphs {
        spinners: Spinners,
        shown: bool,
        clipped: bool,
        renders: usize,
    }

    impl Render for Glyphs {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders += 1;
            div()
                .size_full()
                .overflow_hidden()
                .when(self.shown, |view| {
                    view.child(
                        div()
                            .mt(if self.clipped { px(100.0) } else { px(0.0) })
                            .child(self.spinners.glyph("busy".into(), px(14.0), gpui::red())),
                    )
                })
        }
    }

    struct Host {
        spinners: Spinners,
        glyphs: Entity<Glyphs>,
        blocked: bool,
    }

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.spinners.set_blocked(self.blocked);
            AnyView::from(self.glyphs.clone()).cached(div().size(px(40.0)).style().clone())
        }
    }

    #[gpui::test]
    fn spinner_survives_cached_frames_and_drops_when_removed(cx: &mut TestAppContext) {
        let spinners = Spinners::default();
        let glyphs = cx.new(|_| Glyphs {
            spinners: spinners.clone(),
            shown: true,
            clipped: false,
            renders: 0,
        });
        let (host, cx) = cx.add_window_view(|_, _| Host {
            spinners: spinners.clone(),
            glyphs: glyphs.clone(),
            blocked: false,
        });
        cx.run_until_parked();
        let native = spinners.0.borrow().natives[0].clone();
        let renders = glyphs.read_with(cx, |glyphs, _| glyphs.renders);
        for _ in 0..5 {
            host.update(cx, |_, cx| cx.notify());
            cx.run_until_parked();
            assert!(
                native.upgrade().is_some(),
                "cached paint must retain the animation"
            );
            assert_eq!(spinners.0.borrow().natives.len(), 1);
            assert_eq!(glyphs.read_with(cx, |glyphs, _| glyphs.renders), renders);
        }
        glyphs.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert!(
            native.upgrade().is_some(),
            "repainting must reuse the animation"
        );
        assert_eq!(spinners.0.borrow().natives.len(), 1);

        glyphs.update(cx, |glyphs, cx| {
            glyphs.shown = false;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(
            native.upgrade().is_none(),
            "removal must release in the same frame"
        );
        host.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert!(spinners.0.borrow().natives.is_empty());
    }

    #[gpui::test]
    fn cached_spinner_visibility_tracks_overlays_and_clipping(cx: &mut TestAppContext) {
        let spinners = Spinners::default();
        let glyphs = cx.new(|_| Glyphs {
            spinners: spinners.clone(),
            shown: true,
            clipped: false,
            renders: 0,
        });
        let (host, cx) = cx.add_window_view(|_, _| Host {
            spinners: spinners.clone(),
            glyphs: glyphs.clone(),
            blocked: false,
        });
        cx.run_until_parked();
        let native = spinners.0.borrow().natives[0].clone();
        let renders = glyphs.read_with(cx, |glyphs, _| glyphs.renders);
        for blocked in [true, false, true, false] {
            host.update(cx, |host, cx| {
                host.blocked = blocked;
                cx.notify();
            });
            cx.run_until_parked();
            assert_eq!(native.upgrade().expect("native").visible.get(), !blocked);
            assert_eq!(glyphs.read_with(cx, |glyphs, _| glyphs.renders), renders);
        }
        glyphs.update(cx, |glyphs, cx| {
            glyphs.clipped = true;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(native.upgrade().is_none_or(|native| !native.visible.get()));
        for blocked in [true, false] {
            host.update(cx, |host, cx| {
                host.blocked = blocked;
                cx.notify();
            });
            cx.run_until_parked();
            assert!(native.upgrade().is_none_or(|native| !native.visible.get()));
        }
        glyphs.update(cx, |glyphs, cx| {
            glyphs.clipped = false;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(
            spinners
                .0
                .borrow()
                .natives
                .iter()
                .any(|native| { native.upgrade().is_some_and(|native| native.visible.get()) })
        );
    }
}
