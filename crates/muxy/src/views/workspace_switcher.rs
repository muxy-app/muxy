use crate::state::AppState;
use crate::views::app::AppLayout;
use crate::views::window::MainWindow;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, Styled, div,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;

pub fn workspace_switcher(
    state: &AppState,
    layout: AppLayout,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    if !layout.wide_sidebar {
        return div()
            .id("workspace-switcher")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(metrics.icon_xxl())
            .rounded(metrics.radius_sm())
            .bg(theme.surface)
            .hover(|style| style.bg(theme.hover))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(
                    move |window: &mut MainWindow, event: &MouseDownEvent, view, cx| {
                        window.open_workspace_menu(event.position, view, cx);
                    },
                ),
            )
            .child(IconGlyph::new(
                Icon::ChevronDown,
                metrics.font_caption(),
                theme.fg_muted,
            ))
            .into_any_element();
    }

    div()
        .id("workspace-switcher")
        .flex()
        .flex_row()
        .flex_grow()
        .items_center()
        .gap(metrics.spacing2())
        .px(metrics.spacing4())
        .h(metrics.control_medium())
        .rounded(metrics.radius_md())
        .bg(theme.surface)
        .hover(|style| style.bg(theme.hover))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(
                move |window: &mut MainWindow, event: &MouseDownEvent, view, cx| {
                    window.open_workspace_menu(event.position, view, cx);
                },
            ),
        )
        .child(
            div()
                .flex_grow()
                .text_size(metrics.font_caption())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .truncate()
                .child(SharedString::from(state.workspace.active_group_name())),
        )
        .child(IconGlyph::new(
            Icon::ChevronDown,
            metrics.font_caption(),
            theme.fg_muted,
        ))
        .into_any_element()
}

pub fn sort_button(state: &AppState) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    div()
        .id("sort-projects")
        .group("sort-projects")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(metrics.control_medium())
        .rounded(metrics.radius_md())
        .bg(theme.surface)
        .hover(|style| style.bg(theme.hover))
        .cursor_pointer()
        .child(
            IconGlyph::new(Icon::ArrowUpDown, metrics.font_caption(), theme.fg_muted)
                .hover_in_group("sort-projects", theme.accent),
        )
        .into_any_element()
}
