use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Div, ElementId, InteractiveElement, IntoElement,
    MouseButton, ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, Window,
    div, px,
};
use muxy_ui::components::{IconGlyph, Tooltip};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use crate::model::AppModel;

pub(super) fn actions(model: &AppModel, cx: &mut Context<AppModel>) -> (Div, f32) {
    let project = model.state.current_project();
    let available = project.status() == muxy_app_core::ProjectStatus::Available;
    let existing_count = if available {
        model.existing_terminal_count()
    } else {
        0
    };
    let browser = available && model.browser_enabled();
    let layouts = available && model.has_project_layouts();
    let zoom_tab = project.tabs.iter().find(|tab| {
        available
            && Some(tab.id) == model.active_tab()
            && (tab.zoomed.is_some() || tab.panes.len() > 1)
    });
    let controls = 1
        + u8::from(existing_count > 0)
        + u8::from(browser)
        + u8::from(zoom_tab.is_some())
        + u8::from(layouts);
    let width = f32::from(controls)
        * f32::from(model.metrics.control_medium() + model.metrics.spacing2())
        + model.extension_toolbar_width();
    let bar = div()
        .debug_selector(|| "titlebar-actions".into())
        .flex()
        .flex_none()
        .items_center()
        .when(existing_count > 0, |bar| {
            bar.child(super::tab_strip::existing_terminals_button(
                existing_count,
                model,
                cx,
            ))
        })
        .when(browser, |bar| {
            bar.child(super::tab_strip::browser_button(model, cx))
        })
        .when_some(zoom_tab, |bar, tab| {
            bar.child(super::tab_strip::zoom_control(
                tab.zoomed.is_some(),
                model,
                cx,
            ))
        })
        .when(layouts, |bar| {
            bar.child(super::project_layouts::button(model, cx))
        })
        .child(model.extension_toolbar(cx))
        .child(super::tab_strip::settings_button(model, cx));
    (bar, width)
}

#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = muxy, no_json)]
pub(crate) struct BeginWindowMove;

pub(crate) fn begin_window_move(
    model: &mut AppModel,
    _: &BeginWindowMove,
    _window: &mut Window,
    _: &mut Context<AppModel>,
) {
    if model.overlay.is_some() || model.close_prompt.is_some() {
        return;
    }
    #[cfg(target_os = "macos")]
    if let Some(drag) = &model.window_drag {
        drag.begin();
    }
    #[cfg(not(target_os = "macos"))]
    _window.start_window_move();
}

pub(crate) fn background(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .on_mouse_down(MouseButton::Left, |event, window, cx| {
            cx.stop_propagation();
            if event.click_count == 1 {
                window.dispatch_action(Box::new(BeginWindowMove), cx);
            }
        })
        .on_click(background_click)
}

fn background_click(event: &gpui::ClickEvent, window: &mut Window, cx: &mut App) {
    if event.click_count() == 2 {
        cx.stop_propagation();
        window.dispatch_action(Box::new(super::workspace::Zoom), cx);
    }
}

pub(crate) fn action_button(
    id: impl Into<ElementId>,
    tooltip: impl Into<SharedString>,
    model: &AppModel,
) -> Stateful<Div> {
    let m = model.metrics;
    let theme = model.theme.clone();
    let tooltip = tooltip.into();
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.control_medium())
        .mr(m.spacing2())
        .rounded(m.radius_sm())
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .tooltip(move |_, cx| {
            cx.new(|_| {
                Tooltip::new(
                    tooltip.clone(),
                    theme.raised(),
                    theme.fg,
                    theme.border,
                    theme.bg,
                )
            })
            .into()
        })
}

pub(super) fn icon_button(
    id: impl Into<ElementId>,
    icon: Icon,
    tooltip: impl Into<SharedString>,
    model: &AppModel,
) -> Stateful<Div> {
    action_button(id, tooltip, model).child(IconGlyph::new(
        icon,
        model.metrics.icon_md(),
        model.theme.fg_muted,
    ))
}

pub(crate) fn navigation(
    model: &AppModel,
    sidebar_width: f32,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    background("titlebar-navigation")
        .occlude()
        .absolute()
        .top_0()
        .left_0()
        .w(px(sidebar_width.max(navigation_width(model))))
        .h(m.title_bar_height())
        .flex()
        .items_center()
        .justify_end()
        .pr(m.spacing4())
        .gap(m.spacing1())
        .bg(if model.appearance.sidebar_expanded {
            gpui::transparent_black()
        } else {
            theme.bg
        })
        .border_r_1()
        .border_color(theme.border)
        .child(arrow(model, false, cx))
        .child(arrow(model, true, cx))
        .child(
            div()
                .debug_selector(|| "sidebar-toggle".into())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    muxy_ui::components::IconButton::new(
                        "toggle-sidebar",
                        Icon::PanelLeft,
                        m.font_body(),
                        m.scaled(22.0),
                        theme.fg_muted,
                        theme.fg,
                    )
                    .tooltip(
                        tr!("Toggle Sidebar"),
                        theme.raised(),
                        theme.fg,
                        theme.border,
                        theme.bg,
                    )
                    .on_click(cx.listener(|model, _, window, cx| {
                        cx.stop_propagation();
                        model.toggle_sidebar(window, cx);
                    })),
                ),
        )
        .into_any_element()
}

pub(super) fn navigation_width(model: &AppModel) -> f32 {
    f32::from(model.metrics.traffic_light_width() + model.metrics.navigation_arrows_width())
}

fn arrow(model: &AppModel, forward: bool, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let enabled = model.can_navigate(forward);
    let id = if forward { "nav-forward" } else { "nav-back" };
    let color = if enabled {
        theme.fg_muted
    } else {
        gpui::Hsla {
            a: theme.fg_muted.a * 0.35,
            ..theme.fg_muted
        }
    };
    let glyph = IconGlyph::new(
        if forward {
            Icon::ChevronRight
        } else {
            Icon::ChevronLeft
        },
        m.font_body(),
        color,
    );
    let arrow = div()
        .id(id)
        .debug_selector(move || id.into())
        .group(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.scaled(22.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    if enabled {
        arrow
            .cursor_pointer()
            .on_click(cx.listener(move |model, _, _, cx| {
                cx.stop_propagation();
                model.navigate(forward, cx);
            }))
            .child(glyph.hover_in_group(id, theme.fg))
            .into_any_element()
    } else {
        arrow
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(glyph)
            .into_any_element()
    }
}
