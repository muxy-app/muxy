use muxy_terminal::{Run, Size, Style, Terminal, TerminalError};

type TestResult = Result<(), TerminalError>;

const SIZE: Size = Size { cols: 20, rows: 6 };

fn terminal() -> Result<Terminal, TerminalError> {
    Terminal::new(SIZE, 1 << 20)
}

fn run(text: &str, style: Style) -> Run {
    Run {
        text: text.to_owned(),
        width: u16::try_from(text.chars().count()).unwrap_or(u16::MAX),
        style,
    }
}

#[test]
fn unicode_runs_preserve_cells_across_grapheme_mode_changes() -> TestResult {
    for (input, expected) in [
        ("لاx", vec![("ل", 1), ("ا", 1), ("x", 1)]),
        ("👩‍💻x", vec![("👩‍", 2), ("💻", 2), ("x", 1)]),
        ("❤️x", vec![("❤️", 1), ("x", 1)]),
        ("\u{1b}[?2027h👩‍💻x", vec![("👩‍💻", 2), ("x", 1)]),
        ("\u{1b}[?2027h❤️x", vec![("❤️", 2), ("x", 1)]),
        (
            "👩‍💻\u{1b}[?2027h👩‍💻x",
            vec![("👩‍", 2), ("💻", 2), ("👩‍💻", 2), ("x", 1)],
        ),
        (
            "abce\u{301}xyz",
            vec![("abc", 3), ("e\u{301}", 1), ("xyz", 3)],
        ),
    ] {
        let mut terminal = terminal()?;
        terminal.feed(input.as_bytes());
        let screen = terminal.screen()?;
        let actual: Vec<_> = screen[0]
            .runs
            .iter()
            .map(|run| (run.text.as_str(), run.width))
            .collect();
        assert_eq!(actual, expected, "{input:?}");
        assert_eq!(
            terminal.cursor()?.col,
            expected.iter().map(|(_, width)| width).sum::<u16>()
        );
    }
    Ok(())
}

#[test]
fn changed_rows_report_only_touched_rows_then_nothing() -> TestResult {
    let mut terminal = terminal()?;
    terminal.feed(b"first");
    let initial = terminal.take_changed_rows()?;
    assert_eq!(initial.len(), usize::from(SIZE.rows));

    terminal.feed(b"\r\nsecond");
    let changed = terminal.take_changed_rows()?;
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].index, 1);
    assert_eq!(changed[0].runs, vec![run("second", Style::default())]);

    assert!(terminal.take_changed_rows()?.is_empty());
    Ok(())
}

#[test]
fn resize_returns_every_row_and_clamps_the_cursor() -> TestResult {
    let mut terminal = terminal()?;
    terminal.feed(b"one\r\ntwo\r\nthree\r\nfour");
    terminal.take_changed_rows()?;
    assert_eq!(terminal.cursor()?.row, 3);

    let smaller = Size { cols: 3, rows: 2 };
    terminal.resize(smaller)?;

    let changed = terminal.take_changed_rows()?;
    assert_eq!(changed.len(), usize::from(smaller.rows));
    let cursor = terminal.cursor()?;
    assert!(cursor.row < smaller.rows, "{cursor:?}");
    assert!(cursor.col < smaller.cols, "{cursor:?}");
    assert!(terminal.take_changed_rows()?.is_empty());
    Ok(())
}

#[test]
fn cursor_position_request_writes_a_report_to_the_pty() -> TestResult {
    let mut terminal = terminal()?;
    assert!(terminal.take_pty_output().is_empty());

    terminal.feed(b"\x1b[2;5H\x1b[6n");

    assert_eq!(terminal.take_pty_output(), b"\x1b[2;5R");
    assert!(terminal.take_pty_output().is_empty());
    Ok(())
}
