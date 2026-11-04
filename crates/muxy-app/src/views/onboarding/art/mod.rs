//! The tour's moving pictures, one scene per step on a flat backdrop. A scene is
//! a pure function of the time, drawn fresh each frame by this view alone, so
//! the animation never re-renders what the app has cached.
//!
//! Text and boxes land on whole pixels, so anything drifting slowly would
//! visibly step. Things here move quickly into place and then hold still.

mod agents;
mod extensions;
pub(crate) mod kit;
mod mobile;
mod projects;
mod remote;
mod server;
mod shortcuts;
mod timeline;
mod welcome;

use std::time::Instant;

use gpui::{
    AnyElement, BackgroundExecutor, Context, Div, InteractiveElement, IntoElement, ParentElement,
    PathBuilder, Render, Role, StatefulInteractiveElement as _, Styled, WeakEntity, Window, canvas,
    div, linear_color_stop, linear_gradient, point,
};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::tr;

use self::kit::{BACKDROP, CYAN, Frame, H, INDIGO, PINK, VIOLET, W, solid};
use self::timeline::progress;
use super::{ART_RADIUS, Layout};
use crate::model::AppModel;
use crate::model::onboarding::Step;
use crate::views::shortcut_hints::key_glyph;

/// How large each module of a store's QR code is drawn, big enough to scan from a screen.
pub(crate) const STORE_QR_MODULE: f32 = 3.0;

const SCENE_FADE: f32 = 0.35;
const OUTGOING_FADE: f32 = 0.22;

pub(crate) struct ArtView {
    model: WeakEntity<AppModel>,
    clock: BackgroundExecutor,
    shown: Instant,
    step: Step,
    outgoing: Option<(Step, Instant)>,
    still: bool,
}

impl ArtView {
    pub(crate) fn new(model: WeakEntity<AppModel>, cx: &mut Context<Self>) -> Self {
        let clock = cx.background_executor().clone();
        let shown = clock.now();
        Self {
            model,
            clock,
            shown,
            step: Step::Welcome,
            outgoing: None,
            still: muxy_ui::motion::reduce_motion(),
        }
    }

    pub(crate) fn show(&mut self, step: Step, cx: &mut Context<Self>) {
        self.outgoing = (!self.still).then_some((self.step, self.shown));
        self.step = step;
        self.shown = self.clock.now();
        cx.notify();
    }
}

impl Render for ArtView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(model) = self.model.upgrade() else {
            return div().into_any_element();
        };
        let now = self.clock.now();
        if !self.still && window.is_window_active() {
            window.request_animation_frame();
        }
        let since = |moment: Instant| now.duration_since(moment).as_secs_f32();
        let incoming = since(self.shown);
        if incoming > OUTGOING_FADE {
            self.outgoing = None;
        }
        let model = model.read(cx);
        let m = model.metrics;
        let layout = Layout::of(window.viewport_size(), m);
        let art = layout.art.unwrap_or(layout.card);
        let origin = point(
            (art.width - m.scaled(W)) / 2.0,
            (art.height - m.scaled(H)) / 2.0,
        );
        let stage = |step: Step, time: f32, opacity: f32| {
            let frame = Frame {
                t: if self.still { still_time(step) } else { time },
                m,
                model,
            };
            div()
                .absolute()
                .left(origin.x)
                .top(origin.y)
                .w(m.scaled(W))
                .h(m.scaled(H))
                .opacity(opacity)
                .child(scene(step, &frame))
        };
        let entering = if self.still {
            1.0
        } else {
            timeline::ease_out(progress(incoming, 0.0, SCENE_FADE))
        };
        div()
            .id("onboarding-art")
            .debug_selector(|| "onboarding-art".into())
            .role(Role::Image)
            .aria_label(description(self.step))
            .relative()
            .size_full()
            .overflow_hidden()
            .rounded(m.scaled(ART_RADIUS))
            .bg(solid(BACKDROP))
            .children(self.outgoing.map(|(step, shown)| {
                let leaving = 1.0 - progress(incoming, 0.0, OUTGOING_FADE);
                stage(step, since(shown), leaving)
            }))
            .child(stage(self.step, incoming, entering))
            .into_any_element()
    }
}

fn scene(step: Step, frame: &Frame) -> Div {
    match step {
        Step::Welcome => welcome::scene(frame),
        Step::Server => server::scene(frame),
        Step::Projects => projects::scene(frame),
        Step::Extensions => extensions::scene(frame),
        Step::Agents => agents::scene(frame),
        Step::Remote => remote::scene(frame),
        Step::Mobile => mobile::scene(frame),
        Step::Shortcuts => shortcuts::scene(frame),
    }
}

/// The moment that shows a scene best, for when motion is turned down.
fn still_time(step: Step) -> f32 {
    match step {
        Step::Welcome => welcome::STILL,
        Step::Server => server::STILL,
        Step::Projects => projects::STILL,
        Step::Extensions => extensions::STILL,
        Step::Agents => agents::STILL,
        Step::Remote => remote::STILL,
        Step::Mobile => mobile::STILL,
        Step::Shortcuts => shortcuts::STILL,
    }
}

