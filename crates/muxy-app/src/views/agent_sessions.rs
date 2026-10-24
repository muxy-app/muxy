mod filters;
mod rows;

pub(crate) use filters::providers;
pub(crate) use rows::age_label;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, Focusable, Hsla, InteractiveElement, IntoElement, MouseDownEvent,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, px,
};
use muxy_ui::components::{IconButton, IconGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::panel::{
    PanelAction, PanelChrome, PanelControl, PanelFrame, PanelId, PanelMode, PanelPlacement,
    PanelPosition, PanelSizeBounds, PanelSizing, PanelStyle,
};
use muxy_ui::picker::{Confirm, Dismiss, SelectNext, SelectPrevious, TabPressed};
use muxy_ui::tr;

use super::tab_activity::provider_icon;
use crate::model::AppModel;
use crate::model::agent_sessions::PANEL;

pub(crate) fn button(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let toggle = model.agent_sessions.toggle.clone();
    super::titlebar::icon_button(
        "toggle-agent-sessions",
        Icon::History,
        tr!("Agent Sessions"),
        model,
    )
    .debug_selector(|| "toggle-agent-sessions".into())
    .child(
        canvas(
            move |bounds, _, _| toggle.set(Some(bounds)),
            |_, (), _, _| {},
        )
        .absolute()
        .size_full(),
    )
    .on_click(cx.listener(|model, _, window, cx| {
        cx.stop_propagation();
        model.toggle_agent_sessions(window, cx);
    }))
}

pub(crate) fn panel_content(
    content: AnyElement,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    if !model.appearance.agent_sessions_visible
        || model.panels.placement(&PanelId::new(PANEL)).is_none()
    {
        return content;
    }
    let m = model.metrics;
    let scale = f32::from(m.scaled(1.0));
    let placement = model.agent_sessions_placement();
    let right = placement.position == PanelPosition::Right;
    let pinned = placement.mode == PanelMode::Pinned;
    let sizing = sizing(&placement, model, window);
    let extent = px(sizing.layout().dimension() + 1.0);
    let cursor = placement.position.resize_cursor();
    let owner = cx.weak_entity();
    let frame = PanelFrame::new(
        placement,
        sizing,
        chrome(model, cx),
        body(model, cx),
        move |dimension, _, cx| {
            let _ = owner.update(cx, |model, cx| {
                let size = Some(dimension / scale);
                let saved = if right {
                    model.appearance.agent_sessions_width = size;
                    model.settings.appearance.agent_sessions_width
                } else {
                    model.appearance.agent_sessions_height = size;
                    model.settings.appearance.agent_sessions_height
                };
                if !model.agent_sessions.resize.is_active() && size != saved {
                    model.save_appearance(cx);
                }
                cx.notify();
            });
        },
        PanelStyle::new(model.theme.clone(), m),
    )
    .grip(model.webview_grip(None, cursor));
    let body = div()
        .debug_selector(|| "agent-sessions-panel".into())
        .relative()
        .flex()
        .size_full()
        .min_w_0()
        .min_h_0()
        .when(!right, Styled::flex_col)
        .child(
            div()
                .relative()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(content),
        );
    if pinned {
        return body.child(frame).into_any_element();
    }
    body.child(
        div()
            .id("floating-agent-sessions")
            .absolute()
            .right_0()
            .bottom_0()
            .max_w_full()
            .max_h_full()
            .when(right, |view| view.top_0().w(extent))
            .when(!right, |view| view.left_0().h(extent))
            .on_mouse_down_out(cx.listener(|model, event: &MouseDownEvent, window, cx| {
                let on_toggle = model
                    .agent_sessions
                    .toggle
                    .get()
                    .is_some_and(|bounds| bounds.contains(&event.position));
                if model.overlay.is_none() && model.appearance.agent_sessions_visible && !on_toggle
                {
                    model.toggle_agent_sessions(window, cx);
                }
            }))
            .child(frame)
            .child(model.webview_occlusion()),
    )
    .into_any_element()
}

fn sizing(placement: &PanelPlacement, model: &AppModel, window: &Window) -> PanelSizing {
    let m = model.metrics;
    let viewport = window.viewport_size();
    let (dimension, bounds) = if placement.position == PanelPosition::Right {
        (
            model.appearance.agent_sessions_width.unwrap_or(320.0),
            PanelSizeBounds::new(
                f32::from(m.scaled(240.0)),
                (f32::from(viewport.width) - model.sidebar_width() - 100.0)
                    .min(f32::from(m.scaled(800.0))),
            ),
        )
    } else {
        (
            model.appearance.agent_sessions_height.unwrap_or(220.0),
            PanelSizeBounds::new(
                f32::from(m.scaled(120.0)),
                (f32::from(viewport.height) - 160.0).min(f32::from(m.scaled(600.0))),
            ),
        )
    };
    PanelSizing::new(
        placement,
        f32::from(m.scaled(dimension)),
        bounds,
        model.agent_sessions.resize.clone(),
    )
}

fn focus_search(model: &AppModel, window: &mut Window, cx: &Context<AppModel>) {
    if let Some(input) = &model.agent_sessions.input {
        input.focus_handle(cx).focus(window);
    }
}

fn icon_button(
    id: &'static str,
    icon: Icon,
    tooltip: SharedString,
    color: Hsla,
    model: &AppModel,
) -> IconButton {
    let m = model.metrics;
    let theme = &model.theme;
    IconButton::new(id, icon, m.icon_xs(), m.control_small(), color, theme.fg).tooltip(
        tooltip,
        theme.raised(),
        theme.fg,
        theme.border,
        theme.bg,
    )
}

fn chrome(model: &AppModel, cx: &mut Context<AppModel>) -> PanelChrome {
    let m = model.metrics;
    let placement = model.agent_sessions_placement();
    let [header, refresh, position, mode, close] = model.agent_sessions.controls.clone();
    let refresh = if model.agent_sessions.task.is_some() {
        PanelAction::element(
            "refresh-agent-sessions",
            tr!("Loading session history…"),
            super::tab_activity::spinner_in(
                "agent-session-loading",
                m.icon_sm(),
                cx.entity_id(),
                model,
            ),
            refresh,
            |_, _| {},
        )
    } else {
        let owner = cx.weak_entity();
        PanelAction::icon(
            "refresh-agent-sessions",
            tr!("Refresh sessions"),
            Icon::Refresh,
            refresh,
            move |_, cx| {
                let _ = owner.update(cx, AppModel::refresh_agent_sessions);
            },
        )
    };
    let owner = cx.weak_entity();
    let control = |id: &'static str, control: PanelControl, focus| {
        let owner = owner.clone();
        PanelAction::control(id, control, focus, move |window, cx| {
            let _ = owner.update(cx, |model, cx| match control {
                PanelControl::Close => model.toggle_agent_sessions(window, cx),
                PanelControl::Move(_) => model.move_agent_sessions(false, cx),
                PanelControl::Mode(_) => model.move_agent_sessions(true, cx),
            });
        })
    };
    PanelChrome::new(
        tr!("Agent Sessions"),
        Some(
            IconGlyph::new(Icon::History, m.font_footnote(), model.theme.fg_muted)
                .into_any_element(),
        ),
        header,
        control(
            "agent-sessions-position",
            PanelControl::Move(placement.position),
            position,
        ),
        control(
            "agent-sessions-pin",
            PanelControl::Mode(placement.mode),
            mode,
        ),
        control("close-agent-sessions", PanelControl::Close, close),
        PanelStyle::new(model.theme.clone(), m),
    )
    .with_trailing_action(refresh)
}

