use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, DispatchPhase, FontWeight, Hsla, InteractiveElement, IntoElement,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, px,
};
use muxy_app_core::settings::{AgentsPanelDensity, AgentsPanelDock, AgentsPanelIcon};
use muxy_ui::accessibility::{self, OnPress};
use muxy_ui::components::{IconButton, IconGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::panel::{PanelFrame, PanelSizeBounds, PanelSizing, PanelStyle};
use muxy_ui::picker::{Confirm, Dismiss, SelectNext, SelectPrevious};
use muxy_ui::theme::contrasting_foreground;
use muxy_ui::tr;

use crate::model::AppModel;
use crate::model::agents_panel::{
    AgentStatus, Drag, Entry, EntryKey, MIN_HEIGHT, MIN_WIDTH, PANEL, Placement, RunningAgent,
};
use crate::views::overlays::{Mark, OverlayView};

const DOCKED_MAX_HEIGHT: f32 = 260.0;
const POPUP_WIDTH: f32 = 300.0;
const POPUP_MAX_HEIGHT: f32 = 380.0;
const RESIZE_GRIP: f32 = 18.0;
const RESIZE_EDGE: f32 = 6.0;
const FLOATING_GROUP: &str = "agents-panel-floating";

fn status_color(status: AgentStatus, model: &AppModel) -> Hsla {
    match status {
        AgentStatus::Blocked => model.theme.warning,
        AgentStatus::Working | AgentStatus::Completed => model.theme.accent,
        AgentStatus::Idle => model.theme.fg_dim,
    }
}

/// The agent's status as its terminal tab shows it, when the tab shows one.
fn status_glyph(
    agent: &RunningAgent,
    owner: gpui::EntityId,
    model: &AppModel,
) -> Option<AnyElement> {
    let m = model.metrics;
    let key = EntryKey::Agent(agent.server, agent.session);
    let status = super::tab_activity::agent_status(agent, model);
    if status == super::tab_activity::Status::None {
        return None;
    }
    Some(
        div()
            .debug_selector(move || format!("agents-panel-status-{key}"))
            .flex_none()
            .size(m.icon_sm())
            .flex()
            .items_center()
            .justify_center()
            .child(super::tab_activity::status_glyph_in(
                format!("agents-panel-{key}"),
                status,
                m.icon_sm(),
                Some(owner),
                model,
            ))
            .into_any_element(),
    )
}

impl AppModel {
    fn agents_location(&self, project: muxy_app_core::ProjectId) -> String {
        let Some(project) = self.state.project(project) else {
            return String::new();
        };
        match project
            .parent_id
            .and_then(|parent| self.state.project(parent))
        {
            Some(parent) => format!("{} › {}", parent.name, project.name),
            None => self.project_title(project),
        }
    }

    /// The project's short name in a compact row: a worktree's own name,
    /// since its project's color and icon already lead the row.
    fn agents_project_label(&self, project: muxy_app_core::ProjectId) -> String {
        match self.state.project(project) {
            Some(project) if project.parent_id.is_some() => project.name.clone(),
            Some(project) => self.project_title(project),
            None => String::new(),
        }
    }
}

fn compact(model: &AppModel) -> bool {
    model.appearance.agents_panel_density == AgentsPanelDensity::Compact
}

fn status_name(status: AgentStatus) -> SharedString {
    match status {
        AgentStatus::Blocked => tr!("Needs input"),
        AgentStatus::Working => tr!("Working"),
        AgentStatus::Completed => tr!("Completed"),
        AgentStatus::Idle => tr!("Idle"),
    }
}

/// How many agents a worktree has in each status, as clients read it.
fn counts_description(counts: &[usize; 4]) -> String {
    AgentStatus::ALL
        .into_iter()
        .zip(counts)
        .filter(|(_, count)| **count > 0)
        .map(|(status, count)| format!("{count} {}", status_name(status)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// What a row's icons and colors say, for clients that can't see them.
fn entry_description(entry: &Entry, model: &AppModel) -> String {
    match entry {
        Entry::Agent(agent) => format!(
            "{}, {}",
            model.agents_location(agent.project),
            status_name(agent.status)
        ),
        Entry::Worktree { counts, .. } => counts_description(counts),
    }
}

/// How long ago the agent finished, when the panel shows idle time.
fn idle_age(agent: &RunningAgent, model: &AppModel) -> Option<SharedString> {
    if !model.appearance.agents_panel_idle_time {
        return None;
    }
    let finished_at = agent.finished_at?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Some(super::agent_sessions::age_label(
        now.saturating_sub(finished_at),
    ))
}

fn idle_time(agent: &RunningAgent, model: &AppModel) -> Option<AnyElement> {
    let age = idle_age(agent, model)?;
    let key = EntryKey::Agent(agent.server, agent.session);
    Some(
        div()
            .debug_selector(move || format!("agents-panel-idle-{key}"))
            .flex_none()
            .text_size(model.metrics.font_footnote())
            .text_color(model.theme.fg_dim)
            .child(age)
            .into_any_element(),
    )
}

/// The second line of a normal row: where the agent runs, which agent it
/// is, unless the title already names it, and what it's doing.
fn details(entry: &Entry, title: &str, model: &AppModel) -> String {
    match entry {
        Entry::Agent(agent) => {
            let provider = agent.provider.name();
            [
                model.agents_location(agent.project),
                if title == provider {
                    String::new()
                } else {
                    provider.to_owned()
                },
                status_name(agent.status).to_string(),
            ]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
        }
        Entry::Worktree { counts, .. } => counts_description(counts),
    }
}

fn trailing(entry: &Entry, owner: gpui::EntityId, model: &AppModel) -> AnyElement {
    let m = model.metrics;
    match entry {
        Entry::Agent(agent) => div()
            .flex()
            .flex_none()
            .items_center()
            .gap(m.spacing2())
            .children(status_glyph(agent, owner, model))
            .children(idle_time(agent, model))
            .into_any_element(),
        Entry::Worktree { counts, .. } => div()
            .flex()
            .flex_none()
            .gap(m.spacing2())
            .children(
                AgentStatus::ALL
                    .into_iter()
                    .zip(counts)
                    .filter(|(_, count)| **count > 0)
                    .map(|(status, count)| {
                        div()
                            .flex()
                            .items_center()
                            .gap(m.spacing1())
                            .text_size(m.font_footnote())
                            .text_color(status_color(status, model))
                            .child(
                                div()
                                    .size(m.scaled(7.0))
                                    .rounded_full()
                                    .bg(status_color(status, model)),
                            )
                            .child(count.to_string())
                    }),
            )
            .into_any_element(),
    }
}

fn root_project<'a>(entry: &Entry, model: &'a AppModel) -> Option<&'a muxy_app_core::Project> {
    let project = match entry {
        Entry::Agent(agent) => agent.project,
        Entry::Worktree { project, .. } => *project,
    };
    let project = model.state.project(project)?;
    Some(
        project
            .parent_id
            .and_then(|parent| model.state.project(parent))
            .unwrap_or(project),
    )
}

/// The project's color as a bar down the row's leading edge, so it spans
/// the row at either density and leaves the icon where it is.
fn project_stripe(entry: &Entry, model: &AppModel) -> Option<AnyElement> {
    if !model.appearance.agents_panel_project_colors {
        return None;
    }
    let root = root_project(entry, model)?;
    let m = model.metrics;
    let key = entry.key();
    Some(
        div()
            .debug_selector(move || format!("agents-panel-color-{key}"))
            .absolute()
            .left(m.spacing1())
            .top(m.spacing2())
            .bottom(m.spacing2())
            .w(m.scaled(3.0))
            .rounded_full()
            .bg(super::sidebar::project_color(root, model))
            .into_any_element(),
    )
}

/// How big a row's leading icon is: larger beside two lines of text.
fn icon_size(model: &AppModel) -> gpui::Pixels {
    if compact(model) {
        model.metrics.icon_sm()
    } else {
        model.metrics.icon_lg()
    }
}

fn project_icon(entry: &Entry, model: &AppModel) -> Option<AnyElement> {
    let root = root_project(entry, model)?;
    let key = entry.key();
    Some(
        div()
            .debug_selector(move || format!("agents-panel-project-{key}"))
            .flex_none()
            .child(super::sidebar::project_tile(
                root,
                model,
                icon_size(model),
                None,
            ))
            .into_any_element(),
    )
}

fn leading(entry: &Entry, model: &AppModel) -> Option<AnyElement> {
    match (model.appearance.agents_panel_icon, entry) {
        (AgentsPanelIcon::Project, _) => project_icon(entry, model),
        (AgentsPanelIcon::Agent, Entry::Agent(agent)) => {
            let key = entry.key();
            Some(
                div()
                    .debug_selector(move || format!("agents-panel-icon-{key}"))
                    .flex_none()
                    .child(super::tab_activity::provider_icon(
                        agent.provider,
                        icon_size(model),
                        model,
                    ))
                    .into_any_element(),
            )
        }
        (AgentsPanelIcon::Agent, Entry::Worktree { .. }) => None,
    }
}

/// A row's title, with its details under it in a normal row or its project
/// before it in a compact one.
fn row_text(entry: &Entry, title: String, compact: bool, model: &AppModel) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    let key = entry.key();
    let details = (!compact).then(|| details(entry, &title, model));
    let title = div()
        .debug_selector(move || format!("agents-panel-title-{key}"))
        .when(compact, |title| title.flex_1().min_w(px(0.0)))
        .truncate()
        .text_size(m.font_body())
        .text_color(theme.fg)
        .child(title);
    if let Some(details) = details {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.0))
            .child(title)
            .child(
                div()
                    .debug_selector(move || format!("agents-panel-details-{key}"))
                    .truncate()
                    .text_size(m.font_footnote())
                    .text_color(theme.fg_dim)
                    .child(details),
            );
    }
    let project = match entry {
        Entry::Agent(agent) => {
            Some(model.agents_project_label(agent.project)).filter(|label| !label.is_empty())
        }
        Entry::Worktree { .. } => None,
    };
    div()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .items_baseline()
        .gap(m.spacing2())
        .overflow_hidden()
        .children(project.map(|project| {
            div()
                .debug_selector(move || format!("agents-panel-project-label-{key}"))
                .flex_none()
                .max_w(gpui::relative(0.4))
                .truncate()
                .text_size(m.font_footnote())
                .text_color(theme.fg_dim)
                .child(project)
        }))
        .child(title)
}

