use std::ffi::{CStr, CString};
use std::ops::{BitOr, BitOrAssign};
use std::ptr;

use ghostty_sys::ffi;
use thiserror::Error;

pub const MODS_NONE: ffi::ghostty_input_mods_e = ffi::ghostty_input_mods_e_GHOSTTY_MODS_NONE;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Modifiers(ffi::ghostty_input_mods_e);

impl Modifiers {
    pub const NONE: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_NONE);
    pub const SHIFT: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_SHIFT);
    pub const CONTROL: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_CTRL);
    pub const ALT: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_ALT);
    pub const SUPER: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_SUPER);
    pub const CAPS_LOCK: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_CAPS);
    pub const NUM_LOCK: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_NUM);
    pub const SHIFT_RIGHT: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_SHIFT_RIGHT);
    pub const CONTROL_RIGHT: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_CTRL_RIGHT);
    pub const ALT_RIGHT: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_ALT_RIGHT);
    pub const SUPER_RIGHT: Self = Self(ffi::ghostty_input_mods_e_GHOSTTY_MODS_SUPER_RIGHT);

    pub fn from_flags(shift: bool, control: bool, option: bool, command: bool) -> Self {
        let mut modifiers = Self::NONE;
        if shift {
            modifiers |= Self::SHIFT;
        }
        if control {
            modifiers |= Self::CONTROL;
        }
        if option {
            modifiers |= Self::ALT;
        }
        if command {
            modifiers |= Self::SUPER;
        }
        modifiers
    }

    pub fn consumed_by_text(shift: bool, option: bool) -> Self {
        let mut modifiers = Self::NONE;
        if shift {
            modifiers |= Self::SHIFT;
        }
        if option {
            modifiers |= Self::ALT;
        }
        modifiers
    }

    pub const fn from_raw(raw: ffi::ghostty_input_mods_e) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> ffi::ghostty_input_mods_e {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == ffi::ghostty_input_mods_e_GHOSTTY_MODS_NONE
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.with(rhs)
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.with(rhs);
    }
}

