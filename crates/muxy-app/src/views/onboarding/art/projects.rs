//! A sidebar fills with projects, one opens to show its worktrees, each with its
//! own tabs and panes, and one gets a new color.

use gpui::prelude::FluentBuilder;
use gpui::{Div, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::kit::{
    BLUE, CYAN, Frame, GREEN, INDIGO, ORCHID, PINK, RAISED, TEXT, VIOLET, app_window, caption,
    card, cursor, dim, letter_icon, line, mono, muted, place, solid, text, tint, units,
};
use super::timeline::{cycle, lerp, progress, rise, span, typed};

pub(super) const STILL: f32 = 7.8;

const PERIOD: f32 = 12.0;
const WINDOW_X: f32 = 28.0;
const WINDOW_Y: f32 = 62.0;
const WINDOW_WIDTH: f32 = 436.0;
const WINDOW_HEIGHT: f32 = 394.0;
const SIDEBAR: f32 = 152.0;
const ROW: f32 = 26.0;
const WORKTREE_ROW: f32 = 22.0;

pub(super) fn scene(frame: &Frame) -> Div {
    let local = cycle(frame.t, PERIOD);
    let leaving = 1.0 - rise(local, PERIOD - 0.7, 0.55);
    let expanded = rise(local, 1.5, 0.5) * leaving;
    let on_worktree = rise(local, 3.4, 0.25) * leaving;
    let recolored = rise(local, 7.4, 0.4) * leaving;
    let customizing = span(local, 6.2, 10.4, 0.3);
    let web_color = blend(INDIGO, PINK, recolored);
    div()
        .size_full()
        .relative()
        .child(
            app_window(frame, WINDOW_X, WINDOW_Y, WINDOW_WIDTH, WINDOW_HEIGHT)
                .child(sidebar(frame, expanded, on_worktree, web_color))
                .child(content(frame, on_worktree, local))
                .child(customize(frame, customizing, recolored)),
        )
        .child(caption(
            frame,
            476.0,
            span(local, 0.0, 3.4, 0.25),
            muted(),
            tr!("Every project keeps its own tabs and splits."),
        ))
        .child(caption(
            frame,
            476.0,
            span(local, 3.4, 6.2, 0.25),
            muted(),
            tr!("Give a branch its own worktree."),
        ))
        .child(caption(
            frame,
            476.0,
            span(local, 6.2, PERIOD, 0.25),
            muted(),
            tr!("Right-click a project to make it yours."),
        ))
}

/// Between two colors.
fn blend(from: u32, to: u32, x: f32) -> Hsla {
    let channels = |hex: u32| {
        [
            units(usize::try_from((hex >> 16) & 0xff).unwrap_or(0)),
            units(usize::try_from((hex >> 8) & 0xff).unwrap_or(0)),
            units(usize::try_from(hex & 0xff).unwrap_or(0)),
        ]
    };
    let (from, to) = (channels(from), channels(to));
    gpui::Rgba {
        r: lerp(from[0], to[0], x) / 255.0,
        g: lerp(from[1], to[1], x) / 255.0,
        b: lerp(from[2], to[2], x) / 255.0,
        a: 1.0,
    }
    .into()
}

fn sidebar(frame: &Frame, expanded: f32, on_worktree: f32, web_color: Hsla) -> Div {
    let t = frame.t;
    let added = frame
        .model
        .onboarding
        .tour
        .as_ref()
        .and_then(|tour| tour.added_project.clone());
    let first_name = added.unwrap_or_else(|| "api-server".to_owned());
    let first_letter = first_name
        .chars()
        .next()
        .map(|letter| letter.to_uppercase().to_string())
        .unwrap_or_default();
    let shift = expanded * WORKTREE_ROW * 3.0;
    let slide = |index: usize| rise(t, 0.2 + units(index) * 0.13, 0.45);
    div()
        .absolute()
        .top_0()
        .left_0()
        .bottom_0()
        .w(frame.px(SIDEBAR))
        .bg(tint(RAISED, 0.55))
        .border_r_1()
        .border_color(line())
        .child(place(frame, 12.0, 30.0, 120.0, 16.0).child(text(
            frame,
            9.5,
            dim(),
            tr!("All Projects"),
        )))
        .child(project_row(
            frame,
            52.0,
            slide(0),
            div()
                .child(IconGlyph::new(Icon::House, frame.px(13.0), muted()))
                .into_any_element(),
            &tr!("Home"),
            false,
        ))
        .child(project_row(
            frame,
            52.0 + ROW,
            slide(1),
            letter_icon(frame, 15.0, solid(CYAN), &first_letter).into_any_element(),
            &first_name,
            false,
        ))
        .child(project_row(
            frame,
            52.0 + ROW * 2.0,
            slide(2),
            letter_icon(frame, 15.0, web_color, "W").into_any_element(),
            "web-app",
            on_worktree < 0.5,
        ))
        .children(worktrees(frame, 52.0 + ROW * 3.0, expanded, on_worktree))
        .child(project_row(
            frame,
            52.0 + ROW * 3.0 + shift,
            slide(3),
            letter_icon(frame, 15.0, solid(GREEN), "M").into_any_element(),
            "mobile",
            false,
        ))
        .child(project_row(
            frame,
            52.0 + ROW * 4.0 + shift,
            slide(4),
            letter_icon(frame, 15.0, solid(ORCHID), "D").into_any_element(),
            "docs",
            false,
        ))
        .child(
            place(frame, 12.0, WINDOW_HEIGHT - 28.0, 128.0, 18.0)
                .flex()
                .items_center()
                .gap(frame.px(6.0))
                .child(IconGlyph::new(Icon::Plus, frame.px(11.0), dim()))
                .child(text(frame, 10.0, dim(), tr!("Add Project"))),
        )
}

fn project_row(
    frame: &Frame,
    y: f32,
    slide: f32,
    icon: gpui::AnyElement,
    name: &str,
    selected: bool,
) -> Div {
    place(
        frame,
        6.0 - (1.0 - slide) * 14.0,
        y,
        SIDEBAR - 12.0,
        ROW - 2.0,
    )
    .opacity(slide)
    .flex()
    .items_center()
    .gap(frame.px(7.0))
    .px(frame.px(6.0))
    .rounded(frame.px(5.0))
    .when(selected, |row| row.bg(tint(0xff_ff_ff, 0.08)))
    .child(icon)
    .child(text(frame, 10.5, tint(TEXT, 0.9), name.to_owned()))
}

fn worktrees(frame: &Frame, y: f32, expanded: f32, on_worktree: f32) -> Vec<Div> {
    let rows: [SharedString; 3] = ["main".into(), "feature/login".into(), tr!("+ New Worktree")];
    rows.iter()
        .enumerate()
        .map(|(index, name)| {
            let selected = index == 1 && on_worktree >= 0.5;
            let color = if index == 2 { dim() } else { tint(TEXT, 0.8) };
            place(
                frame,
                16.0,
                y + units(index) * WORKTREE_ROW * expanded,
                SIDEBAR - 22.0,
                WORKTREE_ROW - 2.0,
            )
            .opacity(progress(expanded, 0.3 + units(index) * 0.2, 0.3))
            .flex()
            .items_center()
            .gap(frame.px(6.0))
            .px(frame.px(6.0))
            .rounded(frame.px(5.0))
            .when(selected, |row| row.bg(tint(0xff_ff_ff, 0.08)))
            .when(index < 2, |row| {
                row.child(IconGlyph::new(Icon::GitBranch, frame.px(10.0), dim()))
            })
            .child(text(frame, 9.5, color, name.clone()))
        })
        .collect()
}

fn content(frame: &Frame, on_worktree: f32, local: f32) -> Div {
    let width = WINDOW_WIDTH - SIDEBAR;
    div()
        .absolute()
        .top_0()
        .left(frame.px(SIDEBAR))
        .w(frame.px(width))
        .bottom_0()
        .child(workspace(
            frame,
            width,
            1.0 - on_worktree,
            ["claude", "dev"],
            "npm run dev",
            ("ready on :5173", GREEN),
            ("GET /checkout 200", BLUE),
            local,
        ))
        .child(workspace(
            frame,
            width,
            on_worktree,
            ["login-form", "tests"],
            "cargo test login",
            ("12 passed", GREEN),
            ("building feature/login", VIOLET),
            local - 3.4,
        ))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Describes a whole little workspace, with no shape worth naming"
)]
fn workspace(
    frame: &Frame,
    width: f32,
    opacity: f32,
    tabs: [&str; 2],
    command: &str,
    first: (&str, u32),
    second: (&str, u32),
    time: f32,
) -> Div {
    let half = width / 2.0;
    let top = 30.0;
    div()
        .absolute()
        .size_full()
        .opacity(opacity)
        .child(
            place(frame, 0.0, 0.0, width, top)
                .flex()
                .items_center()
                .gap(frame.px(4.0))
                .px(frame.px(8.0))
                .border_b_1()
                .border_color(line())
                .child(tab(frame, tabs[0], true))
                .child(tab(frame, tabs[1], false)),
        )
        .child(
            place(frame, 0.0, top, half, WINDOW_HEIGHT - top)
                .border_r_1()
                .border_color(line())
                .p(frame.px(10.0))
                .flex()
                .flex_col()
                .gap(frame.px(4.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(frame.px(4.0))
                        .child(mono(frame, 9.5, tint(CYAN, 0.9), "❯"))
                        .child(mono(
                            frame,
                            9.5,
                            tint(TEXT, 0.9),
                            typed(command, time, 0.3, 16.0).to_owned(),
                        ))
                        .child(cursor(frame, 9.5, muted())),
                )
                .child(
                    mono(frame, 9.5, tint(first.1, 0.9), first.0.to_owned())
                        .opacity(progress(time, 1.4, 0.2)),
                ),
        )
        .child(
            place(frame, half, top, half, WINDOW_HEIGHT - top)
                .p(frame.px(10.0))
                .flex()
                .flex_col()
                .gap(frame.px(4.0))
                .child(
                    mono(frame, 9.5, tint(second.1, 0.9), second.0.to_owned())
                        .opacity(progress(time, 0.8, 0.2)),
                )
                .child(
                    mono(frame, 9.5, dim(), "watching for changes")
                        .opacity(progress(time, 1.8, 0.2)),
                ),
        )
}

fn tab(frame: &Frame, name: &str, selected: bool) -> Div {
    div()
        .flex()
        .items_center()
        .h(frame.px(20.0))
        .px(frame.px(8.0))
        .rounded(frame.px(5.0))
        .when(selected, |tab| tab.bg(tint(0xff_ff_ff, 0.08)))
        .child(text(
            frame,
            9.5,
            if selected { tint(TEXT, 0.95) } else { dim() },
            name.to_owned(),
        ))
}

/// The small menu that recolors a project.
fn customize(frame: &Frame, shown: f32, recolored: f32) -> Div {
    let swatches = [CYAN, INDIGO, VIOLET, ORCHID, PINK, GREEN];
    let selected = if recolored > 0.5 { 4 } else { 1 };
    card(
        frame,
        SIDEBAR - 8.0,
        96.0 + (1.0 - shown) * 6.0,
        176.0,
        116.0,
    )
    .bg(solid(RAISED))
    .opacity(shown)
    .flex()
    .flex_col()
    .gap(frame.px(9.0))
    .p(frame.px(11.0))
    .child(text(frame, 10.5, tint(TEXT, 0.95), "web-app").font_weight(FontWeight::SEMIBOLD))
    .child(
        div()
            .flex()
            .gap(frame.px(6.0))
            .children(swatches.iter().enumerate().map(|(index, hex)| {
                div()
                    .size(frame.px(19.0))
                    .rounded_full()
                    .bg(solid(*hex))
                    .border_2()
                    .border_color(if index == selected {
                        solid(0xff_ff_ff)
                    } else {
                        tint(0xff_ff_ff, 0.0)
                    })
            })),
    )
    .child(text(frame, 10.0, muted(), tr!("Change Icon…")))
    .child(text(frame, 10.0, muted(), tr!("Set Logo…")))
}
