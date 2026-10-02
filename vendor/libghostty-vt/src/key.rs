//! Encoding key events into terminal escape sequences,
//!
//! Supports both legacy encoding as well as Kitty Keyboard Protocol.
//!
//! # Basic Usage
//!
//!  1. Create an encoder instance with [`Encoder::new`].
//!  2. Configure encoder options with the various `Encoder::set_*` methods
//!     or [`Encoder::set_options_from_terminal`] if you have a [`Terminal`].
//!  3. For each key event:
//!     *  Create a key event with [`Event::new`] (or reuse an existing one)
//!     *  Set event properties (action, key, modifiers, etc.)
//!     *  Encode with [`Encoder::encode_to_vec`] (with a growable `Vec` buffer)
//!        or [`Encoder::encode`] (with a fixed byte buffer).
use std::mem::MaybeUninit;

use crate::{
    Error,
    alloc::{Allocator, Object},
    error::{Result, from_result, from_result_with_len},
    ffi::{self, KeyEncoderOption as Opt},
    terminal::Terminal,
};

/// Key encoder that converts key events into terminal escape sequences.
#[derive(Debug)]
pub struct Encoder<'alloc>(Object<'alloc, ffi::KeyEncoderImpl>);

impl<'alloc> Encoder<'alloc> {
    /// Create a new key encoder instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new key encoder instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::KeyEncoder = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_key_encoder_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    unsafe fn setopt(
        &mut self,
        option: ffi::KeyEncoderOption::Type,
        value: *const std::ffi::c_void,
    ) {
        unsafe { ffi::ghostty_key_encoder_setopt(self.0.as_raw(), option, value) }
    }

    /// Encode a key event into a terminal escape sequence.
    ///
    /// Converts a key event into the appropriate terminal escape sequence
    /// based on the encoder's current options.
    ///
    /// Not all key events produce output. For example, unmodified modifier
    /// keys typically don't generate escape sequences. Check the returned
    /// `Vec` to determine if any data was written.
    pub fn encode_to_vec(&mut self, event: &Event, vec: &mut Vec<u8>) -> Result<()> {
        let written = match self.encode_to_uninit_buf(event, vec.spare_capacity_mut()) {
            Ok(v) => Ok(v),
            Err(Error::OutOfSpace { required }) => {
                // Retry with more capacity
                vec.reserve(required);
                self.encode_to_uninit_buf(event, vec.spare_capacity_mut())
            }
            Err(e) => Err(e),
        };

        // SAFETY: A successful call to `encode_to_uninit_buf` assures us
        // that a `written` number of bytes have been initialized.
        unsafe { vec.set_len(vec.len() + written?) };
        Ok(())
    }

    /// Encode a key event into a terminal escape sequence.
    ///
    /// Converts a key event into the appropriate terminal escape sequence
    /// based on the encoder's current options. The sequence is written to
    /// the provided buffer.
    ///
    /// Not all key events produce output. For example, unmodified modifier
    /// keys typically don't generate escape sequences. Check the returned
    /// `usize` to determine if any data was written.
    ///
    /// If the output buffer is too small, this returns
    /// `Err(Error::OutOfSpace { required })` where `required` is the required
    /// buffer size. The caller can then allocate a larger buffer and call
    /// the method again.
    pub fn encode(&mut self, event: &Event, buf: &mut [u8]) -> Result<usize> {
        // SAFETY: It is always safe to reinterpret T as a MaybeUninit<T>.
        self.encode_to_uninit_buf(event, unsafe {
            std::slice::from_raw_parts_mut(buf.as_mut_ptr().cast(), buf.len())
        })
    }

    fn encode_to_uninit_buf(
        &mut self,
        event: &Event,
        buf: &mut [MaybeUninit<u8>],
    ) -> Result<usize> {
        let mut written: usize = 0;
        let result = unsafe {
            ffi::ghostty_key_encoder_encode(
                self.0.as_raw(),
                event.inner.as_raw(),
                buf.as_mut_ptr().cast(),
                buf.len(),
                &raw mut written,
            )
        };
        from_result_with_len(result, written)
    }

    /// Set encoder options from a terminal's current state.
    ///
    /// Reads the terminal's current modes and flags and applies them to the
    /// encoder's options. This sets cursor key application mode, keypad mode,
    /// alt escape prefix, modifyOtherKeys state, and Kitty keyboard protocol
    /// flags from the terminal state.
    ///
    /// Note that the `macos_option_as_alt` option cannot be determined from
    /// terminal state and is reset to [`OptionAsAlt::False`] by this call.
    /// Use [`Encoder::set_macos_option_as_alt`] to set it afterward if needed.
    pub fn set_options_from_terminal(&mut self, terminal: &Terminal<'_, '_>) -> &mut Self {
        unsafe {
            ffi::ghostty_key_encoder_setopt_from_terminal(self.0.as_raw(), terminal.inner.as_raw());
        }
        self
    }

    /// Set terminal DEC mode 1: cursor key application mode.
    pub fn set_cursor_key_application(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(
                Opt::CURSOR_KEY_APPLICATION,
                std::ptr::from_ref(&value).cast(),
            );
        }
        self
    }
    /// Set terminal DEC mode 66: keypad key application mode.
    pub fn set_keypad_key_application(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(
                Opt::KEYPAD_KEY_APPLICATION,
                std::ptr::from_ref(&value).cast(),
            );
        }
        self
    }
    /// Set terminal DEC mode 1035: ignore keypad with numlock.
    pub fn set_ignore_keypad_with_numlock(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(
                Opt::IGNORE_KEYPAD_WITH_NUMLOCK,
                std::ptr::from_ref(&value).cast(),
            );
        }
        self
    }
    /// Set terminal DEC mode 1036: alt sends escape prefix.
    pub fn set_alt_esc_prefix(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(Opt::ALT_ESC_PREFIX, std::ptr::from_ref(&value).cast());
        }
        self
    }
    /// Set xterm modifyOtherKeys mode 2.
    pub fn set_modify_other_keys_state_2(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(
                Opt::MODIFY_OTHER_KEYS_STATE_2,
                std::ptr::from_ref(&value).cast(),
            );
        }
        self
    }
    /// Set Kitty keyboard protocol flags.
    pub fn set_kitty_flags(&mut self, value: KittyKeyFlags) -> &mut Self {
        let value = value.bits();
        unsafe {
            self.setopt(Opt::KITTY_FLAGS, std::ptr::from_ref(&value).cast());
        }
        self
    }
    /// Set macOS option-as-alt setting.
    pub fn set_macos_option_as_alt(&mut self, value: OptionAsAlt) -> &mut Self {
        unsafe {
            self.setopt(Opt::MACOS_OPTION_AS_ALT, std::ptr::from_ref(&value).cast());
        }
        self
    }
    /// Backarrow key mode.
    ///
    /// See <https://vt100.net/dec/ek-vt3xx-tp-002.pdf> page 170.
    ///
    /// If `false` (the default), `backspace` emits 0x7f.
    /// If `true`, `backspace` emits 0x08.
    pub fn set_backarrow_key_mode(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(Opt::BACKARROW_KEY_MODE, std::ptr::from_ref(&value).cast());
        }
        self
    }
}

