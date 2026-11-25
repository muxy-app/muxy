use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{Display, Write},
};

use crate::PlatformKeyboardMapper;

pub trait AsKeystroke {
    fn as_keystroke(&self) -> &Keystroke;
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Deserialize, Hash)]
pub struct Keystroke {
    pub modifiers: Modifiers,

    pub key: String,

    pub key_char: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeybindingKeystroke {
    inner: Keystroke,
    #[cfg(target_os = "windows")]
    display_modifiers: Modifiers,
    #[cfg(target_os = "windows")]
    display_key: String,
}

#[derive(Debug)]
pub struct InvalidKeystrokeError {
    pub keystroke: String,
}

impl Error for InvalidKeystrokeError {}

impl Display for InvalidKeystrokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid keystroke \"{}\". {}",
            self.keystroke, KEYSTROKE_PARSE_EXPECTED_MESSAGE
        )
    }
}

pub const KEYSTROKE_PARSE_EXPECTED_MESSAGE: &str = "Expected a sequence of modifiers \
    (`ctrl`, `alt`, `shift`, `fn`, `cmd`, `super`, or `win`) \
    followed by a key, separated by `-`.";

impl Keystroke {
    pub fn should_match(&self, target: &KeybindingKeystroke) -> bool {
        #[cfg(not(target_os = "windows"))]
        if let Some(key_char) = self
            .key_char
            .as_ref()
            .filter(|key_char| key_char != &&self.key)
        {
            let ime_modifiers = Modifiers {
                control: self.modifiers.control,
                platform: self.modifiers.platform,
                ..Default::default()
            };

            if &target.inner.key == key_char && target.inner.modifiers == ime_modifiers {
                return true;
            }
        }

        #[cfg(target_os = "windows")]
        if let Some(key_char) = self
            .key_char
            .as_ref()
            .filter(|key_char| key_char != &&self.key)
        {
            if &target.inner.key == key_char && target.inner.modifiers == Modifiers::none() {
                return true;
            }
        }

        target.inner.modifiers == self.modifiers && target.inner.key == self.key
    }

    pub fn parse(source: &str) -> std::result::Result<Self, InvalidKeystrokeError> {
        let mut modifiers = Modifiers::none();
        let mut key = None;
        let mut key_char = None;

        let mut components = source.split('-').peekable();
        while let Some(component) = components.next() {
            if component.eq_ignore_ascii_case("ctrl") {
                modifiers.control = true;
                continue;
            }
            if component.eq_ignore_ascii_case("alt") {
                modifiers.alt = true;
                continue;
            }
            if component.eq_ignore_ascii_case("shift") {
                modifiers.shift = true;
                continue;
            }
            if component.eq_ignore_ascii_case("fn") {
                modifiers.function = true;
                continue;
            }
            if component.eq_ignore_ascii_case("secondary") {
                if cfg!(target_os = "macos") {
                    modifiers.platform = true;
                } else {
                    modifiers.control = true;
                };
                continue;
            }

            let is_platform = component.eq_ignore_ascii_case("cmd")
                || component.eq_ignore_ascii_case("super")
                || component.eq_ignore_ascii_case("win");

            if is_platform {
                modifiers.platform = true;
                continue;
            }

            let mut key_str = component.to_string();

            if let Some(next) = components.peek() {
                if next.is_empty() && source.ends_with('-') {
                    key = Some(String::from("-"));
                    break;
                } else if next.len() > 1 && next.starts_with('>') {
                    key = Some(key_str);
                    key_char = Some(String::from(&next[1..]));
                    components.next();
                } else {
                    return Err(InvalidKeystrokeError {
                        keystroke: source.to_owned(),
                    });
                }
                continue;
            }

            if component.len() == 1 && component.as_bytes()[0].is_ascii_uppercase() {
                modifiers.shift = true;
                key_str.make_ascii_lowercase();
            } else {
                key_str.make_ascii_lowercase()
            }
            key = Some(key_str);
        }

        key = key.or_else(|| {
            use std::mem;
            if mem::take(&mut modifiers.shift) {
                Some("shift".to_string())
            } else if mem::take(&mut modifiers.control) {
                Some("control".to_string())
            } else if mem::take(&mut modifiers.alt) {
                Some("alt".to_string())
            } else if mem::take(&mut modifiers.platform) {
                Some("platform".to_string())
            } else if mem::take(&mut modifiers.function) {
                Some("function".to_string())
            } else {
                None
            }
        });

        let key = key.ok_or_else(|| InvalidKeystrokeError {
            keystroke: source.to_owned(),
        })?;

        Ok(Keystroke {
            modifiers,
            key,
            key_char,
        })
    }

