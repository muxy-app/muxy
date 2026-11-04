//! Every client talks to one server. The desktop app closes partway through,
//! and the terminals on the server keep going regardless.

use gpui::{Div, FontWeight, ParentElement, Styled, div};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::kit::{
    AMBER, CYAN, Curve, Frame, GREEN, INK, Mark, ORCHID, PINK, TEXT, VIOLET, caption, card, dim,
    line, mono, muted, packet, paint_marks, place, solid, text, tint, units,
};
use super::timeline::{cycle, lerp, progress, span, wave};

pub(super) const STILL: f32 = 5.5;

const PERIOD: f32 = 11.0;
const HUB_X: f32 = 171.0;
const HUB_Y: f32 = 180.0;
const HUB_WIDTH: f32 = 150.0;
const HUB_HEIGHT: f32 = 128.0;
const CLIENT_WIDTH: f32 = 112.0;
const CLIENT_HEIGHT: f32 = 66.0;

struct Client {
    x: f32,
    y: f32,
    icon: Icon,
    hex: u32,
    curve: Curve,
}

fn clients() -> [Client; 4] {
    let left = 26.0;
    let right = 354.0;
    let top = 58.0;
    let bottom = 366.0;
    let hub_left = HUB_X;
    let hub_right = HUB_X + HUB_WIDTH;
    let client_left_edge = left + CLIENT_WIDTH;
    let top_middle = top + CLIENT_HEIGHT / 2.0;
    let bottom_middle = bottom + CLIENT_HEIGHT / 2.0;
    let hub_upper = HUB_Y + 34.0;
    let hub_lower = HUB_Y + HUB_HEIGHT - 34.0;
    let link = |from: (f32, f32), to: (f32, f32)| Curve {
        from,
        a: (lerp(from.0, to.0, 0.55), from.1),
        b: (lerp(from.0, to.0, 0.45), to.1),
        to,
    };
    [
        Client {
            x: left,
            y: top,
            icon: Icon::AppWindow,
            hex: CYAN,
            curve: link((client_left_edge, top_middle), (hub_left, hub_upper)),
        },
        Client {
            x: right,
            y: top,
            icon: Icon::Terminal,
            hex: VIOLET,
            curve: link((right, top_middle), (hub_right, hub_upper)),
        },
        Client {
            x: left,
            y: bottom,
            icon: Icon::Code,
            hex: ORCHID,
            curve: link((client_left_edge, bottom_middle), (hub_left, hub_lower)),
        },
        Client {
            x: right,
            y: bottom,
            icon: Icon::Tower,
            hex: PINK,
            curve: link((right, bottom_middle), (hub_right, hub_lower)),
        },
    ]
}

pub(super) fn scene(frame: &Frame) -> Div {
    let local = cycle(frame.t, PERIOD);
    let closed = span(local, 4.2, 9.2, 0.45);
    let clients = clients();
    let mut marks = Vec::new();
    for (index, client) in clients.iter().enumerate() {
        let quiet = if index == 0 { closed } else { 0.0 };
        marks.push(Mark::Line {
            points: client.curve.points(1.0),
            width: 1.4,
            color: tint(client.hex, lerp(0.42, 0.1, quiet)),
            dash: None,
        });
        for offset in [0.0, 0.5] {
            let phase = cycle(frame.t * 0.42 + units(index) * 0.27 + offset, 1.0);
            let along = if index % 2 == 0 { phase } else { 1.0 - phase };
            packet(
                &mut marks,
                client.curve,
                along,
                2.4,
                client.hex,
                1.0 - quiet,
            );
        }
    }
    let labels = [
        tr!("Desktop app"),
        tr!("Terminal UI"),
        tr!("muxy command"),
        tr!("Phone"),
    ];
    div()
        .size_full()
        .relative()
        .children(rings(frame))
        .child(paint_marks(frame, marks))
        .children(
            clients
                .iter()
                .zip(labels)
                .enumerate()
                .map(|(index, (client, label))| {
                    let quiet = if index == 0 { closed } else { 0.0 };
                    client_card(frame, client, label, quiet)
                }),
        )
        .child(closed_badge(frame, &clients[0], closed))
        .child(hub(frame))
        .child(caption(
            frame,
            470.0,
            1.0 - closed,
            muted(),
            tr!("Every app is a client of the same server."),
        ))
        .child(caption(
            frame,
            470.0,
            closed,
            solid(GREEN),
            tr!("App closed. Your terminals keep running."),
        ))
}

