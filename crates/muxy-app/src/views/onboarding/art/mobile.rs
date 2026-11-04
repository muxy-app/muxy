//! A phone answers an agent's question. Beside it sits the real pairing code,
//! once there is one, and then the phone that used it.

use gpui::{Div, FontWeight, ParentElement, Styled, div};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::kit::{
    AMBER, CYAN, Curve, Frame, GREEN, INK, Mark, RAISED, TEXT, VIOLET, card, cursor, dim, mono,
    muted, packet, paint_marks, place, solid, text, tint,
};
use super::timeline::{cycle, ease_in_out, progress, rise, span, typed};
use crate::views::devices;

pub(super) const STILL: f32 = 2.4;

const PERIOD: f32 = 10.0;
const PHONE_X: f32 = 40.0;
const PHONE_Y: f32 = 106.0;
const PHONE_WIDTH: f32 = 176.0;
const PHONE_HEIGHT: f32 = 356.0;
const SLOT_X: f32 = 252.0;
const SLOT_Y: f32 = 182.0;
const SLOT: f32 = 204.0;

pub(super) fn scene(frame: &Frame) -> Div {
    let access = frame.model.device_access();
    let pairing = access.pairing.as_ref();
    let paired = frame
        .model
        .onboarding
        .tour
        .as_ref()
        .and_then(|tour| tour.pair.paired.clone());
    let link = Curve {
        from: (SLOT_X, SLOT_Y + SLOT / 2.0),
        a: (SLOT_X - 22.0, SLOT_Y + SLOT / 2.0),
        b: (PHONE_X + PHONE_WIDTH + 22.0, PHONE_Y + PHONE_HEIGHT / 2.0),
        to: (PHONE_X + PHONE_WIDTH, PHONE_Y + PHONE_HEIGHT / 2.0),
    };
    let live = pairing.is_some() || paired.is_some();
    let hex = if paired.is_some() { GREEN } else { VIOLET };
    let mut marks = vec![Mark::Line {
        points: link.points(1.0),
        width: 1.4,
        color: tint(hex, if live { 0.5 } else { 0.18 }),
        dash: (!live).then_some(3.0),
    }];
    if live {
        for offset in [0.0, 0.5] {
            packet(
                &mut marks,
                link,
                cycle(frame.t * 0.5 + offset, 1.0),
                2.3,
                hex,
                1.0,
            );
        }
    }
    let slot = if let Some(name) = &paired {
        paired_card(frame, name)
    } else if let Some(code) = pairing.and_then(devices::Pairing::qr) {
        code_card(frame, code)
    } else {
        waiting_card(frame)
    };
    div()
        .size_full()
        .relative()
        .child(paint_marks(frame, marks))
        .child(phone(frame))
        .child(slot)
}

fn phone(frame: &Frame) -> Div {
    let inset = 7.0;
    let rise_in = rise(frame.t, 0.1, 0.7);
    place(
        frame,
        PHONE_X,
        PHONE_Y + (1.0 - rise_in) * 20.0,
        PHONE_WIDTH,
        PHONE_HEIGHT,
    )
    .opacity(rise_in)
    .rounded(frame.px(30.0))
    .bg(solid(0x05_05_08))
    .border_2()
    .border_color(tint(0xff_ff_ff, 0.2))
    .child(
        place(
            frame,
            inset,
            inset,
            PHONE_WIDTH - inset * 2.0 - 4.0,
            PHONE_HEIGHT - inset * 2.0 - 4.0,
        )
        .rounded(frame.px(24.0))
        .bg(solid(INK))
        .overflow_hidden()
        .child(screen(frame)),
    )
    .child(
        place(frame, PHONE_WIDTH / 2.0 - 26.0, 11.0, 48.0, 13.0)
            .rounded_full()
            .bg(solid(0x05_05_08)),
    )
}

/// The app itself: an agent asks, the phone answers, and it gets back to work.
fn screen(frame: &Frame) -> Div {
    let local = cycle(frame.t, PERIOD);
    let tapped = rise(local, 3.2, 0.2);
    let answered = local >= 3.4;
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(frame.px(9.0))
        .pt(frame.px(30.0))
        .px(frame.px(10.0))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    text(frame, 11.5, tint(TEXT, 0.97), "muxy").font_weight(FontWeight::SEMIBOLD),
                )
                .child(div().size(frame.px(6.0)).rounded_full().bg(solid(GREEN))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(frame.px(7.0))
                .p(frame.px(9.0))
                .rounded(frame.px(10.0))
                .bg(solid(RAISED))
                .border_1()
                .border_color(if answered {
                    tint(CYAN, 0.35)
                } else {
                    tint(AMBER, 0.45)
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(frame.px(5.0))
                        .child(gpui::img("icons/provider-claude.svg").size(frame.px(11.0)))
                        .child(text(
                            frame,
                            9.5,
                            if answered { solid(CYAN) } else { solid(AMBER) },
                            if answered {
                                tr!("Working")
                            } else {
                                tr!("Waiting for you")
                            },
                        )),
                )
                .child(
                    div()
                        .text_size(frame.px(10.0))
                        .line_height(frame.px(14.0))
                        .text_color(tint(TEXT, 0.9))
                        .child(tr!("Run the test suite?")),
                )
                .child(answer_button(frame, tapped, answered)),
        )
        .child(output(frame, local))
}

