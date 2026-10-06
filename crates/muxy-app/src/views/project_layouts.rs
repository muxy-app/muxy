use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, div,
};
use muxy_ui::components::IconButton;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use crate::model::AppModel;

pub(super) fn button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let theme = &model.theme;
    let project = model.state.current_project().id;
    div()
        .debug_selector(|| "project-layouts-button".into())
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .pr(model.metrics.spacing2())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new(
                "project-layouts",
                Icon::LayoutSplit,
                model.metrics.scaled(14.0),
                model.metrics.control_medium(),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(
                tr!("Apply Layout"),
                theme.raised(),
                theme.fg,
                theme.border,
                theme.bg,
            )
            .on_click(cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.open_layout_picker(project, window, cx);
            })),
        )
        .into_any_element()
}
