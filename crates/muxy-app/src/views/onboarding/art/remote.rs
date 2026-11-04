//! A secure line reaches out to two servers and their projects join the sidebar.
//! One connection drops and comes back with its terminals still running.

use gpui::prelude::FluentBuilder;
use gpui::{Div, FontWeight, ParentElement, SharedString, Styled, div};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::kit::{
    AMBER, CYAN, Curve, Frame, GREEN, INDIGO, Mark, ORCHID, PINK, RAISED, TEXT, VIOLET, caption,
    card, dim, letter_icon, line, muted, packet, paint_marks, solid, text, tint, units,
};
use super::timeline::{cycle, lerp, progress, rise, span};

pub(super) const STILL: f32 = 4.2;

const PERIOD: f32 = 12.0;
const LAPTOP: (f32, f32) = (40.0, 108.0);
const LAPTOP_SIZE: (f32, f32) = (132.0, 92.0);
const SERVER_X: f32 = 322.0;
const SERVER_SIZE: (f32, f32) = (130.0, 64.0);
const DEVBOX_Y: f32 = 68.0;
const PROD_Y: f32 = 172.0;
const LIST_X: f32 = 72.0;
const LIST_Y: f32 = 276.0;
const LIST_WIDTH: f32 = 348.0;
const ROW: f32 = 46.0;

pub(super) fn scene(frame: &Frame) -> Div {
    let t = frame.t;
    let local = cycle(t, PERIOD);
    let reach = rise(t, 0.3, 0.9);
    let offline = span(local, 6.4, 9.2, 0.35);
    let link = |server_y: f32| {
        let from = (LAPTOP.0 + LAPTOP_SIZE.0, LAPTOP.1 + LAPTOP_SIZE.1 / 2.0);
        let to = (SERVER_X, server_y + SERVER_SIZE.1 / 2.0);
        Curve {
            from,
            a: (lerp(from.0, to.0, 0.55), from.1),
            b: (lerp(from.0, to.0, 0.45), to.1),
            to,
        }
    };
    let devbox = link(DEVBOX_Y);
    let prod = link(PROD_Y);
    let mut marks = vec![
        Mark::Line {
            points: devbox.points(reach),
            width: 1.5,
            color: tint(CYAN, 0.55),
            dash: None,
        },
        Mark::Line {
            points: prod.points(reach),
            width: 1.5,
            color: if offline > 0.5 {
                tint(AMBER, 0.55)
            } else {
                tint(VIOLET, 0.55)
            },
            dash: (offline > 0.5).then_some(4.0),
        },
    ];
    if reach >= 1.0 {
        for offset in [0.0, 0.33, 0.66] {
            packet(
                &mut marks,
                devbox,
                cycle(t * 0.32 + offset, 1.0),
                2.3,
                CYAN,
                1.0,
            );
            packet(
                &mut marks,
                prod,
                cycle(t * 0.27 + offset, 1.0),
                2.3,
                VIOLET,
                1.0 - offline,
            );
        }
    }
    div()
        .size_full()
        .relative()
        .child(paint_marks(frame, marks))
        .child(laptop(frame))
        .child(server(frame, DEVBOX_Y, "devbox", reach, 0.0))
        .child(server(frame, PROD_Y, "prod-1", reach, offline))
        .child(lock(frame, devbox.at(0.5), reach))
        .child(lock(frame, prod.at(0.5), reach))
        .child(list(frame, offline))
        .child(caption(
            frame,
            470.0,
            1.0 - offline,
            muted(),
            tr!("Remote projects sit beside your local ones."),
        ))
        .child(caption(
            frame,
            470.0,
            offline,
            solid(AMBER),
            tr!("Connection lost. Its terminals keep running."),
        ))
}

fn laptop(frame: &Frame) -> Div {
    card(frame, LAPTOP.0, LAPTOP.1, LAPTOP_SIZE.0, LAPTOP_SIZE.1)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(frame.px(7.0))
        .child(IconGlyph::new(Icon::AppWindow, frame.px(22.0), solid(CYAN)))
        .child(text(frame, 11.0, tint(TEXT, 0.95), tr!("This Computer")))
}

fn server(frame: &Frame, y: f32, name: &str, reach: f32, offline: f32) -> Div {
    let (label, color) = if offline > 0.5 {
        (tr!("Reconnecting…"), solid(AMBER))
    } else {
        (tr!("Connected"), solid(GREEN))
    };
    card(frame, SERVER_X, y, SERVER_SIZE.0, SERVER_SIZE.1)
        .opacity(progress(reach, 0.6, 0.4))
        .flex()
        .items_center()
        .gap(frame.px(10.0))
        .px(frame.px(11.0))
        .child(IconGlyph::new(Icon::Network, frame.px(18.0), solid(VIOLET)))
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    text(frame, 11.0, tint(TEXT, 0.96), name.to_owned())
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(text(frame, 9.0, color, label)),
        )
}

fn lock(frame: &Frame, at: (f32, f32), reach: f32) -> Div {
    div()
        .absolute()
        .left(frame.px(at.0 - 15.0))
        .top(frame.px(at.1 - 8.0))
        .h(frame.px(16.0))
        .flex()
        .items_center()
        .px(frame.px(5.0))
        .rounded_full()
        .bg(solid(RAISED))
        .border_1()
        .border_color(line())
        .opacity(progress(reach, 0.8, 0.2))
        .child(text(frame, 8.0, muted(), "SSH"))
}

/// The sidebar they all land in.
fn list(frame: &Frame, offline: f32) -> Div {
    let rows: [(&str, &str, u32, Option<SharedString>); 3] = [
        ("muxy", "M", INDIGO, None),
        ("api", "A", ORCHID, Some("devbox".into())),
        ("infra", "I", PINK, Some("prod-1".into())),
    ];
    card(frame, LIST_X, LIST_Y, LIST_WIDTH, ROW * 3.0 + 34.0)
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(frame.px(34.0))
                .px(frame.px(12.0))
                .border_b_1()
                .border_color(line())
                .child(text(frame, 10.0, dim(), tr!("All Projects"))),
        )
        .children(
            rows.into_iter()
                .enumerate()
                .map(|(index, (name, letter, hex, server))| {
                    let shown = rise(frame.t, 1.2 + units(index) * 0.22, 0.5);
                    let dropped = index == 2 && offline > 0.5;
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(frame.px(10.0))
                        .h(frame.px(ROW))
                        .pl(frame.px(12.0 + (1.0 - shown) * 16.0))
                        .pr(frame.px(12.0))
                        .opacity(shown * if dropped { 0.55 } else { 1.0 })
                        .child(letter_icon(frame, 22.0, solid(hex), letter))
                        .child(div().flex_1().child(text(
                            frame,
                            11.5,
                            tint(TEXT, 0.96),
                            name.to_owned(),
                        )))
                        .when_some(server, |row, server| {
                            row.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(frame.px(5.0))
                                    .px(frame.px(8.0))
                                    .h(frame.px(20.0))
                                    .rounded_full()
                                    .bg(tint(0xff_ff_ff, 0.07))
                                    .child(
                                        div().size(frame.px(5.0)).rounded_full().bg(if dropped {
                                            solid(AMBER)
                                        } else {
                                            solid(GREEN)
                                        }),
                                    )
                                    .child(text(frame, 9.5, muted(), server)),
                            )
                        })
                        .when(index == 0, |row| {
                            row.child(text(frame, 9.5, dim(), tr!("This Computer")))
                        })
                }),
        )
}