fn entry_row(
    entry: &Entry,
    active: bool,
    selected: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let key = entry.key();
    let compact = compact(model);
    let title = match entry {
        Entry::Agent(agent) => model
            .state
            .project(agent.project)
            .and_then(|project| project.tabs.iter().find(|tab| tab.id == agent.tab))
            .map(|tab| tab.title(Some(agent.pane)).trim().to_owned())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| agent.provider.name().to_owned()),
        Entry::Worktree { project, .. } => model.agents_location(*project),
    };
    let trailing = trailing(entry, model.agents_panel.owner.entity_id(), model);
    let leading = leading(entry, model);
    let stripe = project_stripe(entry, model);
    let label = title.clone();
    let text = row_text(entry, title, compact, model);
    div()
        .id(SharedString::from(format!("agents-panel-row-{key}")))
        .role(Role::TreeItem)
        .aria_label(label)
        .aria_description(entry_description(entry, model))
        .aria_selected(active)
        .debug_selector(move || format!("agents-panel-row-{key}"))
        .relative()
        .flex()
        .flex_none()
        .w_full()
        .min_w(px(0.0))
        .items_center()
        .gap(m.spacing3())
        .pl(m.spacing4())
        .pr(m.spacing3())
        .rounded(m.radius_md())
        .h(if compact {
            m.control_medium()
        } else {
            m.control_large() + m.spacing2()
        })
        .cursor_pointer()
        .when(active, |row| row.bg(theme.surface))
        .when(selected, |row| row.bg(theme.hover))
        .hover(|style| style.bg(theme.hover))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, _, _, cx| {
                model.agents_panel.selected = Some(key);
                cx.notify();
            }),
        )
        .on_press(cx.listener(move |model, _, window, cx| {
            model.open_agents_entry(key, window, cx);
        }))
        .children(stripe)
        .children(leading)
        .child(text)
        .child(trailing)
        .into_any_element()
}

