#[path = "../src/views/terminal/selection.rs"]
mod selection;

use muxy_client::RunGrid;
use muxy_protocol::{
    AttachSnapshot, ChannelId, Cursor, HistoryPage, Modes, Row, Run, ServerPath, Size, Style,
};
use selection::{Point, Selection};

fn run(text: &str, width: u16) -> Run {
    Run {
        text: text.into(),
        width,
        style: Style::default(),
    }
}

fn grid() -> RunGrid {
    RunGrid::from_snapshot(&AttachSnapshot {
        links: None,
        graphics: muxy_protocol::Graphics::default(),
        prompts: Vec::new(),
        channel: ChannelId(1),
        size: Size { cols: 20, rows: 3 },
        rows: vec![
            Row {
                index: 0,
                runs: vec![run("one, two.three   ", 17)],
            },
            Row {
                index: 1,
                runs: vec![
                    run("A", 1),
                    run("界", 2),
                    run("e\u{301}", 1),
                    run("👩‍💻", 2),
                    run(" Z   ", 5),
                ],
            },
            Row {
                index: 2,
                runs: vec![run("last   ", 7)],
            },
        ],
        cursor: Cursor {
            shape: muxy_protocol::CursorShape::default(),
            row: 0,
            col: 0,
            visible: true,
        },
        modes: Modes::default(),
        title: String::new(),
        directory: ServerPath(Vec::new()),
        history: vec![Row {
            index: 0,
            runs: vec![run("older   ", 8)],
        }],
        history_cursor: None,
        history_total: 1,
        history_graphics: None,
    })
}

fn selection(a: (isize, u16), b: (isize, u16)) -> Selection {
    Selection {
        anchor: Point {
            row: a.0,
            column: a.1,
        },
        head: Point {
            row: b.0,
            column: b.1,
        },
    }
}

#[test]
fn normalization_and_extraction_match_in_both_directions_across_history() {
    let forward = selection((-1, 0), (0, 8));
    let backward = selection((0, 8), (-1, 0));
    assert_eq!(forward.normalized(), backward.normalized());
    assert_eq!(forward.text(&grid()), "older\none, two");
    assert_eq!(forward.text(&grid()), backward.text(&grid()));
    assert_eq!(selection((0, 0), (0, 0)).text(&grid()), "");
    assert_eq!(selection((0, 0), (1, 0)).text(&grid()), "one, two.three");
}

#[test]
fn wide_cells_and_combining_clusters_are_selected_atomically() {
    let grid = grid();
    for (start, end, expected, columns) in [
        (1, 2, "界", 1..3),
        (2, 3, "界", 1..3),
        (3, 4, "e\u{301}", 3..4),
        (5, 6, "👩‍💻", 4..6),
    ] {
        let selected = selection((1, start), (1, end));
        assert_eq!(selected.text(&grid), expected);
        assert_eq!(selected.columns(1, &grid), columns);
    }
    assert_eq!(
        selection((1, 0), (2, 20)).text(&grid),
        "A界e\u{301}👩‍💻 Z\nlast"
    );
}

#[test]
fn words_stop_at_whitespace_and_punctuation_and_rows_trim_padding() {
    let grid = grid();
    for (column, expected) in [
        (0, "one"),
        (3, ","),
        (4, ""),
        (6, "two"),
        (8, "."),
        (10, "three"),
    ] {
        assert_eq!(
            Selection::word(Point { row: 0, column }, &grid, "").text(&grid),
            expected
        );
    }
    assert_eq!(
        Selection::word(Point { row: 1, column: 2 }, &grid, "").text(&grid),
        "A界e\u{301}"
    );
    assert_eq!(
        Selection::row(Point { row: 1, column: 5 }, &grid).text(&grid),
        "A界e\u{301}👩‍💻 Z"
    );
}

#[test]
fn default_word_selection_keeps_paths_and_urls_together_in_screen_and_history()
-> Result<(), std::num::TryFromIntError> {
    let options = muxy_app_core::settings::TerminalOptions::default();
    for text in [
        "/Users/me/src/project",
        "https://example.com/a-b",
        "~/src/my_project/main.rs",
        "../src/project/file-name.txt",
        "user@example.com:/srv/project",
        "https://example.com/a%20b?q=one+two&lang=en#section",
    ] {
        let line = format!("before (\"{text}\"), after");
        let mut grid = grid();
        grid.size.cols = u16::try_from(line.len())?;
        grid.rows[0] = vec![run(&line, grid.size.cols)];
        grid.history[0].runs = grid.rows[0].clone();
        let end = 9 + u16::try_from(text.len())?;
        for row in [-1, 0] {
            for column in 9..end {
                let selected =
                    Selection::word(Point { row, column }, &grid, &options.selection_word_chars);
                assert_eq!(selected.text(&grid), text, "row {row}, column {column}");
                assert_eq!(selected.columns(row, &grid), 9..end);
            }
            for (column, expected) in [(7, "("), (8, "\""), (end, "\""), (end + 2, ",")] {
                assert_eq!(
                    Selection::word(Point { row, column }, &grid, &options.selection_word_chars)
                        .text(&grid),
                    expected
                );
            }
        }
    }
    Ok(())
}

#[test]
fn custom_word_characters_replace_defaults_and_never_join_whitespace() {
    let mut grid = grid();
    grid.rows[0] = vec![run("a/b-c_d x_y", 11)];
    for (word_chars, column, expected) in [
        ("", 2, "b"),
        ("", 1, "/"),
        ("_", 6, "c_d"),
        ("_", 1, "/"),
        ("/-_", 1, "a/b-c_d"),
        ("/-_ ", 6, "a/b-c_d"),
        ("/-_ ", 8, "x_y"),
    ] {
        assert_eq!(
            Selection::word(Point { row: 0, column }, &grid, word_chars).text(&grid),
            expected
        );
    }
    let whitespace = Selection::word(Point { row: 0, column: 7 }, &grid, "_ ");
    assert_eq!(whitespace.columns(0, &grid), 7..8);
}

#[test]
fn word_characters_work_across_styled_runs_and_unicode_cells() {
    let mut grid = grid();
    let mut styled = run("/tmp/", 5);
    styled.style.bold = true;
    grid.rows[0] = vec![
        styled,
        run("界", 2),
        run("/", 1),
        run("e\u{301}", 1),
        run("-", 1),
        run("👩‍💻", 2),
        run(" ", 1),
    ];
    for column in 0..12 {
        let selected = Selection::word(Point { row: 0, column }, &grid, "/-👩");
        assert_eq!(selected.text(&grid), "/tmp/界/e\u{301}-👩‍💻");
        assert_eq!(selected.columns(0, &grid), 0..12);
    }
    assert_eq!(
        Selection::word(Point { row: 0, column: 11 }, &grid, "/-").text(&grid),
        "👩‍💻"
    );
}

#[test]
fn older_pages_do_not_move_selection_and_selected_rows_detect_changes() {
    let mut grid = grid();
    let selected = selection((-1, 0), (0, 3));
    let before = selected.rows(&grid);
    grid.fetch_older(HistoryPage {
        links: None,
        prompts: Vec::new(),
        rows: vec![Row {
            index: 0,
            runs: vec![run("oldest", 6)],
        }],
        next: None,
        total_rows: 2,
        screen: None,
        graphics: None,
    });
    assert_eq!(selected.text(&grid), "older\none");
    assert_eq!(selected.rows(&grid), before);
    grid.rows[2] = vec![run("elsewhere", 9)];
    assert_eq!(selected.rows(&grid), before);
    grid.rows[0] = vec![run("changed", 7)];
    assert_ne!(selected.rows(&grid), before);
}