impl Drop for Encoder<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_key_encoder_free(self.0.as_raw()) }
    }
}

/// Keyboard input event containing information about the physical key pressed,
/// modifiers, and generated text.
#[derive(Debug)]
pub struct Event<'alloc> {
    inner: Object<'alloc, ffi::KeyEventImpl>,
    text: Option<String>,
}

impl<'alloc> Event<'alloc> {
    /// Create a new key event instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new key event instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::KeyEvent = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_key_event_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self {
            inner: Object::new(raw)?,
            text: None,
        })
    }

    /// Set the key action (press, release, repeat).
    pub fn set_action(&mut self, action: Action) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_action(self.inner.as_raw(), action.into()) }
        self
    }

    /// Get the key action (press, release, repeat).
    #[must_use]
    pub fn action(&self) -> Action {
        Action::try_from(unsafe { ffi::ghostty_key_event_get_action(self.inner.as_raw()) })
            .unwrap_or(Action::Press)
    }

    /// Set the physical key code.
    pub fn set_key(&mut self, key: Key) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_key(self.inner.as_raw(), key.into()) }
        self
    }

    /// Get the physical key code.
    #[must_use]
    pub fn key(&self) -> Key {
        Key::try_from(unsafe { ffi::ghostty_key_event_get_key(self.inner.as_raw()) })
            .unwrap_or(Key::Unidentified)
    }

    /// Set the modifier keys bitmask.
    pub fn set_mods(&mut self, mods: Mods) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_mods(self.inner.as_raw(), mods.bits()) }
        self
    }

    /// Get the modifier keys bitmask.
    #[must_use]
    pub fn mods(&self) -> Mods {
        Mods::from_bits_retain(unsafe { ffi::ghostty_key_event_get_mods(self.inner.as_raw()) })
    }

    /// Set the consumed modifiers bitmask.
    pub fn set_consumed_mods(&mut self, mods: Mods) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_consumed_mods(self.inner.as_raw(), mods.bits()) }
        self
    }

    /// Get the consumed modifiers bitmask.
    #[must_use]
    pub fn consumed_mods(&self) -> Mods {
        Mods::from_bits_retain(unsafe {
            ffi::ghostty_key_event_get_consumed_mods(self.inner.as_raw())
        })
    }

    /// Set whether the key event is part of a composition sequence.
    pub fn set_composing(&mut self, composing: bool) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_composing(self.inner.as_raw(), composing) }
        self
    }

    /// Get whether the key event is part of a composition sequence.
    #[must_use]
    pub fn is_composing(&self) -> bool {
        unsafe { ffi::ghostty_key_event_get_composing(self.inner.as_raw()) }
    }

    /// Set the UTF-8 text generated by the key for the current keyboard layout.
    ///
    /// Must contain the unmodified character before any Ctrl/Meta transformations.
    /// The encoder derives modifier sequences from the logical key and mods
    /// bitmask, not from this text. Do not pass C0 control characters
    /// (U+0000-U+001F, U+007F) or platform function key codes (e.g. macOS PUA
    /// U+F700-U+F8FF); pass NULL instead and let the encoder use the logical key.
    ///
    /// The event makes an internal copy of the text since the C API
    /// may reuse it without any rigid lifetime guarantees.
    pub fn set_utf8<S: Into<String>>(&mut self, text: Option<S>) -> &mut Self {
        self.text = text.map(Into::into);

        match &self.text {
            Some(text) => unsafe {
                ffi::ghostty_key_event_set_utf8(
                    self.inner.as_raw(),
                    text.as_ptr().cast(),
                    text.len(),
                );
            },
            None => unsafe {
                ffi::ghostty_key_event_set_utf8(self.inner.as_raw(), std::ptr::null(), 0);
            },
        }
        self
    }

    /// Get the UTF-8 text generated by the key event.
    pub fn utf8(&mut self) -> Option<&str> {
        // We actually sidestep the `ghostty_key_event_get_utf8` method to
        // avoid unclear lifetimes. See `set_utf8`.
        self.text.as_deref()
    }

    /// Set the unshifted Unicode codepoint.
    pub fn set_unshifted_codepoint(&mut self, codepoint: char) -> &mut Self {
        unsafe {
            ffi::ghostty_key_event_set_unshifted_codepoint(self.inner.as_raw(), codepoint.into());
        }
        self
    }

    /// Get the unshifted Unicode codepoint.
    #[must_use]
    pub fn unshifted_codepoint(&self) -> char {
        unsafe {
            char::from_u32_unchecked(ffi::ghostty_key_event_get_unshifted_codepoint(
                self.inner.as_raw(),
            ))
        }
    }
}

