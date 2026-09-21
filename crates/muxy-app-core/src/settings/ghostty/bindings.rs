use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::settings::{Error, KeyChord, Result};

#[derive(Clone, Debug, PartialEq)]
pub enum TerminalAction {
    Text(Vec<u8>),
    Ignore,
    Unbind,
    Copy,
    Paste,
    SelectAll,
    Reload,
    ScrollTop,
    ScrollBottom,
    IncreaseFontSize(f32),
    DecreaseFontSize(f32),
    ResetFontSize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerminalBindings {
    pub bindings: BTreeMap<KeyChord, TerminalAction>,
    pub clear_defaults: bool,
}

static DEFAULT_BINDINGS: LazyLock<BTreeMap<KeyChord, TerminalAction>> = LazyLock::new(|| {
    [
        ("shift-enter", TerminalAction::Text(b"\n".to_vec())),
        ("cmd-shift-v", TerminalAction::Paste),
    ]
    .into_iter()
    .map(|(key, action)| (KeyChord::default_binding(key), action))
    .collect()
});

impl TerminalBindings {
    pub fn chords_for_action<'a>(
        &'a self,
        action: &'a TerminalAction,
    ) -> impl Iterator<Item = &'a KeyChord> {
        self.bindings
            .iter()
            .chain(
                DEFAULT_BINDINGS.iter().filter(|(chord, _)| {
                    !self.clear_defaults && !self.bindings.contains_key(chord)
                }),
            )
            .filter_map(move |(chord, bound)| (bound == action).then_some(chord))
    }

    pub fn action(&self, chord: &KeyChord) -> Option<&TerminalAction> {
        self.bindings.get(chord).or_else(|| {
            if self.clear_defaults {
                None
            } else {
                DEFAULT_BINDINGS.get(chord)
            }
        })
    }

    pub(super) fn lines(&self) -> Vec<String> {
        let mut lines = vec![if self.clear_defaults {
            "clear".into()
        } else {
            String::new()
        }];
        for (chord, action) in &self.bindings {
            let mut rest = chord.as_str();
            let mut trigger = String::new();
            while let Some((part, tail)) = rest.split_once('-') {
                if !matches!(part, "cmd" | "ctrl" | "alt" | "shift") {
                    break;
                }
                trigger.push_str(part);
                trigger.push('+');
                rest = tail;
            }
            trigger.push_str(match rest {
                "=" => "equal",
                "+" => "plus",
                "-" => "minus",
                other => other,
            });
            let action = match action {
                TerminalAction::Text(bytes) => {
                    use std::fmt::Write as _;
                    let mut text = String::from("text:");
                    for byte in bytes {
                        let _ = write!(text, "\\x{byte:02x}");
                    }
                    text
                }
                TerminalAction::Ignore => "ignore".into(),
                TerminalAction::Unbind => "unbind".into(),
                TerminalAction::Copy => "copy_to_clipboard".into(),
                TerminalAction::Paste => "paste_from_clipboard".into(),
                TerminalAction::SelectAll => "select_all".into(),
                TerminalAction::Reload => "reload_config".into(),
                TerminalAction::ScrollTop => "scroll_to_top".into(),
                TerminalAction::ScrollBottom => "scroll_to_bottom".into(),
                TerminalAction::IncreaseFontSize(amount) => format!("increase_font_size:{amount}"),
                TerminalAction::DecreaseFontSize(amount) => format!("decrease_font_size:{amount}"),
                TerminalAction::ResetFontSize => "reset_font_size".into(),
            };
            lines.push(format!("{trigger}={action}"));
        }
        lines
    }

    pub(super) fn read(&mut self, value: &str) -> Result<Option<String>> {
        if value == "clear" || value.is_empty() {
            self.bindings.clear();
            self.clear_defaults = value == "clear";
            return Ok((value == "clear").then(|| "keybind = clear clears terminal bindings; Muxy application shortcuts remain in settings.toml".into()));
        }
        let (trigger, action) = value
            .split_once('=')
            .ok_or_else(|| Error::new("keybind", "expected trigger=action"))?;
        if trigger.contains([':', '>']) || trigger == "chain" || trigger.contains("catch_all") {
            return Ok(Some(
                "keybind requires a single logical key without prefixes, sequences, or chains"
                    .into(),
            ));
        }
        let key = chord(trigger.trim())?;
        let (name, parameter) = action.trim().split_once(':').unwrap_or((action.trim(), ""));
        if !parameter.is_empty()
            && !matches!(
                name,
                "text" | "esc" | "csi" | "increase_font_size" | "decrease_font_size"
            )
        {
            return Ok(Some(format!(
                "keybind action {name:?} with parameters is not supported by Muxy"
            )));
        }
        let action = match name {
            "text" => TerminalAction::Text(unescape(parameter)?),
            "esc" => TerminalAction::Text([b"\x1b".as_slice(), parameter.as_bytes()].concat()),
            "csi" => TerminalAction::Text([b"\x1b[".as_slice(), parameter.as_bytes()].concat()),
            "ignore" => TerminalAction::Ignore,
            "unbind" => TerminalAction::Unbind,
            "copy_to_clipboard" => TerminalAction::Copy,
            "paste_from_clipboard" => TerminalAction::Paste,
            "select_all" => TerminalAction::SelectAll,
            "reload_config" => TerminalAction::Reload,
            "scroll_to_top" => TerminalAction::ScrollTop,
            "scroll_to_bottom" => TerminalAction::ScrollBottom,
            "increase_font_size" => TerminalAction::IncreaseFontSize(font_amount(parameter)?),
            "decrease_font_size" => TerminalAction::DecreaseFontSize(font_amount(parameter)?),
            "reset_font_size" => TerminalAction::ResetFontSize,
            _ => {
                return Ok(Some(format!(
                    "keybind action {name:?} is not supported by Muxy"
                )));
            }
        };
        self.bindings.insert(key, action);
        Ok(None)
    }
}

