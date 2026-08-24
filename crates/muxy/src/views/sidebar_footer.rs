use crate::state::AppState;
use crate::views::app::AppLayout;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use muxy_ui::components::IconButton;
use muxy_ui::icon::Icon;

use crate::views::window::MainWindow;

pub fn sidebar_footer(
    state: &AppState,
    layout: AppLayout,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;
    let glyph = metrics.scaled(13.0);
    let box_size = metrics.control_medium();

    let toggle = IconButton::new(
        "toggle-sidebar",
        Icon::PanelLeft,
        glyph,
        box_size,
        theme.fg_muted,
        theme.fg,
    )
    .on_click(cx.listener(|window: &mut MainWindow, _, _, cx| {
        window.toggle_sidebar(cx);
    }));

    let notifications = IconButton::new(
        "notifications",
        Icon::Bell,
        glyph,
        box_size,
        theme.fg_muted,
        theme.fg,
    );
    let extensions = IconButton::new(
        "extensions",
        Icon::Puzzle,
        glyph,
        box_size,
        theme.fg_muted,
        theme.fg,
    );
    let theme_picker = IconButton::new(
        "theme-picker",
        Icon::Palette,
        glyph,
        box_size,
        theme.fg_muted,
        theme.fg,
    )
    .on_click(cx.listener(
        |window: &mut MainWindow, _, window_handle: &mut gpui::Window, cx| {
            window.open_theme_picker(window_handle, cx);
        },
    ));

    if layout.wide_sidebar {
        return div()
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(metrics.spacing2())
            .px(metrics.spacing5())
            .pb(metrics.spacing4())
            .child(toggle)
            .child(div().flex_grow())
            .child(notifications)
            .child(extensions)
            .child(theme_picker)
            .into_any_element();
    }

    div()
        .flex()
        .flex_col()
        .flex_none()
        .items_center()
        .gap(metrics.spacing2())
        .pb(metrics.spacing4())
        .child(notifications)
        .child(extensions)
        .child(theme_picker)
        .child(toggle)
        .into_any_element()
}