fn rows(
    entries: &[Entry],
    inset: gpui::Pixels,
    model: &AppModel,
    cx: &mut Context<AppModel>,
    window: &Window,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let focused = model.agents_panel.focus.contains_focused(window, cx);
    let active = model.active_agents_entry(entries);
    let selected = if focused {
        model.selected_agents_entry(entries)
    } else {
        None
    };
    let mut list = div()
        .id("agents-panel-rows")
        .role(Role::Tree)
        .aria_label(tr!("Agents"))
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .gap(m.spacing1())
        .px(inset)
        .pb(m.spacing2())
        .overflow_y_scroll()
        .track_scroll(&model.agents_panel.scroll);
    if entries.is_empty() {
        return list
            .child(
                div()
                    .px(m.spacing4())
                    .py(m.spacing3())
                    .text_size(m.font_footnote())
                    .text_color(theme.fg_dim)
                    .child(accessibility::label(
                        "agents-panel-empty",
                        tr!("No agents running"),
                    )),
            )
            .into_any_element();
    }
    for entry in entries {
        let is_active = active == Some(entry.key());
        let is_selected = selected == Some(entry.key());
        list = list.child(entry_row(entry, is_active, is_selected, model, cx));
    }
    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .child(list)
        .child(scrollbar(model, cx))
        .into_any_element()
}

