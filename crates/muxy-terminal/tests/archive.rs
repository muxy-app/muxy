use std::error::Error;

use muxy_terminal::{Color, Run, Size, Terminal};

type TestResult = Result<(), Box<dyn Error>>;
const SIZE: Size = Size { cols: 20, rows: 4 };

fn text(runs: &[Run]) -> String {
    runs.iter().map(|run| run.text.as_str()).collect::<String>()
}

#[test]
fn capture_preserves_history_styles_wide_text_and_the_next_frame() -> TestResult {
    let mut terminal = Terminal::new(SIZE, 1024 * 1024)?;
    terminal.feed(b"\x1b[31mone\r\ntwo\r\nthree\r\nfour\r\nfive\r\n");
    terminal.feed("\x1b[32m界e\u{301}".as_bytes());
    let before = terminal.screen()?;
    let cursor = terminal.cursor()?;
    let modes = terminal.modes()?;
    let archive = terminal.archive()?;
    assert_eq!(archive.size, SIZE);
    assert_eq!(archive.rows, before);
    assert_eq!(archive.cursor, cursor);
    assert_eq!(archive.history.len(), terminal.history_rows()?);
    assert!(text(&archive.history[0]).starts_with("one"));
    assert_eq!(archive.history[0][0].style.fg, Color::Indexed(1));
    assert!(
        archive
            .rows
            .iter()
            .any(|row| text(&row.runs).contains("界e\u{301}"))
    );
    assert_eq!(terminal.modes()?, modes);
    assert_eq!(terminal.take_changed_rows()?, before);
    assert!(terminal.take_changed_rows()?.is_empty());
    terminal.archive()?;
    assert!(terminal.take_changed_rows()?.is_empty());
    terminal.feed(b"\x1b[1;1Hchanged");
    let changed = terminal.take_changed_rows()?;
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].index, 0);
    Ok(())
}

#[test]
fn alternate_screen_keeps_primary_history_across_fragmented_mode_sequences() -> TestResult {
    for entry in [
        b"\x1b[?1049h".as_slice(),
        b"\x1b[?1;47h",
        b"\x1b[?1047h",
        b"\x1b[?1049\x07h",
    ] {
        let mut terminal = Terminal::new(SIZE, 1024 * 1024)?;
        terminal.feed(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n");
        let primary = terminal.archive()?;
        for byte in entry {
            terminal.feed(&[*byte]);
        }
        terminal.feed(b"\x1b[2J\x1b[Halternate");
        let cursor = terminal.cursor()?;
        let modes = terminal.modes()?;
        let archive = terminal.archive()?;
        assert!(text(&archive.rows[0].runs).starts_with("alternate"));
        assert_eq!(archive.history, primary.history);
        assert_eq!(terminal.cursor()?, cursor);
        assert_eq!(terminal.modes()?, modes);
        terminal.feed(b"\x1b[?1049l");
        assert_eq!(terminal.archive()?.history, primary.history);
    }
    Ok(())
}

#[test]
fn same_write_output_and_alternate_entry_save_the_new_primary_history() -> TestResult {
    let mut terminal = Terminal::new(SIZE, 1024 * 1024)?;
    terminal.feed(b"first\r\nsecond\r\nthird\r\nfourth\r\nfifth\r\n\x1b[?1049h\x1b[Htop");
    let archive = terminal.archive()?;
    assert!(text(&archive.history[0]).starts_with("first"));
    assert!(text(&archive.rows[0].runs).starts_with("top"));
    terminal.feed(b"\x1b[?1049lmore");
    assert!(
        terminal
            .screen()?
            .iter()
            .any(|row| text(&row.runs).contains("more"))
    );
    Ok(())
}
