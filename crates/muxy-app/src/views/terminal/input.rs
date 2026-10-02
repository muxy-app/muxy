use gpui::{Keystroke, Modifiers};
use muxy_protocol::{KeyAction, KeyEvent, KeyModifiers, Modes};

#[derive(Default)]
pub(super) struct Keyboard {
    pub(super) pending: Option<KeyEvent>,
    pressed: std::collections::BTreeMap<String, KeyEvent>,
}

impl Keyboard {
    pub(super) fn is_pressed(&self, key: &str) -> bool {
        self.pressed.contains_key(&identity(key))
    }

    pub(super) fn press(&mut self, event: &KeyEvent) {
        if self.pressed.len() < 64 || self.is_pressed(&event.key) {
            self.pressed.insert(identity(&event.key), event.clone());
        }
    }

    pub(super) fn release(&mut self, key: &Keystroke) -> Option<KeyEvent> {
        self.pending = None;
        let mut event = self.pressed.remove(&identity(&key.key))?;
        event.action = KeyAction::Release;
        event.modifiers = modifiers(key.modifiers);
        Some(event)
    }

    pub(super) fn release_all(&mut self) -> Vec<KeyEvent> {
        self.pending = None;
        std::mem::take(&mut self.pressed)
            .into_values()
            .map(|mut event| {
                event.action = KeyAction::Release;
                event
            })
            .collect()
    }
}

fn identity(key: &str) -> String {
    if let Some(index) = [
        "!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "_", "+", "{", "}", "|", ":", "\"", "<",
        ">", "?", "~",
    ]
    .iter()
    .position(|shifted| *shifted == key)
    {
        return [
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "[", "]", "\\", ";", "'",
            ",", ".", "/", "`",
        ][index]
            .into();
    }
    key.to_lowercase()
}

fn modifiers(mods: Modifiers) -> KeyModifiers {
    let mut result = KeyModifiers::default();
    result.set(KeyModifiers::SHIFT, mods.shift);
    result.set(KeyModifiers::ALT, mods.alt);
    result.set(KeyModifiers::CTRL, mods.control);
    result.set(KeyModifiers::SUPER, mods.platform);
    result
}

pub(super) fn key_event(key: &Keystroke, held: bool, option_as_alt: bool) -> KeyEvent {
    let logical = if key.key == "space" { " " } else { &key.key };
    let character = (logical.chars().count() == 1).then(|| logical.to_owned());
    let text = if key.modifiers.control || (key.modifiers.alt && option_as_alt) {
        character.map(|text| {
            if key.modifiers.shift {
                text.to_uppercase()
            } else {
                text
            }
        })
    } else {
        key.key_char.clone().or(character)
    }
    .filter(|text| {
        !text
            .chars()
            .any(|ch| ch.is_control() || ('\u{f700}'..='\u{f8ff}').contains(&ch))
    })
    .unwrap_or_default();
    let mut consumed = KeyModifiers::default();
    if !text.is_empty() {
        consumed.set(KeyModifiers::SHIFT, key.modifiers.shift);
        consumed.set(KeyModifiers::ALT, key.modifiers.alt && !option_as_alt);
    }
    KeyEvent {
        key: key.key.clone(),
        action: if held {
            KeyAction::Repeat
        } else {
            KeyAction::Press
        },
        modifiers: modifiers(key.modifiers),
        consumed_modifiers: consumed,
        unshifted_codepoint: if logical.chars().count() == 1 {
            logical.chars().next().map_or(0, u32::from)
        } else {
            0
        },
        text,
        option_as_alt,
    }
}

pub(super) fn paste_chunks(mut bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    std::iter::from_fn(move || {
        if bytes.is_empty() {
            return None;
        }
        let mut end = bytes.len().min(muxy_protocol::MAX_INPUT);
        if end < bytes.len() {
            while end > 0 && bytes[end] & 0xc0 == 0x80 {
                end -= 1;
            }
            if end > 0 && bytes[end - 1] == b'\r' && bytes[end] == b'\n' {
                end -= 1;
            }
            if end == 0 {
                end = muxy_protocol::MAX_INPUT;
            }
        }
        let (chunk, rest) = bytes.split_at(end);
        bytes = rest;
        Some(chunk)
    })
}

