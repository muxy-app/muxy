use gpui::{AnyElement, Context, InteractiveElement, IntoElement, StatefulInteractiveElement};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use crate::model::AppModel;

pub(super) fn button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let project = model.state.current_project().id;
    super::titlebar::icon_button(
        "project-layouts",
        Icon::LayoutSplit,
        tr!("Apply Layout"),
        model,
    )
    .debug_selector(|| "project-layouts-button".into())
    .on_click(cx.listener(move |model, _, window, cx| {
        cx.stop_propagation();
        model.open_layout_picker(project, window, cx);
    }))
    .into_any_element()
}
