//! Panels slide in beside a terminal: the file tree, then Source Control. The
//! cards below follow the real installs.

use gpui::prelude::FluentBuilder;
use gpui::{Div, FontWeight, Hsla, ParentElement, SharedString, Styled, div};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::kit::{
    AMBER, CYAN, Frame, GREEN, INDIGO, PINK, RAISED, TEXT, VIOLET, app_window, card, cursor, dim,
    line, mono, muted, place, solid, text, tint,
};
use super::timeline::{cycle, ease_in_out, lerp, progress, rise};
use crate::model::onboarding::OfferState;

pub(super) const STILL: f32 = 3.2;

const PERIOD: f32 = 10.5;
const WINDOW_X: f32 = 28.0;
const WINDOW_Y: f32 = 67.0;
const WINDOW_WIDTH: f32 = 436.0;
const WINDOW_HEIGHT: f32 = 354.0;
const PANEL: f32 = 184.0;
const BAR: f32 = 28.0;

pub(super) fn scene(frame: &Frame) -> Div {
    let local = cycle(frame.t, PERIOD);
    let slid = rise(local, 0.7, 0.55) * (1.0 - ease_in_out(progress(local, PERIOD - 0.8, 0.55)));
    let on_git = ease_in_out(progress(local, 5.2, 0.45));
    div()
        .size_full()
        .relative()
        .child(
            app_window(frame, WINDOW_X, WINDOW_Y, WINDOW_WIDTH, WINDOW_HEIGHT)
                .child(toolbar(frame, slid, on_git))
                .child(terminal(frame, local))
                .child(panel(frame, slid, on_git)),
        )
        .child(status_card(
            frame,
            WINDOW_X,
            "files",
            Icon::Folder,
            tr!("Files"),
        ))
        .child(status_card(
            frame,
            WINDOW_X + WINDOW_WIDTH / 2.0 + 6.0,
            "git",
            Icon::GitBranch,
            tr!("Git"),
        ))
}

fn toolbar(frame: &Frame, slid: f32, on_git: f32) -> Div {
    let lit = |on: bool| if on && slid > 0.5 { solid(CYAN) } else { dim() };
    let highlight = |on: bool, glyph: Div| {
        glyph
            .flex()
            .items_center()
            .justify_center()
            .size(frame.px(20.0))
            .rounded(frame.px(5.0))
            .when(on && slid > 0.5, |glyph| glyph.bg(tint(CYAN, 0.14)))
    };
    place(frame, WINDOW_WIDTH - 60.0, 4.0, 52.0, 20.0)
        .flex()
        .gap(frame.px(4.0))
        .child(highlight(
            on_git < 0.5,
            div().child(IconGlyph::new(
                Icon::Folder,
                frame.px(12.0),
                lit(on_git < 0.5),
            )),
        ))
        .child(highlight(
            on_git >= 0.5,
            div().child(IconGlyph::new(
                Icon::GitBranch,
                frame.px(12.0),
                lit(on_git >= 0.5),
            )),
        ))
}

fn terminal(frame: &Frame, local: f32) -> Div {
    let lines = [
        ("❯ git status", TEXT, 0.2),
        ("modified: checkout.ts", AMBER, 0.7),
        ("modified: checkout.test.ts", AMBER, 0.9),
        ("❯ npm test", TEXT, 1.6),
        ("✓ 128 tests passed", GREEN, 2.3),
    ];
    place(frame, 0.0, BAR, WINDOW_WIDTH, WINDOW_HEIGHT - BAR)
        .border_t_1()
        .border_color(line())
        .p(frame.px(12.0))
        .flex()
        .flex_col()
        .gap(frame.px(5.0))
        .children(lines.map(|(content, hex, at)| {
            mono(frame, 10.0, tint(hex, 0.9), content).opacity(progress(local, at, 0.2))
        }))
        .child(cursor(frame, 10.0, muted()))
}

fn panel(frame: &Frame, slid: f32, on_git: f32) -> Div {
    let height = WINDOW_HEIGHT - BAR;
    place(
        frame,
        lerp(WINDOW_WIDTH, WINDOW_WIDTH - PANEL, slid),
        BAR,
        PANEL,
        height,
    )
    .bg(solid(RAISED))
    .border_l_1()
    .border_t_1()
    .border_color(line())
    .overflow_hidden()
    .child(
        place(frame, -PANEL * on_git, 0.0, PANEL, height)
            .opacity(1.0 - on_git)
            .child(files(frame)),
    )
    .child(
        place(frame, PANEL * (1.0 - on_git), 0.0, PANEL, height)
            .opacity(on_git)
            .child(source_control(frame)),
    )
}

fn files(frame: &Frame) -> Div {
    let rows = [
        (0.0, Icon::FolderOpen, "src", false),
        (1.0, Icon::FolderOpen, "checkout", false),
        (2.0, Icon::File, "checkout.ts", true),
        (2.0, Icon::File, "checkout.test.ts", false),
        (1.0, Icon::File, "main.ts", false),
        (0.0, Icon::Folder, "docs", false),
        (0.0, Icon::Folder, "tests", false),
        (0.0, Icon::File, "package.json", false),
        (0.0, Icon::File, "README.md", false),
    ];
    div()
        .size_full()
        .flex()
        .flex_col()
        .p(frame.px(10.0))
        .gap(frame.px(4.0))
        .child(div().pb(frame.px(5.0)).child(
            text(frame, 10.5, tint(TEXT, 0.95), tr!("Files")).font_weight(FontWeight::SEMIBOLD),
        ))
        .children(rows.map(|(depth, icon, name, selected)| {
            div()
                .flex()
                .items_center()
                .gap(frame.px(6.0))
                .h(frame.px(20.0))
                .pl(frame.px(6.0 + depth * 11.0))
                .rounded(frame.px(4.0))
                .when(selected, |row| row.bg(tint(0xff_ff_ff, 0.08)))
                .child(IconGlyph::new(icon, frame.px(11.0), solid(VIOLET)))
                .child(text(frame, 10.0, tint(TEXT, 0.88), name))
        }))
}

