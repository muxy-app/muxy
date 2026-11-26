use muxy_terminal::{Size, Terminal};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn resume_reports_reach_the_server_whole_and_split_across_reads() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 80, rows: 24 }, 1024)?;
    terminal.feed(b"before\x1b]7577;pid=1:time=2:clear=1:mac=");
    terminal.feed(b"ab\x1b\\after\x1b]7577;pid=3:time=4:argv=dmlt:mac=cd\x07");
    terminal.feed(b"\x1b]7501;state=working\x1b\\\x1b]2;title\x07");
    assert_eq!(
        terminal.take_resume_reports(),
        [
            b"pid=1:time=2:clear=1:mac=ab".to_vec(),
            b"pid=3:time=4:argv=dmlt:mac=cd".to_vec(),
        ]
    );
    assert!(terminal.take_resume_reports().is_empty());
    Ok(())
}

#[test]
fn oversized_resume_reports_are_dropped_instead_of_cut_short() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 80, rows: 24 }, 1024)?;
    let long = format!(
        "\x1b]7577;pid=1:time=2:argv={}:mac=ab\x1b\\",
        "A".repeat(8192)
    );
    terminal.feed(long.as_bytes());
    assert!(terminal.take_resume_reports().is_empty());
    Ok(())
}
