//! Layouts written before rendering kept cursor shapes, graphics and underline
//! styles. Frozen.

use muxy_protocol::Color;
use serde::Deserialize;

use super::layout::{StoredExitReason, StoredSize};

#[derive(Deserialize)]
pub(super) struct Record {
    /// Checked before the rest is read.
    _version: u32,
    pub(super) screen: Screen,
    pub(super) history: Vec<Vec<Run>>,
}

#[derive(Deserialize)]
pub(super) struct Screen {
    size: StoredSize,
    rows: Vec<Row>,
    cursor: Cursor,
    reason: Option<StoredExitReason>,
}

#[derive(Deserialize)]
struct Row {
    index: u16,
    runs: Vec<Run>,
}

#[derive(Deserialize)]
pub(super) struct Run {
    text: String,
    width: u16,
    style: Style,
}

#[derive(Deserialize)]
struct Cursor {
    row: u16,
    col: u16,
    visible: bool,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
struct Style {
    fg: Color,
    bg: Color,
    bold: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
    strikethrough: bool,
    faint: bool,
}

impl From<Screen> for muxy_protocol::SavedScreen {
    fn from(screen: Screen) -> Self {
        Self {
            graphics: muxy_protocol::Graphics::default(),
            size: screen.size.into(),
            rows: screen
                .rows
                .into_iter()
                .map(|row| muxy_protocol::Row {
                    index: row.index,
                    runs: row.runs.into_iter().map(Into::into).collect(),
                })
                .collect(),
            cursor: muxy_protocol::Cursor {
                row: screen.cursor.row,
                col: screen.cursor.col,
                visible: screen.cursor.visible,
                shape: muxy_protocol::CursorShape::Block,
            },
            reason: screen.reason.map(Into::into),
        }
    }
}

impl From<Run> for muxy_protocol::Run {
    fn from(run: Run) -> Self {
        let style = run.style;
        Self {
            text: run.text,
            width: run.width,
            style: muxy_protocol::Style {
                fg: style.fg,
                bg: style.bg,
                bold: style.bold,
                italic: style.italic,
                inverse: style.inverse,
                faint: style.faint,
                strikethrough: style.strikethrough,
                underline: if style.underline {
                    muxy_protocol::Underline::Single
                } else {
                    muxy_protocol::Underline::None
                },
                ..muxy_protocol::Style::default()
            },
        }
    }
}
