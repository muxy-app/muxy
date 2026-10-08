use muxy_protocol::Modes;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Clone, Debug)]
pub(crate) enum Input {
    Key(KeyEvent),
    Paste(String),
    Bytes(Vec<u8>),
}

impl Input {
    pub(crate) fn length(&self) -> usize {
        match self {
            Self::Key(_) => 16,
            Self::Paste(text) => text.len() + 12,
            Self::Bytes(bytes) => bytes.len(),
        }
    }

    pub(crate) fn encode(self, modes: Modes) -> Vec<u8> {
        match self {
            Self::Key(key) => encode(key, modes).unwrap_or_default(),
            Self::Paste(text) => paste(text, modes.bracketed_paste),
            Self::Bytes(bytes) => bytes,
        }
    }
}

pub(crate) fn encode(key: KeyEvent, modes: Modes) -> Option<Vec<u8>> {
    if key.kind == KeyEventKind::Release
        || key
            .modifiers
            .intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META)
    {
        return None;
    }
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let modifier = 1
        + u8::from(key.modifiers.contains(KeyModifiers::SHIFT))
        + 2 * u8::from(alt)
        + 4 * u8::from(control);
    let sequence = |suffix: char| {
        if modifier > 1 {
            format!("\x1b[1;{modifier}{suffix}").into_bytes()
        } else {
            format!(
                "\x1b{}{suffix}",
                if modes.application_cursor_keys {
                    'O'
                } else {
                    '['
                }
            )
            .into_bytes()
        }
    };
    let numbered = |number: u8| {
        if modifier > 1 {
            format!("\x1b[{number};{modifier}~").into_bytes()
        } else {
            format!("\x1b[{number}~").into_bytes()
        }
    };
    let bytes = match key.code {
        KeyCode::Up => return Some(sequence('A')),
        KeyCode::Down => return Some(sequence('B')),
        KeyCode::Right => return Some(sequence('C')),
        KeyCode::Left => return Some(sequence('D')),
        KeyCode::Home => return Some(sequence('H')),
        KeyCode::End => return Some(sequence('F')),
        KeyCode::Insert => return Some(numbered(2)),
        KeyCode::Delete => return Some(numbered(3)),
        KeyCode::PageUp => return Some(numbered(5)),
        KeyCode::PageDown => return Some(numbered(6)),
        KeyCode::F(number @ 1..=4) => {
            let suffix = char::from(b'P' + number - 1);
            return Some(if modifier > 1 {
                format!("\x1b[1;{modifier}{suffix}").into_bytes()
            } else {
                format!("\x1bO{suffix}").into_bytes()
            });
        }
        KeyCode::F(number @ 5..=12) => {
            return Some(numbered(
                [15, 17, 18, 19, 20, 21, 23, 24][usize::from(number - 5)],
            ));
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Esc => vec![0x1b],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Char(character) if control => vec![control_byte(character)?],
        KeyCode::Char(character) => character.to_string().into_bytes(),
        _ => return None,
    };
    if alt {
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
        value if value.is_ascii_alphabetic() => Some(value.to_ascii_uppercase() as u8 - b'@'),
        _ => None,
    }
}

pub(crate) fn paste(text: String, bracketed: bool) -> Vec<u8> {
    if bracketed {
        [
            b"\x1b[200~".as_slice(),
            text.as_bytes(),
            b"\x1b[201~".as_slice(),
        ]
        .concat()
    } else {
        text.into_bytes()
    }
}
