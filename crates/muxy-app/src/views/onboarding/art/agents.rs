//! Agents work, wait, and finish in a panel. A notification calls when one needs
//! you, and every supported agent drifts past below.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::kit::{
    AMBER, CYAN, Frame, GREEN, TEXT, W, card, dim, line, muted, paint_marks, place, solid, spinner,
    text, tint, units,
};
use super::timeline::{cycle, rise, span, wave};

pub(super) const STILL: f32 = 4.4;

const PERIOD: f32 = 13.0;
const PANEL_X: f32 = 36.0;
const PANEL_Y: f32 = 72.0;
const PANEL_WIDTH: f32 = 420.0;
const HEADER: f32 = 40.0;
const ROW: f32 = 58.0;

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Working,
    Waiting,
    Done,
}

struct Agent {
    icon: &'static str,
    name: &'static str,
    task: &'static str,
    project: &'static str,
    schedule: &'static [(f32, State)],
}

const AGENTS: [Agent; 4] = [
    Agent {
        icon: "claude",
        name: "Claude Code",
        task: "Fix the checkout test",
        project: "web-app",
        schedule: &[
            (0.0, State::Working),
            (3.2, State::Waiting),
            (7.6, State::Working),
            (10.6, State::Done),
        ],
    },
    Agent {
        icon: "codex",
        name: "Codex",
        task: "Write the API docs",
        project: "api-server",
        schedule: &[(0.0, State::Working), (5.6, State::Done)],
    },
    Agent {
        icon: "opencode",
        name: "OpenCode",
        task: "Refactor login",
        project: "feature/login",
        schedule: &[
            (0.0, State::Done),
            (1.8, State::Working),
            (9.4, State::Waiting),
        ],
    },
    Agent {
        icon: "cursor",
        name: "Cursor",
        task: "Update dependencies",
        project: "mobile",
        schedule: &[(0.0, State::Working), (8.2, State::Done)],
    },
];

const PROVIDERS: [&str; 12] = [
    "claude",
    "codex",
    "opencode",
    "cursor",
    "copilot",
    "factory",
    "pi",
    "grok",
    "kiro",
    "xal",
    "antigravity",
    "omp",
];

fn state_at(agent: &Agent, local: f32) -> State {
    agent
        .schedule
        .iter()
        .rev()
        .find(|(start, _)| local >= *start)
        .map_or(State::Working, |(_, state)| *state)
}

pub(super) fn scene(frame: &Frame) -> Div {
    let local = cycle(frame.t, PERIOD);
    let mut marks = Vec::new();
    for (index, agent) in AGENTS.iter().enumerate() {
        if state_at(agent, local) == State::Working {
            marks.push(spinner(
                frame.t,
                PANEL_X + PANEL_WIDTH - 80.5,
                PANEL_Y + HEADER + units(index) * ROW + ROW / 2.0,
                5.5,
                solid(CYAN),
            ));
        }
    }
    let running = AGENTS
        .iter()
        .filter(|agent| state_at(agent, local) != State::Done)
        .count();
    div()
        .size_full()
        .relative()
        .child(
            card(frame, PANEL_X, PANEL_Y, PANEL_WIDTH, HEADER + ROW * 4.0)
                .overflow_hidden()
                .child(header(frame, running))
                .children(
                    AGENTS
                        .iter()
                        .enumerate()
                        .map(|(index, agent)| row(frame, agent, index, local)),
                ),
        )
        .child(paint_marks(frame, marks))
        .child(notification(frame, local))
        .child(providers(frame))
}

fn header(frame: &Frame, running: usize) -> Div {
    place(frame, 0.0, 0.0, PANEL_WIDTH, HEADER)
        .flex()
        .items_center()
        .justify_between()
        .px(frame.px(14.0))
        .border_b_1()
        .border_color(line())
        .child(text(frame, 12.0, tint(TEXT, 0.96), tr!("Agents")).font_weight(FontWeight::SEMIBOLD))
        .child(
            div()
                .px(frame.px(8.0))
                .rounded_full()
                .bg(tint(0xff_ff_ff, 0.08))
                .child(text(frame, 10.0, muted(), tr!("%lld running", running))),
        )
}

