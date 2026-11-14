//! The welcome tour's modal: animated art beside one short step, with a way
//! out on every step.

pub(crate) mod art;
mod steps;

use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, Div, FontWeight, InteractiveElement,
    IntoElement, MouseDownEvent, ParentElement, Pixels, Size, StatefulInteractiveElement, Styled,
    Window, div, ease_out_quint, hsla, px,
};
use muxy_ui::accessibility;
use muxy_ui::components::ButtonInteraction;
use muxy_ui::controls::{self, Style};
use muxy_ui::form::{NextControl, PreviousControl};
use muxy_ui::theme::Metrics;
use muxy_ui::tr;

use super::menu::{CloseSubmenu, ConfirmHighlighted, DismissMenu, OpenSubmenu};
use super::overlays::{Dismissal, Mark, OverlayView, Placement};
use crate::model::AppModel;
use crate::model::onboarding::{OfferState, Step};

const CARD_WIDTH: f32 = 900.0;
const CARD_HEIGHT: f32 = 580.0;
const CARD_RADIUS: f32 = 16.0;
const ART_RADIUS: f32 = 10.0;
const ART_MARGIN: f32 = 6.0;
const ART_SHARE: f32 = 0.56;
/// Narrower than this and the words need all of the room.
const ART_MIN_CARD_WIDTH: f32 = 760.0;
const WINDOW_MARGIN: f32 = 16.0;
pub(crate) const BACKDROP_ALPHA: f32 = 0.6;

/// The card and its art at the current window size. The art is left out when
/// the window is too small to spare room for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Layout {
    pub(crate) card: Size<Pixels>,
    pub(crate) art: Option<Size<Pixels>>,
}

impl Layout {
    pub(crate) fn of(viewport: Size<Pixels>, metrics: Metrics) -> Self {
        let margin = px(WINDOW_MARGIN) * 2.0;
        let card = Size {
            width: metrics
                .scaled(CARD_WIDTH)
                .min((viewport.width - margin).max(px(0.0))),
            height: metrics
                .scaled(CARD_HEIGHT)
                .min((viewport.height - margin).max(px(0.0))),
        };
        let inset = metrics.scaled(ART_MARGIN) * 2.0;
        let art = (card.width >= metrics.scaled(ART_MIN_CARD_WIDTH)).then(|| Size {
            width: card.width * ART_SHARE - inset,
            height: card.height - inset,
        });
        Self { card, art }
    }
}

pub(crate) struct OnboardingModal;

impl OverlayView for OnboardingModal {
    fn render(
        &self,
        _: Option<Mark>,
        model: &AppModel,
        window: &Window,
        cx: &mut Context<AppModel>,
    ) -> AnyElement {
        render(model, window, cx)
    }

    fn placement(&self, _: &AppModel) -> Placement {
        Placement::OwnBackdrop
    }

    fn moves_window(&self) -> bool {
        true
    }

    /// The tour only ends by finishing or skipping it, so a stray click or
    /// another command's clean-up can't lose it.
    fn on_dismiss(&self, _: &mut AppModel, _: &mut Context<AppModel>) -> Dismissal {
        Dismissal::Stay
    }

    fn on_close(&self, model: &mut AppModel, cx: &mut Context<AppModel>) {
        model.onboarding_hidden(cx);
    }
}