pub(crate) fn uses_text_input(key: &Keystroke, option_as_alt: bool) -> bool {
    !key.modifiers.platform
        && !key.modifiers.control
        && (!key.modifiers.alt || !option_as_alt)
        && key
            .key_char
            .as_deref()
            .is_some_and(|text| !text.is_empty() && !text.chars().any(char::is_control))
}

#[cfg(test)]
pub(crate) fn encode(key: &Keystroke, modes: Modes, option_as_alt: bool) -> Option<Vec<u8>> {
    encode_with_bindings(key, modes, option_as_alt, true)
}

pub(crate) fn encode_with_bindings(
    key: &Keystroke,
    modes: Modes,
    option_as_alt: bool,
    defaults: bool,
) -> Option<Vec<u8>> {
    let modifiers = key.modifiers;
    if modifiers.platform {
        return None;
    }
    if modifiers.alt && !option_as_alt && uses_text_input(key, option_as_alt) {
        return Some(key.key_char.as_deref()?.as_bytes().to_vec());
    }
    let modifier = 1
        + u8::from(modifiers.shift)
        + 2 * u8::from(modifiers.alt)
        + 4 * u8::from(modifiers.control);
    let sequence = match key.key.as_str() {
        "left" if cfg!(target_os = "macos") && defaults && modifier == 3 => Some("\x1bb".into()),
        "right" if cfg!(target_os = "macos") && defaults && modifier == 3 => Some("\x1bf".into()),
        "up" => Some(cursor('A', modes, modifier)),
        "down" => Some(cursor('B', modes, modifier)),
        "right" => Some(cursor('C', modes, modifier)),
        "left" => Some(cursor('D', modes, modifier)),
        "home" => Some(cursor('H', modes, modifier)),
        "end" => Some(cursor('F', modes, modifier)),
        "pageup" => Some(function(5, modifier)),
        "pagedown" => Some(function(6, modifier)),
        "insert" => Some(function(2, modifier)),
        "delete" => Some(function(3, modifier)),
        "f1" | "f2" | "f3" | "f4" => {
            let suffix = match key.key.as_str() {
                "f1" => 'P',
                "f2" => 'Q',
                "f3" => 'R',
                _ => 'S',
            };
            Some(if modifier == 1 {
                format!("\x1bO{suffix}")
            } else {
                format!("\x1b[1;{modifier}{suffix}")
            })
        }
        "f5" => Some(function(15, modifier)),
        "f6" => Some(function(17, modifier)),
        "f7" => Some(function(18, modifier)),
        "f8" => Some(function(19, modifier)),
        "f9" => Some(function(20, modifier)),
        "f10" => Some(function(21, modifier)),
        "f11" => Some(function(23, modifier)),
        "f12" => Some(function(24, modifier)),
        "f13" => Some(function(25, modifier)),
        "f14" => Some(function(26, modifier)),
        "f15" => Some(function(28, modifier)),
        "f16" => Some(function(29, modifier)),
        "f17" => Some(function(31, modifier)),
        "f18" => Some(function(32, modifier)),
        "f19" => Some(function(33, modifier)),
        "f20" => Some(function(34, modifier)),
        _ => None,
    };
    if let Some(sequence) = sequence {
        return Some(sequence.into_bytes());
    }
    let bytes = match key.key.as_str() {
        "enter" | "return" => b"\r".to_vec(),
        "backspace" if modifiers.control => vec![0x08],
        "backspace" => vec![0x7f],
        "escape" => vec![0x1b],
        "tab" if modifiers.shift => b"\x1b[Z".to_vec(),
        "tab" => b"\t".to_vec(),
        _ if modifiers.control => vec![control(&key.key)?],
        "space" => vec![b' '],
        _ if modifiers.alt && key.key.chars().count() == 1 => {
            if modifiers.shift {
                key.key.to_uppercase().into_bytes()
            } else {
                key.key.as_bytes().to_vec()
            }
        }
        _ => {
            let text = key.key_char.as_deref().filter(|text| !text.is_empty())?;
            if text.chars().any(char::is_control) {
                return None;
            }
            text.as_bytes().to_vec()
        }
    };
    Some(prefix_alt(bytes, modifiers))
}