fn header_buttons(detached: bool, model: &AppModel, cx: &mut Context<AppModel>) -> [AnyElement; 3] {
    let m = model.metrics;
    let theme = &model.theme;
    let button = |id: &'static str, icon: Icon, tooltip: SharedString| {
        IconButton::new(
            id,
            tooltip,
            icon,
            m.icon_xs(),
            m.control_small(),
            theme.fg_muted,
            theme.fg,
        )
        .tooltip(theme)
    };
    let right = model.appearance.agents_panel_dock == AgentsPanelDock::Right;
    let move_dock = button(
        "agents-panel-move",
        if right {
            Icon::PanelLeft
        } else {
            Icon::PanelRight
        },
        if right {
            tr!("Move to Left Sidebar")
        } else {
            tr!("Move to Right")
        },
    )
    .on_click(cx.listener(move |model, _, window, cx| {
        cx.stop_propagation();
        model.set_agents_panel_dock(
            if right {
                AgentsPanelDock::Left
            } else {
                AgentsPanelDock::Right
            },
            window,
            cx,
        );
    }));
    let dock = button(
        if detached {
            "agents-panel-reattach"
        } else {
            "agents-panel-detach"
        },
        if detached {
            if right {
                Icon::PanelRight
            } else {
                Icon::PanelLeft
            }
        } else {
            Icon::Maximize
        },
        if detached {
            if right {
                tr!("Reattach to Right Panel")
            } else {
                tr!("Reattach to Sidebar")
            }
        } else {
            tr!("Detach")
        },
    )
    .on_click(cx.listener(move |model, _, window, cx| {
        cx.stop_propagation();
        model.set_agents_panel_detached(!detached, window, cx);
    }));
    let close = button("agents-panel-hide", Icon::X, tr!("Hide")).on_click(cx.listener(
        |model, _, window, cx| {
            cx.stop_propagation();
            model.hide_agents_panel(window, cx);
        },
    ));
    [
        ("agents-panel-move", move_dock),
        ("agents-panel-dock", dock),
        ("agents-panel-hide", close),
    ]
    .map(|(id, button)| {
        div()
            .debug_selector(move || id.into())
            .child(button)
            .into_any_element()
    })
}

/// Space between the panel's edges and its rows. Docked, the sidebar's own
/// margin already lines rows up with the sidebar's tabs.
fn inset(placement: Placement, model: &AppModel) -> gpui::Pixels {
    if placement == Placement::Docked {
        px(0.0)
    } else {
        model.metrics.spacing3()
    }
}

