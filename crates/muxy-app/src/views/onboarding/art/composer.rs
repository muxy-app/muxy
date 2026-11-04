//! The composer opens over two terminals, a long prompt gets written with an
//! image attached, and it goes to both splits at once. The keys shown are the
//! user's own.

use gpui::prelude::FluentBuilder;
use gpui::{Div, FontWeight, ParentElement, Styled, div};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::kit::{
    CYAN, Frame, INDIGO, RAISED, TEXT, VIOLET, app_window, card, cursor, dim, line, mono, muted,
    place, solid, text, tint,
};
use super::shortcuts::{Beat, keys};
use super::timeline::{cycle, ease_in_out, progress, rise, typed};

pub(super) const STILL: f32 = 5.4;

const PERIOD: f32 = 12.0;
const WINDOW_X: f32 = 28.0;
const WINDOW_Y: f32 = 44.0;
const WINDOW_WIDTH: f32 = 436.0;
const WINDOW_HEIGHT: f32 = 372.0;
const BAR: f32 = 26.0;
const COMPOSER_X: f32 = 24.0;
const COMPOSER_WIDTH: f32 = 388.0;
const COMPOSER_HEIGHT: f32 = 168.0;
const LINE_ONE: &str = "Fix the failing checkout test.";
const LINE_TWO: &str = "Keep the API the same and add a regression test.";
const TYPING_RATE: f32 = 30.0;
const OPEN_AT: f32 = 0.9;
const TYPE_AT: f32 = 1.3;
const SECOND_LINE_AT: f32 = 2.5;
const ATTACH_AT: f32 = 4.3;
const ALL_SPLITS_AT: f32 = 5.0;
const SEND_AT: f32 = 6.5;

const BEATS: [Beat; 2] = [
    Beat {
        start: 0.2,
        end: 3.4,
        press: OPEN_AT - 0.1,
        shortcut: ShortcutId::ToggleComposer,
    },
    Beat {
        start: 5.7,
        end: 8.6,
        press: SEND_AT,
        shortcut: ShortcutId::ComposerSubmit,
    },
];

pub(super) fn scene(frame: &Frame) -> Div {
    let local = cycle(frame.t, PERIOD);
    let opened = rise(local, OPEN_AT, 0.35);
    let sent = ease_in_out(progress(local, SEND_AT + 0.15, 0.3));
    let shown = opened * (1.0 - sent);
    let keys_y = WINDOW_Y + WINDOW_HEIGHT + 16.0;
    div()
        .size_full()
        .relative()
        .child(
            app_window(frame, WINDOW_X, WINDOW_Y, WINDOW_WIDTH, WINDOW_HEIGHT)
                .child(tab_bar(frame))
                .child(pane(frame, 0.0, "claude", local))
                .child(pane(frame, WINDOW_WIDTH / 2.0, "codex", local))
                .child(composer(frame, shown, local)),
        )
        .child(keys(
            frame,
            &BEATS[0],
            tr!("Open the composer"),
            local,
            keys_y,
        ))
        .child(keys(frame, &BEATS[1], tr!("Send it"), local, keys_y))
}

fn tab_bar(frame: &Frame) -> Div {
    place(frame, 46.0, 0.0, WINDOW_WIDTH - 46.0, BAR)
        .flex()
        .items_center()
        .gap(frame.px(4.0))
        .child(
            div()
                .flex()
                .items_center()
                .h(frame.px(18.0))
                .px(frame.px(8.0))
                .rounded(frame.px(5.0))
                .bg(tint(0xff_ff_ff, 0.08))
                .child(text(frame, 9.5, tint(TEXT, 0.92), "web-app")),
        )
}

