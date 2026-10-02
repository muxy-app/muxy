use libghostty_vt::{Error, key, terminal::Terminal};
use muxy_protocol::{KeyAction, KeyEvent, KeyModifiers};

#[derive(Debug)]
pub(crate) struct Keyboard {
    encoder: key::Encoder<'static>,
    event: key::Event<'static>,
}

impl Keyboard {
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self {
            encoder: key::Encoder::new()?,
            event: key::Event::new()?,
        })
    }

    pub(crate) fn encode(
        &mut self,
        terminal: &Terminal<'_, '_>,
        event: KeyEvent,
    ) -> Result<Vec<u8>, Error> {
        self.encoder
            .set_options_from_terminal(terminal)
            .set_macos_option_as_alt(if event.option_as_alt {
                key::OptionAsAlt::True
            } else {
                key::OptionAsAlt::False
            });
        self.event
            .set_action(match event.action {
                KeyAction::Press => key::Action::Press,
                KeyAction::Repeat => key::Action::Repeat,
                KeyAction::Release => key::Action::Release,
            })
            .set_key(key_code(&event.key))
            .set_mods(modifiers(event.modifiers))
            .set_consumed_mods(modifiers(event.consumed_modifiers))
            .set_composing(false)
            .set_unshifted_codepoint(char::from_u32(event.unshifted_codepoint).unwrap_or('\0'))
            .set_utf8((!event.text.is_empty()).then_some(event.text));
        let mut bytes = Vec::with_capacity(32);
        self.encoder.encode_to_vec(&self.event, &mut bytes)?;
        Ok(bytes)
    }
}

fn modifiers(mods: KeyModifiers) -> key::Mods {
    let mut result = key::Mods::empty();
    for (source, target) in [
        (KeyModifiers::SHIFT, key::Mods::SHIFT),
        (KeyModifiers::ALT, key::Mods::ALT),
        (KeyModifiers::CTRL, key::Mods::CTRL),
        (KeyModifiers::SUPER, key::Mods::SUPER),
        (KeyModifiers::CAPS_LOCK, key::Mods::CAPS_LOCK),
        (KeyModifiers::NUM_LOCK, key::Mods::NUM_LOCK),
    ] {
        result.set(target, mods.contains(source));
    }
    result
}

pub(crate) fn paste(mut data: Vec<u8>, bracketed: bool) -> Result<Vec<u8>, Error> {
    if data.is_empty() {
        return Ok(data);
    }
    let mut read = 0;
    let mut written = 0;
    while read < data.len() {
        if data[read..].starts_with(b"\r\n") {
            read += 1;
        } else if data[read..].starts_with(b"\xc2\x9b") {
            data[written] = b' ';
            written += 1;
            read += 2;
            continue;
        }
        data[written] = data[read];
        written += 1;
        read += 1;
    }
    data.truncate(written);
    let mut bytes = vec![0; data.len() + 12];
    let length = libghostty_vt::paste::encode(&mut data, bracketed, &mut bytes)?;
    bytes.truncate(length);
    Ok(bytes)
}

fn key_code(name: &str) -> key::Key {
    use key::Key;
    match name {
        "a" | "A" => Key::A,
        "b" | "B" => Key::B,
        "c" | "C" => Key::C,
        "d" | "D" => Key::D,
        "e" | "E" => Key::E,
        "f" | "F" => Key::F,
        "g" | "G" => Key::G,
        "h" | "H" => Key::H,
        "i" | "I" => Key::I,
        "j" | "J" => Key::J,
        "k" | "K" => Key::K,
        "l" | "L" => Key::L,
        "m" | "M" => Key::M,
        "n" | "N" => Key::N,
        "o" | "O" => Key::O,
        "p" | "P" => Key::P,
        "q" | "Q" => Key::Q,
        "r" | "R" => Key::R,
        "s" | "S" => Key::S,
        "t" | "T" => Key::T,
        "u" | "U" => Key::U,
        "v" | "V" => Key::V,
        "w" | "W" => Key::W,
        "x" | "X" => Key::X,
        "y" | "Y" => Key::Y,
        "z" | "Z" => Key::Z,
        "0" | ")" => Key::Digit0,
        "1" | "!" => Key::Digit1,
        "2" | "@" => Key::Digit2,
        "3" | "#" => Key::Digit3,
        "4" | "$" => Key::Digit4,
        "5" | "%" => Key::Digit5,
        "6" | "^" => Key::Digit6,
        "7" | "&" => Key::Digit7,
        "8" | "*" => Key::Digit8,
        "9" | "(" => Key::Digit9,
        "`" | "~" => Key::Backquote,
        "\\" | "|" => Key::Backslash,
        "[" | "{" => Key::BracketLeft,
        "]" | "}" => Key::BracketRight,
        "," | "<" => Key::Comma,
        "=" | "+" => Key::Equal,
        "-" | "_" => Key::Minus,
        "." | ">" => Key::Period,
        "'" | "\"" => Key::Quote,
        ";" | ":" => Key::Semicolon,
        "/" | "?" => Key::Slash,
        " " | "space" => Key::Space,
        "enter" | "return" => Key::Enter,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "escape" => Key::Escape,
        "delete" => Key::Delete,
        "insert" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::ArrowUp,
        "down" => Key::ArrowDown,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        "numpad0" => Key::Numpad0,
        "numpad1" => Key::Numpad1,
        "numpad2" => Key::Numpad2,
        "numpad3" => Key::Numpad3,
        "numpad4" => Key::Numpad4,
        "numpad5" => Key::Numpad5,
        "numpad6" => Key::Numpad6,
        "numpad7" => Key::Numpad7,
        "numpad8" => Key::Numpad8,
        "numpad9" => Key::Numpad9,
        "numpadenter" => Key::NumpadEnter,
        "numpadadd" => Key::NumpadAdd,
        "numpadsubtract" => Key::NumpadSubtract,
        "numpadmultiply" => Key::NumpadMultiply,
        "numpaddivide" => Key::NumpadDivide,
        "numpaddecimal" => Key::NumpadDecimal,
        "numpadequal" => Key::NumpadEqual,
        _ => function_key(name).unwrap_or(Key::Unidentified),
    }
}

fn function_key(name: &str) -> Option<key::Key> {
    use key::Key;
    let number = name.strip_prefix('f')?.parse::<usize>().ok()?;
    [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
        Key::F13,
        Key::F14,
        Key::F15,
        Key::F16,
        Key::F17,
        Key::F18,
        Key::F19,
        Key::F20,
        Key::F21,
        Key::F22,
        Key::F23,
        Key::F24,
        Key::F25,
    ]
    .get(number.checked_sub(1)?)
    .copied()
}