impl Drop for Event<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_key_event_free(self.inner.as_raw()) }
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[expect(missing_docs, reason = "self-explanatory")]
pub enum Key {
    Unidentified = 0,
    Backquote = 1,
    Backslash = 2,
    BracketLeft = 3,
    BracketRight = 4,
    Comma = 5,
    Digit0 = 6,
    Digit1 = 7,
    Digit2 = 8,
    Digit3 = 9,
    Digit4 = 10,
    Digit5 = 11,
    Digit6 = 12,
    Digit7 = 13,
    Digit8 = 14,
    Digit9 = 15,
    Equal = 16,
    IntlBackslash = 17,
    IntlRo = 18,
    IntlYen = 19,
    A = 20,
    B = 21,
    C = 22,
    D = 23,
    E = 24,
    F = 25,
    G = 26,
    H = 27,
    I = 28,
    J = 29,
    K = 30,
    L = 31,
    M = 32,
    N = 33,
    O = 34,
    P = 35,
    Q = 36,
    R = 37,
    S = 38,
    T = 39,
    U = 40,
    V = 41,
    W = 42,
    X = 43,
    Y = 44,
    Z = 45,
    Minus = 46,
    Period = 47,
    Quote = 48,
    Semicolon = 49,
    Slash = 50,
    AltLeft = 51,
    AltRight = 52,
    Backspace = 53,
    CapsLock = 54,
    ContextMenu = 55,
    ControlLeft = 56,
    ControlRight = 57,
    Enter = 58,
    MetaLeft = 59,
    MetaRight = 60,
    ShiftLeft = 61,
    ShiftRight = 62,
    Space = 63,
    Tab = 64,
    Convert = 65,
    KanaMode = 66,
    NonConvert = 67,
    Delete = 68,
    End = 69,
    Help = 70,
    Home = 71,
    Insert = 72,
    PageDown = 73,
    PageUp = 74,
    ArrowDown = 75,
    ArrowLeft = 76,
    ArrowRight = 77,
    ArrowUp = 78,
    NumLock = 79,
    Numpad0 = 80,
    Numpad1 = 81,
    Numpad2 = 82,
    Numpad3 = 83,
    Numpad4 = 84,
    Numpad5 = 85,
    Numpad6 = 86,
    Numpad7 = 87,
    Numpad8 = 88,
    Numpad9 = 89,
    NumpadAdd = 90,
    NumpadBackspace = 91,
    NumpadClear = 92,
    NumpadClearEntry = 93,
    NumpadComma = 94,
    NumpadDecimal = 95,
    NumpadDivide = 96,
    NumpadEnter = 97,
    NumpadEqual = 98,
    NumpadMemoryAdd = 99,
    NumpadMemoryClear = 100,
    NumpadMemoryRecall = 101,
    NumpadMemoryStore = 102,
    NumpadMemorySubtract = 103,
    NumpadMultiply = 104,
    NumpadParenLeft = 105,
    NumpadParenRight = 106,
    NumpadSubtract = 107,
    NumpadSeparator = 108,
    NumpadUp = 109,
    NumpadDown = 110,
    NumpadRight = 111,
    NumpadLeft = 112,
    NumpadBegin = 113,
    NumpadHome = 114,
    NumpadEnd = 115,
    NumpadInsert = 116,
    NumpadDelete = 117,
    NumpadPageUp = 118,
    NumpadPageDown = 119,
    Escape = 120,
    F1 = 121,
    F2 = 122,
    F3 = 123,
    F4 = 124,
    F5 = 125,
    F6 = 126,
    F7 = 127,
    F8 = 128,
    F9 = 129,
    F10 = 130,
    F11 = 131,
    F12 = 132,
    F13 = 133,
    F14 = 134,
    F15 = 135,
    F16 = 136,
    F17 = 137,
    F18 = 138,
    F19 = 139,
    F20 = 140,
    F21 = 141,
    F22 = 142,
    F23 = 143,
    F24 = 144,
    F25 = 145,
    Fn = 146,
    FnLock = 147,
    PrintScreen = 148,
    ScrollLock = 149,
    Pause = 150,
    BrowserBack = 151,
    BrowserFavorites = 152,
    BrowserForward = 153,
    BrowserHome = 154,
    BrowserRefresh = 155,
    BrowserSearch = 156,
    BrowserStop = 157,
    Eject = 158,
    LaunchApp1 = 159,
    LaunchApp2 = 160,
    LaunchMail = 161,
    MediaPlayPause = 162,
    MediaSelect = 163,
    MediaStop = 164,
    MediaTrackNext = 165,
    MediaTrackPrevious = 166,
    Power = 167,
    Sleep = 168,
    AudioVolumeDown = 169,
    AudioVolumeMute = 170,
    AudioVolumeUp = 171,
    WakeUp = 172,
    Copy = 173,
    Cut = 174,
    Paste = 175,
}

