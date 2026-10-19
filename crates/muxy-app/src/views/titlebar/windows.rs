//! Caption buttons for the titlebar the app draws on Windows. Each is marked
//! as the window control it stands for, so Windows handles the clicks, shows
//! Snap layouts over Maximize, and closes through the window's close
//! request.

use gpui::{
    Div, InteractiveElement, IntoElement, ParentElement, Rgba, Stateful, Styled, Window,
    WindowControlArea, div, px, rgb, svg,
};
use muxy_ui::theme::Theme;

const BUTTON_WIDTH: f32 = 46.0;
/// The color Windows gives a close button under the pointer.
const CLOSE_HOVER: u32 = 0x00c4_2b1c;

pub(in crate::views) fn controls_width() -> f32 {
    BUTTON_WIDTH * 3.0
}

pub(in crate::views) fn controls(window: &Window, theme: &Theme) -> Stateful<Div> {
    let maximize = if window.is_maximized() {
        "icons/window-restore.svg"
    } else {
        "icons/window-maximize.svg"
    };
    div()
        .id("window-controls")
        .debug_selector(|| "window-controls".into())
        // Windows acts on a press only when no focusable view behind took it.
        .occlude()
        .flex()
        .flex_none()
        .h_full()
        .child(button(
            "window-minimize",
            "icons/window-minimize.svg",
            WindowControlArea::Min,
            theme.hover.into(),
            theme,
        ))
        .child(button(
            "window-maximize",
            maximize,
            WindowControlArea::Max,
            theme.hover.into(),
            theme,
        ))
        .child(button(
            "window-close",
            "icons/window-close.svg",
            WindowControlArea::Close,
            rgb(CLOSE_HOVER),
            theme,
        ))
}

/// The buttons in the top right corner of a window whose titlebar is
/// `32` pixels high.
pub(in crate::views) fn corner(window: &Window, theme: &Theme) -> Div {
    div()
        .absolute()
        .right_0()
        .top_0()
        .h(px(32.0))
        .child(controls(window, theme))
}

fn button(
    id: &'static str,
    icon: &'static str,
    area: WindowControlArea,
    hover: Rgba,
    theme: &Theme,
) -> impl IntoElement {
    let foreground = theme.fg;
    let hover_foreground = if area == WindowControlArea::Close {
        gpui::white()
    } else {
        foreground
    };
    div()
        .id(id)
        .group(id)
        .debug_selector(move || id.into())
        .window_control_area(area)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .w(px(BUTTON_WIDTH))
        .h_full()
        .hover(move |style| style.bg(hover))
        .child(
            svg()
                .path(icon)
                .size(px(16.0))
                .flex_none()
                .text_color(theme.fg_muted)
                .group_hover(id, move |style| style.text_color(hover_foreground)),
        )
}