fn render(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let Some(tour) = &model.onboarding.tour else {
        return div().into_any_element();
    };
    let m = model.metrics;
    let theme = &model.theme;
    let viewport = window.viewport_size();
    let layout = Layout::of(viewport, m);
    let step = tour.step;
    let card = muxy_ui::popover::modal_card(
        theme,
        m,
        viewport,
        "onboarding",
        tr!("Welcome to Muxy"),
        layout.card.width,
    )
    .debug_selector(|| "onboarding".into())
    .accessibility_id("onboarding")
    .key_context("Menu Form")
    .track_focus(&model.overlay_focus)
    .on_action(cx.listener(|model, _: &DismissMenu, _, cx| model.finish_onboarding(cx)))
    .on_action(cx.listener(|model, _: &CloseSubmenu, _, cx| model.step_onboarding(false, cx)))
    .on_action(cx.listener(|model, _: &OpenSubmenu, _, cx| {
        if model
            .onboarding
            .tour
            .as_ref()
            .is_some_and(|tour| tour.step.next().is_some())
        {
            model.step_onboarding(true, cx);
        }
    }))
    .on_action(cx.listener(|model, _: &ConfirmHighlighted, _, cx| model.step_onboarding(true, cx)))
    .on_action(cx.listener(|model, _: &NextControl, window, cx| {
        muxy_ui::form::advance_focus_within(&model.overlay_focus, false, window, cx);
    }))
    .on_action(cx.listener(|model, _: &PreviousControl, window, cx| {
        muxy_ui::form::advance_focus_within(&model.overlay_focus, true, window, cx);
    }))
    .flex_row()
    .p_0()
    .gap_0()
    .h(layout.card.height)
    .max_h(layout.card.height)
    .overflow_hidden()
    .rounded(m.scaled(CARD_RADIUS))
    .when_some(layout.art, |card, art| {
        card.child(
            window_mover("onboarding-art-drag", cx)
                .flex_none()
                .relative()
                .m(m.scaled(ART_MARGIN))
                .w(art.width)
                .h(art.height)
                .child(tour.art.clone()),
        )
    })
    .child(column(step, layout.art.is_some(), model, cx));
    let card = if model.quick.reduce_motion() {
        card.into_any_element()
    } else {
        card.with_animation(
            "onboarding-card",
            Animation::new(Duration::from_millis(420)).with_easing(ease_out_quint()),
            |card, delta| card.opacity(delta).mt(px((1.0 - delta) * 14.0)),
        )
        .into_any_element()
    };
    window_mover("onboarding-backdrop", cx)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(hsla(0.0, 0.0, 0.0, BACKDROP_ALPHA))
        .occlude()
        .child(card)
        .into_any_element()
}

/// Empty space that moves the window. The move starts directly, not as an
/// action: actions travel up from whatever has focus, and a focused button may
/// be gone by now, such as Back on the first step.
fn window_mover(id: &'static str, cx: &mut Context<AppModel>) -> gpui::Stateful<Div> {
    super::titlebar::background_with(
        id,
        cx.listener(|model, _: &MouseDownEvent, window, cx| {
            model.overlay_focus.focus(window);
            super::titlebar::begin_window_move(
                model,
                &super::titlebar::BeginWindowMove,
                window,
                cx,
            );
        }),
    )
}