fn answer_button(frame: &Frame, tapped: f32, answered: bool) -> Div {
    let flash = span(tapped, 0.0, 1.0, 0.5);
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(frame.px(24.0))
        .rounded(frame.px(6.0))
        .bg(if answered {
            tint(CYAN, 0.16)
        } else {
            tint(VIOLET, 0.75 + flash * 0.25)
        })
        .child(text(
            frame,
            10.0,
            solid(0xff_ff_ff),
            if answered {
                tr!("Allowed")
            } else {
                tr!("Allow")
            },
        ))
}

fn output(frame: &Frame, local: f32) -> Div {
    let run = "❯ npm test";
    let finished = local > 6.6;
    div()
        .flex()
        .flex_col()
        .gap(frame.px(4.0))
        .pt(frame.px(4.0))
        .opacity(progress(local, 3.6, 0.3) * (1.0 - progress(local, PERIOD - 0.5, 0.4)))
        .child(mono(
            frame,
            9.0,
            tint(TEXT, 0.82),
            typed(run, local, 3.8, 18.0).to_owned(),
        ))
        .children((local > 4.8).then(|| mono(frame, 9.0, dim(), "running 128 tests")))
        .children((local > 5.8).then(|| mono(frame, 9.0, dim(), "checkout ✓")))
        .children(finished.then(|| mono(frame, 9.0, solid(GREEN), "128 passed")))
        .children(finished.then(|| cursor(frame, 9.0, muted())))
}

fn slot_label(frame: &Frame, content: gpui::SharedString) -> Div {
    place(
        frame,
        SLOT_X - 20.0,
        SLOT_Y + SLOT + 12.0,
        SLOT + 40.0,
        18.0,
    )
    .flex()
    .justify_center()
    .child(text(frame, 10.5, muted(), content))
}

/// Where the code will appear, with a beam sweeping it while it waits.
fn waiting_card(frame: &Frame) -> Div {
    let sweep = ease_in_out(cycle(frame.t, 2.8) / 2.8);
    let corners = |x: f32, y: f32| {
        let arm = 22.0;
        let horizontal = if x > SLOT_X { -arm } else { arm };
        let vertical = if y > SLOT_Y { -arm } else { arm };
        Mark::Line {
            points: vec![(x + horizontal, y), (x, y), (x, y + vertical)],
            width: 2.0,
            color: tint(VIOLET, 0.8),
            dash: None,
        }
    };
    div()
        .absolute()
        .size_full()
        .child(paint_marks(
            frame,
            vec![
                corners(SLOT_X, SLOT_Y),
                corners(SLOT_X + SLOT, SLOT_Y),
                corners(SLOT_X, SLOT_Y + SLOT),
                corners(SLOT_X + SLOT, SLOT_Y + SLOT),
            ],
        ))
        .child(
            place(
                frame,
                SLOT_X + 10.0,
                SLOT_Y + 10.0 + sweep * (SLOT - 22.0),
                SLOT - 20.0,
                2.0,
            )
            .rounded_full()
            .bg(tint(CYAN, 0.8))
            .shadow(vec![gpui::BoxShadow {
                color: tint(CYAN, 0.55),
                offset: gpui::point(gpui::px(0.0), gpui::px(0.0)),
                blur_radius: frame.px(14.0),
                spread_radius: gpui::px(0.0),
            }]),
        )
        .child(
            place(frame, SLOT_X, SLOT_Y, SLOT, SLOT)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(frame.px(8.0))
                .child(IconGlyph::new(
                    Icon::Grid,
                    frame.px(30.0),
                    tint(0xff_ff_ff, 0.3),
                ))
                .child(text(frame, 10.5, dim(), tr!("Your code appears here"))),
        )
        .child(slot_label(frame, tr!("No cloud, no account")))
}

fn code_card(frame: &Frame, code: std::sync::Arc<muxy_app_core::qr::QrCode>) -> Div {
    let side = devices::CODE_SIZE;
    let inset = (SLOT - side) / 2.0;
    div()
        .absolute()
        .size_full()
        .child(
            place(frame, SLOT_X + inset, SLOT_Y + inset, side, side)
                .rounded(frame.px(8.0))
                .overflow_hidden()
                .child(devices::qr(code).size_full()),
        )
        .child(slot_label(frame, tr!("Scan with the Muxy app")))
}

fn paired_card(frame: &Frame, name: &str) -> Div {
    card(frame, SLOT_X, SLOT_Y, SLOT, SLOT)
        .border_color(tint(GREEN, 0.5))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(frame.px(10.0))
        .child(IconGlyph::new(
            Icon::CircleCheck,
            frame.px(44.0),
            solid(GREEN),
        ))
        .child(text(frame, 13.0, tint(TEXT, 0.97), tr!("Paired")).font_weight(FontWeight::SEMIBOLD))
        .child(
            div()
                .max_w(frame.px(SLOT - 24.0))
                .truncate()
                .text_size(frame.px(11.0))
                .text_color(muted())
                .child(name.to_owned()),
        )
}
