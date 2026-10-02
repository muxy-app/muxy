//! Encoding mouse events into terminal escape sequences.
//!
//! Supports X10, UTF-8, SGR, urxvt, and SGR-Pixels mouse protocols.
//!
//! # Basic Usage
//!
//!  1. Create an encoder instance with [`Encoder::new`].
//!  2. Configure encoder options with the various `Encoder::with_*` methods
//!     or [`Encoder::set_options_from_terminal`].
//!  3. For each mouse event:
//!     *  Create a mouse event with [`Event::new`] (or reuse an old one).
//!     *  Set event properties (action, button, modifiers, position).
//!     *  Encode with [`Encoder::encode_to_vec`] (with a growable `Vec` buffer)
//!        or [`Encoder::encode`] (with a fixed byte buffer).

use std::mem::MaybeUninit;

use crate::{
    alloc::{Allocator, Object},
    error::{Error, Result, from_result, from_result_with_len},
    ffi::{self, MouseEncoderOption as Opt},
    key,
    terminal::Terminal,
};

#[doc(inline)]
pub use ffi::MousePosition as Position;

/// Mouse encoder that converts normalized mouse events into
/// terminal escape sequences.
#[derive(Debug)]
pub struct Encoder<'alloc>(Object<'alloc, ffi::MouseEncoderImpl>);

impl<'alloc> Encoder<'alloc> {
    /// Create a new mouse encoder instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new mouse encoder instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::MouseEncoder = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_mouse_encoder_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    unsafe fn setopt(
        &mut self,
        option: ffi::MouseEncoderOption::Type,
        value: *const std::ffi::c_void,
    ) {
        unsafe { ffi::ghostty_mouse_encoder_setopt(self.0.as_raw(), option, value) }
    }

    /// Encode a key event into a terminal escape sequence.
    ///
    /// Converts a key event into the appropriate terminal escape sequence
    /// based on the encoder's current options. The provided `Vec` byte buffer
    /// will be grown automatically if more capacity is needed.
    ///
    /// Not all key events produce output. For example, unmodified modifier
    /// keys typically don't generate escape sequences. Check the returned
    /// `usize` to determine if any data was written.
    pub fn encode_to_vec(&mut self, event: &Event, vec: &mut Vec<u8>) -> Result<()> {
        let remaining = vec.capacity() - vec.len();

        let written = match self.encode_to_uninit_buf(event, vec.spare_capacity_mut()) {
            Ok(v) => Ok(v),
            Err(Error::OutOfSpace { required }) => {
                // Retry with more capacity
                vec.reserve(required - remaining);
                self.encode_to_uninit_buf(event, vec.spare_capacity_mut())
            }
            Err(e) => Err(e),
        };

        // SAFETY: A successful call to `encode_to_uninit_buf` assures us
        // that a `written` number of bytes have been initialized.
        unsafe { vec.set_len(vec.len() + written?) };
        Ok(())
    }

    /// Encode a mouse event into a terminal escape sequence.
    ///
    /// Not all mouse events produce output. In such cases this returns `Ok(0)`.
    ///
    /// If the output buffer is too small, this returns
    /// `Err(Error::OutOfSpace { required })` where `required` is the required size.
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
            ffi::ghostty_mouse_encoder_encode(
                self.0.as_raw(),
                event.0.as_raw(),
                buf.as_mut_ptr().cast(),
                buf.len(),
                &raw mut written,
            )
        };
        from_result_with_len(result, written)
    }

    /// Set encoder options from a terminal's current state.
    ///
    /// This sets tracking mode and output format from terminal state.
    /// It does not modify size or any-button state.
    pub fn set_options_from_terminal(&mut self, terminal: &Terminal<'_, '_>) -> &mut Self {
        unsafe {
            ffi::ghostty_mouse_encoder_setopt_from_terminal(
                self.0.as_raw(),
                terminal.inner.as_raw(),
            );
        }
        self
    }
    /// Set mouse tracking mode.
    pub fn set_tracking_mode(&mut self, value: TrackingMode) -> &mut Self {
        unsafe {
            self.setopt(Opt::EVENT, std::ptr::from_ref(&value).cast());
        }
        self
    }
    /// Set mouse output format.
    pub fn set_format(&mut self, value: Format) -> &mut Self {
        unsafe {
            self.setopt(Opt::FORMAT, std::ptr::from_ref(&value).cast());
        }
        self
    }
    /// Set renderer size context.
    pub fn set_size(&mut self, value: EncoderSize) -> &mut Self {
        let raw: ffi::MouseEncoderSize = value.into();
        unsafe {
            self.setopt(Opt::SIZE, std::ptr::from_ref(&raw).cast());
        }
        self
    }
    /// Set whether any mouse button is currently pressed.
    pub fn set_any_button_pressed(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(Opt::ANY_BUTTON_PRESSED, std::ptr::from_ref(&value).cast());
        }
        self
    }
    /// Set whether to enable motion deduplication by last cell.
    pub fn set_track_last_cell(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(Opt::TRACK_LAST_CELL, std::ptr::from_ref(&value).cast());
        }
        self
    }

    /// Reset internal encoder state.
    ///
    /// This clears motion deduplication state (last tracked cell).
    pub fn reset(&mut self) {
        unsafe { ffi::ghostty_mouse_encoder_reset(self.0.as_raw()) }
    }
}

impl Drop for Encoder<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_mouse_encoder_free(self.0.as_raw()) }
    }
}

/// Normalized mouse input event containing action, button, modifiers, and
/// surface-space position.
#[derive(Debug)]
pub struct Event<'alloc>(Object<'alloc, ffi::MouseEventImpl>);

