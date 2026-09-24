use crate::model::AppModel;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
    Window, div, px, rgba,
};

pub(crate) fn render(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let Some(request) = &model.webviews.modal else {
        return div().into_any_element();
    };
    let metrics = model.metrics;
    let (width, height, top) = dimensions(&request.options, metrics, window.viewport_size());
    let outside = request.options.dismiss_on_outside_click;
    div()
        .absolute()
        .inset_0()
        .flex()
        .justify_center()
        .items_start()
        .pt(top)
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(
            cx.listener(|model, _: &crate::views::menu::DismissMenu, _, cx| {
                model.dismiss_webview_modal(cx);
            }),
        )
        .bg(rgba(0x0000_004d))
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, _, _, cx| {
                if outside {
                    model.dismiss_webview_modal(cx);
                }
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .w(width)
                .h(height)
                .rounded(metrics.radius_xl())
                .bg(model.theme.bg)
                .border_1()
                .border_color(model.theme.border)
                .overflow_hidden()
                .shadow(muxy_ui::theme::Elevation::Modal.shadow(model.theme.bg))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(request.surface.view.clone()),
        )
        .into_any_element()
}

fn dimensions(
    options: &muxy_app_core::webview::ModalOptions,
    metrics: muxy_ui::theme::Metrics,
    viewport: gpui::Size<gpui::Pixels>,
) -> (gpui::Pixels, gpui::Pixels, gpui::Pixels) {
    let margin = metrics.scaled(20.0);
    let top = metrics.scaled(60.0).min(viewport.height / 4.0);
    #[allow(clippy::cast_possible_truncation)]
    let width = metrics
        .scaled(options.width as f32)
        .min((viewport.width - margin * 2.0).max(px(0.0)));
    #[allow(clippy::cast_possible_truncation)]
    let height = metrics
        .scaled(options.height as f32)
        .min((viewport.height - top - margin).max(px(0.0)));
    (width, height, top)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_modals_fit_small_windows_at_every_interface_scale() {
        let options = muxy_app_core::webview::ModalOptions {
            width: 900.0,
            height: 760.0,
            ..Default::default()
        }
        .normalized();
        for scale in [0.75, 1.0, 1.5, 2.0] {
            let metrics = muxy_ui::theme::Metrics::new(scale);
            for viewport in [
                gpui::size(px(640.0), px(400.0)),
                gpui::size(px(1200.0), px(900.0)),
            ] {
                let (width, height, top) = dimensions(&options, metrics, viewport);
                assert!(width > px(0.0) && height > px(0.0));
                assert!(width + metrics.scaled(40.0) <= viewport.width);
                assert!(top + height + metrics.scaled(20.0) <= viewport.height);
            }
        }
    }
}