fn header(
    placement: Placement,
    agents: &[RunningAgent],
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let count = |status| agents.iter().filter(|agent| agent.status == status).count();
    let summary = [
        (
            AgentStatus::Blocked,
            tr!("%lld need input", count(AgentStatus::Blocked)),
        ),
        (
            AgentStatus::Working,
            tr!("%lld working", count(AgentStatus::Working)),
        ),
    ]
    .into_iter()
    .filter(|(status, _)| count(*status) > 0)
    .map(|(_, label)| label.to_string())
    .collect::<Vec<_>>()
    .join(" · ");
    let detached = placement == Placement::Floating;
    div()
        .id("agents-panel-header")
        .debug_selector(|| "agents-panel-header".into())
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing2())
        .pl(inset(placement, model) + m.spacing4())
        .pr(inset(placement, model))
        .h(m.control_large())
        .when(detached, |header| {
            header.cursor(gpui::CursorStyle::OpenHand).on_mouse_down(
                MouseButton::Left,
                cx.listener(|model, event: &gpui::MouseDownEvent, window, cx| {
                    model.begin_agents_panel_drag(None, event.position, window, cx);
                    model.focus_agents_panel(window);
                    window.prevent_default();
                    cx.stop_propagation();
                }),
            )
        })
        .child(
            div()
                .debug_selector(|| "agents-panel-name".into())
                .flex_none()
                .text_size(m.font_caption())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .child(tr!("Agents")),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(m.font_footnote())
                .text_color(theme.fg_dim)
                .when(!summary.is_empty(), |text| {
                    text.child(accessibility::label("agents-panel-summary", summary))
                }),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(header_buttons(detached, model, cx)),
        )
        .into_any_element()
}

fn body(
    placement: Placement,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> gpui::Stateful<gpui::Div> {
    let agents = model.running_agents();
    let entries = model.agents_panel_entries_from(agents.clone());
    div()
        .id("agents-panel")
        .role(Role::Group)
        .accessibility_id("agents-panel")
        .aria_label(tr!("Agents"))
        .debug_selector(|| "agents-panel".into())
        .track_focus(&model.agents_panel.focus)
        .key_context("Picker")
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|model, _, window, cx| {
                model.focus_owner.place(&model.agents_panel.focus, window);
                cx.notify();
            }),
        )
        .on_action(cx.listener(|model, _: &SelectNext, _, cx| {
            model.move_agents_selection(1, cx);
        }))
        .on_action(cx.listener(|model, _: &SelectPrevious, _, cx| {
            model.move_agents_selection(-1, cx);
        }))
        .on_action(cx.listener(|model, _: &Confirm, window, cx| {
            model.open_agents_selection(window, cx);
        }))
        .on_action(cx.listener(move |model, _: &Dismiss, window, cx| {
            if placement == Placement::Popup {
                model.close_agents_popup(window, cx);
            } else {
                model.focus_active(window, cx);
                cx.notify();
            }
        }))
        .relative()
        .flex()
        .flex_col()
        .min_h(px(0.0))
        .child(header(placement, &agents, model, cx))
        .child(rows(&entries, inset(placement, model), model, cx, window))
        .child(drag_tracker(cx))
}

pub(crate) fn right_panel_content(
    content: AnyElement,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    if model.agents_panel_placement() != Placement::Right || !model.agents_panel_shown() {
        return content;
    }
    let m = model.metrics;
    let scale = m.multiplier();
    let maximum = f32::from(window.viewport_size().width) - model.sidebar_width() - 100.0;
    let placement = AppModel::agents_right_placement();
    let sizing = PanelSizing::new(
        &placement,
        model.appearance.agents_panel_right_width.unwrap_or(320.0) * scale,
        PanelSizeBounds::new(
            f32::from(m.scaled(MIN_WIDTH)),
            maximum.min(f32::from(m.scaled(800.0))),
        ),
        model.agents_panel.resize.clone(),
    );
    let owner = cx.weak_entity();
    let frame = PanelFrame::new(
        placement,
        sizing,
        div(),
        body(Placement::Right, model, window, cx).flex_1(),
        move |width, _, cx| {
            let _ = owner.update(cx, |model, cx| {
                model.appearance.agents_panel_right_width = Some(width / scale);
                if !model.agents_panel.resize.is_active()
                    && model.appearance.agents_panel_right_width
                        != model.settings.appearance.agents_panel_right_width
                {
                    model.save_appearance(cx);
                }
                cx.notify();
            });
        },
        PanelStyle::new(model.theme.clone(), m),
    )
    .grip(model.webview_grip(None, gpui::CursorStyle::ResizeLeftRight));
    div()
        .debug_selector(|| "agents-panel-right".into())
        .flex()
        .size_full()
        .min_w_0()
        .min_h_0()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(content),
        )
        .child(frame)
        .into_any_element()
}

