use std::mem::MaybeUninit;

use crate::{
    Error,
    alloc::{Allocator, Object},
    error::{Result, from_result, from_result_with_len},
    ffi::{self, KeyEncoderOption as Opt},
    terminal::Terminal,
};

#[derive(Debug)]
pub struct Encoder<'alloc>(Object<'alloc, ffi::KeyEncoderImpl>);

impl<'alloc> Encoder<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
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

    pub fn encode_to_vec(&mut self, event: &Event, vec: &mut Vec<u8>) -> Result<()> {
        let written = match self.encode_to_uninit_buf(event, vec.spare_capacity_mut()) {
            Ok(v) => Ok(v),
            Err(Error::OutOfSpace { required }) => {
                vec.reserve(required);
                self.encode_to_uninit_buf(event, vec.spare_capacity_mut())
            }
            Err(e) => Err(e),
        };

        unsafe { vec.set_len(vec.len() + written?) };
        Ok(())
    }

    pub fn encode(&mut self, event: &Event, buf: &mut [u8]) -> Result<usize> {
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

    pub fn set_options_from_terminal(&mut self, terminal: &Terminal<'_, '_>) -> &mut Self {
        unsafe {
            ffi::ghostty_key_encoder_setopt_from_terminal(self.0.as_raw(), terminal.inner.as_raw());
        }
        self
    }

    pub fn set_cursor_key_application(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(
                Opt::CURSOR_KEY_APPLICATION,
                std::ptr::from_ref(&value).cast(),
            );
        }
        self
    }
    pub fn set_keypad_key_application(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(
                Opt::KEYPAD_KEY_APPLICATION,
                std::ptr::from_ref(&value).cast(),
            );
        }
        self
    }
    pub fn set_ignore_keypad_with_numlock(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(
                Opt::IGNORE_KEYPAD_WITH_NUMLOCK,
                std::ptr::from_ref(&value).cast(),
            );
        }
        self
    }
    pub fn set_alt_esc_prefix(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(Opt::ALT_ESC_PREFIX, std::ptr::from_ref(&value).cast());
        }
        self
    }
    pub fn set_modify_other_keys_state_2(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(
                Opt::MODIFY_OTHER_KEYS_STATE_2,
                std::ptr::from_ref(&value).cast(),
            );
        }
        self
    }
    pub fn set_kitty_flags(&mut self, value: KittyKeyFlags) -> &mut Self {
        let value = value.bits();
        unsafe {
            self.setopt(Opt::KITTY_FLAGS, std::ptr::from_ref(&value).cast());
        }
        self
    }
    pub fn set_macos_option_as_alt(&mut self, value: OptionAsAlt) -> &mut Self {
        unsafe {
            self.setopt(Opt::MACOS_OPTION_AS_ALT, std::ptr::from_ref(&value).cast());
        }
        self
    }
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

#[derive(Debug)]
pub struct Event<'alloc> {
    inner: Object<'alloc, ffi::KeyEventImpl>,
    text: Option<String>,
}

impl<'alloc> Event<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
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

    pub fn set_action(&mut self, action: Action) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_action(self.inner.as_raw(), action.into()) }
        self
    }

    #[must_use]
    pub fn action(&self) -> Action {
        Action::try_from(unsafe { ffi::ghostty_key_event_get_action(self.inner.as_raw()) })
            .unwrap_or(Action::Press)
    }

    pub fn set_key(&mut self, key: Key) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_key(self.inner.as_raw(), key.into()) }
        self
    }

    #[must_use]
    pub fn key(&self) -> Key {
        Key::try_from(unsafe { ffi::ghostty_key_event_get_key(self.inner.as_raw()) })
            .unwrap_or(Key::Unidentified)
    }

    pub fn set_mods(&mut self, mods: Mods) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_mods(self.inner.as_raw(), mods.bits()) }
        self
    }

    #[must_use]
    pub fn mods(&self) -> Mods {
        Mods::from_bits_retain(unsafe { ffi::ghostty_key_event_get_mods(self.inner.as_raw()) })
    }

    pub fn set_consumed_mods(&mut self, mods: Mods) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_consumed_mods(self.inner.as_raw(), mods.bits()) }
        self
    }

    #[must_use]
    pub fn consumed_mods(&self) -> Mods {
        Mods::from_bits_retain(unsafe {
            ffi::ghostty_key_event_get_consumed_mods(self.inner.as_raw())
        })
    }

    pub fn set_composing(&mut self, composing: bool) -> &mut Self {
        unsafe { ffi::ghostty_key_event_set_composing(self.inner.as_raw(), composing) }
        self
    }

    #[must_use]
    pub fn is_composing(&self) -> bool {
        unsafe { ffi::ghostty_key_event_get_composing(self.inner.as_raw()) }
    }

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

    pub fn utf8(&mut self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn set_unshifted_codepoint(&mut self, codepoint: char) -> &mut Self {
        unsafe {
            ffi::ghostty_key_event_set_unshifted_codepoint(self.inner.as_raw(), codepoint.into());
        }
        self
    }

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

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
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

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum Action {
    Press = ffi::KeyAction::PRESS,
    Release = ffi::KeyAction::RELEASE,
    Repeat = ffi::KeyAction::REPEAT,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum OptionAsAlt {
    False = ffi::OptionAsAlt::FALSE,
    True = ffi::OptionAsAlt::TRUE,
    Left = ffi::OptionAsAlt::LEFT,
    Right = ffi::OptionAsAlt::RIGHT,
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Mods: u16 {
        const SHIFT = ffi::MODS_SHIFT;
        const ALT = ffi::MODS_ALT;
        const CTRL = ffi::MODS_CTRL;
        const SUPER = ffi::MODS_SUPER;
        const CAPS_LOCK = ffi::MODS_CAPS_LOCK;
        const NUM_LOCK = ffi::MODS_NUM_LOCK;
        const SHIFT_SIDE = ffi::MODS_SHIFT_SIDE;
        const ALT_SIDE = ffi::MODS_ALT_SIDE;
        const CTRL_SIDE = ffi::MODS_CTRL_SIDE;
        const SUPER_SIDE = ffi::MODS_SUPER_SIDE;
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct KittyKeyFlags: u8 {
        const DISABLED = ffi::KITTY_KEY_DISABLED;
        const DISAMBIGUATE = ffi::KITTY_KEY_DISAMBIGUATE;
        const REPORT_EVENTS = ffi::KITTY_KEY_REPORT_EVENTS;
        const REPORT_ALTERNATES = ffi::KITTY_KEY_REPORT_ALTERNATES;
        const REPORT_ALL = ffi::KITTY_KEY_REPORT_ALL;
        const REPORT_ASSOCIATED = ffi::KITTY_KEY_REPORT_ASSOCIATED;
        const ALL = ffi::KITTY_KEY_ALL;
    }
}
