use muxy_terminal::{Size, Terminal, TerminalEvent};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bell_is_coalesced_and_not_triggered_by_osc_terminators() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 80, rows: 24 }, 1024)?;
    terminal.feed(b"\x07\x07");
    assert_eq!(terminal.take_events(), [TerminalEvent::Bell]);
    assert!(terminal.take_events().is_empty());
    terminal.feed(b"\x1b]2;hello\x07");
    assert_eq!(
        terminal.take_events(),
        [TerminalEvent::Title("hello".into())]
    );
    terminal.feed(b"\x07");
    assert_eq!(terminal.take_events(), [TerminalEvent::Bell]);
    Ok(())
}