    pub fn unparse(&self) -> String {
        unparse(&self.modifiers, &self.key)
    }

    pub fn is_ime_in_progress(&self) -> bool {
        self.key_char.is_none()
            && (is_printable_key(&self.key) || self.key.is_empty())
            && !(self.modifiers.platform
                || self.modifiers.control
                || self.modifiers.function
                || self.modifiers.alt)
    }

    pub fn with_simulated_ime(mut self) -> Self {
        if self.key_char.is_none()
            && !self.modifiers.platform
            && !self.modifiers.control
            && !self.modifiers.function
            && !self.modifiers.alt
        {
            self.key_char = match self.key.as_str() {
                "space" => Some(" ".into()),
                "tab" => Some("\t".into()),
                "enter" => Some("\n".into()),
                key if !is_printable_key(key) || key.is_empty() => None,
                key => {
                    if self.modifiers.shift {
                        Some(key.to_uppercase())
                    } else {
                        Some(key.into())
                    }
                }
            }
        }
        self
    }
}

impl KeybindingKeystroke {
    #[cfg(target_os = "windows")]
    pub(crate) fn new(inner: Keystroke, display_modifiers: Modifiers, display_key: String) -> Self {
        KeybindingKeystroke {
            inner,
            display_modifiers,
            display_key,
        }
    }

    pub fn new_with_mapper(
        inner: Keystroke,
        use_key_equivalents: bool,
        keyboard_mapper: &dyn PlatformKeyboardMapper,
    ) -> Self {
        keyboard_mapper.map_key_equivalent(inner, use_key_equivalents)
    }

    pub fn from_keystroke(keystroke: Keystroke) -> Self {
        #[cfg(target_os = "windows")]
        {
            let key = keystroke.key.clone();
            let modifiers = keystroke.modifiers;
            KeybindingKeystroke {
                inner: keystroke,
                display_modifiers: modifiers,
                display_key: key,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            KeybindingKeystroke { inner: keystroke }
        }
    }

    pub fn inner(&self) -> &Keystroke {
        &self.inner
    }

    pub fn modifiers(&self) -> &Modifiers {
        #[cfg(target_os = "windows")]
        {
            &self.display_modifiers
        }
        #[cfg(not(target_os = "windows"))]
        {
            &self.inner.modifiers
        }
    }

    pub fn key(&self) -> &str {
        #[cfg(target_os = "windows")]
        {
            &self.display_key
        }
        #[cfg(not(target_os = "windows"))]
        {
            &self.inner.key
        }
    }

    pub fn set_modifiers(&mut self, modifiers: Modifiers) {
        self.inner.modifiers = modifiers;
        #[cfg(target_os = "windows")]
        {
            self.display_modifiers = modifiers;
        }
    }

    pub fn set_key(&mut self, key: String) {
        #[cfg(target_os = "windows")]
        {
            self.display_key = key.clone();
        }
        self.inner.key = key;
    }

    pub fn unparse(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            unparse(&self.display_modifiers, &self.display_key)
        }
        #[cfg(not(target_os = "windows"))]
        {
            unparse(&self.inner.modifiers, &self.inner.key)
        }
    }

