use gpui::{
    AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use muxy_ui::components::{ButtonInteraction, IconGlyph, Tooltip};
use muxy_ui::icon::Icon;

use crate::model::AppModel;

pub(crate) fn render(model: &AppModel, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    let (kind, message) = model.visible_banner()?;
    let theme = &model.theme;
    let m = model.metrics;
    let tooltip_theme = theme.clone();
    Some(
        div()
            .id("workspace-banner")
            .debug_selector(|| "workspace-banner".into())
            .flex()
            .flex_none()
            .items_start()
            .gap(m.spacing4())
            .px(m.spacing6())
            .py(m.spacing3())
            .bg(theme.surface)
            .text_color(theme.fg)
            .text_size(m.font_footnote())
            .child(
                div()
                    .id("workspace-banner-message")
                    .debug_selector(|| "workspace-banner-message".into())
                    .flex_1()
                    .min_w(px(0.0))
                    .max_h(m.scaled(80.0))
                    .overflow_y_scroll()
                    .py(m.spacing2())
                    .child(message.to_owned()),
            )
            .child(
                div()
                    .id("dismiss-workspace-banner")
                    .debug_selector(|| "dismiss-workspace-banner".into())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(m.control_small())
                    .rounded(m.radius_sm())
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.hover))
                    .tooltip(move |_, cx| {
                        cx.new(|_| {
                            Tooltip::new(
                                "Dismiss message",
                                tooltip_theme.raised(),
                                tooltip_theme.fg,
                                tooltip_theme.border,
                            )
                        })
                        .into()
                    })
                    .button_interaction(cx.listener(move |model, _, window, cx| {
                        model.dismiss_banner(kind);
                        model.focus_active(window, cx);
                        cx.notify();
                    }))
                    .child(IconGlyph::new(Icon::X, m.icon_sm(), theme.fg_muted)),
            )
            .into_any_element(),
    )
}
