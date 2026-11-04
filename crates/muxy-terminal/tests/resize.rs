use muxy_terminal::{Row, Size, Style, Terminal, TerminalError};

type TestResult = Result<(), TerminalError>;

fn texts(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|row| row.runs.iter().map(|run| run.text.as_str()).collect())
        .collect()
}

#[test]
fn resizing_keeps_the_cursor_line_rows_even_when_the_cursor_is_in_the_middle() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 8, rows: 4 }, 1024 * 1024)?;
    terminal.feed(b"out\r\n0123456789abcdef\x1b[2;3H");
    terminal.resize(Size { cols: 5, rows: 4 })?;
    assert_eq!(texts(&terminal.screen()?), ["out", "01234", "89abc", ""]);
    assert_eq!((terminal.cursor()?.row, terminal.cursor()?.col), (1, 2));
    terminal.resize(Size { cols: 16, rows: 4 })?;
    assert_eq!(texts(&terminal.screen()?), ["out", "01234", "89abc", ""]);
    assert_eq!((terminal.cursor()?.row, terminal.cursor()?.col), (1, 2));
    Ok(())
}

#[test]
fn resizing_a_prompt_after_wrapped_output_preserves_physical_rows() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 8, rows: 4 }, 1024 * 1024)?;
    terminal.feed(b"abcdefghij\x1b]133;P;k=i\x07$ \x1b]133;B\x07");
    terminal.resize(Size { cols: 16, rows: 4 })?;
    assert_eq!(texts(&terminal.screen()?), ["abcdefgh", "ij$", "", ""]);
    assert_eq!((terminal.cursor()?.row, terminal.cursor()?.col), (1, 4));
    terminal.feed(b"\r\x1b[2C\x1b[K\x1b]133;P;k=i\x07$ \x1b]133;B\x07");
    assert_eq!(texts(&terminal.screen()?), ["abcdefgh", "ij$", "", ""]);
    Ok(())
}

#[test]
fn resizing_keeps_empty_continuation_rows_in_the_cursor_line() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 8, rows: 6 }, 1024 * 1024)?;
    terminal.feed(b"out\r\nabcdefghijklmnopqrstuvwx\x1b[3;8H\x1b[1K\x1b[4;1H");
    assert_eq!(
        texts(&terminal.screen()?),
        ["out", "abcdefgh", "", "qrstuvwx", "", ""]
    );
    for cols in [5, 16] {
        terminal.resize(Size { cols, rows: 6 })?;
        assert_eq!(
            texts(&terminal.screen()?),
            ["out", "abcde", "", "qrstu", "", ""],
            "{cols}"
        );
        assert_eq!((terminal.cursor()?.row, terminal.cursor()?.col), (3, 0));
    }
    Ok(())
}

#[test]
fn resizing_preserves_completed_output_across_history_pages_and_height_changes() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 30, rows: 4 }, 1024 * 1024)?;
    let output = "0123456789abcdef".repeat(500);
    terminal.feed(output.as_bytes());
    terminal.feed(b"\r\n$ ");
    for size in [
        Size { cols: 7, rows: 4 },
        Size { cols: 51, rows: 2 },
        Size { cols: 3, rows: 7 },
        Size { cols: 80, rows: 6 },
    ] {
        terminal.compress_idle()?;
        terminal.resize(size)?;
        let count = terminal.history_rows()?;
        let mut rows = terminal.history(0..count)?;
        rows.extend(terminal.screen()?);
        assert_eq!(texts(&rows).concat(), format!("{output}$"), "{size:?}");
        assert!(texts(&rows).iter().any(|row| row == "$"), "{size:?}");
        assert!(terminal.cursor()?.row < size.rows);
        assert!(terminal.cursor()?.col < size.cols);
    }
    Ok(())
}

#[test]
fn resizing_a_cursor_line_keeps_styles_links_and_whole_graphemes() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 8, rows: 4 }, 1024 * 1024)?;
    terminal.feed("ok\r\n\x1b[31m\x1b]8;;https://example.com\x1b\\ab界e\u{301}12345".as_bytes());
    for cols in [3, 12] {
        terminal.resize(Size { cols, rows: 4 })?;
        let screen = terminal.take_changed_rows()?;
        assert_eq!(texts(&screen), ["ok", "ab", "45", ""]);
        assert_ne!(screen[1].runs[0].style, Style::default());
        let links = terminal.screen_links();
        assert_eq!(links.len(), 2);
        assert!(links.iter().all(|row| row.spans.iter().all(|span| {
            span.start == 0 && span.end == 2 && span.uri == "https://example.com"
        })));
    }
    Ok(())
}

#[test]
fn resizing_reflows_the_inactive_primary_screen_but_not_the_alternate_screen() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 8, rows: 4 }, 1024 * 1024)?;
    terminal.feed(b"abcdefghijk\x1b[?1049h\x1b[Halternate");
    terminal.resize(Size { cols: 16, rows: 4 })?;
    assert_eq!(texts(&terminal.screen()?), ["alternat", "e", "", ""]);
    terminal.feed(b"\x1b[?1049l");
    assert_eq!(texts(&terminal.screen()?), ["abcdefghijk", "", "", ""]);
    assert_eq!((terminal.cursor()?.row, terminal.cursor()?.col), (0, 11));
    Ok(())
}