pub(crate) fn docked(
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    if model.agents_panel_placement() != Placement::Docked || !model.appearance.agents_panel_visible
    {
        return None;
    }
    let m = model.metrics;
    let height = model
        .appearance
        .agents_panel_docked_height
        .filter(|height| height.is_finite())
        .map(|height| model.clamp_docked_height(height, window.viewport_size()));
    let measured = model.agents_panel.docked_height.clone();
    let start = measured.clone();
    let active = matches!(model.agents_panel.drag, Some(Drag::Docked { .. }));
    let background = model.sidebar_background();
    let separator = Hsla::from(contrasting_foreground(background.into())).opacity(0.08);
    let panel = div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .mx(m.spacing3())
        .mb(m.spacing3())
        .map(|panel| match height {
            Some(height) => panel.h(m.scaled(height)),
            None => panel.max_h(m.scaled(DOCKED_MAX_HEIGHT)),
        })
        .child(
            canvas(
                move |bounds, _, _| measured.set(bounds.size.height),
                |_, (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(body(Placement::Docked, model, window, cx).flex_1())
        .child(
            div()
                .id("agents-panel-docked-resize")
                .debug_selector(|| "agents-panel-docked-resize".into())
                .group("agents-panel-docked-resize")
                .absolute()
                .top(-m.scaled(RESIZE_EDGE / 2.0))
                .left(-m.spacing3())
                .right(-m.spacing3())
                .h(m.scaled(RESIZE_EDGE))
                .cursor(gpui::CursorStyle::ResizeUpDown)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                        model.begin_agents_panel_docked_drag(event.position.y, start.get(), cx);
                        window.prevent_default();
                        cx.stop_propagation();
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top(m.scaled(RESIZE_EDGE / 2.0) - px(1.0))
                        .w_full()
                        .h(px(1.0))
                        .group_hover("agents-panel-docked-resize", |line| {
                            line.bg(model.theme.accent)
                        }),
                ),
        );
    Some(
        div()
            .flex_none()
            .bg(background)
            .border_t_1()
            .border_color(separator)
            .when(active, |panel| panel.border_color(model.theme.accent))
            .child(panel)
            .into_any_element(),
    )
}

fn scrollbar(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    use muxy_ui::scrollbar::{MINIMUM_THUMB_LENGTH, ThumbGeometry, WIDTH};
    let m = model.metrics;
    let color = model.theme.fg_dim.opacity(0.5);
    let handle = model.agents_panel.scroll.clone();
    let press = handle.clone();
    let thumb = |handle: &gpui::ScrollHandle, track: gpui::Pixels| {
        let visible = f64::from(f32::from(handle.bounds().size.height));
        let max = f64::from(f32::from(handle.max_offset().height));
        ThumbGeometry::from_lengths(
            visible + max,
            visible,
            f64::from(f32::from(-handle.offset().y)),
            f64::from(f32::from(track)),
            MINIMUM_THUMB_LENGTH,
        )
    };
    let width = m.scaled(WIDTH);
    div()
        .id("agents-panel-scrollbar")
        .debug_selector(|| "agents-panel-scrollbar".into())
        .absolute()
        .top_0()
        .bottom(m.spacing2())
        .right(m.scaled(2.0))
        .w(width)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, event: &gpui::MouseDownEvent, _, cx| {
                let track = press.bounds();
                let Some(geometry) = thumb(&press, track.size.height) else {
                    return;
                };
                cx.stop_propagation();
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "Thumb lengths are a window's pixels"
                )]
                let (origin, length) = (geometry.origin as f32, geometry.length as f32);
                let travel = (f32::from(track.size.height) - length).max(1.0);
                let ratio = f32::from(press.max_offset().height) / travel;
                let y = f32::from(event.position.y - track.top());
                if y < origin || y > origin + length {
                    let target = ((y - length / 2.0) * ratio)
                        .clamp(0.0, f32::from(press.max_offset().height));
                    press.set_offset(gpui::point(press.offset().x, -px(target)));
                }
                model.begin_agents_panel_scroll_drag(event.position.y, ratio, cx);
            }),
        )
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let Some(geometry) = thumb(&handle, bounds.size.height) else {
                        return;
                    };
                    #[allow(
                        clippy::cast_possible_truncation,
                        reason = "Thumb lengths are a window's pixels"
                    )]
                    let (origin, length) = (geometry.origin as f32, geometry.length as f32);
                    window.paint_quad(
                        gpui::fill(
                            gpui::Bounds::new(
                                gpui::point(bounds.left(), bounds.top() + px(origin)),
                                gpui::size(bounds.size.width, px(length)),
                            ),
                            color,
                        )
                        .corner_radii(width / 2.0),
                    );
                },
            )
            .size_full(),
        )
        .into_any_element()
}