impl<'alloc> Event<'alloc> {
    /// Create a new mouse event instance.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new mouse event instance with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::MouseEvent = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_mouse_event_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    /// Set the event action.
    pub fn set_action(&mut self, action: Action) -> &mut Self {
        unsafe {
            ffi::ghostty_mouse_event_set_action(self.0.as_raw(), action as ffi::MouseAction::Type);
        }
        self
    }

    /// Get the event action.
    #[must_use]
    pub fn action(&self) -> Action {
        unsafe { ffi::ghostty_mouse_event_get_action(self.0.as_raw()) }
            .try_into()
            .unwrap_or(Action::Press)
    }

    /// Set the event button.
    pub fn set_button(&mut self, button: Option<Button>) -> &mut Self {
        if let Some(button) = button {
            unsafe {
                ffi::ghostty_mouse_event_set_button(
                    self.0.as_raw(),
                    button as ffi::MouseButton::Type,
                );
            }
        } else {
            unsafe { ffi::ghostty_mouse_event_clear_button(self.0.as_raw()) }
        }
        self
    }

    /// Get the event button.
    #[must_use]
    pub fn button(&self) -> Option<Button> {
        let mut button = ffi::MouseButton::UNKNOWN;
        let has_button =
            unsafe { ffi::ghostty_mouse_event_get_button(self.0.as_raw(), &raw mut button) };
        if has_button {
            Some(button.try_into().unwrap_or(Button::Unknown))
        } else {
            None
        }
    }

    /// Set keyboard modifiers held during the event.
    pub fn set_mods(&mut self, mods: key::Mods) -> &mut Self {
        unsafe { ffi::ghostty_mouse_event_set_mods(self.0.as_raw(), mods.bits()) }
        self
    }

    /// Get keyboard modifiers held during the event.
    #[must_use]
    pub fn mods(&self) -> key::Mods {
        key::Mods::from_bits_retain(unsafe { ffi::ghostty_mouse_event_get_mods(self.0.as_raw()) })
    }

    /// Set the event position in surface-space pixels.
    pub fn set_position(&mut self, pos: Position) -> &mut Self {
        unsafe { ffi::ghostty_mouse_event_set_position(self.0.as_raw(), pos) }
        self
    }

    /// Get the event position in surface-space pixels.
    #[must_use]
    pub fn position(&self) -> Position {
        unsafe { ffi::ghostty_mouse_event_get_position(self.0.as_raw()) }
    }
}

impl Drop for Event<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_mouse_event_free(self.0.as_raw()) }
    }
}

/// Mouse tracking mode.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum TrackingMode {
    /// Mouse reporting disabled.
    None = ffi::MouseTrackingMode::NONE,
    /// X10 mouse mode.
    X10 = ffi::MouseTrackingMode::X10,
    /// Normal mouse mode (press/release only).
    Normal = ffi::MouseTrackingMode::NORMAL,
    /// Button-event tracking mode.
    Button = ffi::MouseTrackingMode::BUTTON,
    /// Any-event tracking mode.
    Any = ffi::MouseTrackingMode::ANY,
}

/// Mouse output format.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[expect(missing_docs, reason = "missing upstream docs")]
pub enum Format {
    X10 = ffi::MouseFormat::X10,
    Utf8 = ffi::MouseFormat::UTF8,
    Sgr = ffi::MouseFormat::SGR,
    Urxvt = ffi::MouseFormat::URXVT,
    SgrPixels = ffi::MouseFormat::SGR_PIXELS,
}

/// Mouse encoder size and geometry context.
///
/// This describes the rendered terminal geometry used to convert surface-space
/// positions into encoded coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncoderSize {
    /// Full screen width in pixels.
    pub screen_width: u32,
    /// Full screen height in pixels.
    pub screen_height: u32,
    /// Cell width in pixels. Must be non-zero.
    pub cell_width: u32,
    /// Cell height in pixels. Must be non-zero.
    pub cell_height: u32,
    /// Top padding in pixels.
    pub padding_top: u32,
    /// Bottom padding in pixels.
    pub padding_bottom: u32,
    /// Right padding in pixels.
    pub padding_right: u32,
    /// Left padding in pixels.
    pub padding_left: u32,
}

impl From<EncoderSize> for ffi::MouseEncoderSize {
    fn from(value: EncoderSize) -> Self {
        Self {
            size: std::mem::size_of::<Self>(),
            screen_width: value.screen_width,
            screen_height: value.screen_height,
            cell_width: value.cell_width,
            cell_height: value.cell_height,
            padding_top: value.padding_top,
            padding_bottom: value.padding_bottom,
            padding_right: value.padding_right,
            padding_left: value.padding_left,
        }
    }
}

/// Mouse event action type.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum Action {
    /// Mouse button was pressed.
    Press = ffi::MouseAction::PRESS,
    /// Mouse button was released.
    Release = ffi::MouseAction::RELEASE,
    /// Mouse moved.
    Motion = ffi::MouseAction::MOTION,
}

/// Mouse event action identity.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[expect(missing_docs, reason = "self-explanatory")]
pub enum Button {
    Unknown = ffi::MouseButton::UNKNOWN,
    Left = ffi::MouseButton::LEFT,
    Right = ffi::MouseButton::RIGHT,
    Middle = ffi::MouseButton::MIDDLE,
    Four = ffi::MouseButton::FOUR,
    Five = ffi::MouseButton::FIVE,
    Six = ffi::MouseButton::SIX,
    Seven = ffi::MouseButton::SEVEN,
    Eight = ffi::MouseButton::EIGHT,
    Nine = ffi::MouseButton::NINE,
    Ten = ffi::MouseButton::TEN,
    Eleven = ffi::MouseButton::ELEVEN,
}