pub fn mods_from_flags(
    shift: bool,
    control: bool,
    option: bool,
    command: bool,
) -> ffi::ghostty_input_mods_e {
    Modifiers::from_flags(shift, control, option, command).raw()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyAction {
    Release,
    Press,
    Repeat,
}

impl KeyAction {
    pub const fn from_key_down(is_repeat: bool) -> Self {
        if is_repeat { Self::Repeat } else { Self::Press }
    }

    const fn is_key_down(self) -> bool {
        matches!(self, Self::Press | Self::Repeat)
    }

    pub(crate) const fn as_raw(self) -> ffi::ghostty_input_action_e {
        match self {
            Self::Release => ffi::ghostty_input_action_e_GHOSTTY_ACTION_RELEASE,
            Self::Press => ffi::ghostty_input_action_e_GHOSTTY_ACTION_PRESS,
            Self::Repeat => ffi::ghostty_input_action_e_GHOSTTY_ACTION_REPEAT,
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum KeyboardInputError {
    #[error("keyboard text contains an interior NUL byte")]
    TextContainsNul,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardInput {
    action: KeyAction,
    keycode: u32,
    modifiers: Modifiers,
    consumed_modifiers: Modifiers,
    unshifted_codepoint: u32,
    text: Option<CString>,
    composing: bool,
}

impl KeyboardInput {
    pub fn new(
        action: KeyAction,
        keycode: u32,
        modifiers: Modifiers,
        unshifted_codepoint: u32,
        characters: Option<&str>,
    ) -> Self {
        let text = action
            .is_key_down()
            .then_some(characters)
            .flatten()
            .filter(|text| is_printable_text(text))
            .and_then(|text| CString::new(text).ok());

        Self {
            action,
            keycode,
            modifiers,
            consumed_modifiers: Modifiers::NONE,
            unshifted_codepoint,
            text,
            composing: false,
        }
    }

    pub fn with_text(mut self, text: impl AsRef<str>) -> Result<Self, KeyboardInputError> {
        self.text =
            Some(CString::new(text.as_ref()).map_err(|_| KeyboardInputError::TextContainsNul)?);
        Ok(self)
    }

    pub fn without_text(mut self) -> Self {
        self.text = None;
        self
    }

    pub const fn with_consumed_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.consumed_modifiers = modifiers;
        self
    }

    pub const fn with_composing(mut self, composing: bool) -> Self {
        self.composing = composing;
        self
    }

    pub const fn action(&self) -> KeyAction {
        self.action
    }

    pub const fn keycode(&self) -> u32 {
        self.keycode
    }

    pub const fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub const fn consumed_modifiers(&self) -> Modifiers {
        self.consumed_modifiers
    }

    pub const fn unshifted_codepoint(&self) -> u32 {
        self.unshifted_codepoint
    }

    pub fn text(&self) -> Option<&CStr> {
        self.text.as_deref()
    }

    pub const fn is_composing(&self) -> bool {
        self.composing
    }

    pub(crate) fn as_ffi(&self) -> ffi::ghostty_input_key_s {
        ffi::ghostty_input_key_s {
            action: self.action.as_raw(),
            mods: self.modifiers.raw(),
            consumed_mods: self.consumed_modifiers.raw(),
            keycode: self.keycode,
            text: self.text.as_ref().map_or(ptr::null(), |text| text.as_ptr()),
            unshifted_codepoint: self.unshifted_codepoint,
            composing: self.composing,
        }
    }
}

pub fn is_special_character(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_special_codepoint)
}

pub fn is_printable_text(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| !is_special_codepoint(character))
}

fn is_special_codepoint(character: char) -> bool {
    let codepoint = character as u32;
    character.is_control()
        || (0xE000..=0xF8FF).contains(&codepoint)
        || (0xF0000..=0xFFFFD).contains(&codepoint)
        || (0x100000..=0x10FFFD).contains(&codepoint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_helpers_preserve_every_known_bit() {
        let all = Modifiers::SHIFT
            | Modifiers::CONTROL
            | Modifiers::ALT
            | Modifiers::SUPER
            | Modifiers::CAPS_LOCK
            | Modifiers::NUM_LOCK
            | Modifiers::SHIFT_RIGHT
            | Modifiers::CONTROL_RIGHT
            | Modifiers::ALT_RIGHT
            | Modifiers::SUPER_RIGHT;

        assert!(all.contains(Modifiers::SHIFT | Modifiers::ALT_RIGHT));
        assert!(!all.is_empty());
        assert!(!all.without(Modifiers::SHIFT).contains(Modifiers::SHIFT));
        assert_eq!(Modifiers::from_raw(all.raw()), all);
    }

    #[test]
    fn appkit_and_consumed_modifier_builders_use_expected_bits() {
        assert_eq!(
            Modifiers::from_flags(true, true, true, true),
            Modifiers::SHIFT | Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER
        );
        assert_eq!(
            Modifiers::consumed_by_text(true, true),
            Modifiers::SHIFT | Modifiers::ALT
        );
        assert_eq!(Modifiers::consumed_by_text(false, false), Modifiers::NONE);
    }

    #[test]
    fn key_actions_match_the_pinned_abi() {
        assert_eq!(
            KeyAction::Release.as_raw(),
            ffi::ghostty_input_action_e_GHOSTTY_ACTION_RELEASE
        );
        assert_eq!(
            KeyAction::Press.as_raw(),
            ffi::ghostty_input_action_e_GHOSTTY_ACTION_PRESS
        );
        assert_eq!(
            KeyAction::Repeat.as_raw(),
            ffi::ghostty_input_action_e_GHOSTTY_ACTION_REPEAT
        );
        assert_eq!(KeyAction::from_key_down(false), KeyAction::Press);
        assert_eq!(KeyAction::from_key_down(true), KeyAction::Repeat);
    }

    #[test]
    fn special_and_mixed_characters_are_not_printable_text() {
        assert!(is_special_character("\u{7f}"));
        assert!(is_special_character("\u{f700}"));
        assert!(is_special_character("\u{e000}"));
        assert!(!is_special_character("a\u{7f}"));

        assert!(is_printable_text("Aé🙂"));
        assert!(!is_printable_text(""));
        assert!(!is_printable_text("\n"));
        assert!(!is_printable_text("a\u{7f}"));
        assert!(!is_printable_text("\u{f0000}"));
    }

    #[test]
    fn complete_keyboard_abi_is_preserved() {
        let modifiers = Modifiers::SHIFT | Modifiers::SUPER_RIGHT;
        let consumed = Modifiers::SHIFT | Modifiers::ALT;
        let input = KeyboardInput::new(KeyAction::Repeat, 0x31, modifiers, 'a' as u32, Some("A"))
            .with_consumed_modifiers(consumed)
            .with_composing(true);
        let raw = input.as_ffi();

        assert_eq!(
            raw.action,
            ffi::ghostty_input_action_e_GHOSTTY_ACTION_REPEAT
        );
        assert_eq!(raw.keycode, 0x31);
        assert_eq!(raw.mods, modifiers.raw());
        assert_eq!(raw.consumed_mods, consumed.raw());
        assert_eq!(raw.unshifted_codepoint, 'a' as u32);
        assert!(!raw.text.is_null());
        assert_eq!(input.text().map(CStr::to_bytes), Some(&b"A"[..]));
        assert!(raw.composing);
    }

    #[test]
    fn release_and_special_keys_have_no_automatic_text() {
        let released =
            KeyboardInput::new(KeyAction::Release, 0x33, Modifiers::NONE, 0x7f, Some("x"));
        assert!(released.text().is_none());
        assert!(released.as_ffi().text.is_null());

        let special =
            KeyboardInput::new(KeyAction::Press, 0x7b, Modifiers::NONE, 0, Some("\u{f702}"));
        assert!(special.text().is_none());
        assert!(special.as_ffi().text.is_null());
    }

    #[test]
    fn explicit_text_supports_ime_output_and_rejects_nul() {
        let input = KeyboardInput::new(KeyAction::Press, 0, Modifiers::NONE, 0, None)
            .with_text("日本語")
            .expect("valid IME text");
        assert_eq!(input.text().map(CStr::to_bytes), Some("日本語".as_bytes()));

        assert_eq!(
            KeyboardInput::new(KeyAction::Press, 0, Modifiers::NONE, 0, None)
                .with_text("bad\0text"),
            Err(KeyboardInputError::TextContainsNul)
        );
    }
}