pub(crate) fn collapsed_button(model: &AppModel, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    let open = matches!(model.overlay, Some(super::overlays::Overlay::Agents));
    if model.agents_panel_placement() != Placement::Popup
        || model.appearance.sidebar_expanded
        || !(open || model.appearance.agents_panel_visible)
    {
        return None;
    }
    let m = model.metrics;
    let theme = &model.theme;
    let urgent = model
        .running_agents()
        .first()
        .map(|agent| agent.status)
        .filter(|status| *status != AgentStatus::Idle);
    Some(
        div()
            .id("agents-panel-button")
            .role(Role::Button)
            .accessibility_id("agents-panel-button")
            .aria_label(tr!("Agents"))
            .aria_expanded(open)
            .debug_selector(|| "agents-panel-button".into())
            .group("agents-panel-button")
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .mx_auto()
            .mb(m.spacing2())
            .size(super::sidebar::collapsed_item_size(model))
            .rounded(m.radius_xl())
            .when(open, |button| button.bg(theme.hover))
            .cursor_pointer()
            .hover(|style| style.bg(theme.hover))
            .tooltip(muxy_ui::components::tooltip(
                tr!("Agents").to_string(),
                &model.theme,
            ))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|model, _, window, cx| {
                if matches!(model.overlay, Some(super::overlays::Overlay::Agents)) {
                    model.close_agents_popup(window, cx);
                } else {
                    model.toggle_agents_panel(window, cx);
                }
            }))
            .child(
                IconGlyph::new(
                    Icon::TerminalStack,
                    m.font_body(),
                    if open { theme.fg } else { theme.fg_muted },
                )
                .hover_in_group("agents-panel-button", theme.fg),
            )
            .children(urgent.map(|status| {
                div()
                    .absolute()
                    .top(m.scaled(5.0))
                    .right(m.scaled(5.0))
                    .size(m.scaled(7.0))
                    .rounded_full()
                    .bg(status_color(status, model))
            }))
            .into_any_element(),
    )
}

/// The Agents panel as a popup beside the sidebar.
pub(crate) struct AgentsPopup;

impl OverlayView for AgentsPopup {
    fn render(
        &self,
        mark: Option<Mark>,
        model: &AppModel,
        window: &Window,
        cx: &mut Context<AppModel>,
    ) -> AnyElement {
        popup(mark, model, window, cx)
    }

    fn marks_bounds(&self, _: &AppModel) -> bool {
        true
    }
}

fn popup(
    mark: Option<Mark>,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let bottom = if model.appearance.status_bar_visible {
        m.scaled(36.0)
    } else {
        m.spacing4()
    };
    div()
        .absolute()
        .left(px(model.sidebar_width()) + m.spacing4())
        .bottom(bottom)
        .w(m.scaled(POPUP_WIDTH))
        .max_h(m.scaled(POPUP_MAX_HEIGHT))
        .flex()
        .flex_col()
        .rounded(m.radius_xl())
        .border_1()
        .border_color(theme.border)
        .bg(theme.raised())
        .shadow_lg()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(body(Placement::Popup, model, window, cx).flex_1())
        .children(mark.map(|mark| mark.of_except(model, model.agents_panel.owner.entity_id())))
        .into_any_element()
}