fn description(step: Step) -> gpui::SharedString {
    match step {
        Step::Welcome => tr!("Muxy's logo above terminals running side by side."),
        Step::Server => {
            tr!(
                "The desktop app, terminal UI, command line, and phone all connected to one server."
            )
        }
        Step::Projects => tr!("A sidebar of projects with their worktrees."),
        Step::Extensions => tr!("File and Git panels sliding in beside a terminal."),
        Step::Agents => tr!("AI agents working, waiting for you, and done."),
        Step::Remote => tr!("This computer connected to remote servers over SSH."),
        Step::Mobile => tr!("A phone beside the code that pairs it."),
        Step::Shortcuts => tr!("Keys being pressed as a window splits and the palette opens."),
    }
}

/// Muxy's two chevrons, each sliding into place as its reveal reaches 1.
fn logo(frame: &Frame, x: f32, y: f32, side: f32, reveals: [f32; 2]) -> AnyElement {
    const LEFT: [(f32, f32); 6] = [
        (220.44, 186.07),
        (574.68, 512.0),
        (220.44, 837.93),
        (220.44, 696.22),
        (418.82, 512.0),
        (220.44, 327.78),
    ];
    const RIGHT: [(f32, f32); 6] = [
        (449.32, 186.07),
        (803.56, 512.0),
        (449.32, 837.93),
        (449.32, 696.22),
        (647.69, 512.0),
        (449.32, 327.78),
    ];
    let m = frame.m;
    let shapes = [
        (LEFT, 135.0, CYAN, INDIGO, reveals[0]),
        (RIGHT, 45.0, PINK, VIOLET, reveals[1]),
    ];
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            for (shape, angle, from, to, reveal) in shapes {
                if reveal <= 0.0 {
                    continue;
                }
                let shift = (1.0 - reveal) * -side * 0.22;
                let at = |(across, down): (f32, f32)| {
                    bounds.origin
                        + point(
                            m.scaled(across / 1024.0 * side + shift),
                            m.scaled(down / 1024.0 * side),
                        )
                };
                let mut builder = PathBuilder::fill();
                builder.move_to(at(shape[0]));
                for corner in &shape[1..] {
                    builder.line_to(at(*corner));
                }
                builder.close();
                if let Ok(path) = builder.build() {
                    let fill = linear_gradient(
                        angle,
                        linear_color_stop(gpui::rgb(from), 0.0),
                        linear_color_stop(gpui::rgb(to), 1.0),
                    );
                    window.paint_path(path, fill.opacity(reveal));
                }
            }
        },
    )
    .absolute()
    .left(frame.px(x))
    .top(frame.px(y))
    .size(frame.px(side))
    .into_any_element()
}

/// The keys of the shortcut the user's keymap gives `id`, in press order.
pub(crate) fn key_caps(id: ShortcutId, model: &AppModel) -> Vec<String> {
    model
        .settings
        .keymap
        .chord(id)
        .map(|chord| caps(chord.as_str()))
        .unwrap_or_default()
}

/// The shortcut as one piece of text, for a sentence.
pub(crate) fn chord_text(id: ShortcutId, model: &AppModel) -> gpui::SharedString {
    let separator = if cfg!(target_os = "macos") { "" } else { "+" };
    key_caps(id, model).join(separator).into()
}

fn caps(chord: &str) -> Vec<String> {
    let Ok(key) = gpui::Keystroke::parse(chord) else {
        return vec![chord.to_owned()];
    };
    let modifiers = key.modifiers;
    let names = if cfg!(target_os = "macos") {
        ["⌃", "⌥", "⇧", "⌘"]
    } else {
        ["Ctrl", "Alt", "Shift", "Super"]
    };
    let held = [
        modifiers.control,
        modifiers.alt,
        modifiers.shift,
        modifiers.platform,
    ];
    names
        .into_iter()
        .zip(held)
        .filter(|(_, held)| *held)
        .map(|(name, _)| name.to_owned())
        .chain([key_glyph(&key.key)])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chord_splits_into_keys_with_modifiers_first() {
        let keys = caps("cmd-shift-p");
        if cfg!(target_os = "macos") {
            assert_eq!(keys, ["⇧", "⌘", "P"]);
        } else {
            assert_eq!(keys, ["Shift", "Super", "P"]);
        }
    }

    #[test]
    fn named_keys_read_as_their_symbols() {
        assert_eq!(caps("enter").last().map(String::as_str), Some("↩"));
    }

    #[test]
    fn a_chord_that_will_not_parse_stays_as_written() {
        assert_eq!(
            caps("not-a-real-chord-at-all!"),
            ["not-a-real-chord-at-all!"]
        );
    }
}
