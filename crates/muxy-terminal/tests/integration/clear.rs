use muxy_terminal::{Size, Terminal};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;
const SIZE: Size = Size { cols: 20, rows: 5 };

fn text(terminal: &mut Terminal) -> std::result::Result<Vec<String>, muxy_terminal::TerminalError> {
    Ok(terminal
        .screen()?
        .iter()
        .map(|row| {
            row.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect())
}

#[test]
fn clear_removes_screen_and_history_but_preserves_the_current_line() -> Result {
    let mut terminal = Terminal::new(SIZE, 1024 * 1024)?;
    for _ in 0..20 {
        terminal.feed(b"old output\r\n");
    }
    terminal.feed(b"prompt> input");
    let generation = terminal.history_generation()?;
    let column = terminal.cursor()?.col;
    assert!(terminal.clear_screen()?);
    assert_eq!(terminal.history_rows()?, 0);
    assert_ne!(terminal.history_generation()?, generation);
    assert_eq!(text(&mut terminal)?, vec!["prompt> input", "", "", "", ""]);
    assert_eq!(terminal.cursor()?.row, 0);
    assert_eq!(terminal.cursor()?.col, column);
    assert!(terminal.take_pty_output().is_empty());
    assert!(terminal.archive()?.history.is_empty());
    assert_eq!(terminal.take_changed_rows()?.len(), usize::from(SIZE.rows));
    terminal.feed(b"X");
    assert_eq!(text(&mut terminal)?[0], "prompt> inputX");
    Ok(())
}

#[test]
fn clear_is_inert_on_the_alternate_screen() -> Result {
    let mut terminal = Terminal::new(SIZE, 1024 * 1024)?;
    for _ in 0..20 {
        terminal.feed(b"history\r\n");
    }
    let primary = terminal.archive()?;
    terminal.feed(b"\x1b[?1049hfullscreen\x1b[?2004h\x1b[?1000h");
    let screen = terminal.screen()?;
    let cursor = terminal.cursor()?;
    let modes = terminal.input_modes()?;
    assert!(!terminal.clear_screen()?);
    assert_eq!(terminal.screen()?, screen);
    assert_eq!(terminal.cursor()?, cursor);
    assert_eq!(terminal.input_modes()?, modes);
    assert!(terminal.take_pty_output().is_empty());
    terminal.feed(b"\x1b[?1049l");
    assert_eq!(terminal.archive()?.history, primary.history);
    Ok(())
}

#[test]
fn clear_does_not_interrupt_partial_output_sequences() -> Result {
    for (first, last) in [
        (b"\x1b[31".as_slice(), b"m".as_slice()),
        (b"\x1b]2;title", b"\x1b\\"),
        (b"\x1bP$q", b"m\x1b\\"),
        (b"\x1b_ignored", b"\x1b\\"),
        (b"\x1b(", b"B"),
        (&"界".as_bytes()[..1], &"界".as_bytes()[1..]),
        (&"👩".as_bytes()[..2], &"👩".as_bytes()[2..]),
    ] {
        let mut terminal = Terminal::new(SIZE, 1024 * 1024)?;
        let mut uninterrupted = Terminal::new(SIZE, 1024 * 1024)?;
        for term in [&mut terminal, &mut uninterrupted] {
            term.feed(b"old\r\ncurrent");
            term.feed(first);
        }
        assert!(terminal.clear_screen()?);
        terminal.feed(last);
        uninterrupted.feed(last);
        assert_eq!(text(&mut terminal)?[0], text(&mut uninterrupted)?[1]);
        assert_eq!(terminal.take_pty_output(), uninterrupted.take_pty_output());
        assert_eq!(terminal.take_events(), uninterrupted.take_events());
    }
    Ok(())
}

#[test]
fn clear_at_a_semantic_prompt_requests_a_shell_redraw_without_retaining_old_output() -> Result {
    let mut terminal = Terminal::new(SIZE, 1024 * 1024)?;
    for _ in 0..20 {
        terminal.feed(b"old output\r\n");
    }
    terminal.feed(b"\x1b]133;A\x07prompt\r\n> \x1b]133;B\x07multi-line input");
    assert!(terminal.clear_screen()?);
    assert_eq!(terminal.take_pty_output(), b"\x0c");
    assert_eq!(terminal.history_rows()?, 0);
    assert!(text(&mut terminal)?.iter().all(String::is_empty));
    terminal.feed(b"\x1b[H\x1b[2J\x1b]133;A\x07prompt\r\n> \x1b]133;B\x07multi-line input");
    assert_eq!(terminal.history_rows()?, 0);
    assert_eq!(text(&mut terminal)?[..2], ["prompt", "> multi-line input"]);
    Ok(())
}

#[test]
fn clear_discards_inline_graphics() -> Result {
    let mut terminal = Terminal::new(SIZE, 1024 * 1024)?;
    terminal.feed(b"\x1b_Ga=T,f=24,s=1,v=1;AAAA\x1b\\");
    assert!(!terminal.graphics()?.images.is_empty());
    terminal.take_pty_output();
    assert!(terminal.clear_screen()?);
    assert!(terminal.graphics()?.images.is_empty());
    assert!(terminal.graphics()?.placements.is_empty());
    assert!(terminal.take_pty_output().is_empty());
    Ok(())
}