/// The words and controls beside the art.
fn column(step: Step, art_shown: bool, model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let body = steps::body(step, art_shown, model, cx);
    let body = if model.quick.reduce_motion() {
        body.into_any_element()
    } else {
        body.with_animation(
            ("onboarding-step", step.index()),
            Animation::new(Duration::from_millis(360)).with_easing(ease_out_quint()),
            |body, delta| body.opacity(delta).ml(px((1.0 - delta) * 16.0)),
        )
        .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .child(header(step, model, cx))
        .child(
            div()
                .id("onboarding-body")
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .px(m.spacing9())
                .pb(m.spacing6())
                .child(body),
        )
        .child(footer(step, model, cx))
}

fn header(step: Step, model: &AppModel, cx: &mut Context<AppModel>) -> gpui::Stateful<Div> {
    let m = model.metrics;
    let theme = &model.theme;
    window_mover("onboarding-header", cx)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .h(m.scaled(48.0))
        .pl(m.spacing9())
        .pr(m.spacing7())
        .child(
            div()
                .text_size(m.font_footnote())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.accent)
                .child(accessibility::label(
                    "onboarding-progress",
                    tr!("%lld of %lld", step.index() + 1, Step::ALL.len()),
                )),
        )
        .when(step.next().is_some(), |header| {
            header.child(
                div()
                    .id("onboarding-skip")
                    .debug_selector(|| "onboarding-skip".into())
                    .accessibility_id("onboarding-skip")
                    .role(gpui::Role::Button)
                    .aria_label(tr!("Skip Tour"))
                    .px(m.spacing4())
                    .py(m.spacing2())
                    .rounded(m.radius_sm())
                    .cursor_pointer()
                    .text_size(m.font_footnote())
                    .text_color(theme.fg_muted)
                    .hover(|style| style.bg(theme.hover).text_color(theme.fg))
                    .focus(|style| style.bg(theme.hover).text_color(theme.fg))
                    .on_mouse_down(gpui::MouseButton::Left, |_, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .child(tr!("Skip Tour"))
                    .button_interaction(cx.listener(|model, _, _, cx| model.finish_onboarding(cx))),
            )
        })
}

fn footer(step: Step, model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let style = Style {
        theme: &model.theme,
        metrics: &m,
    };
    let next_label = match step {
        Step::Welcome => tr!("Start Tour"),
        Step::Shortcuts => tr!("Get Started"),
        _ => tr!("Continue"),
    };
    let next = if install_is_the_next_move(step, model) {
        controls::Button::secondary(&next_label)
    } else {
        controls::Button::primary(&next_label)
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap(m.spacing4())
        .px(m.spacing9())
        .py(m.spacing7())
        .child(dots(step, model, cx))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(m.spacing3())
                .when_some(step.previous(), |row, _| {
                    row.child(div().debug_selector(|| "onboarding-back".into()).child(
                        controls::button(
                            style,
                            "onboarding-back",
                            controls::Button::secondary(&tr!("Back")),
                            cx.listener(|model, _, _, cx| model.step_onboarding(false, cx)),
                        ),
                    ))
                })
                .child(
                    div()
                        .debug_selector(|| "onboarding-next".into())
                        .child(controls::button(
                            style,
                            "onboarding-next",
                            next,
                            cx.listener(|model, _, _, cx| model.step_onboarding(true, cx)),
                        )),
                ),
        )
}

/// While the install button still has work to offer, it is the step's one
/// primary button.
fn install_is_the_next_move(step: Step, model: &AppModel) -> bool {
    step == Step::Extensions
        && model.offers_to_install() > 0
        && !model
            .onboarding
            .offers
            .iter()
            .any(|offer| matches!(offer.state, OfferState::Failed(_)))
}

/// One dot per step; each jumps to its step.
fn dots(step: Step, model: &AppModel, cx: &mut Context<AppModel>) -> Div {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing1())
        .children(Step::ALL.into_iter().map(|target| {
            let current = target == step;
            div()
                .id(("onboarding-dot", target.index()))
                .debug_selector(move || format!("onboarding-dot-{}", target.index()))
                .accessibility_id(format!("onboarding-dot-{}", target.index()))
                .role(gpui::Role::Button)
                .aria_label(tr!(
                    "Go to step %lld of %lld",
                    target.index() + 1,
                    Step::ALL.len()
                ))
                .flex()
                .items_center()
                .justify_center()
                .size(m.scaled(14.0))
                .rounded_full()
                .cursor_pointer()
                .focus(|style| style.bg(theme.hover))
                .button_interaction(cx.listener(move |model, _, _, cx| {
                    model.show_onboarding_step(target, cx);
                }))
                .child(
                    div()
                        .size(m.scaled(if current { 8.0 } else { 6.0 }))
                        .rounded_full()
                        .bg(if current { theme.accent } else { theme.fg_dim }),
                )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn a_roomy_window_shows_the_card_at_full_size_with_art() {
        let layout = Layout::of(size(px(1400.0), px(900.0)), Metrics::new(1.0));
        assert_eq!(layout.card, size(px(CARD_WIDTH), px(CARD_HEIGHT)));
        assert!(layout.art.is_some());
    }

    #[test]
    fn a_small_window_shrinks_the_card_and_drops_the_art() {
        let layout = Layout::of(size(px(640.0), px(400.0)), Metrics::new(1.0));
        assert_eq!(layout.card, size(px(608.0), px(368.0)));
        assert_eq!(layout.art, None);
    }

    #[test]
    fn a_larger_interface_size_scales_the_card_until_the_window_stops_it() {
        let layout = Layout::of(size(px(1000.0), px(1000.0)), Metrics::new(1.5));
        assert_eq!(layout.card, size(px(968.0), px(CARD_HEIGHT * 1.5)));
        assert_eq!(layout.art, None);
    }
}