fn search(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
    let has_query = model
        .agent_sessions
        .input
        .as_ref()
        .is_some_and(|input| !input.read(cx).text().is_empty());
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(m.scaled(32.0))
        .pl(m.spacing4())
        .pr(m.spacing2())
        .gap(m.spacing3())
        .border_b_1()
        .border_color(theme.border)
        .child(IconGlyph::new(Icon::Search, m.icon_md(), theme.fg_muted))
        .child(
            div()
                .debug_selector(|| "agent-session-search-input".into())
                .flex()
                .flex_1()
                .min_w_0()
                .children(model.agent_sessions.input.clone()),
        )
        .when(has_query, |bar| {
            bar.child(
                div()
                    .debug_selector(|| "clear-session-search".into())
                    .child(
                        icon_button(
                            "clear-session-search",
                            Icon::X,
                            tr!("Clear search"),
                            theme.fg_muted,
                            model,
                        )
                        .on_click(cx.listener(|model, _, window, cx| {
                            model.clear_session_search(cx);
                            focus_search(model, window, cx);
                        })),
                    ),
            )
        })
        .child(filters::button(model, cx))
}

fn body(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    div()
        .key_context("Picker")
        .on_action(cx.listener(|model, _: &SelectNext, _, cx| model.move_agent_session(false, cx)))
        .on_action(
            cx.listener(|model, _: &SelectPrevious, _, cx| model.move_agent_session(true, cx)),
        )
        .on_action(cx.listener(|model, _: &Confirm, _, cx| model.resume_selected_agent_session(cx)))
        .on_action(cx.listener(|model, _: &TabPressed, window, cx| {
            model.toggle_session_providers(window, cx);
        }))
        .on_action(cx.listener(|model, _: &Dismiss, _, cx| {
            let has_query = model
                .agent_sessions
                .input
                .as_ref()
                .is_some_and(|input| !input.read(cx).text().is_empty());
            if has_query {
                model.clear_session_search(cx);
            } else {
                model.focus_requested = true;
                cx.notify();
            }
        }))
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .overflow_hidden()
        .child(search(model, cx))
        .child(div().flex_1().min_h_0().overflow_hidden().map(|body| {
            if model.agent_sessions.filtered.is_empty() {
                body.child(empty(model, cx))
            } else {
                body.child(rows::list(model, cx))
            }
        }))
        .children(notices(model))
}

