//! Keys press in turn, and a little window does what they say: splits right,
//! splits down, opens the palette, and hides the sidebar. The keys shown are
//! the user's own.

use gpui::prelude::FluentBuilder;
use gpui::{Div, FontWeight, ParentElement, SharedString, Styled, div};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::tr;

use super::key_caps;
use super::kit::{
    CYAN, Frame, GREEN, H, RAISED, TEXT, VIOLET, W, app_window, card, cursor, dim, keycap, line,
    mono, muted, place, solid, text, tint,
};
use super::timeline::{cycle, ease_in_out, lerp, progress, span};

pub(super) const STILL: f32 = 6.4;

const PERIOD: f32 = 12.0;
const WINDOW_X: f32 = 34.0;
const WINDOW_Y: f32 = 76.0;
const WINDOW_WIDTH: f32 = 424.0;
const WINDOW_HEIGHT: f32 = 300.0;
const SIDEBAR: f32 = 96.0;
const BAR: f32 = 26.0;

struct Beat {
    start: f32,
    end: f32,
    press: f32,
    shortcut: ShortcutId,
}

const BEATS: [Beat; 4] = [
    Beat {
        start: 0.3,
        end: 3.0,
        press: 0.9,
        shortcut: ShortcutId::SplitRight,
    },
    Beat {
        start: 3.0,
        end: 5.7,
        press: 3.6,
        shortcut: ShortcutId::SplitDown,
    },
    Beat {
        start: 5.7,
        end: 8.4,
        press: 6.3,
        shortcut: ShortcutId::ToggleCommandPalette,
    },
    Beat {
        start: 8.4,
        end: 11.1,
        press: 9.0,
        shortcut: ShortcutId::ToggleSidebar,
    },
];

pub(super) fn scene(frame: &Frame) -> Div {
    let local = cycle(frame.t, PERIOD);
    let done = 1.0 - progress(local, PERIOD - 0.7, 0.6);
    let arrive = |at: f32| ease_in_out(progress(local, at + 0.08, 0.32));
    let split_right = arrive(BEATS[0].press);
    let split_down = arrive(BEATS[1].press);
    let palette = span(local, BEATS[2].press + 0.08, BEATS[2].end + 0.2, 0.2);
    let hidden = arrive(BEATS[3].press);
    let labels = [
        tr!("Split right"),
        tr!("Split down"),
        tr!("Command palette"),
        tr!("Show or hide the sidebar"),
    ];
    div()
        .size_full()
        .relative()
        .child(
            app_window(frame, WINDOW_X, WINDOW_Y, WINDOW_WIDTH, WINDOW_HEIGHT)
                .child(panes(frame, split_right, split_down, hidden, done))
                .child(sidebar(frame, hidden, done))
                .child(palette_card(frame, palette)),
        )
        .children(
            BEATS
                .iter()
                .zip(labels)
                .map(|(beat, label)| keys(frame, beat, label, local)),
        )
}

fn sidebar(frame: &Frame, hidden: f32, done: f32) -> Div {
    let width = SIDEBAR * (1.0 - hidden * done);
    let names: [SharedString; 4] = [tr!("Home"), "web-app".into(), "api".into(), "docs".into()];
    div()
        .absolute()
        .top_0()
        .left_0()
        .bottom_0()
        .w(frame.px(width))
        .overflow_hidden()
        .bg(tint(RAISED, 0.7))
        .border_r_1()
        .border_color(line())
        .child(
            place(frame, 0.0, 30.0, SIDEBAR, 120.0)
                .flex()
                .flex_col()
                .gap(frame.px(3.0))
                .px(frame.px(6.0))
                .children(names.iter().enumerate().map(|(index, name)| {
                    div()
                        .flex()
                        .items_center()
                        .h(frame.px(20.0))
                        .px(frame.px(6.0))
                        .rounded(frame.px(4.0))
                        .when(index == 1, |row| row.bg(tint(0xff_ff_ff, 0.08)))
                        .child(text(frame, 9.5, tint(TEXT, 0.82), name.clone()))
                })),
        )
}

