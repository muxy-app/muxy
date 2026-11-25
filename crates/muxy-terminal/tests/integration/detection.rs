use muxy_terminal::{Size, Terminal};

#[test]
fn detection_reads_live_tail_without_consuming_frame_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = Terminal::new(
        Size {
            cols: 200,
            rows: 100,
        },
        1024 * 1024,
    )?;
    terminal.feed(b"old approval\r\n");
    terminal.feed("transcript 界\r\n".repeat(130).as_bytes());
    terminal.feed(b"working now");
    let screen = terminal.screen()?;
    let text = terminal.detection_text()?;
    assert!(text.contains("working now"));
    assert!(!text.contains("old approval"));
    assert_eq!(text.lines().count(), 80);
    assert_eq!(terminal.take_changed_rows()?, screen);
    assert!(terminal.take_changed_rows()?.is_empty());
    terminal.feed(b"\x1b[2J\x1b[Hnew prompt");
    assert!(!terminal.detection_text()?.contains("new prompt"));
    assert!(!terminal.take_changed_rows()?.is_empty());
    Ok(())
}