fn notices(model: &AppModel) -> Option<Div> {
    let m = model.metrics;
    let listing = &model.agent_sessions.listing;
    let messages: Vec<SharedString> = listing
        .limited
        .then(|| tr!("Some history exceeds the scan limit."))
        .into_iter()
        .chain(
            listing
                .warnings
                .iter()
                .map(|(provider, _)| tr!("Could not read some %@ sessions.", provider.name())),
        )
        .collect();
    (!messages.is_empty()).then(|| {
        div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(m.spacing1())
            .px(m.spacing4())
            .py(m.spacing3())
            .border_t_1()
            .border_color(model.theme.border)
            .text_size(m.font_footnote())
            .text_color(model.theme.fg_muted)
            .children(messages)
    })
}

fn empty(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
    let (message, reset) = if !model.state.current_project().server_id.is_local() {
        (
            tr!("Select a local project to browse its saved agent sessions."),
            false,
        )
    } else if model.agent_sessions.scope.is_none() || model.agent_sessions.roots.is_none() {
        (
            tr!("The project or home directory could not be found."),
            false,
        )
    } else if model.agent_sessions.task.is_some() {
        (tr!("Loading session history…"), false)
    } else if model.agent_sessions.listing.sessions.is_empty() {
        (tr!("No agent sessions in this project"), false)
    } else if model.appearance.agent_sessions_providers.is_empty() {
        (tr!("No providers selected"), true)
    } else {
        (tr!("No matching sessions"), true)
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .p(m.spacing6())
        .gap(m.spacing4())
        .child(
            div()
                .text_center()
                .text_size(m.font_body())
                .text_color(theme.fg_muted)
                .child(message),
        )
        .when(reset, |body| {
            body.child(
                muxy_ui::controls::button(
                    muxy_ui::controls::Style { theme, metrics: &m },
                    "reset-session-filters",
                    &tr!("Reset filters"),
                    true,
                    cx.listener(|model, _, window, cx| {
                        model.clear_session_search(cx);
                        model.all_session_providers(cx);
                        focus_search(model, window, cx);
                    }),
                )
                .debug_selector(|| "reset-session-filters".into()),
            )
        })
}