fn font_amount(value: &str) -> Result<f32> {
    let amount = if value.is_empty() {
        1.0
    } else {
        value
            .parse::<f32>()
            .map_err(|error| Error::new("font size", error))?
    };
    if !amount.is_finite() || !(0.0..=256.0).contains(&amount) {
        return Err(Error::new(
            "font size",
            "expected a finite amount between 0 and 256",
        ));
    }
    Ok(amount)
}

fn chord(value: &str) -> Result<KeyChord> {
    let mut modifiers = Vec::new();
    let mut key = None;
    for part in value.split('+').map(str::trim) {
        let part = match part {
            "super" | "command" | "cmd" => "cmd",
            "control" | "ctrl" => "ctrl",
            "option" | "opt" | "alt" => "alt",
            "arrow_left" | "ArrowLeft" => "left",
            "arrow_right" | "ArrowRight" => "right",
            "arrow_up" | "ArrowUp" => "up",
            "arrow_down" | "ArrowDown" => "down",
            "page_up" | "PageUp" => "pageup",
            "page_down" | "PageDown" => "pagedown",
            "left_bracket" => "[",
            "right_bracket" => "]",
            "backslash" => "\\",
            "slash" => "/",
            "comma" => ",",
            "period" => ".",
            "equal" => "=",
            "semicolon" => ";",
            "quote" => "'",
            "grave_accent" => "`",
            other => other,
        };
        if matches!(part, "cmd" | "ctrl" | "alt" | "shift") {
            modifiers.push(part);
        } else if key.replace(part).is_some() {
            return Err(Error::new("keybind", "expected one key per trigger"));
        }
    }
    modifiers.push(key.ok_or_else(|| Error::new("keybind", "missing key"))?);
    modifiers.join("-").parse()
}

fn unescape(value: &str) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
            continue;
        }
        match chars.next() {
            Some('n') => result.push(b'\n'),
            Some('r') => result.push(b'\r'),
            Some('t') => result.push(b'\t'),
            Some('e') => result.push(0x1b),
            Some('\\') => result.push(b'\\'),
            Some('"') => result.push(b'"'),
            Some('x') => {
                let mut byte = 0;
                for _ in 0..2 {
                    let digit = chars
                        .next()
                        .and_then(|ch| ch.to_digit(16))
                        .ok_or_else(|| Error::new("text", "expected two hex digits after \\x"))?;
                    byte = (byte << 4)
                        | u8::try_from(digit).map_err(|error| Error::new("text", error))?;
                }
                result.push(byte);
            }
            _ => return Err(Error::new("text", "invalid escape sequence")),
        }
    }
    Ok(result)
}
