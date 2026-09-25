//! Keys an on-screen keyboard or accessory bar can send, encoded as a terminal expects.

use muxy_protocol::Modes;

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum Key {
    /// Typed text; with Control, only its first character counts.
    Character {
        text: String,
    },
    Enter,
    Tab,
    BackTab,
    Escape,
    Backspace,
    Insert,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// F1 to F12.
    Function {
        number: u8,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, uniffi::Record)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
}

pub(crate) fn encode(key: &Key, modifiers: Modifiers, modes: Modes) -> Option<Vec<u8>> {
    let modifier = 1
        + u8::from(modifiers.shift)
        + 2 * u8::from(modifiers.alt)
        + 4 * u8::from(modifiers.control);
    let sequence = |suffix: char| {
        if modifier > 1 {
            format!("\x1b[1;{modifier}{suffix}").into_bytes()
        } else if modes.application_cursor_keys {
            format!("\x1bO{suffix}").into_bytes()
        } else {
            format!("\x1b[{suffix}").into_bytes()
        }
    };
    let numbered = |number: u8| {
        if modifier > 1 {
            format!("\x1b[{number};{modifier}~").into_bytes()
        } else {
            format!("\x1b[{number}~").into_bytes()
        }
    };
    let bytes = match key {
        Key::Up => return Some(sequence('A')),
        Key::Down => return Some(sequence('B')),
        Key::Right => return Some(sequence('C')),
        Key::Left => return Some(sequence('D')),
        Key::Home => return Some(sequence('H')),
        Key::End => return Some(sequence('F')),
        Key::Insert => return Some(numbered(2)),
        Key::Delete => return Some(numbered(3)),
        Key::PageUp => return Some(numbered(5)),
        Key::PageDown => return Some(numbered(6)),
        Key::Function {
            number: number @ 1..=4,
        } => {
            let suffix = char::from(b'P' + number - 1);
            return Some(if modifier > 1 {
                format!("\x1b[1;{modifier}{suffix}").into_bytes()
            } else {
                format!("\x1bO{suffix}").into_bytes()
            });
        }
        Key::Function {
            number: number @ 5..=12,
        } => {
            return Some(numbered(
                [15, 17, 18, 19, 20, 21, 23, 24][usize::from(number - 5)],
            ));
        }
        Key::Function { .. } => return None,
        Key::Enter => vec![b'\r'],
        Key::Tab => vec![b'\t'],
        Key::BackTab => b"\x1b[Z".to_vec(),
        Key::Escape => vec![0x1b],
        Key::Backspace => vec![0x7f],
        Key::Character { text } if modifiers.control => {
            vec![control_byte(text.chars().next()?)?]
        }
        Key::Character { text } => text.clone().into_bytes(),
    };
    if modifiers.alt {
        Some([&[0x1b][..], &bytes].concat())
    } else {
        Some(bytes)
    }
}

fn control_byte(character: char) -> Option<u8> {
    match character {
        ' ' | '@' | '2' => Some(0),
        '[' | '3' => Some(0x1b),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '7' | '/' => Some(0x1f),
        '?' | '8' => Some(0x7f),
        value if value.is_ascii_alphabetic() => u8::try_from(value.to_ascii_uppercase())
            .ok()?
            .checked_sub(b'@'),
        _ => None,
    }
}

/// Pasted text sends line breaks as Return and drops escape characters, so it
/// can never end a bracketed paste early and run as typed input.
pub(crate) fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text
        .replace("\r\n", "\n")
        .replace('\n', "\r")
        .replace(['\x1b', '\u{009b}'], "");
    if bracketed && !text.is_empty() {
        [b"\x1b[200~".as_slice(), text.as_bytes(), b"\x1b[201~"].concat()
    } else {
        text.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modes(application_cursor_keys: bool) -> Modes {
        Modes {
            application_cursor_keys,
            bracketed_paste: false,
        }
    }

    #[test]
    fn keys_encode_like_the_terminal_client() {
        let none = Modifiers::default();
        let control = Modifiers {
            control: true,
            ..none
        };
        let alt = Modifiers { alt: true, ..none };
        assert_eq!(
            encode(&Key::Up, none, modes(false)),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            encode(&Key::Up, none, modes(true)),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            encode(&Key::Left, control, modes(true)),
            Some(b"\x1b[1;5D".to_vec())
        );
        assert_eq!(
            encode(&Key::Character { text: "c".into() }, control, modes(false)),
            Some(vec![3])
        );
        assert_eq!(
            encode(&Key::Character { text: "x".into() }, alt, modes(false)),
            Some(b"\x1bx".to_vec())
        );
        assert_eq!(
            encode(&Key::Function { number: 5 }, none, modes(false)),
            Some(b"\x1b[15~".to_vec())
        );
        assert_eq!(
            encode(&Key::Function { number: 13 }, none, modes(false)),
            None
        );
        assert_eq!(encode(&Key::Escape, none, modes(false)), Some(vec![0x1b]));
    }

    #[test]
    fn pastes_are_bracketed_only_when_the_program_asks() {
        assert_eq!(paste("ls", false), b"ls");
        assert_eq!(paste("ls", true), b"\x1b[200~ls\x1b[201~");
    }

    #[test]
    fn pasted_text_cannot_escape_its_brackets() {
        assert_eq!(
            paste("a\r\nb\n\x1b[201~rm\u{009b}201~", true),
            b"\x1b[200~a\rb\r[201~rm201~\x1b[201~"
        );
    }
}
