//! The tour art's colors and drawing pieces. Sizes are in the scene's own units
//! and scale with the interface size.

use gpui::{
    AnyElement, BoxShadow, Div, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Point,
    SharedString, Styled, canvas, div, point, px, size,
};
use muxy_ui::theme::Metrics;

use super::timeline::wave;
use crate::model::AppModel;

/// The scene's size. Every scene is laid out for it and centered in the art.
pub(super) const W: f32 = 492.0;
pub(super) const H: f32 = 568.0;

pub(super) const CYAN: u32 = 0x22_d3_ee;
pub(super) const BLUE: u32 = 0x4f_8e_f7;
pub(super) const INDIGO: u32 = 0x63_66_f1;
pub(super) const VIOLET: u32 = 0xa7_8b_fa;
pub(super) const ORCHID: u32 = 0xc7_7d_db;
pub(super) const PINK: u32 = 0xec_48_99;
pub(super) const GREEN: u32 = 0x34_d3_99;
pub(super) const AMBER: u32 = 0xfb_bf_24;
pub(super) const WHITE: u32 = 0xff_ff_ff;
pub(super) const BLACK: u32 = 0x00_00_00;
pub(super) const BACKDROP: u32 = 0x0e_0d_13;
pub(super) const INK: u32 = 0x0b_0b_11;
pub(super) const PANEL: u32 = 0x16_15_1d;
pub(super) const RAISED: u32 = 0x20_1e_29;
pub(super) const TEXT: u32 = 0xf2_f0_f7;

/// `hex` at `alpha`.
pub(super) fn tint(hex: u32, alpha: f32) -> Hsla {
    let mut color: Hsla = gpui::rgb(hex).into();
    color.a = alpha;
    color
}

pub(super) fn solid(hex: u32) -> Hsla {
    gpui::rgb(hex).into()
}

pub(super) fn muted() -> Hsla {
    tint(WHITE, 0.62)
}

pub(super) fn dim() -> Hsla {
    tint(WHITE, 0.38)
}

pub(super) fn line() -> Hsla {
    tint(WHITE, 0.1)
}

/// A count as a coordinate.
pub(crate) fn units(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

/// One frame of a scene: its time, in seconds since the step appeared.
pub(super) struct Frame<'a> {
    pub(super) t: f32,
    pub(super) m: Metrics,
    pub(super) model: &'a AppModel,
}

impl Frame<'_> {
    pub(super) fn px(&self, value: f32) -> Pixels {
        self.m.scaled(value)
    }
}

/// A box at a place in the scene.
pub(super) fn place(frame: &Frame, x: f32, y: f32, width: f32, height: f32) -> Div {
    div()
        .absolute()
        .left(frame.px(x))
        .top(frame.px(y))
        .w(frame.px(width))
        .h(frame.px(height))
}

/// One line of text.
pub(super) fn text(
    frame: &Frame,
    points: f32,
    color: Hsla,
    content: impl Into<SharedString>,
) -> Div {
    div()
        .flex_none()
        .text_size(frame.px(points))
        .line_height(frame.px(points * 1.4))
        .text_color(color)
        .whitespace_nowrap()
        .child(content.into())
}

/// One line of terminal text.
pub(super) fn mono(
    frame: &Frame,
    points: f32,
    color: Hsla,
    content: impl Into<SharedString>,
) -> Div {
    text(frame, points, color, content).font_family(muxy_ui::font::monospace())
}

fn window_shadow(frame: &Frame) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: tint(BLACK, 0.55),
            offset: point(px(0.0), frame.px(18.0)),
            blur_radius: frame.px(38.0),
            spread_radius: frame.px(-6.0),
        },
        BoxShadow {
            color: tint(BLACK, 0.35),
            offset: point(px(0.0), frame.px(2.0)),
            blur_radius: frame.px(6.0),
            spread_radius: px(0.0),
        },
    ]
}

/// A raised card with no window controls.
pub(super) fn card(frame: &Frame, x: f32, y: f32, width: f32, height: f32) -> Div {
    place(frame, x, y, width, height)
        .rounded(frame.px(10.0))
        .bg(solid(PANEL))
        .border_1()
        .border_color(line())
        .shadow(window_shadow(frame))
}

/// A little app window: a card with its traffic lights in the corner.
pub(super) fn app_window(frame: &Frame, x: f32, y: f32, width: f32, height: f32) -> Div {
    card(frame, x, y, width, height)
        .overflow_hidden()
        .child(traffic_lights(frame, 10.0, 9.0))
}

fn traffic_lights(frame: &Frame, x: f32, y: f32) -> Div {
    let light = |offset: f32, hex: u32| {
        place(frame, x + offset, y, 7.0, 7.0)
            .rounded_full()
            .bg(tint(hex, 0.9))
    };
    div()
        .child(light(0.0, 0xff_5f_57))
        .child(light(11.0, 0xfe_bc_2e))
        .child(light(22.0, 0x28_c8_40))
}

/// A project's square icon with its first letter.
pub(super) fn letter_icon(frame: &Frame, side: f32, color: Hsla, letter: &str) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(frame.px(side))
        .rounded(frame.px(side * 0.26))
        .bg(color)
        .text_size(frame.px(side * 0.58))
        .text_color(solid(WHITE))
        .child(letter.to_owned())
}

/// A short caption at the foot of the scene, fading with `opacity`.
pub(super) fn caption(
    frame: &Frame,
    y: f32,
    opacity: f32,
    color: Hsla,
    content: impl Into<SharedString>,
) -> Div {
    place(frame, 0.0, y, W, 28.0)
        .flex()
        .justify_center()
        .opacity(opacity)
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(frame.px(28.0))
                .px(frame.px(13.0))
                .rounded_full()
                .bg(tint(INK, 0.72))
                .border_1()
                .border_color(line())
                .child(text(frame, 11.0, color, content)),
        )
}

