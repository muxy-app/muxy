use muxy_protocol::{ProgressState, SessionProgress, TerminalProgress};
use muxy_terminal::{Size, Terminal, TerminalEvent};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn terminal() -> Result<Terminal, muxy_terminal::TerminalError> {
    Terminal::new(Size { cols: 80, rows: 24 }, 1024)
}

fn progress(state: ProgressState, percent: Option<u8>) -> TerminalEvent {
    TerminalEvent::Progress(SessionProgress {
        progress: Some(TerminalProgress { state, percent }),
        completed: 0,
    })
}

#[test]
fn progress_handles_every_read_boundary_and_both_terminators() -> TestResult {
    for sequence in [b"\x1b]9;4;1;42\x07".as_slice(), b"\x1b]9;4;1;42\x1b\\"] {
        for split in 0..sequence.len() {
            let mut terminal = terminal()?;
            terminal.feed(&sequence[..split]);
            assert!(terminal.take_events().is_empty());
            terminal.feed(&sequence[split..]);
            assert_eq!(
                terminal.take_events(),
                [progress(ProgressState::Running, Some(42))]
            );
            assert!(terminal.take_events().is_empty());
        }
    }
    Ok(())
}

#[test]
fn malformed_unrelated_cancelled_and_oversized_strings_do_not_report_progress() -> TestResult {
    let mut terminal = terminal()?;
    for sequence in [
        "\x1b]9;4;5;20\x07",
        "\x1b]9;4;1;-2\x07",
        "\x1b]9;4;1;bad\x07",
        "\x1b]9;4;1;5;9\x07",
        "\x1b]9;notification\x07",
        "\x1b]133;C\x07",
        "\x1b]9;4;1;20\x18\x1b\\",
        "\x1b]9;4;1;20\x1a\x1b\\",
        "\x1b]9;4;1;20\x1b[0m",
        "\x1b]9;4;1;999999999999999999999999999999999999999999999\x07",
        "\x1bPpayload ]9;4;3\x1b\\",
        "\x1b_payload ]9;4;3\x1b\\",
        "\u{041d}9;4;3\u{041c}",
    ] {
        terminal.feed(sequence.as_bytes());
        assert!(
            !terminal
                .take_events()
                .iter()
                .any(|event| matches!(event, TerminalEvent::Progress(_))),
            "{sequence:?}"
        );
    }
    terminal.feed(b"\x1b]9;4;3\x07");
    assert_eq!(
        terminal.take_events(),
        [progress(ProgressState::Indeterminate, None)]
    );
    Ok(())
}

#[test]
fn reports_are_coalesced_without_changing_screen_title_or_bells() -> TestResult {
    let mut terminal = terminal()?;
    terminal.feed(b"hello");
    let before = terminal.screen()?;
    let cursor = terminal.cursor()?;
    for _ in 0..1000 {
        terminal.feed(b"\x1b]9;4;1;10\x07\x1b]9;4;1;20\x1b\\");
    }
    assert_eq!(
        terminal.take_events(),
        [progress(ProgressState::Running, Some(20))]
    );
    assert_eq!(terminal.screen()?, before);
    assert_eq!(terminal.cursor()?, cursor);
    terminal.feed(b"\x1b]2;title\x07\x1b]9;4;0\x07\x07");
    assert_eq!(
        terminal.take_events(),
        [
            TerminalEvent::Progress(SessionProgress {
                progress: None,
                completed: 1
            }),
            TerminalEvent::Title("title".into()),
            TerminalEvent::Bell
        ]
    );
    terminal.feed(b"\x1b]9;4;3\x07");
    terminal.take_events();
    terminal.feed(b"\x1bc");
    assert!(
        terminal
            .take_events()
            .contains(&TerminalEvent::Progress(SessionProgress {
                progress: None,
                completed: 2
            }))
    );
    Ok(())
}

#[test]
fn coalescing_retains_completions_and_utf8_boundaries_never_start_an_osc() -> TestResult {
    let mut terminal = terminal()?;
    terminal.feed(b"\x1b]9;4;3\x07\x1b]9;4;0\x07\x1b]9;4;1;10\x07");
    assert_eq!(
        terminal.take_events(),
        [TerminalEvent::Progress(SessionProgress {
            progress: Some(TerminalProgress {
                state: ProgressState::Running,
                percent: Some(10)
            }),
            completed: 1,
        })]
    );
    let text = "\u{041d}9;4;3\u{041c}";
    for split in 0..text.len() {
        terminal.feed(&text.as_bytes()[..split]);
        terminal.feed(&text.as_bytes()[split..]);
        assert!(terminal.take_events().is_empty());
    }
    terminal.feed(b"\x9d9;4;0\x9c");
    assert_eq!(
        terminal.take_events(),
        [TerminalEvent::Progress(SessionProgress {
            progress: None,
            completed: 2
        })]
    );
    Ok(())
}
