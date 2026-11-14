use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Role,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_ui::components::{ButtonInteraction, IconGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

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
    let close = div()
        .id("dismiss-workspace-toast")
        .role(Role::Button)
        .accessibility_id("dismiss-workspace-toast")
        .aria_label(tr!("Dismiss"))
        .debug_selector(|| "dismiss-workspace-toast".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.control_small())
        .rounded(m.radius_sm())
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover))
        .tooltip(muxy_ui::components::tooltip(tr!("Dismiss"), theme))
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
    let id = if close.is_some() {
        "workspace-problem-toast"
    } else {
        "workspace-notice-toast"
    };
    div()
        .flex()
        .justify_center()
        .max_w_full()
        .mt(-model.metrics.scaled(12.0) * (1.0 - progress))
        .opacity(progress)
        .child(muxy_ui::toast::announced(
            muxy_ui::toast::toast(
                toast.kind,
                &toast.message,
                toast.body.as_deref(),
                &model.theme,
                model.metrics,
            )
            .id(id)
            .accessibility_id(id)
            .min_w(px(0.0))
            .children(close)
            .child(model.webview_occlusion()),
            toast.kind,
            &toast.message,
            toast.body.as_deref(),
        ))
        .into_any_element()
}
