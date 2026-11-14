use crate::model::AppModel;
use crate::model::extensions::Placement;
use crate::views::overlays::{Mark, OverlayView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};

/// The open extension popover.
pub(crate) struct ExtensionPopover;

impl OverlayView for ExtensionPopover {
    fn render(
        &self,
        _: Option<Mark>,
        model: &AppModel,
        _: &Window,
        cx: &mut Context<AppModel>,
    ) -> AnyElement {
        render(model, cx)
    }

    /// Outside clicks and toggles close it without asking the page.
    fn on_close(&self, model: &mut AppModel, cx: &mut Context<AppModel>) {
        model.close_extension_popover(cx);
    }
}

/// The open extension popover: below its topbar item, left of its rail item,
/// or above its status-bar item.
fn render(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
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
    let weak = cx.entity().downgrade();
    if !model.extension_item_shown(&popover.anchor_key, popover.placement) {
        popover.anchor.set(None);
        let weak = weak.clone();
        cx.defer(move |cx| {
            let _ = weak.update(cx, AppModel::close_extension_popover);
        });
    }
    let lost = move |_: &mut Window, cx: &mut gpui::App| {
        let _ = weak.update(cx, AppModel::close_extension_popover);
    };
    let anchor = popover.anchor.clone();
    match popover.placement {
        Placement::Topbar => muxy_ui::popover::anchored_popover(anchor, content, lost),
        Placement::Rail => muxy_ui::popover::anchored_popover_left(anchor, content, lost),
        Placement::StatusBar => muxy_ui::popover::anchored_popover_above(anchor, content, lost),
    }
}
