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

#[derive(Debug)]
pub struct Encoder<'alloc>(Object<'alloc, ffi::MouseEncoderImpl>);

impl<'alloc> Encoder<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
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

    pub fn encode_to_vec(&mut self, event: &Event, vec: &mut Vec<u8>) -> Result<()> {
        let remaining = vec.capacity() - vec.len();

        let written = match self.encode_to_uninit_buf(event, vec.spare_capacity_mut()) {
            Ok(v) => Ok(v),
            Err(Error::OutOfSpace { required }) => {
                vec.reserve(required - remaining);
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

    pub fn set_options_from_terminal(&mut self, terminal: &Terminal<'_, '_>) -> &mut Self {
        unsafe {
            ffi::ghostty_mouse_encoder_setopt_from_terminal(
                self.0.as_raw(),
                terminal.inner.as_raw(),
            );
        }
        self
    }
    pub fn set_tracking_mode(&mut self, value: TrackingMode) -> &mut Self {
        unsafe {
            self.setopt(Opt::EVENT, std::ptr::from_ref(&value).cast());
        }
        self
    }
    pub fn set_format(&mut self, value: Format) -> &mut Self {
        unsafe {
            self.setopt(Opt::FORMAT, std::ptr::from_ref(&value).cast());
        }
        self
    }
    pub fn set_size(&mut self, value: EncoderSize) -> &mut Self {
        let raw: ffi::MouseEncoderSize = value.into();
        unsafe {
            self.setopt(Opt::SIZE, std::ptr::from_ref(&raw).cast());
        }
        self
    }
    pub fn set_any_button_pressed(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(Opt::ANY_BUTTON_PRESSED, std::ptr::from_ref(&value).cast());
        }
        self
    }
    pub fn set_track_last_cell(&mut self, value: bool) -> &mut Self {
        unsafe {
            self.setopt(Opt::TRACK_LAST_CELL, std::ptr::from_ref(&value).cast());
        }
        self
    }

    pub fn reset(&mut self) {
        unsafe { ffi::ghostty_mouse_encoder_reset(self.0.as_raw()) }
    }
}

impl Drop for Encoder<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_mouse_encoder_free(self.0.as_raw()) }
    }
}

#[derive(Debug)]
pub struct Event<'alloc>(Object<'alloc, ffi::MouseEventImpl>);

impl<'alloc> Event<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::MouseEvent = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_mouse_event_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    pub fn set_action(&mut self, action: Action) -> &mut Self {
        unsafe {
            ffi::ghostty_mouse_event_set_action(self.0.as_raw(), action as ffi::MouseAction::Type);
        }
        self
    }

    #[must_use]
    pub fn action(&self) -> Action {
        unsafe { ffi::ghostty_mouse_event_get_action(self.0.as_raw()) }
            .try_into()
            .unwrap_or(Action::Press)
    }

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

    pub fn set_mods(&mut self, mods: key::Mods) -> &mut Self {
        unsafe { ffi::ghostty_mouse_event_set_mods(self.0.as_raw(), mods.bits()) }
        self
    }

    #[must_use]
    pub fn mods(&self) -> key::Mods {
        key::Mods::from_bits_retain(unsafe { ffi::ghostty_mouse_event_get_mods(self.0.as_raw()) })
    }

    pub fn set_position(&mut self, pos: Position) -> &mut Self {
        unsafe { ffi::ghostty_mouse_event_set_position(self.0.as_raw(), pos) }
        self
    }

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

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum TrackingMode {
    None = ffi::MouseTrackingMode::NONE,
    X10 = ffi::MouseTrackingMode::X10,
    Normal = ffi::MouseTrackingMode::NORMAL,
    Button = ffi::MouseTrackingMode::BUTTON,
    Any = ffi::MouseTrackingMode::ANY,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum Format {
    X10 = ffi::MouseFormat::X10,
    Utf8 = ffi::MouseFormat::UTF8,
    Sgr = ffi::MouseFormat::SGR,
    Urxvt = ffi::MouseFormat::URXVT,
    SgrPixels = ffi::MouseFormat::SGR_PIXELS,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncoderSize {
    pub screen_width: u32,
    pub screen_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub padding_right: u32,
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

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum Action {
    Press = ffi::MouseAction::PRESS,
    Release = ffi::MouseAction::RELEASE,
    Motion = ffi::MouseAction::MOTION,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
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

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum Shape {
    Default = ffi::MouseShape::DEFAULT,
    ContextMenu = ffi::MouseShape::CONTEXT_MENU,
    Help = ffi::MouseShape::HELP,
    Pointer = ffi::MouseShape::POINTER,
    Progress = ffi::MouseShape::PROGRESS,
    Wait = ffi::MouseShape::WAIT,
    Cell = ffi::MouseShape::CELL,
    Crosshair = ffi::MouseShape::CROSSHAIR,
    Text = ffi::MouseShape::TEXT,
    VerticalText = ffi::MouseShape::VERTICAL_TEXT,
    Alias = ffi::MouseShape::ALIAS,
    Copy = ffi::MouseShape::COPY,
    Move = ffi::MouseShape::MOVE,
    NoDrop = ffi::MouseShape::NO_DROP,
    NotAllowed = ffi::MouseShape::NOT_ALLOWED,
    Grab = ffi::MouseShape::GRAB,
    Grabbing = ffi::MouseShape::GRABBING,
    AllScroll = ffi::MouseShape::ALL_SCROLL,
    ColResize = ffi::MouseShape::COL_RESIZE,
    RowResize = ffi::MouseShape::ROW_RESIZE,
    NResize = ffi::MouseShape::N_RESIZE,
    EResize = ffi::MouseShape::E_RESIZE,
    SResize = ffi::MouseShape::S_RESIZE,
    WResize = ffi::MouseShape::W_RESIZE,
    NeResize = ffi::MouseShape::NE_RESIZE,
    NwResize = ffi::MouseShape::NW_RESIZE,
    SeResize = ffi::MouseShape::SE_RESIZE,
    SwResize = ffi::MouseShape::SW_RESIZE,
    EwResize = ffi::MouseShape::EW_RESIZE,
    NsResize = ffi::MouseShape::NS_RESIZE,
    NeswResize = ffi::MouseShape::NESW_RESIZE,
    NwseResize = ffi::MouseShape::NWSE_RESIZE,
    ZoomIn = ffi::MouseShape::ZOOM_IN,
    ZoomOut = ffi::MouseShape::ZOOM_OUT,
}