    pub fn remove_key_char(&mut self) {
        self.inner.key_char = None;
    }
}

fn is_printable_key(key: &str) -> bool {
    !matches!(
        key,
        "f1" | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
            | "f13"
            | "f14"
            | "f15"
            | "f16"
            | "f17"
            | "f18"
            | "f19"
            | "f20"
            | "f21"
            | "f22"
            | "f23"
            | "f24"
            | "f25"
            | "f26"
            | "f27"
            | "f28"
            | "f29"
            | "f30"
            | "f31"
            | "f32"
            | "f33"
            | "f34"
            | "f35"
            | "backspace"
            | "delete"
            | "left"
            | "right"
            | "up"
            | "down"
            | "pageup"
            | "pagedown"
            | "insert"
            | "home"
            | "end"
            | "back"
            | "forward"
            | "escape"
    )
}

impl std::fmt::Display for Keystroke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_modifiers(&self.modifiers, f)?;
        display_key(&self.key, f)
    }
}

impl std::fmt::Display for KeybindingKeystroke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_modifiers(self.modifiers(), f)?;
        display_key(self.key(), f)
    }
}

#[doc = " The state of the modifier keys at some point in time"]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize, Hash, JsonSchema)]
pub struct Modifiers {
    #[doc = " The control key"]
    #[serde(default)]
    pub control: bool,

    #[doc = " The alt key"]
    #[doc = " Sometimes also known as the 'meta' key"]
    #[serde(default)]
    pub alt: bool,

    #[doc = " The shift key"]
    #[serde(default)]
    pub shift: bool,

    #[doc = " The command key, on macos"]
    #[doc = " the windows key, on windows"]
    #[doc = " the super key, on linux"]
    #[serde(default)]
    pub platform: bool,

    #[doc = " The function key"]
    #[serde(default)]
    pub function: bool,
}

impl Modifiers {
    pub fn modified(&self) -> bool {
        self.control || self.alt || self.shift || self.platform || self.function
    }

    pub fn secondary(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.platform
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.control
        }
    }

    pub fn number_of_modifiers(&self) -> u8 {
        self.control as u8
            + self.alt as u8
            + self.shift as u8
            + self.platform as u8
            + self.function as u8
    }

    pub fn none() -> Modifiers {
        Default::default()
    }

    pub fn command() -> Modifiers {
        Modifiers {
            platform: true,
            ..Default::default()
        }
    }

    pub fn secondary_key() -> Modifiers {
        #[cfg(target_os = "macos")]
        {
            Modifiers {
                platform: true,
                ..Default::default()
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Modifiers {
                control: true,
                ..Default::default()
            }
        }
    }

    pub fn windows() -> Modifiers {
        Modifiers {
            platform: true,
            ..Default::default()
        }
    }

    pub fn super_key() -> Modifiers {
        Modifiers {
            platform: true,
            ..Default::default()
        }
    }

    pub fn control() -> Modifiers {
        Modifiers {
            control: true,
            ..Default::default()
        }
    }

    pub fn alt() -> Modifiers {
        Modifiers {
            alt: true,
            ..Default::default()
        }
    }

    pub fn shift() -> Modifiers {
        Modifiers {
            shift: true,
            ..Default::default()
        }
    }

    pub fn command_shift() -> Modifiers {
        Modifiers {
            shift: true,
            platform: true,
            ..Default::default()
        }
    }

    pub fn control_shift() -> Modifiers {
        Modifiers {
            shift: true,
            control: true,
            ..Default::default()
        }
    }

    pub fn is_subset_of(&self, other: &Modifiers) -> bool {
        (*other & *self) == *self
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(mut self, other: Self) -> Self::Output {
        self |= other;
        self
    }
}

impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, other: Self) {
        self.control |= other.control;
        self.alt |= other.alt;
        self.shift |= other.shift;
        self.platform |= other.platform;
        self.function |= other.function;
    }
}