fn source_control(frame: &Frame) -> Div {
    let changes = [
        ("checkout.ts", "+24", "−8"),
        ("checkout.test.ts", "+61", "−2"),
        ("cart.ts", "+3", "−1"),
    ];
    div()
        .size_full()
        .flex()
        .flex_col()
        .p(frame.px(10.0))
        .gap(frame.px(7.0))
        .child(
            text(frame, 10.5, tint(TEXT, 0.95), tr!("Source Control"))
                .font_weight(FontWeight::SEMIBOLD),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(frame.px(5.0))
                .child(IconGlyph::new(Icon::GitBranch, frame.px(10.0), solid(PINK)))
                .child(mono(frame, 9.5, muted(), "feature/login")),
        )
        .child(
            div()
                .h(frame.px(40.0))
                .p(frame.px(7.0))
                .rounded(frame.px(5.0))
                .border_1()
                .border_color(line())
                .child(text(frame, 9.5, dim(), tr!("Commit message"))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(frame.px(22.0))
                .rounded(frame.px(5.0))
                .bg(solid(INDIGO))
                .child(text(frame, 10.0, solid(0xff_ff_ff), tr!("Commit"))),
        )
        .child(
            div()
                .pt(frame.px(3.0))
                .child(text(frame, 9.5, dim(), tr!("Changes"))),
        )
        .children(changes.map(|(name, added, removed)| {
            div()
                .flex()
                .items_center()
                .gap(frame.px(5.0))
                .h(frame.px(19.0))
                .child(mono(frame, 9.5, solid(AMBER), "M"))
                .child(div().flex_1().min_w(gpui::px(0.0)).child(text(
                    frame,
                    9.5,
                    tint(TEXT, 0.88),
                    name,
                )))
                .child(mono(frame, 9.0, solid(GREEN), added))
                .child(mono(frame, 9.0, solid(PINK), removed))
        }))
}

/// How an offered extension is really doing, from the tour's install queue.
struct Status {
    label: SharedString,
    color: Hsla,
    busy: bool,
    done: bool,
}

fn status(frame: &Frame, name: &str) -> Status {
    let plain = |label: SharedString, color: Hsla, busy: bool, done: bool| Status {
        label,
        color,
        busy,
        done,
    };
    if frame.model.extensions.registry.enabled(name).is_some() {
        return plain(tr!("Enabled"), solid(GREEN), false, true);
    }
    let offer = frame
        .model
        .onboarding
        .offers
        .iter()
        .find(|offer| offer.name == name);
    let installed = frame
        .model
        .extensions
        .registry
        .extensions
        .contains_key(name);
    match offer.map(|offer| &offer.state) {
        Some(OfferState::Queued) => plain(tr!("Waiting…"), muted(), true, false),
        Some(OfferState::Installing) => plain(tr!("Installing…"), solid(CYAN), true, false),
        Some(OfferState::Enabling) => plain(tr!("Enabling…"), solid(CYAN), true, false),
        Some(OfferState::Failed(_)) if installed => {
            plain(tr!("Could not enable"), solid(AMBER), false, false)
        }
        Some(OfferState::Failed(_)) => plain(tr!("Not installed"), solid(AMBER), false, false),
        _ if installed => plain(tr!("Installed, not enabled"), muted(), false, false),
        _ => plain(tr!("Not installed yet"), muted(), false, false),
    }
}

fn status_card(frame: &Frame, x: f32, name: &str, icon: Icon, title: SharedString) -> Div {
    let width = WINDOW_WIDTH / 2.0 - 6.0;
    let status = status(frame, name);
    card(frame, x, WINDOW_Y + WINDOW_HEIGHT + 16.0, width, 64.0)
        .when(status.done, |card| card.border_color(tint(GREEN, 0.45)))
        .overflow_hidden()
        .flex()
        .items_center()
        .gap(frame.px(10.0))
        .px(frame.px(12.0))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(frame.px(32.0))
                .rounded(frame.px(8.0))
                .bg(tint(VIOLET, 0.16))
                .child(IconGlyph::new(icon, frame.px(16.0), solid(VIOLET))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(frame.px(1.0))
                .child(text(frame, 11.5, tint(TEXT, 0.96), title).font_weight(FontWeight::SEMIBOLD))
                .child(text(frame, 10.0, status.color, status.label)),
        )
        .when(status.done, |card| {
            card.child(IconGlyph::new(
                Icon::CircleCheck,
                frame.px(16.0),
                solid(GREEN),
            ))
        })
        .when(status.busy, |card| card.child(shimmer(frame, width)))
}

/// A sliver moving along the card's foot while its install runs.
fn shimmer(frame: &Frame, width: f32) -> Div {
    let sliver = 70.0;
    let x = lerp(-sliver, width, cycle(frame.t, 1.3) / 1.3);
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(frame.px(2.0))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left(frame.px(x))
                .top_0()
                .h_full()
                .w(frame.px(sliver))
                .bg(solid(CYAN)),
        )
}