fn prefix_alt(mut bytes: Vec<u8>, modifiers: Modifiers) -> Vec<u8> {
    if modifiers.alt {
        bytes.insert(0, 0x1b);
    }
    bytes
}

fn cursor(suffix: char, modes: Modes, modifier: u8) -> String {
    if modifier > 1 {
        format!("\x1b[1;{modifier}{suffix}")
    } else if modes.application_cursor_keys {
        format!("\x1bO{suffix}")
    } else {
        format!("\x1b[{suffix}")
    }
}

fn function(number: u8, modifier: u8) -> String {
    if modifier > 1 {
        format!("\x1b[{number};{modifier}~")
    } else {
        format!("\x1b[{number}~")
    }
}

fn control(key: &str) -> Option<u8> {
    match key {
        "space" | " " | "@" | "2" => Some(0),
        "[" | "3" => Some(0x1b),
        "\\" | "4" => Some(0x1c),
        "]" | "5" => Some(0x1d),
        "^" | "6" => Some(0x1e),
        "_" | "-" | "7" | "/" => Some(0x1f),
        "?" | "8" => Some(0x7f),
        _ if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() => {
            Some(key.as_bytes()[0].to_ascii_uppercase() - b'@')
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn cursor_and_function_keys_preserve_every_modifier_combination() {
        for application_cursor_keys in [false, true] {
            let modes = Modes {
                application_cursor_keys,
                ..Modes::default()
            };
            for (prefix, modifier) in [
                ("", 1),
                ("shift-", 2),
                ("alt-", 3),
                ("alt-shift-", 4),
                ("ctrl-", 5),
                ("ctrl-shift-", 6),
                ("ctrl-alt-", 7),
                ("ctrl-alt-shift-", 8),
            ] {
                for (key, suffix) in [
                    ("up", 'A'),
                    ("down", 'B'),
                    ("right", 'C'),
                    ("left", 'D'),
                    ("home", 'H'),
                    ("end", 'F'),
                    ("f1", 'P'),
                    ("f4", 'S'),
                ] {
                    let expected = if cfg!(target_os = "macos")
                        && modifier == 3
                        && matches!(key, "left" | "right")
                    {
                        if key == "left" {
                            "\x1bb".into()
                        } else {
                            "\x1bf".into()
                        }
                    } else if modifier != 1 {
                        format!("\x1b[1;{modifier}{suffix}")
                    } else if key.starts_with('f') || application_cursor_keys {
                        format!("\x1bO{suffix}")
                    } else {
                        format!("\x1b[{suffix}")
                    };
                    assert_eq!(
                        encode(
                            &Keystroke::parse(&format!("{prefix}{key}")).unwrap(),
                            modes,
                            true
                        ),
                        Some(expected.into_bytes())
                    );
                }
                for (key, number) in [
                    ("insert", 2),
                    ("delete", 3),
                    ("pageup", 5),
                    ("pagedown", 6),
                    ("f5", 15),
                    ("f12", 24),
                    ("f20", 34),
                ] {
                    let expected = if modifier == 1 {
                        format!("\x1b[{number}~")
                    } else {
                        format!("\x1b[{number};{modifier}~")
                    };
                    assert_eq!(
                        encode(
                            &Keystroke::parse(&format!("{prefix}{key}")).unwrap(),
                            modes,
                            true
                        ),
                        Some(expected.into_bytes())
                    );
                }
            }
        }
    }

    #[test]
    fn structured_keys_preserve_text_modifiers_and_pair_only_delivered_presses() {
        let mut key = Keystroke::parse("alt-b").unwrap();
        key.key_char = Some("∫".into());
        let alt = key_event(&key, false, true);
        assert_eq!(alt.text, "b");
        assert_eq!(alt.modifiers, KeyModifiers::ALT);
        assert_eq!(alt.consumed_modifiers, KeyModifiers::default());
        let option = key_event(&key, true, false);
        assert_eq!(option.text, "∫");
        assert_eq!(option.consumed_modifiers, KeyModifiers::ALT);
        assert_eq!(option.action, KeyAction::Repeat);
        let mut keyboard = Keyboard {
            pending: Some(alt.clone()),
            ..Keyboard::default()
        };
        assert!(keyboard.release(&key).is_none());
        assert!(keyboard.pending.is_none());
        keyboard.press(&alt);
        assert!(keyboard.is_pressed("b"));
        assert_eq!(keyboard.release(&key).unwrap().action, KeyAction::Release);
        assert!(!keyboard.is_pressed("b"));
        let shifted = key_event(&Keystroke::parse("!").unwrap(), false, true);
        keyboard.press(&shifted);
        assert!(keyboard.release(&Keystroke::parse("1").unwrap()).is_some());
        keyboard.press(&alt);
        assert_eq!(keyboard.release_all().len(), 1);
        assert!(keyboard.release_all().is_empty());
    }

    #[test]
    fn paste_chunks_preserve_utf8_and_crlf_boundaries() {
        for suffix in ["界\u{009b}tail", "\r\ntail"] {
            let text = "a".repeat(muxy_protocol::MAX_INPUT - 1) + suffix;
            let chunks: Vec<_> = paste_chunks(text.as_bytes()).collect();
            assert_eq!(chunks.len(), 2);
            assert!(
                chunks
                    .iter()
                    .all(|chunk| chunk.len() <= muxy_protocol::MAX_INPUT)
            );
            assert!(
                chunks
                    .iter()
                    .all(|chunk| std::str::from_utf8(chunk).is_ok())
            );
            assert!(!chunks[0].ends_with(b"\r"));
            assert_eq!(chunks.concat(), text.as_bytes());
        }
        assert_eq!(paste_chunks(&[]).count(), 0);
        let invalid = vec![0xff; muxy_protocol::MAX_INPUT + 1];
        assert_eq!(paste_chunks(&invalid).collect::<Vec<_>>().concat(), invalid);
    }

    #[test]
    fn option_text_and_control_keys() {
        for (chord, character, expected) in [
            ("alt-b", "∫", "\x1bb"),
            ("alt-shift-b", "ı", "\x1bB"),
            ("alt-.", "≥", "\x1b."),
            ("alt-~", "˘", "\x1b~"),
        ] {
            let mut key = Keystroke::parse(chord).unwrap();
            key.key_char = Some(character.into());
            assert_eq!(
                encode(&key, Modes::default(), true),
                Some(expected.as_bytes().to_vec())
            );
            assert_eq!(
                encode(&key, Modes::default(), false),
                Some(character.as_bytes().to_vec())
            );
            assert!(!uses_text_input(&key, true));
            assert!(uses_text_input(&key, false));
        }
        for (chord, expected) in [
            ("ctrl-a", "\x01"),
            ("ctrl-space", "\0"),
            ("ctrl-backspace", "\x08"),
            ("alt-backspace", "\x1b\x7f"),
            ("ctrl-alt-a", "\x1b\x01"),
            ("alt-space", "\x1b "),
            ("shift-tab", "\x1b[Z"),
        ] {
            for option_as_alt in [false, true] {
                assert_eq!(
                    encode(
                        &Keystroke::parse(chord).unwrap(),
                        Modes::default(),
                        option_as_alt
                    ),
                    Some(expected.as_bytes().to_vec())
                );
            }
        }
        assert_eq!(
            encode(&Keystroke::parse("cmd-c").unwrap(), Modes::default(), true),
            None
        );
        let text = Keystroke {
            key: "a".into(),
            key_char: Some("é".into()),
            modifiers: Modifiers::default(),
        };
        assert!(uses_text_input(&text, true));
    }
}