impl std::ops::BitXor for Modifiers {
    type Output = Self;
    fn bitxor(mut self, rhs: Self) -> Self::Output {
        self ^= rhs;
        self
    }
}

impl std::ops::BitXorAssign for Modifiers {
    fn bitxor_assign(&mut self, other: Self) {
        self.control ^= other.control;
        self.alt ^= other.alt;
        self.shift ^= other.shift;
        self.platform ^= other.platform;
        self.function ^= other.function;
    }
}

impl std::ops::BitAnd for Modifiers {
    type Output = Self;
    fn bitand(mut self, rhs: Self) -> Self::Output {
        self &= rhs;
        self
    }
}

impl std::ops::BitAndAssign for Modifiers {
    fn bitand_assign(&mut self, other: Self) {
        self.control &= other.control;
        self.alt &= other.alt;
        self.shift &= other.shift;
        self.platform &= other.platform;
        self.function &= other.function;
    }
}

#[doc = " The state of the capslock key at some point in time"]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize, Hash, JsonSchema)]
pub struct Capslock {
    #[doc = " The capslock key is on"]
    #[serde(default)]
    pub on: bool,
}

impl AsKeystroke for Keystroke {
    fn as_keystroke(&self) -> &Keystroke {
        self
    }
}

impl AsKeystroke for KeybindingKeystroke {
    fn as_keystroke(&self) -> &Keystroke {
        &self.inner
    }
}

fn display_modifiers(modifiers: &Modifiers, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if modifiers.control {
        #[cfg(target_os = "macos")]
        f.write_char('^')?;

        #[cfg(not(target_os = "macos"))]
        write!(f, "ctrl-")?;
    }
    if modifiers.alt {
        #[cfg(target_os = "macos")]
        f.write_char('⌥')?;

        #[cfg(not(target_os = "macos"))]
        write!(f, "alt-")?;
    }
    if modifiers.platform {
        #[cfg(target_os = "macos")]
        f.write_char('⌘')?;

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        f.write_char('❖')?;

        #[cfg(target_os = "windows")]
        f.write_char('⊞')?;
    }
    if modifiers.shift {
        #[cfg(target_os = "macos")]
        f.write_char('⇧')?;

        #[cfg(not(target_os = "macos"))]
        write!(f, "shift-")?;
    }
    Ok(())
}

fn display_key(key: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let key = match key {
        #[cfg(target_os = "macos")]
        "backspace" => '⌫',
        #[cfg(target_os = "macos")]
        "up" => '↑',
        #[cfg(target_os = "macos")]
        "down" => '↓',
        #[cfg(target_os = "macos")]
        "left" => '←',
        #[cfg(target_os = "macos")]
        "right" => '→',
        #[cfg(target_os = "macos")]
        "tab" => '⇥',
        #[cfg(target_os = "macos")]
        "escape" => '⎋',
        #[cfg(target_os = "macos")]
        "shift" => '⇧',
        #[cfg(target_os = "macos")]
        "control" => '⌃',
        #[cfg(target_os = "macos")]
        "alt" => '⌥',
        #[cfg(target_os = "macos")]
        "platform" => '⌘',

        key if key.len() == 1 => key.chars().next().unwrap().to_ascii_uppercase(),
        key => return f.write_str(key),
    };
    f.write_char(key)
}

#[inline]
fn unparse(modifiers: &Modifiers, key: &str) -> String {
    let mut result = String::new();
    if modifiers.function {
        result.push_str("fn-");
    }
    if modifiers.control {
        result.push_str("ctrl-");
    }
    if modifiers.alt {
        result.push_str("alt-");
    }
    if modifiers.platform {
        #[cfg(target_os = "macos")]
        result.push_str("cmd-");

        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        result.push_str("super-");

        #[cfg(target_os = "windows")]
        result.push_str("win-");
    }
    if modifiers.shift {
        result.push_str("shift-");
    }
    result.push_str(&key);
    result
}
