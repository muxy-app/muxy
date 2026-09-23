use gpui::{
    AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_ui::components::{ButtonInteraction, IconGlyph, Tooltip};
use muxy_ui::icon::Icon;

use crate::model::{AppModel, banners::Toast};

pub(crate) fn render(
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    if model.problem_toast.is_none() && model.notice_toast.is_none() {
        return None;
    }
    let m = model.metrics;
    Some(
        div()
            .absolute()
            .top(m.scaled(40.0))
            .left_0()
            .right_0()
            .px(m.spacing7())
            .flex()
            .flex_col()
            .items_center()
            .gap(m.spacing3())
            .children(
                model
                    .problem_toast
                    .as_ref()
                    .map(|toast| problem(toast, model, window, cx)),
            )
            .children(
                model
                    .notice_toast
                    .as_ref()
                    .map(|toast| placed(toast, None, model, window)),
            )
            .into_any_element(),
    )
}

fn problem(
    toast: &Toast,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    let tooltip_theme = theme.clone();
    let close = div()
        .id("dismiss-workspace-toast")
        .debug_selector(|| "dismiss-workspace-toast".into())
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
                    "Dismiss",
                    tooltip_theme.raised(),
                    tooltip_theme.fg,
                    tooltip_theme.border,
                )
            })
            .into()
        })
        .button_interaction(cx.listener(|model, _, window, cx| {
            model.dismiss_problem(cx);
            model.focus_active(window, cx);
        }))
        .child(IconGlyph::new(Icon::X, m.icon_sm(), theme.fg_muted))
        .into_any_element();
    placed(toast, Some(close), model, window)
}

fn placed(
    toast: &Toast,
    close: Option<AnyElement>,
    model: &AppModel,
    window: &Window,
) -> AnyElement {
    let progress = toast.motion.sample(window);
    div()
        .flex()
        .justify_center()
        .max_w_full()
        .mt(-model.metrics.scaled(12.0) * (1.0 - progress))
        .opacity(progress)
        .child(
            muxy_ui::toast::toast(
                toast.kind,
                &toast.message,
                toast.body.as_deref(),
                &model.theme,
                model.metrics,
            )
            .min_w(px(0.0))
            .children(close)
            .child(model.webview_occlusion()),
        )
        .into_any_element()
}
