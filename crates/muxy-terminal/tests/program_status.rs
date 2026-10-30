use muxy_protocol::{ProgressState, SessionProgress, TerminalProgress};
use muxy_terminal::{ProgramState, ProgramStatus, Size, Terminal, TerminalEvent};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn terminal() -> Result<Terminal, muxy_terminal::TerminalError> {
    Terminal::new(Size { cols: 80, rows: 24 }, 1024)
}

fn status(state: ProgramState, progress: Option<u8>, app: &str) -> ProgramStatus {
    ProgramStatus {
        state,
        progress,
        app: app.into(),
    }
}

fn progress(progress: Option<(ProgressState, Option<u8>)>, completed: u64) -> TerminalEvent {
    TerminalEvent::Progress(SessionProgress {
        progress: progress.map(|(state, percent)| TerminalProgress { state, percent }),
        completed,
    })
}

#[test]
fn root_reports_replace_the_record_and_drive_progress() -> TestResult {
    let mut terminal = terminal()?;
    terminal.feed(b"\x1b]7501;state=working:app=cargo:progress=40:msg=QnVpbGRpbmc=\x1b\\");
    assert_eq!(
        terminal.program_status(),
        Some(status(ProgramState::Working, Some(40), "cargo"))
    );
    assert_eq!(
        terminal.take_events(),
        [progress(Some((ProgressState::Running, Some(40))), 0)]
    );
    terminal.feed(b"\x1b]7501;state=blocked:kind=permission\x07");
    assert_eq!(
        terminal.program_status(),
        Some(status(ProgramState::Blocked, None, ""))
    );
    assert_eq!(
        terminal.take_events(),
        [progress(Some((ProgressState::Paused, None)), 0)]
    );
    terminal.feed(b"\x1b]7501;state=error\x1b\\");
    assert_eq!(
        terminal.take_events(),
        [progress(Some((ProgressState::Error, None)), 0)]
    );
    terminal.feed(b"\x1b]7501;state=done\x1b\\");
    assert_eq!(terminal.take_events(), [progress(None, 1)]);
    terminal.feed(b"\x1b]7501;state=clear\x1b\\");
    assert_eq!(terminal.program_status(), None);
    Ok(())
}

#[test]
fn other_records_and_invalid_reports_leave_the_root_alone() -> TestResult {
    let mut terminal = terminal()?;
    terminal.feed(b"\x1b]7501;state=working\x1b\\");
    terminal.take_events();
    for sequence in [
        "\x1b]7501;state=done:id=build\x1b\\",
        "\x1b]7501;state=clear:id=build\x1b\\",
        "\x1b]7501;state=sleeping\x1b\\",
        "\x1b]7501;state=done:msg=Cg==\x1b\\",
    ] {
        terminal.feed(sequence.as_bytes());
        assert_eq!(
            terminal.program_status(),
            Some(status(ProgramState::Working, None, "")),
            "{sequence:?}"
        );
        assert!(terminal.take_events().is_empty(), "{sequence:?}");
    }
    Ok(())
}

#[test]
fn a_program_status_record_hides_progress_reports_until_it_is_cleared() -> TestResult {
    let mut terminal = terminal()?;
    terminal.feed(b"\x1b]9;4;1;10\x07");
    terminal.take_events();
    terminal.feed(b"\x1b]7501;state=idle\x1b\\");
    assert_eq!(terminal.take_events(), [progress(None, 1)]);
    terminal.feed(b"\x1b]9;4;1;20\x07");
    assert_eq!(terminal.take_events(), []);
    terminal.feed(b"\x1b]7501;state=clear\x1b\\");
    assert_eq!(
        terminal.take_events(),
        [progress(Some((ProgressState::Running, Some(20))), 1)]
    );
    Ok(())
}

#[test]
fn the_end_of_a_program_keeps_only_finished_work() -> TestResult {
    let mut terminal = terminal()?;
    for (sequence, kept) in [
        ("state=working", None),
        ("state=blocked", None),
        ("state=idle", None),
        ("state=done", Some(status(ProgramState::Done, None, ""))),
        ("state=error", Some(status(ProgramState::Error, None, ""))),
    ] {
        terminal.feed(format!("\x1b]7501;{sequence}\x1b\\").as_bytes());
        terminal.end_program_status();
        assert_eq!(terminal.program_status(), kept, "{sequence}");
        terminal.feed(format!("\x1b]7501;{sequence}\x1b\\").as_bytes());
        terminal.feed(b"\x1b]133;A\x07");
        assert_eq!(terminal.program_status(), kept, "{sequence} at a prompt");
        terminal.feed(b"\x1bc");
        assert_eq!(terminal.program_status(), None, "{sequence} after a reset");
    }
    Ok(())
}

#[test]
fn support_queries_are_answered_with_their_own_terminator() -> TestResult {
    let mut terminal = terminal()?;
    for (query, terminator) in [
        (&b"\x1b]7501;?\x1b\\"[..], &b"\x1b\\"[..]),
        (b"\x1b]7501;?\x07", b"\x07"),
    ] {
        terminal.feed(query);
        let reply = terminal.take_pty_output();
        assert!(reply.starts_with(b"\x1b]7501;?:states="), "{reply:?}");
        assert!(reply.ends_with(terminator), "{reply:?}");
    }
    Ok(())
}

#[test]
fn finished_work_left_by_an_ended_program_yields_to_later_progress() -> TestResult {
    let mut terminal = terminal()?;
    terminal.feed(b"\x1b]7501;state=error\x1b\\");
    let reports = terminal.program_reports();
    terminal.end_program_status();
    terminal.take_events();
    terminal.feed(b"\x1b]9;4;1;30\x07");
    assert_eq!(terminal.program_status(), None);
    assert_ne!(terminal.program_reports(), reports);
    assert_eq!(
        terminal.take_events(),
        [progress(Some((ProgressState::Running, Some(30))), 0)]
    );
    terminal.feed(b"\x1b]7501;state=done\x1b\\\x1b]9;4;1;40\x07");
    assert_eq!(
        terminal.program_status(),
        Some(status(ProgramState::Done, None, ""))
    );
    Ok(())
}

#[test]
fn progress_reported_before_a_program_ended_does_not_come_back() -> TestResult {
    for record in ["state=working:progress=40", "state=error"] {
        let mut terminal = terminal()?;
        terminal.feed(format!("\x1b]9;4;1;10\x07\x1b]7501;{record}\x1b\\").as_bytes());
        terminal.take_events();
        terminal.feed(b"\x1b]133;A\x07");
        terminal.feed(b"\x1b]7501;state=clear\x1b\\");
        assert_eq!(terminal.take_events(), [progress(None, 1)], "{record}");
    }
    Ok(())
}
