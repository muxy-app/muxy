use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Bounds, Hsla, IntoElement, ParentElement, PathBuilder, Pixels,
    SharedString, StatefulInteractiveElement, Styled, canvas, div, point, px, svg,
};
use muxy_app_core::{
    PaneContent, ProjectId, ServerId, Tab,
    activity::{ActivityIndicator, indicator},
    settings::AppLayout,
};
use muxy_protocol::{AgentProvider, ProgressState, SessionId, TerminalProgress};
use muxy_ui::components::Tooltip;
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::spinner::NativeSpinner;
use muxy_ui::tr;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::model::AppModel;
use gpui::InteractiveElement;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Status {
    None,
    /// Muxy is creating or removing a worktree; the key describes it.
    Busy(&'static str),
    Progress(TerminalProgress),
    Blocked,
    Unread(usize),
    Completed,
}

/// Sessions are numbered per server, so they are looked up on `server`.
fn resolve(
    server: ServerId,
    sessions: &[SessionId],
    completion: bool,
    count_unread: bool,
    model: &AppModel,
) -> Status {
    let Some(runtime) = model.servers.get(server) else {
        return Status::None;
    };
    let snapshot = &runtime.activity.snapshot;
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
            runtime
                .progress
                .get(session)
                .and_then(|state| state.progress),
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
    let server = tab
        .panes
        .first()
        .and_then(|pane| model.state.pane_server(pane.id))
        .unwrap_or_else(ServerId::local);
    let sessions: Vec<_> = tab
        .panes
        .iter()
        .filter_map(|pane| match pane.content {
            PaneContent::Terminal { session } => session,
            PaneContent::Settings | PaneContent::Webview(_) => None,
        })
        .collect();
    resolve(
        server,
        &sessions,
        tab.panes
            .iter()
            .any(|pane| model.completions.contains(&pane.id)),
        false,
        model,
    )
}

pub(super) fn project_status(id: ProjectId, model: &AppModel) -> Status {
    let include_children = model.appearance.layout == AppLayout::ProjectFocused;
    let worktrees_shown = include_children
        && model.appearance.sidebar_expanded
        && model.expanded_worktrees.contains(&id)
        && model
            .state
            .project(id)
            .is_some_and(|project| model.has_worktrees(project));
    // The worktree list shows its own rows' work.
    if worktrees_shown {
        return Status::None;
    }
    if let Some(busy) = worktree_work(id, include_children, model) {
        return busy;
    }
    if !include_children && model.project_expanded(id) {
        return Status::None;
    }
    project_scope_status(id, include_children, model)
}

pub(super) fn worktree_status(id: ProjectId, model: &AppModel) -> Status {
    if model.worktree_removing(id) {
        return Status::Busy(REMOVING);
    }
    project_scope_status(id, false, model)
}

pub(super) const CREATING: &str = tr_key!("Creating worktree");
const REMOVING: &str = tr_key!("Removing worktree");

/// Worktrees `id` is creating, or the removal of `id` or, with
/// `include_children`, of one of its worktrees.
fn worktree_work(id: ProjectId, include_children: bool, model: &AppModel) -> Option<Status> {
    let removing = model.worktree_removing(id)
        || include_children
            && model
                .worktree_children(id)
                .iter()
                .any(|child| model.worktree_removing(child.id));
    if removing {
        Some(Status::Busy(REMOVING))
    } else if model.worktree_creations(id).next().is_some() {
        Some(Status::Busy(CREATING))
    } else {
        None
    }
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
        model
            .state
            .project_server(id)
            .unwrap_or_else(ServerId::local),
        &sessions,
        completion,
        model.appearance.worktree_show_unread,
        model,
    )
}

pub(super) fn icon(tab: &Tab, model: &AppModel, size: Pixels, fallback: AnyElement) -> AnyElement {
    let provider = tab
        .displayed_pane(model.state.shown_pane(tab))
        .and_then(|pane| model.pane_agent(pane.id))
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
    bounds: Cell<Option<Bounds<Pixels>>>,
    in_bounds: Cell<bool>,
    visible: Cell<bool>,
}

impl Spinners {
    /// Hides every spinner, for when native views can't be placed reliably.
    pub(crate) fn set_blocked(&self, blocked: bool) {
        self.0.borrow_mut().blocked = blocked;
    }

    /// Shows each spinner unless it is clipped, blocked, or under `covers`:
    /// app content painted above native views, such as a menu.
    pub(crate) fn present(&self, covers: &[Bounds<Pixels>]) {
        let mut registry = self.0.borrow_mut();
        let blocked = registry.blocked;
        registry.natives.retain(|native| {
            let Some(native) = native.upgrade() else {
                return false;
            };
            let visible = !blocked
                && native.in_bounds.get()
                && native
                    .bounds
                    .get()
                    .is_some_and(|bounds| !covers.iter().any(|cover| cover.intersects(&bounds)));
            native.visible.set(visible);
            if let Some(spinner) = &native.spinner {
                spinner.set_visible(visible);
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
                                bounds: Cell::new(None),
                                in_bounds: Cell::new(false),
                                visible: Cell::new(false),
                            });
                            registry.natives.retain(|native| native.strong_count() > 0);
                            registry.natives.push(Rc::downgrade(&native));
                            native
                        });
                        let mask = window.content_mask().bounds;
                        native.bounds.set(Some(bounds));
                        native.in_bounds.set(
                            mask.contains(&bounds.origin) && mask.contains(&bounds.bottom_right()),
                        );
                        // `present` decides visibility once everything above is laid out.
                        if let Some(spinner) = &native.spinner {
                            spinner.show(
                                bounds,
                                color.into(),
                                native.visible.get(),
                                window.scale_factor(),
                            );
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

/// An indeterminate spinner in the accent color.
pub(super) fn spinner(id: impl Into<String>, size: Pixels, model: &AppModel) -> AnyElement {
    model.spinners.glyph(id.into(), size, model.theme.accent)
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
        Status::Busy(key) => (
            "busy",
            translate(key).to_string(),
            model.spinners.glyph(id.clone(), size, theme.accent),
        ),
        Status::Progress(progress) => {
            let tooltip = match progress.state {
                ProgressState::Error => tr!("Work reported an error."),
                ProgressState::Paused => tr!("Work is paused."),
                ProgressState::Running
                | ProgressState::Indeterminate
                | ProgressState::Unrecognized(_) => tr!("Work is in progress."),
            };
            (
                "progress",
                tooltip.to_string(),
                progress_circle(&id, progress, size, model),
            )
        }
        Status::Blocked | Status::Completed => {
            let blocked = status == Status::Blocked;
            let color = if blocked { theme.warning } else { theme.accent };
            let tooltip = if blocked {
                tr!("An agent is waiting for your attention.")
            } else {
                tr!("Work finished and is ready to review.")
            };
            (
                if blocked { "blocked" } else { "completion" },
                tooltip.to_string(),
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
            if count == 1 {
                tr!("1 unread notification").to_string()
            } else {
                tr!("%lld unread notifications", count).to_string()
            },
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
        ProgressState::Running | ProgressState::Indeterminate | ProgressState::Unrecognized(_) => {
            theme.accent
        }
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

pub(super) fn provider_icon(provider: AgentProvider, size: Pixels, model: &AppModel) -> AnyElement {
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
        AgentProvider::Unrecognized(_) => {
            return svg()
                .path("icons/terminal.svg")
                .size(size)
                .text_color(model.theme.fg)
                .into_any_element();
        }
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
