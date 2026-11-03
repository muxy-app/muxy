use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, ClickEvent, Decorations, Div, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Role, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Window, div, svg,
};
use muxy_ui::components::Tooltip;
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::tr;

const BUTTON_SIZE: f32 = 24.0;
const BUTTON_GAP: f32 = 4.0;
const GROUP_PADDING: f32 = 8.0;
const GROUP_MARGIN: f32 = 4.0;

pub(in crate::views) fn controls_width(window: &Window, metrics: Metrics) -> f32 {
    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return 0.0;
    }
    let capabilities = window.window_controls();
    let count = 1 + u8::from(capabilities.minimize) + u8::from(capabilities.maximize);
    f32::from(metrics.scaled(
        BUTTON_SIZE * f32::from(count)
            + BUTTON_GAP * f32::from(count - 1)
            + GROUP_PADDING * 2.0
            + GROUP_MARGIN,
    ))
}

pub(in crate::views) fn controls(
    window: &Window,
    theme: &Theme,
    metrics: Metrics,
    close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Option<Stateful<Div>> {
    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return None;
    }
    let capabilities = window.window_controls();
    Some(
        div()
            .id("window-controls")
            .debug_selector(|| "window-controls".into())
            .flex()
            .flex_none()
            .items_center()
            .h_full()
            .gap(metrics.scaled(BUTTON_GAP))
            .ml(metrics.scaled(GROUP_MARGIN))
            .px(metrics.scaled(GROUP_PADDING))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .when(capabilities.minimize, |controls| {
                controls.child(button(
                    "window-minimize",
                    tr!("Minimize"),
                    "icons/window-minimize.svg",
                    theme,
                    metrics,
                    |_, window, _| window.minimize_window(),
                ))
            })
            .when(capabilities.maximize, |controls| {
                controls.child(button(
                    "window-maximize",
                    if window.is_maximized() {
                        tr!("Restore Window")
                    } else {
                        tr!("Maximize")
                    },
                    if window.is_maximized() {
                        "icons/window-restore.svg"
                    } else {
                        "icons/window-maximize.svg"
                    },
                    theme,
                    metrics,
                    |_, window, _| window.zoom_window(),
                ))
            })
            .child(button(
                "window-close",
                tr!("Close Window"),
                "icons/window-close.svg",
                theme,
                metrics,
                close,
            )),
    )
}

fn button(
    id: &'static str,
    label: SharedString,
    icon: &'static str,
    theme: &Theme,
    metrics: Metrics,
    action: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let (background, foreground, border, theme_background) =
        (theme.raised(), theme.fg, theme.border, theme.bg);
    div()
        .id(id)
        .role(Role::Button)
        .accessibility_id(id)
        .aria_label(label.clone())
        .group(id)
        .debug_selector(move || id.into())
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(metrics.scaled(BUTTON_SIZE))
        .rounded_full()
        .hover(|style| style.bg(theme.hover))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            action(event, window, cx);
        })
        .tooltip(move |_, cx| {
            cx.new(|_| {
                Tooltip::new(
                    label.clone(),
                    background,
                    foreground,
                    border,
                    theme_background,
                )
            })
            .into()
        })
        .child(
            svg()
                .path(icon)
                .size(metrics.scaled(16.0))
                .flex_none()
                .text_color(theme.fg_muted)
                .group_hover(id, |style| style.text_color(foreground)),
        )
        .into_any_element()
}
