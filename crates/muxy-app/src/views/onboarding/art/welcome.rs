//! The logo arrives, and terminals fan out beneath it, each running something.

use gpui::{Div, ParentElement, Styled, div};

use super::kit::{
    CYAN, Frame, GREEN, TEXT, VIOLET, app_window, cursor, dim, mono, muted, solid, tint, units,
};
use super::logo;
use super::timeline::{cycle, lerp, progress, rise, typed};

pub(super) const STILL: f32 = 7.5;

const PERIOD: f32 = 8.5;

struct Terminal {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    title: &'static str,
    command: &'static str,
    output: &'static [(&'static str, u32)],
    delay: f32,
}

const TERMINALS: [Terminal; 3] = [
    Terminal {
        x: 26.0,
        y: 268.0,
        width: 222.0,
        height: 138.0,
        title: "npm run dev",
        command: "npm run dev",
        output: &[
            ("VITE ready in 312 ms", GREEN),
            ("→ Local: localhost:5173", CYAN),
            ("page reload src/App.tsx", TEXT),
        ],
        delay: 2.4,
    },
    Terminal {
        x: 244.0,
        y: 248.0,
        width: 222.0,
        height: 138.0,
        title: "cargo watch",
        command: "cargo watch -x test",
        output: &[
            ("Compiling muxy v2.1.0", TEXT),
            ("Finished in 1.84s", GREEN),
            ("test result: ok. 128 passed", GREEN),
        ],
        delay: 1.2,
    },
    Terminal {
        x: 112.0,
        y: 352.0,
        width: 268.0,
        height: 160.0,
        title: "claude",
        command: "claude",
        output: &[
            ("✻ Reading checkout.test.ts", VIOLET),
            ("✻ Fixing the failing test", VIOLET),
            ("✓ All tests pass", GREEN),
            ("Anything else?", TEXT),
        ],
        delay: 0.0,
    },
];

pub(super) fn scene(frame: &Frame) -> Div {
    let t = frame.t;
    div()
        .size_full()
        .relative()
        .children(
            TERMINALS
                .iter()
                .enumerate()
                .map(|(index, terminal)| window(frame, terminal, index)),
        )
        .child(logo(
            frame,
            166.0,
            58.0,
            160.0,
            [rise(t, 0.05, 0.75), rise(t, 0.28, 0.75)],
        ))
}

fn window(frame: &Frame, terminal: &Terminal, index: usize) -> Div {
    let t = frame.t;
    let spread = rise(t, 0.4 + units(index) * 0.12, 0.9);
    let x = lerp(246.0 - terminal.width / 2.0, terminal.x, spread);
    let y = lerp(330.0, terminal.y, spread);
    app_window(frame, x, y, terminal.width, terminal.height)
        .opacity(progress(t, 0.35 + units(index) * 0.12, 0.45))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h(frame.px(25.0))
                .flex()
                .items_center()
                .justify_center()
                .child(mono(frame, 9.5, dim(), terminal.title)),
        )
        .child(screen(frame, terminal))
}

fn screen(frame: &Frame, terminal: &Terminal) -> Div {
    let local = cycle((frame.t - 0.8 - terminal.delay).max(0.0), PERIOD);
    let ending = 1.0 - progress(local, PERIOD - 0.5, 0.4);
    let typed_command = typed(terminal.command, local, 0.15, 20.0);
    let done_typing = units(terminal.command.chars().count()) / 20.0 + 0.25;
    let shown = terminal
        .output
        .iter()
        .enumerate()
        .filter_map(|(line, (text, hex))| {
            let at = done_typing + 0.5 + units(line) * 0.85;
            (local >= at).then(|| {
                mono(frame, 10.0, tint(*hex, 0.92), *text).opacity(progress(local, at, 0.2))
            })
        });
    let finished = local >= done_typing + 0.5 + units(terminal.output.len()) * 0.85;
    div()
        .absolute()
        .top(frame.px(28.0))
        .left(frame.px(12.0))
        .flex()
        .flex_col()
        .gap(frame.px(3.0))
        .opacity(ending)
        .child(
            div()
                .flex()
                .items_center()
                .gap(frame.px(5.0))
                .child(mono(frame, 10.0, tint(CYAN, 0.9), "❯"))
                .child(mono(frame, 10.0, solid(TEXT), typed_command.to_owned()))
                .children((local < done_typing).then(|| cursor(frame, 10.0, muted()))),
        )
        .children(shown)
        .children(finished.then(|| cursor(frame, 10.0, muted())))
}
