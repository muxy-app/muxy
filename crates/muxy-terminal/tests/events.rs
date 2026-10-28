use muxy_terminal::{Size, Terminal, TerminalEvent};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn repeated_a_osc_52(length: usize) -> String {
    let mut encoded = "YWFh".repeat(length / 3);
    match length % 3 {
        1 => encoded.push_str("YQ=="),
        2 => encoded.push_str("YWE="),
        _ => {}
    }
    format!("\x1b]52;c;{encoded}\x07")
}

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

#[test]
fn osc_52_clipboard_writes_accept_supported_text_and_coalesce() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 80, rows: 24 }, 1024)?;
    terminal.feed(b"\x1b]52;c;Zmlyc3Q=\x07");
    terminal.feed(b"\x1b]52;c;56ys5LiA6KGMCnNlY29uZCBsaW5l\x1b\\");
    assert_eq!(
        terminal.take_events(),
        [TerminalEvent::ClipboardWrite("第一行\nsecond line".into())]
    );
    assert!(terminal.take_events().is_empty());

    for chunk in [b"\x1b]52;c;ZnJh".as_slice(), b"Z21lbnRlZA==\x07"] {
        terminal.feed(chunk);
    }
    assert_eq!(
        terminal.take_events(),
        [TerminalEvent::ClipboardWrite("fragmented".into())]
    );

    terminal.feed(repeated_a_osc_52(muxy_protocol::MAX_CLIPBOARD_TEXT).as_bytes());
    let events = terminal.take_events();
    let [TerminalEvent::ClipboardWrite(text)] = events.as_slice() else {
        return Err("expected a maximum-size clipboard write".into());
    };
    assert_eq!(text.len(), muxy_protocol::MAX_CLIPBOARD_TEXT);
    assert!(text.bytes().all(|byte| byte == b'a'));
    Ok(())
}

#[test]
fn osc_52_clipboard_writes_ignore_unsupported_or_invalid_requests() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 80, rows: 24 }, 1024)?;
    for bytes in [
        b"\x1b]52;c;?\x07".as_slice(),
        b"\x1b]52;c;not-base64!\x07",
        b"\x1b]52;c;/w==\x07",
        b"\x1b]52;p;cHJpbWFyeQ==\x07",
        b"\x1b]52;s;c2VsZWN0aW9u\x07",
        b"\x1b]52;c;\x07",
    ] {
        terminal.feed(bytes);
    }
    assert!(terminal.take_events().is_empty());

    terminal.feed(repeated_a_osc_52(muxy_protocol::MAX_CLIPBOARD_TEXT + 1).as_bytes());
    assert!(terminal.take_events().is_empty());
    Ok(())
}

#[test]
fn working_directory_keeps_bytes_that_are_not_utf8() -> TestResult {
    let mut terminal = Terminal::new(Size { cols: 80, rows: 24 }, 1024)?;
    terminal.feed(b"\x1b]7;file://host/tmp/caf\xe9\x07");
    assert_eq!(
        terminal.take_events(),
        [TerminalEvent::Directory(
            b"file://host/tmp/caf\xe9".to_vec()
        )]
    );
    Ok(())
}
