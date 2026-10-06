use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, canvas, div,
};
use muxy_ui::components::{IconButton, IconGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use crate::model::AppModel;
use crate::model::tips::{TIPS, TipPlacement};

/// The tip card or tip button at the bottom of the built-in sidebar.
pub(super) fn footer(model: &AppModel, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    let m = model.metrics;
    let theme = &model.theme;
    Some(match model.tip_placement()? {
        TipPlacement::Card => content(model, cx)
            .debug_selector(|| "sidebar-tip".into())
            .flex_none()
            .mx(m.spacing3())
            .mb(m.spacing3())
            .p(m.spacing6())
            .rounded(m.radius_lg())
            .border_1()
            .border_color(theme.border)
            .bg(theme.surface)
            .into_any_element(),
        TipPlacement::Button => {
            let anchor = model.tips.anchor.clone();
            div()
                .flex()
                .flex_none()
                .justify_center()
                .pb(m.spacing4())
                .child(
                    div()
                        .relative()
                        .debug_selector(|| "sidebar-tip-button".into())
                        .child(
                            canvas(
                                move |bounds, _, _| anchor.set(Some(bounds)),
                                |_, (), _, _| {},
                            )
                            .absolute()
                            .size_full(),
                        )
                        .child(
                            IconButton::new(
                                "sidebar-tip-button",
                                Icon::Lightbulb,
                                m.font_body(),
                                m.control_large(),
                                theme.fg_muted,
                                theme.fg,
                            )
                            .tooltip(
                                tr!("Show Muxy Tip"),
                                theme.raised(),
                                theme.fg,
                                theme.border,
                                theme.bg,
                            )
                            .on_click(cx.listener(
                                |model, _, window, cx| {
                                    model.toggle_tip_popover(window, cx);
                                },
                            )),
                        ),
                )
                .into_any_element()
        }
    })
}

/// The tip card, opened from the collapsed sidebar's tip button.
pub(in crate::views) fn popover_card(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    muxy_ui::popover::surface(&model.theme, m)
        .id("tip-popover")
        .debug_selector(|| "tip-popover".into())
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(
            cx.listener(|model, _: &crate::views::menu::DismissMenu, _, cx| {
                model.dismiss_overlay(cx);
            }),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .w(m.scaled(300.0))
        .p(m.spacing7())
        .child(content(model, cx))
        .into_any_element()
}

fn content(model: &AppModel, cx: &mut Context<AppModel>) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    let header = div()
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
        .child(
            div().debug_selector(|| "hide-tips".into()).child(
                IconButton::new(
                    "hide-tips",
                    Icon::X,
                    m.icon_xs(),
                    m.control_small(),
                    theme.fg_dim,
                    theme.fg,
                )
                .tooltip(
                    tr!("Hide Tips"),
                    theme.raised(),
                    theme.fg,
                    theme.border,
                    theme.bg,
                )
                .on_click(cx.listener(|model, _, _, cx| model.confirm_hide_tips(cx))),
            ),
        );
    let controls = div()
        .flex()
        .items_center()
        .gap(m.spacing2())
        .child(
            div()
                .debug_selector(|| "tip-position".into())
                .flex_1()
                .text_size(m.font_caption())
                .text_color(theme.fg_dim)
                .child(tr!("%lld of %lld", model.tips.position(), TIPS.len())),
        )
        .child(step_button(false, model, cx))
        .child(step_button(true, model, cx));
    div()
        .flex()
        .flex_col()
        .gap(m.spacing5())
        .child(header)
        .child(
            div()
                .debug_selector(|| "tip-text".into())
                .text_size(m.font_body())
                .text_color(theme.fg)
                .child(model.tips.current()),
        )
        .child(controls)
}

fn step_button(forward: bool, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let (id, icon) = if forward {
        ("next-tip", Icon::ChevronRight)
    } else {
        ("previous-tip", Icon::ChevronLeft)
    };
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(m.control_medium())
        .h(m.control_small())
        .rounded(m.radius_sm())
        .border_1()
        .border_color(theme.border)
        .bg(theme.hover)
        .cursor_pointer()
        .hover(|style| style.bg(theme.surface))
        .on_click(cx.listener(move |model, _, _, cx| model.step_tip(forward, cx)))
        .child(IconGlyph::new(icon, m.icon_xs(), theme.fg_muted))
        .into_any_element()
}
