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
