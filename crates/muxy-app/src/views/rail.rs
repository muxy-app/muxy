//! The rail: a column of extension icons on the right edge of the workspace,
//! opened from a toggle at the end of the top bar.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use muxy_ui::components::IconButton;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use crate::model::AppModel;

/// The top bar's rail toggle, shown while the rail is on and has items. It
/// mirrors the sidebar toggle on the left.
pub(super) fn toggle(model: &AppModel, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    if model.rail_items().is_empty() {
        return None;
    }
    let m = model.metrics;
    let theme = &model.theme;
    Some(
        div()
            .debug_selector(|| "toggle-rail".into())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(m.control_medium())
            .mr(m.spacing2())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                IconButton::new(
                    "toggle-rail",
                    tr!("Toggle Rail"),
                    Icon::SidebarRight,
                    m.font_body(),
                    m.scaled(22.0),
                    theme.fg_muted,
                    theme.fg,
                )
                .expanded(model.appearance.rail_expanded)
                .tooltip(theme)
                .on_click(cx.listener(|model, _, _, cx| {
                    cx.stop_propagation();
                    model.toggle_rail(cx);
                })),
            )
            .into_any_element(),
    )
}

/// `content` with the open rail on its right.
pub(super) fn content(
    content: AnyElement,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    if !model.rail_shown() {
        return content;
    }
    let m = model.metrics;
    div()
        .flex()
        .size_full()
        .child(div().flex_1().min_w(px(0.0)).h_full().child(content))
        .child(
            div()
                .id("rail")
                .debug_selector(|| "rail".into())
                .flex()
                .flex_col()
                .flex_none()
                .items_center()
                .gap(m.spacing2())
                .w(px(width(model)))
                .h_full()
                .py(m.spacing2())
                .overflow_y_scroll()
                .bg(model.theme.bg)
                .border_l_1()
                .border_color(model.theme.border)
                .children(model.rail_buttons(cx)),
        )
        .into_any_element()
}

/// The width the open rail takes from the workspace.
pub(crate) fn width(model: &AppModel) -> f32 {
    if !model.rail_shown() {
        return 0.0;
    }
    let m = model.metrics;
    f32::from(m.control_medium() + m.spacing2() * 2.0)
}
