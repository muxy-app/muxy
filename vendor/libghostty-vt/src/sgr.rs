//! Handling SGR (Select Graphic Rendition) escape sequences.

use crate::{
    alloc::{Allocator, Object},
    error::{Error, Result, from_result},
    ffi,
    style::{PaletteIndex, RgbColor, Underline},
};

/// SGR (Select Graphic Rendition) attribute parser.
///
/// SGR sequences are the syntax used to set styling attributes such as bold,
/// italic, underline, and colors for text in terminal emulators. For example,
/// you may be familiar with sequences like `ESC[1;31m`. The 1;31 is the SGR
/// attribute list.
///
/// The parser processes SGR parameters from CSI sequences (e.g., `ESC[1;31m`)
/// and returns individual text attributes like bold, italic, colors, etc. It
/// supports both semicolon (`;`) and colon (`:`) separators, possibly mixed,
/// and handles various color formats including 8-color, 16-color, 256-color,
/// X11 named colors, and RGB in multiple formats.
///
/// # Example
/// ```rust
/// use libghostty_vt::sgr::{Parser, Attribute};
///
/// let mut parser = Parser::new().unwrap();
/// parser.set_params(&[1, 31], None).unwrap();
///
/// while let Some(attr) = parser.next().unwrap() {
///     match attr {
///         Attribute::Bold => println!("Bold enabled"),
///         Attribute::Fg8(color) => println!("Foreground color: {color:?}"),
///         _ => {},
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Parser<'alloc>(Object<'alloc, ffi::SgrParserImpl>);

impl<'alloc> Parser<'alloc> {
    /// Create a new SGR parser.
    pub fn new() -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    /// Create a new SGR parser with a custom allocator.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::SgrParser = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_sgr_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    /// Set SGR parameters for parsing.
    ///
    /// Parameters are the numeric values from a CSI SGR sequence (e.g., for `ESC[1;31m`, params
    /// would be `[1, 31]`).
    ///
    /// The `separators` slice optionally specifies the separator type for each parameter position.
    /// Each byte should be either `b';'` for semicolon or `b':'` for colon.
    /// This is needed for certain color formats that use colon separators (e.g., `ESC[4:3m`
    /// for curly underline). Any invalid separator values are treated as semicolons.
    ///
    /// If `separators` is `None`, all parameters are assumed to be semicolon-separated.
    ///
    /// After calling this function, the parser is automatically reset and ready to iterate from
    /// the beginning.
    ///
    /// # Panics
    ///
    /// **Panics** if `separators` is not `None` and is not the same length as `params`.
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

    /// Get the next SGR attribute.
    ///
    /// Parses and returns the next attribute from the parameter list.
    /// Call this function repeatedly until it returns `None` to process all
    /// attributes in the sequence.
    ///
    /// This cannot be expressed as a regular iterator since the returned
    /// attribute borrows memory from the parser directly.
    #[expect(
        clippy::should_implement_trait,
        reason = "lending `next` cannot implement trait"
    )]
    pub fn next(&mut self) -> Result<Option<Attribute<'_>>> {
        let mut raw_attr = ffi::SgrAttribute::default();
        let has_next = unsafe { ffi::ghostty_sgr_next(self.0.as_raw(), &raw mut raw_attr) };
        if has_next {
            // This shouldn't really *ever* fail, so the fact it failed
            // suggests we should stop anyways.
            Ok(Some(Attribute::from_raw(raw_attr)?))
        } else {
            Ok(None)
        }
    }

    /// Reset an SGR parser instance to the beginning of the parameter list.
    ///
    /// Resets the parser's iteration state without clearing the parameters.
    /// After calling this, [`Parser::next`] will start from the beginning of the
    /// parameter list again.
    pub fn reset(&mut self) {
        unsafe { ffi::ghostty_sgr_reset(self.0.as_raw()) }
    }
}

impl Drop for Parser<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_sgr_free(self.0.as_raw()) }
    }
}

/// An SGR attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[expect(missing_docs, reason = "missing upstream docs")]
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
    /// This should never return None, but just to be safe.
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

/// Unknown SGR attribute data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unknown<'p> {
    /// Full parameter list.
    pub full: &'p [u16],
    /// Partial list where parsing encountered an unknown or invalid sequence.
    pub partial: &'p [u16],
}

impl From<ffi::SgrUnknown> for Unknown<'_> {
    fn from(value: ffi::SgrUnknown) -> Self {
        // SAFETY: We trust libghostty to give us two valid slices
        // of u16s that last at least as long as the current iteration,
        // which is guaranteed by Rust's mutation XOR sharability property
        // (e.g. one cannot reset the parser when this object still
        // borrows the parser mutably).
        let full = unsafe { std::slice::from_raw_parts(value.full_ptr, value.full_len) };
        let partial = unsafe { std::slice::from_raw_parts(value.partial_ptr, value.partial_len) };
        Self { full, partial }
    }
}