fn row(frame: &Frame, agent: &Agent, index: usize, local: f32) -> Div {
    let state = state_at(agent, local);
    let waiting = state == State::Waiting;
    let attention = if waiting { wave(frame.t, 1.4) } else { 0.0 };
    place(frame, 0.0, HEADER + units(index) * ROW, PANEL_WIDTH, ROW)
        .flex()
        .items_center()
        .gap(frame.px(12.0))
        .px(frame.px(14.0))
        .when(index > 0, |row| row.border_t_1().border_color(line()))
        .when(waiting, |row| row.bg(tint(AMBER, 0.05 + attention * 0.07)))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(frame.px(32.0))
                .rounded(frame.px(8.0))
                .bg(tint(0xff_ff_ff, 0.07))
                .child(provider(frame, agent.icon, 18.0)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(gpui::px(0.0))
                .child(text(frame, 11.5, tint(TEXT, 0.96), agent.task))
                .child(text(
                    frame,
                    9.5,
                    dim(),
                    format!("{} · {}", agent.name, agent.project),
                )),
        )
        .child(state_label(frame, state, attention))
}

fn state_label(frame: &Frame, state: State, attention: f32) -> AnyElement {
    let (label, color): (SharedString, Hsla) = match state {
        State::Working => (tr!("Working"), solid(CYAN)),
        State::Waiting => (tr!("Waiting"), solid(AMBER)),
        State::Done => (tr!("Done"), solid(GREEN)),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(frame.px(6.0))
        .w(frame.px(72.0))
        .child(match state {
            State::Working => div().flex_none().size(frame.px(11.0)).into_any_element(),
            State::Waiting => div()
                .flex_none()
                .size(frame.px(9.0))
                .mx(frame.px(1.0))
                .rounded_full()
                .bg(tint(AMBER, 0.55 + attention * 0.45))
                .into_any_element(),
            State::Done => {
                IconGlyph::new(Icon::Check, frame.px(11.0), solid(GREEN)).into_any_element()
            }
        })
        .child(text(frame, 10.5, color, label))
        .into_any_element()
}

/// Claude's mark keeps its color; the others take the text color.
fn provider(frame: &Frame, name: &str, side: f32) -> AnyElement {
    let path = SharedString::from(format!("icons/provider-{name}.svg"));
    if name == "claude" {
        return gpui::img(path).size(frame.px(side)).into_any_element();
    }
    gpui::svg()
        .path(path)
        .size(frame.px(side))
        .text_color(tint(0xff_ff_ff, 0.9))
        .into_any_element()
}

fn notification(frame: &Frame, local: f32) -> Div {
    let shown = span(local, 3.4, 7.6, 0.4);
    let y = PANEL_Y + HEADER + ROW * 4.0 + 14.0 + (1.0 - shown) * 14.0;
    card(frame, PANEL_X + 40.0, y, PANEL_WIDTH - 80.0, 58.0)
        .opacity(shown)
        .bg(solid(super::kit::RAISED))
        .border_color(tint(AMBER, 0.4))
        .flex()
        .items_center()
        .gap(frame.px(11.0))
        .px(frame.px(12.0))
        .child(provider(frame, "claude", 22.0))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(gpui::px(0.0))
                .child(
                    text(frame, 11.0, tint(TEXT, 0.97), tr!("Claude Code needs you"))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(text(
                    frame,
                    9.5,
                    muted(),
                    tr!("Allow it to run the test suite?"),
                )),
        )
        .child(IconGlyph::new(Icon::Bell, frame.px(14.0), solid(AMBER)))
}

/// Every supported agent in a row, with a highlight passing along it.
fn providers(frame: &Frame) -> Div {
    const PITCH: f32 = 36.0;
    const SIDE: f32 = 22.0;
    let count = units(PROVIDERS.len());
    let left = (W - PITCH * (count - 1.0) - SIDE) / 2.0;
    let highlight = cycle(frame.t * 2.2, count + 4.0) - 2.0;
    place(frame, 0.0, 436.0, W, 44.0).children(PROVIDERS.iter().enumerate().map(|(index, name)| {
        let lit = 1.0 - ((units(index) - highlight).abs() / 2.0).min(1.0);
        place(frame, left + units(index) * PITCH, 11.0, SIDE, SIDE)
            .opacity(rise(frame.t, 0.2 + units(index) * 0.05, 0.4) * (0.5 + lit * 0.5))
            .child(provider(frame, name, SIDE))
    }))
}