fn resize_handle(
    id: &'static str,
    axes: [bool; 2],
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .id(id)
        .debug_selector(move || id.into())
        .absolute()
        .map(|handle| match axes {
            [true, false] => handle
                .top_0()
                .bottom(m.scaled(RESIZE_GRIP))
                .right_0()
                .w(m.scaled(RESIZE_EDGE))
                .cursor(gpui::CursorStyle::ResizeLeftRight),
            [false, true] => handle
                .left_0()
                .right(m.scaled(RESIZE_GRIP))
                .bottom_0()
                .h(m.scaled(RESIZE_EDGE))
                .cursor(gpui::CursorStyle::ResizeUpDown),
            _ => handle
                .right_0()
                .bottom_0()
                .size(m.scaled(RESIZE_GRIP))
                .cursor(gpui::CursorStyle::ResizeUpLeftDownRight)
                .child(
                    div()
                        .invisible()
                        .group_hover(FLOATING_GROUP, Styled::visible)
                        .absolute()
                        .right(m.scaled(4.0))
                        .bottom(m.scaled(4.0))
                        .size(m.scaled(7.0))
                        .border_r_2()
                        .border_b_2()
                        .rounded_br(m.scaled(2.0))
                        .border_color(theme.fg_dim),
                ),
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                model.begin_agents_panel_drag(Some(axes), event.position, window, cx);
                window.prevent_default();
                cx.stop_propagation();
            }),
        )
        .into_any_element()
}

fn drag_tracker(cx: &mut Context<AppModel>) -> AnyElement {
    let weak = cx.weak_entity();
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let moving = weak.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = moving.update(cx, |model, cx| {
                    if model.agents_panel.drag.is_none() {
                        return;
                    }
                    if event.pressed_button == Some(MouseButton::Left) {
                        model.move_agents_panel_drag(event.position, window, cx);
                    } else {
                        model.finish_agents_panel_drag(cx);
                    }
                    cx.stop_propagation();
                });
            });
            let ending = weak.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                let _ = ending.update(cx, |model, cx| {
                    if model.agents_panel.drag.is_some() {
                        model.move_agents_panel_drag(event.position, window, cx);
                        model.finish_agents_panel_drag(cx);
                        cx.stop_propagation();
                    }
                });
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

/// The detached agents panel's card, for the floating layer.
pub(crate) fn floating(
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    if model.agents_panel_placement() != Placement::Floating
        || !model.appearance.agents_panel_visible
    {
        return None;
    }
    let m = model.metrics;
    let theme = &model.theme;
    let [x, y, width, height] = model.agents_panel_frame(window.viewport_size());
    let panel = div()
        .group(FLOATING_GROUP)
        .absolute()
        .left(m.scaled(x))
        .top(m.scaled(y))
        .w(m.scaled(width.max(MIN_WIDTH)))
        .h(m.scaled(height.max(MIN_HEIGHT)))
        .flex()
        .flex_col()
        .rounded(m.radius_xl())
        .border_1()
        .border_color(theme.border)
        .bg(theme.raised())
        .shadow_lg()
        .occlude()
        .capture_any_mouse_down(cx.listener(|model, _, _, cx| {
            model.raise_floating(&muxy_ui::panel::PanelId::new(PANEL), cx);
        }))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(body(Placement::Floating, model, window, cx).flex_1())
        .child(resize_handle(
            "agents-panel-resize-right",
            [true, false],
            model,
            cx,
        ))
        .child(resize_handle(
            "agents-panel-resize-bottom",
            [false, true],
            model,
            cx,
        ))
        .child(resize_handle(
            "agents-panel-resize",
            [true, true],
            model,
            cx,
        ))
        .child(model.webview_occlusion_except(Some(model.agents_panel.owner.entity_id())));
    Some(panel.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::counts_description;

    #[test]
    fn a_worktree_reads_only_the_statuses_it_has() {
        assert_eq!(counts_description(&[2, 0, 0, 1]), "2 Needs input, 1 Idle");
        assert_eq!(counts_description(&[0, 0, 0, 0]), "");
    }
}
