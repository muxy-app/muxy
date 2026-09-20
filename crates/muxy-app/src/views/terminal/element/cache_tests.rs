#![allow(clippy::unwrap_used)]

use std::io::Write;

use super::*;
use gpui::AppContext;
use muxy_protocol::{Cursor, CursorShape, Row, SavedScreen};

fn pane(cx: &mut gpui::Context<TerminalPane>, height: u16) -> TerminalPane {
    let mut pane = TerminalPane::new(
        Palette::new(true),
        muxy_app_core::settings::TerminalSettings::default(),
        cx,
    );
    pane.grid = Some(muxy_client::RunGrid::from_saved(SavedScreen {
        graphics: muxy_protocol::Graphics::default(),
        size: Size {
            cols: 120,
            rows: height,
        },
        rows: (0..height)
            .map(|index| Row {
                index,
                runs: vec![Run {
                    text: format!("{index:03} {}", "terminal output ".repeat(7)),
                    width: 116,
                    style: Style::default(),
                }],
            })
            .collect(),
        cursor: Cursor {
            row: 0,
            col: 0,
            visible: true,
            shape: CursorShape::default(),
        },
        reason: None,
    }));
    pane
}

fn frame(pane: &TerminalPane, window: &mut Window) -> Painting {
    prepare(
        pane,
        point(px(0.0), px(0.0)),
        size(px(8.0), px(16.0)),
        &pane.palette,
        &font("Menlo"),
        px(13.0),
        window,
    )
}

#[gpui::test]
fn animation_prepares_only_changed_rows_and_cursor_blink_reuses_text(
    cx: &mut gpui::TestAppContext,
) {
    let pane = cx.new(|cx| pane(cx, 40));
    cx.add_empty_window().update(|window, cx| {
        pane.update(cx, |pane, _| {
            let first = frame(pane, window);
            pane.grid.as_mut().unwrap().rows[20][0]
                .text
                .replace_range(..1, "x");
            let next = frame(pane, window);
            assert_eq!(next.rows.len(), 40);
            for (index, (before, after)) in first.rows.iter().zip(&next.rows).enumerate() {
                assert_eq!(Rc::ptr_eq(before, after), index != 20, "row {index}");
            }
            pane.grid.as_mut().unwrap().cursor.visible = true;
            pane.cursor_blink.visible = false;
            let hidden = frame(pane, window);
            assert!(hidden.cursor.is_none());
            pane.cursor_blink.visible = true;
            let shown = frame(pane, window);
            assert!(shown.cursor.is_some());
            for (before, after) in hidden.rows.iter().zip(&shown.rows) {
                assert!(Rc::ptr_eq(before, after));
            }
            pane.grid.as_mut().unwrap().cursor.row = 1;
            let moved = frame(pane, window);
            for (index, (before, after)) in shown.rows.iter().zip(&moved.rows).enumerate() {
                assert_eq!(Rc::ptr_eq(before, after), index > 1, "cursor row {index}");
            }
        });
    });
}

#[gpui::test]
fn row_cache_tracks_styles_selection_scrolling_and_render_configuration(
    cx: &mut gpui::TestAppContext,
) {
    use super::super::selection::{Point as CellPoint, Selection};
    let pane = cx.new(|cx| pane(cx, 3));
    cx.add_empty_window().update(|window, cx| {
        pane.update(cx, |pane, _| {
            let first = frame(pane, window);
            pane.grid.as_mut().unwrap().rows[1][0].style.bold = true;
            let styled = frame(pane, window);
            assert!(Rc::ptr_eq(&first.rows[0], &styled.rows[0]));
            assert!(!Rc::ptr_eq(&first.rows[1], &styled.rows[1]));
            pane.palette.selection_foreground =
                Some(muxy_app_core::settings::TerminalColor::Rgb(0xff_0000));
            let unselected = frame(pane, window);
            pane.selection = Some(Selection {
                anchor: CellPoint { row: 1, column: 1 },
                head: CellPoint { row: 1, column: 4 },
            });
            let selected = frame(pane, window);
            assert!(Rc::ptr_eq(&unselected.rows[0], &selected.rows[0]));
            assert!(!Rc::ptr_eq(&unselected.rows[1], &selected.rows[1]));
            pane.selection = None;
            let cleared = frame(pane, window);
            assert!(!Rc::ptr_eq(&selected.rows[1], &cleared.rows[1]));
            pane.grid.as_mut().unwrap().rows.swap(1, 2);
            let scrolled = frame(pane, window);
            assert!(!Rc::ptr_eq(&cleared.rows[1], &scrolled.rows[1]));
            assert!(!Rc::ptr_eq(&cleared.rows[2], &scrolled.rows[2]));
            let mut previous = scrolled;
            for change in 0..7 {
                match change {
                    0 => pane.palette.foreground ^= 0xff,
                    1 => pane.terminal.font.features.push(("liga".into(), 0)),
                    2 => pane.terminal.font.thicken = true,
                    _ => {}
                }
                let next = prepare(
                    pane,
                    point(px(if change >= 3 { 5.0 } else { 0.0 }), px(0.0)),
                    size(px(if change >= 4 { 9.0 } else { 8.0 }), px(16.0)),
                    &pane.palette,
                    &font(if change >= 5 { "Monaco" } else { "Menlo" }),
                    px(if change >= 6 { 14.0 } else { 13.0 }),
                    window,
                );
                for (before, after) in previous.rows.iter().zip(&next.rows) {
                    assert!(!Rc::ptr_eq(before, after), "change {change}");
                }
                previous = next;
            }
        });
    });
}

#[gpui::test]
fn row_cache_releases_removed_content(cx: &mut gpui::TestAppContext) {
    let pane = cx.new(|cx| pane(cx, 3));
    cx.add_empty_window().update(|window, cx| {
        pane.update(cx, |pane, _| {
            let painting = frame(pane, window);
            let removed = Rc::downgrade(&painting.rows[2]);
            let retained = Rc::downgrade(&painting.rows[0]);
            drop(painting);
            pane.grid.as_mut().unwrap().size.rows = 1;
            drop(frame(pane, window));
            assert!(removed.upgrade().is_none());
            assert!(retained.upgrade().is_some());
            pane.grid = None;
            drop(frame(pane, window));
            assert!(retained.upgrade().is_none());
        });
    });
}

#[gpui::test]
#[ignore = "manual CPU benchmark; uses the headless GPUI test platform"]
fn terminal_row_preparation_benchmark(cx: &mut gpui::TestAppContext) {
    let pane = cx.new(|cx| pane(cx, 60));
    cx.add_empty_window().update(|window, cx| {
        pane.update(cx, |pane, _| {
            for cached in [false, true] {
                pane.rows.borrow_mut().bypass = !cached;
                drop(frame(pane, window));
                let start = std::time::Instant::now();
                for tick in 0..600 {
                    pane.grid.as_mut().unwrap().rows[30][0]
                        .text
                        .replace_range(..1, if tick % 2 == 0 { "x" } else { "y" });
                    std::hint::black_box(frame(pane, window));
                }
                writeln!(
                    std::io::stderr(),
                    "60x120, 600 single-row updates, cached={cached}: {:?}",
                    start.elapsed()
                )
                .unwrap();
            }
        });
    });
}
