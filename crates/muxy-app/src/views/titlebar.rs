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

#[cfg(target_os = "linux")]
pub(crate) mod frame;
#[cfg(target_os = "linux")]
pub(crate) mod linux;

pub(crate) fn content_size(window: &Window) -> gpui::Size<gpui::Pixels> {
    #[cfg(target_os = "linux")]
    return frame::bounds(window).size;
    #[cfg(not(target_os = "linux"))]
    window.viewport_size()
}

pub(crate) fn overlay(element: impl IntoElement, window: &Window) -> AnyElement {
    #[cfg(target_os = "linux")]
    return frame::overlay(element, window);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = window;
        element.into_any_element()
    }
}

pub(super) fn top_corners<T: Styled>(element: T, window: &Window, left: bool, right: bool) -> T {
    #[cfg(target_os = "linux")]
    return frame::top_corners(element, window, left, right);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (window, left, right);
        element
    }
}

pub(super) fn actions(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> (Div, f32) {
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
    let controls = 2
        + u8::from(existing_count > 0)
        + u8::from(browser)
        + u8::from(zoom_tab.is_some())
        + u8::from(layouts);
    let mut width = f32::from(controls)
        * f32::from(model.metrics.control_medium() + model.metrics.spacing2())
        + model.extension_toolbar_width();
    #[cfg(target_os = "linux")]
    let window_controls = linux::controls_width(window);
    #[cfg(not(target_os = "linux"))]
    let window_controls = 0.0;
    let room = f32::from(content_size(window).width)
        - navigation_width(model, window).max(model.sidebar_width())
        - width
        - window_controls;
    let project_apps =
        super::project_apps::control(model, room, window, cx).map(|(control, used)| {
            width += used;
            control
        });
    let bar = div()
        .debug_selector(|| "titlebar-actions".into())
        .flex()
        .flex_none()
        .items_center()
        .when(cfg!(target_os = "linux"), Styled::h_full)
        .children(project_apps)
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
        .child(super::agent_sessions::button(model, cx))
        .child(super::tab_strip::settings_button(model, cx));
    #[cfg(target_os = "linux")]
    if let Some(controls) = linux::controls(
        window,
        &model.theme,
        cx.listener(|model, _, _, cx| model.quit(cx)),
    ) {
        return (
            bar.child(controls.border_l_1().border_color(model.theme.border)),
            width + linux::controls_width(window) + 1.0,
        );
    }
    (bar, width)
}

#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = muxy, no_json)]
pub(crate) struct BeginWindowMove;

pub(crate) fn begin_window_move(
    model: &mut AppModel,
    _: &BeginWindowMove,
    window: &mut Window,
    _: &mut Context<AppModel>,
) {
    #[cfg(target_os = "macos")]
    let _ = &window;
    if model.coverage().modal() {
        return;
    }
    #[cfg(target_os = "macos")]
    if let Some(drag) = &model.window_drag {
        drag.begin();
    }
    #[cfg(not(target_os = "macos"))]
    window.start_window_move();
}

pub(crate) fn background(id: &'static str) -> Stateful<Div> {
    let background = div()
        .id(id)
        .debug_selector(move || id.into())
        .on_mouse_down(MouseButton::Left, |event, window, cx| {
            cx.stop_propagation();
            if event.click_count == 1 {
                window.dispatch_action(Box::new(BeginWindowMove), cx);
            }
        })
        .on_click(background_click);
    #[cfg(target_os = "linux")]
    let background = background.on_mouse_down(MouseButton::Right, |event, window, cx| {
        cx.stop_propagation();
        if window.window_controls().window_menu {
            window.show_window_menu(event.position);
        }
    });
    background
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
    let hover = model.theme.hover;
    with_tooltip(
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
            .hover(move |style| style.bg(hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        tooltip,
        model,
    )
}

pub(super) fn with_tooltip(
    element: Stateful<Div>,
    tooltip: impl Into<SharedString>,
    model: &AppModel,
) -> Stateful<Div> {
    let theme = model.theme.clone();
    let tooltip = tooltip.into();
    element.tooltip(move |_, cx| {
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
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let toggle = sidebar_toggle(model, cx);
    background("titlebar-navigation")
        .occlude()
        .absolute()
        .top_0()
        .left_0()
        .w(px(sidebar_width.max(navigation_width(model, window))))
        .h(m.title_bar_height())
        .flex()
        .items_center()
        .justify_end()
        .pr(m.spacing4())
        .bg(if model.appearance.sidebar_expanded {
            gpui::transparent_black()
        } else {
            theme.bg
        })
        .border_r_1()
        .border_color(theme.border)
        .map(|bar| top_corners(bar, window, true, false))
        .map(|bar| {
            if cfg!(target_os = "linux") {
                bar.child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .w(m.sidebar_collapsed_width())
                        .child(toggle),
                )
                .when(model.appearance.sidebar_expanded, |bar| {
                    bar.child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .truncate()
                            .text_size(m.font_body())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_muted)
                            .child("Muxy"),
                    )
                })
                .child(arrow(model, false, cx))
                .child(arrow(model, true, cx))
            } else {
                bar.gap(m.spacing1())
                    .child(arrow(model, false, cx))
                    .child(arrow(model, true, cx))
                    .child(toggle)
            }
        })
        .into_any_element()
}

fn sidebar_toggle(model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
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
        )
}

pub(super) fn navigation_width(model: &AppModel, _: &Window) -> f32 {
    if cfg!(target_os = "linux") {
        return f32::from(model.metrics.sidebar_collapsed_width() + model.metrics.scaled(52.0));
    }
    let arrows = model.metrics.navigation_arrows_width();
    f32::from(if cfg!(target_os = "macos") {
        model.metrics.traffic_light_width() + arrows
    } else {
        arrows
    })
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
