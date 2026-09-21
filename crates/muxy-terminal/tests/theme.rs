use muxy_protocol::{CursorShape, TerminalColors};
use muxy_terminal::{Size, Terminal, TerminalError};

fn defaults(background: [u8; 3]) -> TerminalColors {
    TerminalColors {
        foreground: [200; 3],
        background,
        cursor: [200; 3],
        ansi: [[40; 3]; 16],
        palette: std::collections::BTreeMap::new(),
        cursor_style: None,
        cursor_blink: None,
    }
}

fn terminal() -> Result<Terminal, TerminalError> {
    Terminal::new(Size { cols: 20, rows: 6 }, 1 << 20)
}

#[test]
fn theme_queries_follow_background_defaults_without_a_subscription() -> Result<(), TerminalError> {
    let mut terminal = terminal()?;
    for (background, expected) in [
        ([20; 3], b"\x1b[?997;1n"),
        ([240; 3], b"\x1b[?997;2n"),
        ([0, 255, 0], b"\x1b[?997;2n"),
        ([255, 0, 0], b"\x1b[?997;1n"),
    ] {
        terminal.set_defaults(&defaults(background))?;
        assert!(terminal.take_pty_output().is_empty());
        terminal.feed(b"\x1b[?996n");
        assert_eq!(terminal.take_pty_output(), expected);
    }
    Ok(())
}

#[test]
fn subscribed_tuis_receive_live_palette_changes_on_the_alternate_screen()
-> Result<(), TerminalError> {
    let mut terminal = terminal()?;
    let mut colors = defaults([20; 3]);
    terminal.set_defaults(&colors)?;
    terminal.feed(b"\x1b[?1049h\x1b[?2031h\x1b[?2031$p");
    assert_eq!(terminal.take_pty_output(), b"\x1b[?2031;1$y");

    colors.background = [240; 3];
    terminal.set_defaults(&colors)?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;2n");
    colors.background = [20; 3];
    terminal.set_defaults(&colors)?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");
    colors.background = [30; 3];
    terminal.set_defaults(&colors)?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");

    colors.foreground = [220; 3];
    terminal.set_defaults(&colors)?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");
    colors.ansi[1] = [80; 3];
    terminal.set_defaults(&colors)?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");
    colors.palette.insert(196, [10, 20, 30]);
    terminal.set_defaults(&colors)?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");
    terminal.feed(b"\x1b]4;196;?\x07");
    assert_eq!(
        terminal.take_pty_output(),
        b"\x1b]4;196;rgb:0a0a/1414/1e1e\x07"
    );
    colors.palette.clear();
    terminal.set_defaults(&colors)?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");

    terminal.set_defaults(&colors)?;
    assert!(terminal.take_pty_output().is_empty());
    colors.cursor_style = Some(CursorShape::Bar);
    colors.cursor_blink = Some(false);
    terminal.set_defaults(&colors)?;
    assert!(terminal.take_pty_output().is_empty());

    terminal.feed(b"\x1b[?2031l");
    colors.background = [240; 3];
    terminal.set_defaults(&colors)?;
    assert!(terminal.take_pty_output().is_empty());
    terminal.feed(b"\x1b[?996n");
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;2n");
    Ok(())
}

#[test]
fn program_color_overrides_do_not_trigger_theme_notifications() -> Result<(), TerminalError> {
    let mut terminal = terminal()?;
    let mut colors = defaults([20; 3]);
    terminal.set_defaults(&colors)?;
    terminal.feed(b"\x1b[?2031h\x1b]11;#123456\x07\x1b]4;1;#abcdef\x07");
    assert!(terminal.take_pty_output().is_empty());
    colors.background = [240; 3];
    terminal.set_defaults(&colors)?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;2n");
    terminal.feed(b"\x1b[?996n\x1b]11;?\x07\x1b]4;1;?\x07");
    assert_eq!(
        terminal.take_pty_output(),
        b"\x1b[?997;2n\x1b]11;rgb:1212/3434/5656\x07\x1b]4;1;rgb:abab/cdcd/efef\x07"
    );
    Ok(())
}
