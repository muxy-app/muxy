use std::collections::BTreeMap;

use muxy_protocol::ScreenFrame;

pub(super) fn merge(older: &mut ScreenFrame, newer: ScreenFrame) {
    if newer.reset {
        *older = newer;
        return;
    }
    let mut rows: BTreeMap<_, _> = older.rows.drain(..).map(|row| (row.index, row)).collect();
    rows.extend(newer.rows.into_iter().map(|row| (row.index, row)));
    older.rows = rows.into_values().collect();
    older.seq = newer.seq;
    older.size = newer.size;
    older.cursor = newer.cursor;
    older.modes = newer.modes;
    if newer.graphics.is_some() {
        older.graphics = newer.graphics;
    }
}

#[cfg(test)]
mod tests {
    use muxy_protocol::{Cursor, Modes, Row};

    use super::*;

    pub(super) fn frame(seq: u64, reset: bool, indexes: &[u16]) -> ScreenFrame {
        ScreenFrame {
            size: muxy_protocol::Size { cols: 80, rows: 24 },
            graphics: None,
            seq,
            reset,
            rows: indexes
                .iter()
                .map(|&index| Row {
                    index,
                    runs: vec![],
                })
                .collect(),
            cursor: Cursor {
                shape: muxy_protocol::CursorShape::default(),
                row: 0,
                col: 0,
                visible: true,
            },
            modes: Modes {
                application_cursor_keys: false,
                bracketed_paste: false,
            },
        }
    }

    #[test]
    fn newer_rows_cursor_modes_and_sequence_win() {
        let mut older = frame(1, false, &[0, 1]);
        older.rows[1].runs.push(muxy_protocol::Run {
            text: "old".into(),
            width: 3,
            style: muxy_protocol::Style::default(),
        });
        let mut newer = frame(3, false, &[1, 2]);
        newer.cursor.col = 4;
        newer.modes.bracketed_paste = true;
        let expected = ScreenFrame {
            rows: frame(3, false, &[0, 1, 2]).rows,
            ..newer.clone()
        };
        merge(&mut older, newer);
        assert_eq!(older, expected);
    }

    #[test]
    fn newer_reset_drops_old_rows_and_survives_later_deltas() {
        let mut older = frame(1, false, &[0, 5]);
        let mut reset = frame(2, true, &[0, 1]);
        reset.size = muxy_protocol::Size { cols: 20, rows: 2 };
        merge(&mut older, reset.clone());
        assert_eq!(older, reset);
        let mut delta = frame(4, false, &[1]);
        delta.size = reset.size;
        merge(&mut older, delta);
        reset.seq = 4;
        assert_eq!(older, reset);
    }
    #[test]
    fn image_snapshots_survive_text_deltas_and_explicit_deletion() {
        let mut older = frame(1, false, &[]);
        let graphics = muxy_protocol::Graphics {
            cell: muxy_protocol::CellSize {
                width: 16,
                height: 32,
            },
            ..muxy_protocol::Graphics::default()
        };
        older.graphics = Some(graphics.clone());
        merge(&mut older, frame(2, false, &[1]));
        assert_eq!(older.graphics, Some(graphics));
        let mut deletion = frame(3, false, &[]);
        deletion.graphics = Some(muxy_protocol::Graphics::default());
        merge(&mut older, deletion);
        assert_eq!(older.graphics, Some(muxy_protocol::Graphics::default()));
    }
}