/// The panes, sized by how far each split has come.
fn panes(frame: &Frame, right: f32, down: f32, hidden: f32, done: f32) -> Div {
    let left = SIDEBAR * (1.0 - hidden * done);
    let area = WINDOW_WIDTH - left;
    let height = WINDOW_HEIGHT - BAR;
    let right_width = area * 0.5 * right * done;
    let main_width = area - right_width;
    let lower_height = height * 0.5 * down * done;
    let upper_height = height - lower_height;
    div()
        .absolute()
        .top(frame.px(BAR))
        .left(frame.px(left))
        .w(frame.px(area))
        .h(frame.px(height))
        .border_t_1()
        .border_color(line())
        .child(pane(frame, 0.0, 0.0, main_width, height, "❯ nvim", true))
        .when(right_width > 1.0, |panes| {
            panes.child(pane(
                frame,
                main_width,
                0.0,
                right_width,
                upper_height,
                "❯ npm run dev",
                false,
            ))
        })
        .when(lower_height > 1.0 && right_width > 1.0, |panes| {
            panes.child(pane(
                frame,
                main_width,
                upper_height,
                right_width,
                lower_height,
                "❯ claude",
                false,
            ))
        })
}

fn pane(
    frame: &Frame,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    command: &'static str,
    focused: bool,
) -> Div {
    place(frame, x, y, width, height)
        .overflow_hidden()
        .border_l_1()
        .border_t_1()
        .border_color(line())
        .p(frame.px(9.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(frame.px(4.0))
                .child(mono(frame, 9.5, tint(CYAN, 0.92), command))
                .when(focused, |row| row.child(cursor(frame, 9.5, muted()))),
        )
}

fn palette_card(frame: &Frame, shown: f32) -> Div {
    let rows = [
        tr!("New Tab"),
        tr!("Split Right"),
        tr!("Toggle Composer"),
        tr!("Settings…"),
    ];
    card(
        frame,
        WINDOW_WIDTH / 2.0 - 124.0,
        30.0 + (1.0 - shown) * -8.0,
        248.0,
        150.0,
    )
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
            .h(frame.px(32.0))
            .px(frame.px(10.0))
            .border_b_1()
            .border_color(line())
            .child(text(frame, 10.0, dim(), tr!("Search commands…"))),
    )
    .children(rows.into_iter().enumerate().map(|(index, row)| {
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(frame.px(28.0))
            .px(frame.px(10.0))
            .when(index == 0, |line| line.bg(tint(VIOLET, 0.18)))
            .child(text(frame, 10.0, tint(TEXT, 0.92), row))
    }))
}

/// One beat's keys, pressing as its moment comes, with what they do beneath.
fn keys(frame: &Frame, beat: &Beat, label: SharedString, local: f32) -> Div {
    let shown = span(local, beat.start, beat.end, 0.25);
    let pressed = span(local, beat.press - 0.02, beat.press + 0.46, 0.08);
    let caps = key_caps(beat.shortcut, frame.model);
    if caps.is_empty() {
        return div();
    }
    let result = progress(local, beat.press + 0.1, 0.25);
    let y = WINDOW_Y + WINDOW_HEIGHT + 26.0 + lerp(10.0, 0.0, shown);
    place(frame, 0.0, y, W, H - y)
        .opacity(shown)
        .flex()
        .flex_col()
        .items_center()
        .gap(frame.px(14.0))
        .child(
            div()
                .flex()
                .items_center()
                .h(frame.px(58.0))
                .gap(frame.px(8.0))
                .children(
                    caps.into_iter()
                        .map(|cap| keycap(frame, 52.0, pressed, cap)),
                ),
        )
        .child(
            div()
                .opacity(result)
                .flex()
                .items_center()
                .gap(frame.px(6.0))
                .child(div().size(frame.px(6.0)).rounded_full().bg(solid(GREEN)))
                .child(text(frame, 12.0, tint(TEXT, 0.92), label).font_weight(FontWeight::MEDIUM)),
        )
}