/// A terminal that gets the prompt once it's sent, like every split beside it.
fn pane(frame: &Frame, x: f32, program: &'static str, local: f32) -> Div {
    let arrived = progress(local, SEND_AT + 0.35, 0.2);
    let ending = 1.0 - progress(local, PERIOD - 0.6, 0.45);
    place(frame, x, BAR, WINDOW_WIDTH / 2.0, WINDOW_HEIGHT - BAR)
        .overflow_hidden()
        .border_t_1()
        .when(x > 0.0, Styled::border_l_1)
        .border_color(line())
        .p(frame.px(10.0))
        .flex()
        .flex_col()
        .gap(frame.px(4.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(frame.px(5.0))
                .child(mono(frame, 9.5, tint(CYAN, 0.92), "❯"))
                .child(mono(frame, 9.5, tint(TEXT, 0.9), program)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(frame.px(4.0))
                .opacity(arrived * ending)
                .child(mono(frame, 9.5, tint(TEXT, 0.9), LINE_ONE))
                .child(mono(
                    frame,
                    9.5,
                    tint(TEXT, 0.9),
                    "Keep the API the same and",
                ))
                .child(mono(frame, 9.5, tint(TEXT, 0.9), "add a regression test."))
                .child(mono(frame, 9.5, muted(), "[Image 1]"))
                .child(
                    mono(frame, 9.5, tint(VIOLET, 0.95), "✻ Working").opacity(progress(
                        local,
                        SEND_AT + 1.0,
                        0.2,
                    )),
                ),
        )
}

fn composer(frame: &Frame, shown: f32, local: f32) -> Div {
    let y = WINDOW_HEIGHT - 14.0 - COMPOSER_HEIGHT + (1.0 - shown) * 24.0;
    let first = typed(LINE_ONE, local, TYPE_AT, TYPING_RATE);
    let second = typed(LINE_TWO, local, SECOND_LINE_AT, TYPING_RATE);
    let writing = local < ATTACH_AT;
    card(frame, COMPOSER_X, y, COMPOSER_WIDTH, COMPOSER_HEIGHT)
        .bg(solid(RAISED))
        .opacity(shown)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(frame.px(30.0))
                .px(frame.px(12.0))
                .border_b_1()
                .border_color(line())
                .child(
                    text(frame, 10.5, tint(TEXT, 0.95), tr!("Composer"))
                        .font_weight(FontWeight::SEMIBOLD),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(frame.px(5.0))
                .p(frame.px(12.0))
                .child(mono(frame, 10.5, tint(TEXT, 0.94), first.to_owned()))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(frame.px(3.0))
                        .child(mono(frame, 10.5, tint(TEXT, 0.94), second.to_owned()))
                        .when(writing && local >= TYPE_AT, |row| {
                            row.child(cursor(frame, 10.5, muted()))
                        }),
                )
                .child(attachment(frame, local)),
        )
        .child(footer(frame, local))
}

/// The attached image, shown as the token it is sent as.
fn attachment(frame: &Frame, local: f32) -> Div {
    let attached = rise(local, ATTACH_AT, 0.25);
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(frame.px(5.0))
        .opacity(attached)
        .child(IconGlyph::new(Icon::File, frame.px(11.0), solid(VIOLET)))
        .child(mono(frame, 10.0, tint(VIOLET, 0.95), "[Image 1]"))
}

fn footer(frame: &Frame, local: f32) -> Div {
    let all_splits = local >= ALL_SPLITS_AT;
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .h(frame.px(36.0))
        .px(frame.px(12.0))
        .border_t_1()
        .border_color(line())
        .child(
            div()
                .flex()
                .items_center()
                .gap(frame.px(6.0))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(frame.px(13.0))
                        .rounded(frame.px(3.0))
                        .border_1()
                        .border_color(if all_splits { solid(INDIGO) } else { dim() })
                        .when(all_splits, |tick| tick.bg(solid(INDIGO)))
                        .when(all_splits, |tick| {
                            tick.child(IconGlyph::new(
                                Icon::Check,
                                frame.px(10.0),
                                solid(0xff_ff_ff),
                            ))
                        }),
                )
                .child(text(
                    frame,
                    10.0,
                    if all_splits {
                        tint(TEXT, 0.92)
                    } else {
                        muted()
                    },
                    tr!("Send to all splits"),
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .h(frame.px(22.0))
                .px(frame.px(12.0))
                .rounded(frame.px(5.0))
                .bg(solid(INDIGO))
                .child(text(frame, 10.0, solid(0xff_ff_ff), tr!("Send"))),
        )
}