/// A keyboard key.
pub(super) fn keycap(frame: &Frame, side: f32, pressed: f32, cap: String) -> Div {
    let depth = (1.0 - pressed) * 3.0;
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(frame.px(side))
        .h(frame.px(side))
        .px(frame.px(side * 0.22))
        .mt(frame.px(3.0 - depth))
        .rounded(frame.px(side * 0.2))
        .bg(tint(RAISED, 1.0))
        .border_1()
        .border_color(tint(WHITE, 0.14 + pressed * 0.4))
        .shadow(vec![
            BoxShadow {
                color: tint(BLACK, 0.7),
                offset: point(px(0.0), frame.px(depth)),
                blur_radius: px(0.0),
                spread_radius: px(0.0),
            },
            BoxShadow {
                color: tint(VIOLET, pressed * 0.45),
                offset: point(px(0.0), px(0.0)),
                blur_radius: frame.px(18.0),
                spread_radius: px(0.0),
            },
        ])
        .text_size(frame.px(side * 0.42))
        .text_color(tint(TEXT, 0.82 + pressed * 0.18))
        .child(cap)
}

/// What a scene strokes and dots over its boxes.
pub(super) enum Mark {
    /// A line through the points.
    Line {
        points: Vec<(f32, f32)>,
        width: f32,
        color: Hsla,
        dash: Option<f32>,
    },
    /// A filled circle.
    Dot { x: f32, y: f32, r: f32, color: Hsla },
}

/// A curve between two places in the scene.
#[derive(Clone, Copy)]
pub(super) struct Curve {
    pub(super) from: (f32, f32),
    pub(super) a: (f32, f32),
    pub(super) b: (f32, f32),
    pub(super) to: (f32, f32),
}

impl Curve {
    /// The point `x` of the way along it.
    pub(super) fn at(self, x: f32) -> (f32, f32) {
        let inverse = 1.0 - x;
        let blend = |p: f32, q: f32, r: f32, s: f32| {
            inverse * inverse * inverse * p
                + 3.0 * inverse * inverse * x * q
                + 3.0 * inverse * x * x * r
                + x * x * x * s
        };
        (
            blend(self.from.0, self.a.0, self.b.0, self.to.0),
            blend(self.from.1, self.a.1, self.b.1, self.to.1),
        )
    }

    /// The first `reach` of it, as a line.
    pub(super) fn points(self, reach: f32) -> Vec<(f32, f32)> {
        const SEGMENTS: usize = 28;
        (0..=SEGMENTS)
            .map(|index| self.at(units(index) / units(SEGMENTS) * reach))
            .collect()
    }
}

/// Paints `marks` over the whole scene.
pub(super) fn paint_marks(frame: &Frame, marks: Vec<Mark>) -> AnyElement {
    let m = frame.m;
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let at = |(x, y): (f32, f32)| -> Point<Pixels> {
                bounds.origin + point(m.scaled(x), m.scaled(y))
            };
            for mark in marks {
                match mark {
                    Mark::Line {
                        points,
                        width,
                        color,
                        dash,
                    } => {
                        let mut builder = PathBuilder::stroke(m.scaled(width));
                        if let Some(dash) = dash {
                            builder = builder.dash_array(&[m.scaled(dash), m.scaled(dash)]);
                        }
                        let mut points = points.into_iter();
                        let Some(first) = points.next() else {
                            continue;
                        };
                        builder.move_to(at(first));
                        for next in points {
                            builder.line_to(at(next));
                        }
                        if let Ok(path) = builder.build() {
                            window.paint_path(path, color);
                        }
                    }
                    Mark::Dot { x, y, r, color } => {
                        let center = at((x, y));
                        let radius = m.scaled(r);
                        let side = radius * 2.0;
                        window.paint_quad(
                            gpui::fill(
                                gpui::Bounds::new(center - point(radius, radius), size(side, side)),
                                color,
                            )
                            .corner_radii(radius),
                        );
                    }
                }
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// A dot traveling along `curve`, with a soft halo.
pub(super) fn packet(marks: &mut Vec<Mark>, curve: Curve, x: f32, r: f32, hex: u32, alpha: f32) {
    let (px_, py) = curve.at(x);
    marks.push(Mark::Dot {
        x: px_,
        y: py,
        r: r * 2.4,
        color: tint(hex, alpha * 0.22),
    });
    marks.push(Mark::Dot {
        x: px_,
        y: py,
        r,
        color: tint(hex, alpha),
    });
}

/// A turning arc, for something at work.
pub(super) fn spinner(t: f32, x: f32, y: f32, r: f32, color: Hsla) -> Mark {
    const SEGMENTS: usize = 18;
    let start = t * std::f32::consts::TAU * 1.1;
    let sweep = std::f32::consts::TAU * 0.72;
    let points = (0..=SEGMENTS)
        .map(|index| {
            let angle = start + sweep * units(index) / units(SEGMENTS);
            (x + r * angle.cos(), y + r * angle.sin())
        })
        .collect();
    Mark::Line {
        points,
        width: 1.6,
        color,
        dash: None,
    }
}

/// A blinking block cursor.
pub(super) fn cursor(frame: &Frame, points: f32, color: Hsla) -> Div {
    let blink = if wave(frame.t, 1.1) > 0.5 { 1.0 } else { 0.25 };
    div()
        .flex_none()
        .w(frame.px(points * 0.6))
        .h(frame.px(points * 1.2))
        .bg(color)
        .opacity(blink)
}
