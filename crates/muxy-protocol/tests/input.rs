use muxy_protocol::{ErrorCode, KeyEvent, MAX_INPUT, Message, TerminalInput};

#[test]
fn structured_input_is_bounded_and_rejects_invalid_key_text() {
    for input in [
        TerminalInput::Key(KeyEvent::default()),
        TerminalInput::Paste(vec![0xff; MAX_INPUT]),
        TerminalInput::Focus(false),
        TerminalInput::ClearScreen,
    ] {
        assert_eq!(Message::TerminalInput(input).validate(), Ok(()));
    }
    assert_eq!(
        TerminalInput::Paste(vec![0; MAX_INPUT + 1]).validate(),
        Err(ErrorCode::BadRequest)
    );
    for text in ["\0", "\x1b", "\u{f700}", "\u{009b}"] {
        assert_eq!(
            TerminalInput::Key(KeyEvent {
                text: text.into(),
                ..KeyEvent::default()
            })
            .validate(),
            Err(ErrorCode::BadRequest)
        );
    }
    for codepoint in [0xd800, 0x0011_0000, 0x1b, 0xf700] {
        assert_eq!(
            TerminalInput::Key(KeyEvent {
                unshifted_codepoint: codepoint,
                ..KeyEvent::default()
            })
            .validate(),
            Err(ErrorCode::BadRequest)
        );
    }
    for event in [
        KeyEvent {
            key: "a".repeat(33),
            ..KeyEvent::default()
        },
        KeyEvent {
            text: "a".repeat(4097),
            ..KeyEvent::default()
        },
    ] {
        assert_eq!(
            TerminalInput::Key(event).validate(),
            Err(ErrorCode::BadRequest)
        );
    }
    assert_eq!(
        TerminalInput::Key(KeyEvent {
            text: "é界👩‍💻".into(),
            ..KeyEvent::default()
        })
        .validate(),
        Ok(())
    );
}
