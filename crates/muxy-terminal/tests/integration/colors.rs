use muxy_protocol::TerminalColors;
use muxy_terminal::{Size, Terminal};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn terminal() -> Result<Terminal, muxy_terminal::TerminalError> {
    Terminal::new(Size { cols: 80, rows: 24 }, 1024)
}

fn colors(background: [u8; 3]) -> TerminalColors {
    TerminalColors {
        foreground: [0xcc, 0xcc, 0xcc],
        background,
        cursor: [0xcc, 0xcc, 0xcc],
        ..TerminalColors::default()
    }
}

#[test]
fn background_query_and_scheme_mode_report_a_dark_theme() -> TestResult {
    let mut terminal = terminal()?;
    terminal.set_defaults(&colors([0x19, 0x17, 0x1f]))?;
    assert!(terminal.take_pty_output().is_empty());

    terminal.feed(b"\x1b]11;?\x07");
    assert_eq!(
        terminal.take_pty_output(),
        b"\x1b]11;rgb:1919/1717/1f1f\x07"
    );

    terminal.feed(b"\x1b[?2031h");
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");
    Ok(())
}

#[test]
fn scheme_mode_reports_a_light_background() -> TestResult {
    let mut terminal = terminal()?;
    terminal.set_defaults(&colors([0xf5, 0xf5, 0xf5]))?;

    terminal.feed(b"\x1b[?2031h");
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;2n");
    Ok(())
}

#[test]
fn scheme_mode_reports_on_each_rising_edge() -> TestResult {
    let mut terminal = terminal()?;
    terminal.set_defaults(&colors([0x19, 0x17, 0x1f]))?;

    terminal.feed(b"\x1b[?20");
    assert!(terminal.take_pty_output().is_empty());
    terminal.feed(b"31h");
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");

    terminal.feed(b"\x1b[?2031h");
    assert!(terminal.take_pty_output().is_empty());

    terminal.feed(b"\x1b[?2031l");
    assert!(terminal.take_pty_output().is_empty());
    terminal.feed(b"\x1b[?2031h");
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");
    Ok(())
}

#[test]
fn scheme_mode_follows_a_later_background_change() -> TestResult {
    let mut terminal = terminal()?;
    terminal.set_defaults(&colors([0x19, 0x17, 0x1f]))?;
    terminal.feed(b"\x1b[?2031h");
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;1n");

    terminal.set_defaults(&colors([0xf5, 0xf5, 0xf5]))?;
    assert_eq!(terminal.take_pty_output(), b"\x1b[?997;2n");
    Ok(())
}
