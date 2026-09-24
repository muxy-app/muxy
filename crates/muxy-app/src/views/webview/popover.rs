use crate::model::AppModel;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};

/// The open extension popover, below its topbar item or above its status-bar item.
pub(crate) fn render(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let Some(popover) = &model.webviews.popover else {
        return div().into_any_element();
    };
    let metrics = model.metrics;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Popover sizes are clamped to a few hundred points"
    )]
    let content = muxy_ui::popover::surface(&model.theme, metrics)
        .w(metrics.scaled(popover.width as f32))
        .h(metrics.scaled(popover.height as f32))
        .p_0()
        .gap_0()
        .overflow_hidden()
        .child(popover.surface.view.clone())
        .into_any_element();
    if popover.above && !model.appearance.status_bar_visible {
        popover.anchor.set(None);
    }
    let weak = cx.entity().downgrade();
    let lost = move |_: &mut gpui::Window, cx: &mut gpui::App| {
        let _ = weak.update(cx, AppModel::close_extension_popover);
    };
    if popover.above {
        muxy_ui::popover::anchored_popover_above(popover.anchor.clone(), content, lost)
    } else {
        muxy_ui::popover::anchored_popover(popover.anchor.clone(), content, lost)
    }
}
