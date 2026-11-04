use std::rc::Rc;

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::components::IconGlyph;
use muxy_ui::controls::{self, Style};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::command_palette::Handler;
use crate::model::AppModel;

pub(super) fn register_commands(registry: &mut Registry<Handler>, model: &AppModel) {
    if !model.appearance.tips_visible {
        return;
    }
    let show: Handler = Rc::new(|model, _, cx| model.show_tip(cx));
    registry.register(Command::new("show_tip", tr!("Show Muxy Tip"), show).keywords("Tips"));
}

/// The tip toast in the window's bottom-right corner, above the status bar.
pub(super) fn toast(
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let toast = model.tips.toast.as_ref()?;
    let m = model.metrics;
    let theme = &model.theme;
    let progress = toast.motion.sample(window);
    let status_bar = if model.appearance.status_bar_visible {
        m.status_bar_height()
    } else {
        px(0.0)
    };
    let text = model.tips.current();
    let card = muxy_ui::popover::surface(theme, m)
        .id("tip-toast")
        .accessibility_id("tip-toast")
        .aria_label(tr!("MUXY TIP"))
        .debug_selector(|| "tip-toast".into())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_hover(cx.listener(|model, hovered: &bool, _, cx| model.hover_tip(*hovered, cx)))
        .gap(m.spacing5())
        .w(m.scaled(300.0))
        .max_w((window.viewport_size().width - m.spacing7()).max(px(0.0)))
        .p(m.spacing7())
        .child(heading(model))
        .child(
            div()
                .debug_selector(|| "tip-text".into())
                .text_size(m.font_body())
                .text_color(theme.fg)
                .child(text.clone()),
        )
        .child(actions(model, cx))
        .child(model.webview_occlusion());
    Some(
        div()
            .absolute()
            .right(m.spacing6())
            .bottom(status_bar + m.spacing6() - m.scaled(8.0) * (1.0 - progress))
            .opacity(progress)
            .child(muxy_ui::accessibility::announced(
                card,
                format!("{}\n{text}", tr!("MUXY TIP")),
                false,
            ))
            .into_any_element(),
    )
}

fn heading(model: &AppModel) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .items_center()
        .gap(m.spacing3())
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(m.control_small())
                .rounded(m.radius_md())
                .bg(theme.accent_soft)
                .child(IconGlyph::new(Icon::Lightbulb, m.icon_xs(), theme.accent)),
        )
        .child(
            div()
                .flex_1()
                .text_size(m.font_caption())
                .font_weight(FontWeight::BOLD)
                .text_color(theme.accent)
                .child(tr!("MUXY TIP")),
        )
}

fn actions(model: &AppModel, cx: &mut Context<AppModel>) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .flex_wrap()
        .justify_end()
        .gap(m.spacing3())
        .child(
            div().debug_selector(|| "tip-turn-off".into()).child(
                controls::button(
                    Style { theme, metrics: &m },
                    "tip-turn-off",
                    controls::Button::secondary(&tr!("Don't Show Tips")),
                    cx.listener(|model, _, _, cx| model.turn_off_tips(cx)),
                )
                .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default()),
            ),
        )
        .child(
            div().debug_selector(|| "tip-got-it".into()).child(
                controls::button(
                    Style { theme, metrics: &m },
                    "tip-got-it",
                    controls::Button::primary(&tr!("Got It")),
                    cx.listener(|model, _, _, cx| model.hide_tip(cx)),
                )
                .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default()),
            ),
        )
}
