use muxy_protocol::{KeyAction, KeyEvent, KeyModifiers};
use muxy_terminal::{Size, Terminal};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn terminal() -> std::result::Result<Terminal, muxy_terminal::TerminalError> {
    Terminal::new(Size { cols: 80, rows: 24 }, 1024 * 1024)
}

fn key(name: &str) -> KeyEvent {
    KeyEvent {
        key: name.into(),
        text: if name.chars().count() == 1 {
            name.into()
        } else {
            String::new()
        },
        unshifted_codepoint: if name.chars().count() == 1 {
            name.chars().next().map_or(0, u32::from)
        } else {
            0
        },
        option_as_alt: true,
        ..KeyEvent::default()
    }
}

#[test]
fn keys_use_live_cursor_keypad_and_backarrow_modes_without_a_render() -> Result {
    let mut terminal = terminal()?;
    assert_eq!(terminal.encode_key(key("up"))?, b"\x1b[A");
    terminal.feed(b"\x1b[?1h\x1b=\x1b[?67h");
    assert_eq!(terminal.encode_key(key("up"))?, b"\x1bOA");
    let mut numpad = key("numpad1");
    numpad.text = "1".into();
    assert_eq!(terminal.encode_key(numpad.clone())?, b"1");
    terminal.feed(b"\x1b[?1035l");
    assert_eq!(terminal.encode_key(numpad)?, b"\x1bOq");
    assert_eq!(terminal.encode_key(key("backspace"))?, b"\x08");
    terminal.feed(b"\x1b[?1l\x1b>\x1b[?67l");
    assert_eq!(terminal.encode_key(key("up"))?, b"\x1b[A");
    assert_eq!(terminal.encode_key(key("backspace"))?, b"\x7f");
    Ok(())
}

#[test]
fn modify_other_keys_and_kitty_modes_encode_press_repeat_release_and_text() -> Result {
    let mut terminal = terminal()?;
    let mut event = key("a");
    event.modifiers = KeyModifiers::CTRL;
    assert_eq!(terminal.encode_key(event.clone())?, b"\x01");
    terminal.feed(b"\x1b[>4;2m");
    // Ghostty preserves C0 shortcuts; modified printable keys use modifyOtherKeys.
    assert_eq!(terminal.encode_key(event.clone())?, b"\x01");
    let mut modified = key("h");
    modified.text = "H".into();
    modified.modifiers = KeyModifiers(KeyModifiers::CTRL.0 | KeyModifiers::SHIFT.0);
    assert_eq!(terminal.encode_key(modified)?, b"\x1b[27;6;72~");
    terminal.feed(b"\x1b[>4;0m\x1b[>3u");
    for (action, expected) in [
        (KeyAction::Press, "\r"),
        (KeyAction::Repeat, "\r"),
        (KeyAction::Release, ""),
    ] {
        let mut event = key("enter");
        event.action = action;
        assert_eq!(terminal.encode_key(event)?, expected.as_bytes());
    }
    terminal.feed(b"\x1b[<u\x1b[>31u");
    assert_eq!(terminal.encode_key(event)?, b"\x1b[97;5u");
    for (action, expected) in [
        (KeyAction::Press, "\x1b[97;;97u"),
        (KeyAction::Repeat, "\x1b[97;1:2;97u"),
        (KeyAction::Release, "\x1b[97;1:3u"),
    ] {
        let mut event = key("a");
        event.action = action;
        assert_eq!(terminal.encode_key(event)?, expected.as_bytes());
    }
    let mut event = key("j");
    event.text = "J".into();
    event.modifiers = KeyModifiers::SHIFT;
    event.consumed_modifiers = KeyModifiers::SHIFT;
    assert_eq!(terminal.encode_key(event)?, b"\x1b[106:74;2;74u");
    terminal.feed(b"\x1b[<u");
    assert_eq!(terminal.encode_key(key("a"))?, b"a");
    let mut event = key("a");
    event.action = KeyAction::Release;
    assert!(terminal.encode_key(event)?.is_empty());
    Ok(())
}

#[test]
fn option_settings_and_reused_encoder_do_not_leak_between_clients() -> Result {
    let mut terminal = terminal()?;
    let mut event = key("b");
    event.modifiers = KeyModifiers::ALT;
    assert_eq!(terminal.encode_key(event.clone())?, b"\x1bb");
    event.option_as_alt = false;
    event.consumed_modifiers = KeyModifiers::ALT;
    event.text = "∫".into();
    assert_eq!(terminal.encode_key(event)?, "∫".as_bytes());
    assert_eq!(terminal.encode_key(key("enter"))?, b"\r");
    assert_eq!(terminal.encode_key(key("é"))?, "é".as_bytes());
    assert_eq!(terminal.encode_key(key("unknown"))?, b"");
    Ok(())
}

#[test]
fn key_encoding_grows_past_the_initial_buffer_without_losing_text() -> Result {
    let mut terminal = terminal()?;
    for length in [31, 32, 33, 40, 63, 64, 65, 4096] {
        let mut event = key("a");
        event.text = "x".repeat(length);
        assert_eq!(terminal.encode_key(event.clone())?, event.text.as_bytes());
    }
    terminal.feed(b"\x1b[>31u");
    let mut event = key("a");
    event.text = "xxxxxxxx".into();
    assert_eq!(
        terminal.encode_key(event)?,
        b"\x1b[97;;120:120:120:120:120:120:120:120u"
    );
    let mut encoder = libghostty_vt::key::Encoder::new()?;
    let mut event = libghostty_vt::key::Event::new()?;
    event.set_utf8(Some("x".repeat(40)));
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(b"prefix:");
    encoder.encode_to_vec(&event, &mut bytes)?;
    assert_eq!(bytes, [b"prefix:".as_slice(), &[b'x'; 40]].concat());
    Ok(())
}

#[test]
fn paste_and_focus_use_current_modes_and_preserve_unicode_and_newline_intent() -> Result {
    let mut terminal = terminal()?;
    let text = "one\r\ntwo\n\x1b[201~\0\x03\x15\u{009b}201~界\t";
    assert_eq!(
        terminal.encode_paste(text.as_bytes().to_vec())?,
        "one\rtwo\r [201~    201~界\t".as_bytes()
    );
    assert!(terminal.encode_focus(true)?.is_empty());
    terminal.feed(b"\x1b[?2004h\x1b[?1004h");
    assert_eq!(
        terminal.encode_paste(text.as_bytes().to_vec())?,
        "\x1b[200~one\ntwo\n [201~    201~界\t\x1b[201~".as_bytes()
    );
    assert!(terminal.encode_paste(Vec::new())?.is_empty());
    assert_eq!(terminal.encode_focus(true)?, b"\x1b[I");
    assert_eq!(terminal.encode_focus(false)?, b"\x1b[O");
    terminal.feed(b"\x1b[?2004l\x1b[?1004l");
    assert_eq!(terminal.encode_paste(b"one\ntwo".to_vec())?, b"one\rtwo");
    assert!(terminal.encode_focus(false)?.is_empty());
    Ok(())
}
