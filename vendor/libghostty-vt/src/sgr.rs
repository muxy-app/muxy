use crate::{
    alloc::{Allocator, Object},
    error::{Error, Result, from_result},
    ffi,
    style::{PaletteIndex, RgbColor, Underline},
};

#[derive(Debug)]
pub struct Parser<'alloc>(Object<'alloc, ffi::SgrParserImpl>);

impl<'alloc> Parser<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::SgrParser = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_sgr_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    pub fn set_params(&mut self, params: &[u16], separators: Option<&[u8]>) -> Result<()> {
        let sep = match separators {
            Some(seps) => {
                assert!(
                    seps.len() == params.len(),
                    "separators length must equal params length"
                );
                seps.as_ptr().cast()
            }
            None => std::ptr::null(),
        };
        let result = unsafe {
            ffi::ghostty_sgr_set_params(self.0.as_raw(), params.as_ptr(), sep, params.len())
        };
        from_result(result)
    }

    #[expect(
        clippy::should_implement_trait,
        reason = "lending `next` cannot implement trait"
    )]
    pub fn next(&mut self) -> Result<Option<Attribute<'_>>> {
        let mut raw_attr = ffi::SgrAttribute::default();
        let has_next = unsafe { ffi::ghostty_sgr_next(self.0.as_raw(), &raw mut raw_attr) };
        if has_next {
            Ok(Some(Attribute::from_raw(raw_attr)?))
        } else {
            Ok(None)
        }
    }

    pub fn reset(&mut self) {
        unsafe { ffi::ghostty_sgr_reset(self.0.as_raw()) }
    }
}

impl Drop for Parser<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_sgr_free(self.0.as_raw()) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Attribute<'p> {
    Unset,
    Unknown(Unknown<'p>),
    Bold,
    ResetBold,
    Italic,
    ResetItalic,
    Faint,
    Underline(Underline),
    UnderlineColor(RgbColor),
    UnderlineColor256(PaletteIndex),
    ResetUnderlineColor,
    Overline,
    ResetOverline,
    Blink,
    ResetBlink,
    Inverse,
    ResetInverse,
    Invisible,
    ResetInvisible,
    Strikethrough,
    ResetStrikethrough,
    DirectColorFg(RgbColor),
    DirectColorBg(RgbColor),
    Bg8(PaletteIndex),
    Fg8(PaletteIndex),
    ResetFg,
    ResetBg,
    BrightBg8(PaletteIndex),
    BrightFg8(PaletteIndex),
    Bg256(PaletteIndex),
    Fg256(PaletteIndex),
}

impl Attribute<'_> {
    fn from_raw(value: ffi::SgrAttribute) -> Result<Self> {
        Ok(match value.tag {
            0 => Self::Unset,
            1 => Self::Unknown(unsafe { value.value.unknown }.into()),
            2 => Self::Bold,
            3 => Self::ResetBold,
            4 => Self::Italic,
            5 => Self::ResetItalic,
            6 => Self::Faint,
            7 => Self::Underline(
                Underline::try_from(unsafe { value.value.underline })
                    .map_err(|_| Error::InvalidValue)?,
            ),
            8 => Self::UnderlineColor(unsafe { value.value.underline_color }.into()),
            9 => Self::UnderlineColor256(PaletteIndex(unsafe { value.value.underline_color_256 })),
            10 => Self::ResetUnderlineColor,
            11 => Self::Overline,
            12 => Self::ResetOverline,
            13 => Self::Blink,
            14 => Self::ResetBlink,
            15 => Self::Inverse,
            16 => Self::ResetInverse,
            17 => Self::Invisible,
            18 => Self::ResetInvisible,
            19 => Self::Strikethrough,
            20 => Self::ResetStrikethrough,
            21 => Self::DirectColorFg(unsafe { value.value.direct_color_fg }.into()),
            22 => Self::DirectColorBg(unsafe { value.value.direct_color_bg }.into()),
            23 => Self::Bg8(PaletteIndex(unsafe { value.value.bg_8 })),
            24 => Self::Fg8(PaletteIndex(unsafe { value.value.fg_8 })),
            25 => Self::ResetFg,
            26 => Self::ResetBg,
            27 => Self::BrightBg8(PaletteIndex(unsafe { value.value.bright_bg_8 })),
            28 => Self::BrightFg8(PaletteIndex(unsafe { value.value.bright_fg_8 })),
            29 => Self::Bg256(PaletteIndex(unsafe { value.value.bg_256 })),
            30 => Self::Fg256(PaletteIndex(unsafe { value.value.fg_256 })),
            _ => return Err(Error::InvalidValue),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unknown<'p> {
    pub full: &'p [u16],
    pub partial: &'p [u16],
}

impl From<ffi::SgrUnknown> for Unknown<'_> {
    fn from(value: ffi::SgrUnknown) -> Self {
        let full = unsafe { std::slice::from_raw_parts(value.full_ptr, value.full_len) };
        let partial = unsafe { std::slice::from_raw_parts(value.partial_ptr, value.partial_len) };
        Self { full, partial }
    }
}
