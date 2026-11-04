use muxy_terminal::{Size, Terminal};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn prompt_rows_survive_history_reads_compression_and_screen_switches() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 40, rows: 4 }, 1024 * 1024)?;
    for _ in 0..10 {
        terminal
            .feed(b"\x1b]133;A\x07$ \x1b]133;B\x07echo hi\r\n\x1b]133;C\x07hi\r\n\x1b]133;D;0\x07");
    }
    terminal.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07");
    let count = terminal.history_rows()?;
    let screen = terminal.screen()?;
    let prompts = terminal.screen_prompts()?;
    assert_eq!(prompts, [1, 3]);
    terminal.take_changed_rows()?;
    terminal.compress_idle()?;
    let (rows, history_prompts) = terminal.history_with_prompts(3..count)?;
    assert_eq!(rows.len(), count - 3);
    assert_eq!(
        history_prompts,
        (1..count - 3)
            .step_by(2)
            .map(|row| u16::try_from(row).unwrap_or(u16::MAX))
            .collect::<Vec<_>>()
    );
    assert_eq!(terminal.screen()?, screen);
    assert_eq!(terminal.screen_prompts()?, prompts);
    assert!(terminal.take_changed_rows()?.is_empty());
    terminal.feed(b"\x1b[?1049h");
    assert!(terminal.screen_prompts()?.is_empty());
    terminal.feed(b"\x1b[?1049l");
    assert_eq!(terminal.screen_prompts()?, prompts);
    terminal.feed(b"\x1b[2J\x1b[H");
    assert!(terminal.screen_prompts()?.is_empty());
    Ok(())
}

#[test]
fn resizing_a_redrawn_prompt_preserves_preceding_output() -> TestResult {
    for marker in [
        "",
        "\x1b]133;A\x07",
        "\x1b]133;A;redraw=1\x07",
        "\x1b]133;A;redraw=last\x07",
    ] {
        let mut terminal = Terminal::new(Size { cols: 34, rows: 8 }, 1024 * 1024)?;
        let end = if marker.is_empty() {
            ""
        } else {
            "\x1b]133;B\x07"
        };
        let prompt = format!("{marker}muxy@host-with-a-long-name ~ $ {end}");
        terminal.feed(b"awake-42\r\nlater-48\r\nwindow-b-32\r\n");
        terminal.feed(prompt.as_bytes());
        terminal.resize(Size { cols: 24, rows: 8 })?;
        terminal.feed(b"\r\x1b[J");
        terminal.feed(prompt.as_bytes());
        terminal.resize(Size { cols: 71, rows: 8 })?;
        terminal.feed(b"\r\x1b[A\x1b[J");
        terminal.feed(prompt.as_bytes());
        let rows: Vec<String> = terminal
            .screen()?
            .iter()
            .map(|row| row.runs.iter().map(|run| run.text.as_str()).collect())
            .collect();
        assert_eq!(rows[0], "awake-42", "{marker:?}");
        assert_eq!(rows[1], "later-48", "{marker:?}");
        assert_eq!(rows[2], "window-b-32", "{marker:?}");
        assert_eq!(rows[3], "muxy@host-with-a-long-name ~ $", "{marker:?}");
        assert!(rows[4..].iter().all(String::is_empty), "{marker:?}");
    }
    Ok(())
}

#[test]
fn resizing_a_prompt_on_a_hard_newline_does_not_duplicate_it() -> TestResult {
    for marker in [
        "\x1b]133;A\x07",
        "\x1b]133;A;redraw=1\x07",
        "\x1b]133;A;redraw=last\x07",
    ] {
        let mut terminal = Terminal::new(Size { cols: 34, rows: 8 }, 1024 * 1024)?;
        let prompt = format!("{marker}muxy@Saeeds-Virtual-Machine ~ $ \x1b]133;B\x07");
        terminal.feed(b"awake-42\r\nlater-48\r\nwindow-b-32\r\n");
        terminal.feed(prompt.as_bytes());
        terminal.resize(Size { cols: 32, rows: 8 })?;
        terminal.feed(b"\r\x1b[J");
        terminal.feed(prompt.as_bytes());
        terminal.feed(b"\r\n\x1b[K");
        for cols in [31, 71] {
            terminal.resize(Size { cols, rows: 8 })?;
            terminal.feed(b"\r\x1b[A\x1b[J");
            terminal.feed(prompt.as_bytes());
        }
        let rows: Vec<String> = terminal
            .screen()?
            .iter()
            .map(|row| row.runs.iter().map(|run| run.text.as_str()).collect())
            .collect();
        assert_eq!(
            rows,
            [
                "awake-42",
                "later-48",
                "window-b-32",
                "muxy@Saeeds-Virtual-Machine ~ $",
                "",
                "",
                "",
                "",
            ],
            "{marker:?}"
        );
    }
    Ok(())
}

#[test]
fn wrapped_prompt_has_only_one_navigation_stop() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 8, rows: 8 }, 1024 * 1024)?;
    terminal.feed(b"\x1b]133;A\x07a long prompt> \x1b]133;B\x07echo hi");
    assert_eq!(terminal.screen_prompts()?, [0]);
    terminal.resize(Size { cols: 4, rows: 8 })?;
    assert_eq!(terminal.screen_prompts()?, [0]);
    Ok(())
}