/// Key event action type.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum Action {
    /// Key was pressed.
    Press = ffi::KeyAction::PRESS,
    /// Key was released.
    Release = ffi::KeyAction::RELEASE,
    /// Key is being repeated (held down).
    Repeat = ffi::KeyAction::REPEAT,
}

/// macOS option key behavior.
///
/// Determines whether the "option" key on macOS is treated as "alt" or not.
/// See the Ghostty `macos-option-as-alt` configuration option for more details.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum OptionAsAlt {
    /// Option key is not treated as alt.
    False = ffi::OptionAsAlt::FALSE,
    /// Option key is treated as alt.
    True = ffi::OptionAsAlt::TRUE,
    /// Only left option key is treated as alt.
    Left = ffi::OptionAsAlt::LEFT,
    /// Only right option key is treated as alt.
    Right = ffi::OptionAsAlt::RIGHT,
}

bitflags::bitflags! {
    /// Keyboard modifier keys bitmask.
    ///
    /// A bitmask representing all keyboard modifiers. This tracks which modifier
    /// keys are pressed and, where supported by the platform, which side (left or
    /// right) of each modifier is active.
    ///
    /// Modifier side bits are only meaningful when the corresponding modifier bit
    /// is set. Not all platforms support distinguishing between left and right
    /// modifier keys and Ghostty is built to expect that some platforms may not
    /// provide this information.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Mods: u16 {
        /// Shift key is pressed.
        const SHIFT = ffi::MODS_SHIFT;
        /// Alt key is pressed.
        const ALT = ffi::MODS_ALT;
        /// Control key is pressed.
        const CTRL = ffi::MODS_CTRL;
        /// Super/Command/Windows key is pressed.
        const SUPER = ffi::MODS_SUPER;
        /// Caps Lock is active.
        const CAPS_LOCK = ffi::MODS_CAPS_LOCK;
        /// Num Lock is active.
        const NUM_LOCK = ffi::MODS_NUM_LOCK;
        /// Right Shift is pressed (unset = left, set = right).
        ///
        /// Only valid when [`Mods::SHIFT`] is set.
        const SHIFT_SIDE = ffi::MODS_SHIFT_SIDE;
        /// Right Alt is pressed (unset = left, set = right).
        ///
        /// Only valid when [`Mods::ALT`] is set.
        const ALT_SIDE = ffi::MODS_ALT_SIDE;
        /// Right Control is pressed (unset = left, set = right).
        ///
        /// Only valid when [`Mods::CTRL`] is set.
        const CTRL_SIDE = ffi::MODS_CTRL_SIDE;
        /// Right Super is pressed (unset = left, set = right).
        ///
        /// Only valid when [`Mods::SUPER`] is set.
        const SUPER_SIDE = ffi::MODS_SUPER_SIDE;
    }

    /// Kitty keyboard protocol flags.
    ///
    /// Bitflags representing the various modes of the Kitty keyboard protocol.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct KittyKeyFlags: u8 {
        /// Kitty keyboard protocol disabled (all flags off).
        const DISABLED = ffi::KITTY_KEY_DISABLED;
        /// Disambiguate escape codes.
        const DISAMBIGUATE = ffi::KITTY_KEY_DISAMBIGUATE;
        /// Report key press and release events.
        const REPORT_EVENTS = ffi::KITTY_KEY_REPORT_EVENTS;
        /// Report alternate key codes.
        const REPORT_ALTERNATES = ffi::KITTY_KEY_REPORT_ALTERNATES;
        /// Report all key events including those normally handled by the terminal.
        const REPORT_ALL = ffi::KITTY_KEY_REPORT_ALL;
        /// Report associated text with key events
        const REPORT_ASSOCIATED = ffi::KITTY_KEY_REPORT_ASSOCIATED;
        /// All Kitty keyboard protocol flags enabled
        const ALL = ffi::KITTY_KEY_ALL;
    }
}