fn rings(frame: &Frame) -> impl Iterator<Item = Div> + use<> {
    let center = (HUB_X + HUB_WIDTH / 2.0, HUB_Y + HUB_HEIGHT / 2.0);
    let time = frame.t;
    let scale = frame.m;
    (0..3).map(move |index| {
        let x = cycle(time / 3.6 + units(index) / 3.0, 1.0);
        let radius = lerp(70.0, 180.0, x);
        div()
            .absolute()
            .left(scale.scaled(center.0 - radius))
            .top(scale.scaled(center.1 - radius))
            .size(scale.scaled(radius * 2.0))
            .rounded_full()
            .border_1()
            .border_color(tint(VIOLET, (1.0 - x) * 0.2))
    })
}

fn client_card(frame: &Frame, client: &Client, label: gpui::SharedString, quiet: f32) -> Div {
    card(frame, client.x, client.y, CLIENT_WIDTH, CLIENT_HEIGHT)
        .opacity(lerp(1.0, 0.45, quiet))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(frame.px(6.0))
        .child(IconGlyph::new(
            client.icon,
            frame.px(17.0),
            solid(client.hex),
        ))
        .child(text(frame, 10.0, tint(TEXT, 0.88), label))
}

/// Says the app closed, at full strength beside its faded card.
fn closed_badge(frame: &Frame, client: &Client, closed: f32) -> Div {
    place(
        frame,
        client.x + CLIENT_WIDTH - 44.0,
        client.y - 9.0,
        52.0,
        18.0,
    )
    .flex()
    .items_center()
    .justify_center()
    .rounded_full()
    .bg(solid(AMBER))
    .opacity(progress(closed, 0.4, 0.4))
    .child(text(frame, 9.5, solid(INK), tr!("Closed")))
}

/// The server, with its terminals busy whatever the clients are doing.
fn hub(frame: &Frame) -> Div {
    let sessions = [
        ("claude", 3.1, 0.0),
        ("npm dev", 2.3, 1.2),
        ("zsh", 4.4, 2.6),
    ];
    card(frame, HUB_X, HUB_Y, HUB_WIDTH, HUB_HEIGHT)
        .bg(solid(super::kit::RAISED))
        .border_color(tint(VIOLET, 0.5))
        .flex()
        .flex_col()
        .gap(frame.px(7.0))
        .p(frame.px(11.0))
        .child(mono(frame, 11.5, solid(TEXT), "muxy-server").font_weight(FontWeight::SEMIBOLD))
        .child(div().h(gpui::px(1.0)).bg(line()))
        .children(sessions.map(|(name, period, phase)| {
            let level = wave(frame.t + phase, period);
            div()
                .flex()
                .items_center()
                .gap(frame.px(6.0))
                .child(
                    div()
                        .flex_none()
                        .size(frame.px(5.0))
                        .rounded_full()
                        .bg(solid(GREEN)),
                )
                .child(
                    div()
                        .w(frame.px(48.0))
                        .child(mono(frame, 9.5, muted(), name)),
                )
                .child(
                    div()
                        .h(frame.px(3.0))
                        .w(frame.px(14.0 + level * 38.0))
                        .rounded_full()
                        .bg(tint(CYAN, 0.75)),
                )
        }))
        .child(text(frame, 9.0, dim(), tr!("Always running")))
}
