use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::ErrorCode;

/// Input interpreted by the session owner using its current terminal modes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum TerminalInput {
    #[n(0)]
    Key(#[n(0)] KeyEvent),
    #[n(1)]
    Paste(
        #[n(0)]
        #[cbor(with = "crate::wire::cbor::bytes")]
        Vec<u8>,
    ),
    #[n(2)]
    Focus(#[n(0)] bool),
    #[n(3)]
    ClearScreen,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum KeyAction {
    #[default]
    #[n(0)]
    Press,
    #[n(1)]
    Repeat,
    #[n(2)]
    Release,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[cbor(transparent)]
pub struct KeyModifiers(#[n(0)] pub u8);

impl KeyModifiers {
    pub const SHIFT: Self = Self(1);
    pub const ALT: Self = Self(2);
    pub const CTRL: Self = Self(4);
    pub const SUPER: Self = Self(8);
    pub const CAPS_LOCK: Self = Self(16);
    pub const NUM_LOCK: Self = Self(32);

    pub fn contains(self, modifier: Self) -> bool {
        self.0 & modifier.0 == modifier.0
    }

    pub fn set(&mut self, modifier: Self, pressed: bool) {
        if pressed {
            self.0 |= modifier.0;
        } else {
            self.0 &= !modifier.0;
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct KeyEvent {
    /// Logical character or lowercase key name: enter, up, pageup, f1, numpad0, etc.
    /// Empty for text without a corresponding key. Not a native library's key number.
    #[n(0)]
    pub key: String,
    #[n(1)]
    pub action: KeyAction,
    #[n(2)]
    pub modifiers: KeyModifiers,
    #[n(3)]
    pub consumed_modifiers: KeyModifiers,
    /// Layout-generated text before control/meta transformations, not pasted text.
    #[n(4)]
    pub text: String,
    /// Zero when the platform cannot supply an unshifted Unicode scalar.
    #[n(5)]
    pub unshifted_codepoint: u32,
    #[n(6)]
    pub option_as_alt: bool,
}

impl TerminalInput {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match self {
            Self::Key(event) => {
                let printable =
                    |ch: char| !ch.is_control() && !('\u{f700}'..='\u{f8ff}').contains(&ch);
                if event.key.len() > 32
                    || event.key.chars().any(char::is_control)
                    || event.text.len() > 4096
                    || !event.text.chars().all(printable)
                    || (event.unshifted_codepoint != 0
                        && !char::from_u32(event.unshifted_codepoint).is_some_and(printable))
                {
                    return Err(ErrorCode::BadRequest);
                }
                Ok(())
            }
            Self::Paste(bytes) => crate::validate_input(bytes),
            Self::Focus(_) | Self::ClearScreen => Ok(()),
        }
    }
}
