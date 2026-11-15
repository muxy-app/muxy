use std::{marker::PhantomData, mem::MaybeUninit, ptr::NonNull};

use crate::{
    error::{Error, Result, from_optional_result_uninit, from_result, from_result_with_len},
    ffi,
    style::{self, PaletteIndex, RgbColor, Style},
    terminal::{Point, PointCoordinate, PointSpace, Terminal},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, int_enum::IntEnum)]
#[repr(i32)]
pub enum Screen {
    #[default]
    Primary = ffi::TerminalScreen::PRIMARY,
    Alternate = ffi::TerminalScreen::ALTERNATE,
}

#[derive(Clone, Debug)]
pub struct GridRef<'t> {
    pub(crate) inner: ffi::GridRef,
    pub(crate) _phan: PhantomData<&'t ffi::Terminal>,
}

impl GridRef<'_> {
    pub(crate) unsafe fn from_raw(inner: ffi::GridRef) -> Self {
        Self {
            inner,
            _phan: PhantomData,
        }
    }

    pub fn row(&self) -> Result<Row> {
        let mut v = ffi::Row::default();
        let result =
            unsafe { ffi::ghostty_grid_ref_row(std::ptr::from_ref(&self.inner), &raw mut v) };
        from_result(result)?;
        Ok(Row(v))
    }
    pub fn cell(&self) -> Result<Cell> {
        let mut v = ffi::Cell::default();
        let result =
            unsafe { ffi::ghostty_grid_ref_cell(std::ptr::from_ref(&self.inner), &raw mut v) };
        from_result(result)?;
        Ok(Cell(v))
    }
    pub fn style(&self) -> Result<Style> {
        let mut v = ffi::Style::default();
        let result =
            unsafe { ffi::ghostty_grid_ref_style(std::ptr::from_ref(&self.inner), &raw mut v) };
        from_result(result)?;
        Style::try_from(v)
    }

    pub fn graphemes(&self, buf: &mut [char]) -> Result<usize> {
        let mut len = 0;
        let result = unsafe {
            ffi::ghostty_grid_ref_graphemes(
                std::ptr::from_ref(&self.inner),
                std::ptr::from_mut(buf).cast(),
                buf.len(),
                &raw mut len,
            )
        };
        from_result_with_len(result, len)
    }

    pub fn hyperlink_uri(&self, buf: &mut [u8]) -> Result<usize> {
        let mut len = 0;
        let result = unsafe {
            ffi::ghostty_grid_ref_hyperlink_uri(
                std::ptr::from_ref(&self.inner),
                std::ptr::from_mut(buf).cast(),
                buf.len(),
                &raw mut len,
            )
        };
        from_result_with_len(result, len)
    }
}

#[derive(Debug)]
pub struct TrackedGridRef {
    inner: NonNull<ffi::TrackedGridRefImpl>,
    terminal: NonNull<ffi::TerminalImpl>,
}

impl TrackedGridRef {
    pub(crate) fn new(
        inner: NonNull<ffi::TrackedGridRefImpl>,
        terminal: NonNull<ffi::TerminalImpl>,
    ) -> Self {
        Self { inner, terminal }
    }

    #[must_use]
    pub fn has_value(&self) -> bool {
        unsafe { ffi::ghostty_tracked_grid_ref_has_value(self.inner.as_ptr()) }
    }

    pub fn snapshot<'t>(&self, terminal: &'t Terminal<'_, '_>) -> Result<Option<GridRef<'t>>> {
        if self.terminal != terminal.inner.ptr {
            return Err(Error::InvalidValue);
        }
        let mut grid_ref = MaybeUninit::new(ffi::sized!(ffi::GridRef));
        let result = unsafe {
            ffi::ghostty_tracked_grid_ref_snapshot(self.inner.as_ptr(), grid_ref.as_mut_ptr())
        };

        from_optional_result_uninit(result, grid_ref).map(|value| {
            value.map(|raw| unsafe {
                GridRef::from_raw(raw)
            })
        })
    }

    pub fn point(&self, space: PointSpace) -> Result<Option<PointCoordinate>> {
        let mut point = MaybeUninit::<ffi::PointCoordinate>::zeroed();
        let result = unsafe {
            ffi::ghostty_tracked_grid_ref_point(
                self.inner.as_ptr(),
                space.into_raw(),
                point.as_mut_ptr(),
            )
        };

        from_optional_result_uninit(result, point).map(|value| value.map(Into::into))
    }

    pub fn set(&mut self, terminal: &mut Terminal<'_, '_>, point: Point) -> Result<&mut Self> {
        let result = unsafe {
            ffi::ghostty_tracked_grid_ref_set(
                self.inner.as_ptr(),
                terminal.inner.as_raw(),
                point.into(),
            )
        };
        from_result(result)?;
        Ok(self)
    }
}

impl Drop for TrackedGridRef {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_tracked_grid_ref_free(self.inner.as_ptr()) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row(pub(crate) ffi::Row);

impl Row {
    fn get<T>(&self, tag: ffi::RowData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe { ffi::ghostty_row_get(self.0, tag, value.as_mut_ptr().cast()) };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    pub fn is_wrapped(self) -> Result<bool> {
        self.get(ffi::RowData::WRAP)
    }
    pub fn is_wrap_continuation(self) -> Result<bool> {
        self.get(ffi::RowData::WRAP_CONTINUATION)
    }
    pub fn has_grapheme_cluster(self) -> Result<bool> {
        self.get(ffi::RowData::GRAPHEME)
    }
    pub fn is_styled(self) -> Result<bool> {
        self.get(ffi::RowData::STYLED)
    }
    pub fn has_hyperlink(self) -> Result<bool> {
        self.get(ffi::RowData::HYPERLINK)
    }
    pub fn semantic_prompt(self) -> Result<RowSemanticPrompt> {
        self.get::<ffi::RowSemanticPrompt::Type>(ffi::RowData::SEMANTIC_PROMPT)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    pub fn has_kitty_virtual_placeholder(self) -> Result<bool> {
        self.get(ffi::RowData::KITTY_VIRTUAL_PLACEHOLDER)
    }
    pub fn is_dirty(self) -> Result<bool> {
        self.get(ffi::RowData::DIRTY)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell(pub(crate) ffi::Cell);

impl Cell {
    fn get<T>(&self, tag: ffi::CellData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe { ffi::ghostty_cell_get(self.0, tag, value.as_mut_ptr().cast()) };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    pub fn codepoint(self) -> Result<u32> {
        self.get(ffi::CellData::CODEPOINT)
    }
    pub fn content_tag(self) -> Result<CellContentTag> {
        self.get::<ffi::CellContentTag::Type>(ffi::CellData::CONTENT_TAG)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    pub fn wide(self) -> Result<CellWide> {
        self.get::<ffi::CellWide::Type>(ffi::CellData::WIDE)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    pub fn has_text(self) -> Result<bool> {
        self.get(ffi::CellData::HAS_TEXT)
    }
    pub fn has_styling(self) -> Result<bool> {
        self.get(ffi::CellData::HAS_STYLING)
    }
    pub fn style_id(self) -> Result<style::Id> {
        self.get(ffi::CellData::STYLE_ID).map(style::Id)
    }
    pub fn has_hyperlink(self) -> Result<bool> {
        self.get(ffi::CellData::HAS_HYPERLINK)
    }
    pub fn is_protected(self) -> Result<bool> {
        self.get(ffi::CellData::PROTECTED)
    }
    pub fn semantic_content(self) -> Result<CellSemanticContent> {
        self.get::<ffi::CellSemanticContent::Type>(ffi::CellData::SEMANTIC_CONTENT)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    pub fn bg_color_palette(self) -> Result<PaletteIndex> {
        self.get(ffi::CellData::COLOR_PALETTE).map(PaletteIndex)
    }
    pub fn bg_color_rgb(self) -> Result<RgbColor> {
        Ok(self.get::<ffi::ColorRgb>(ffi::CellData::COLOR_RGB)?.into())
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum RowSemanticPrompt {
    None = ffi::RowSemanticPrompt::NONE,
    Prompt = ffi::RowSemanticPrompt::PROMPT,
    Continuation = ffi::RowSemanticPrompt::PROMPT_CONTINUATION,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum CellContentTag {
    Codepoint = ffi::CellContentTag::CODEPOINT,
    CodepointGrapheme = ffi::CellContentTag::CODEPOINT_GRAPHEME,
    BgColorPalette = ffi::CellContentTag::BG_COLOR_PALETTE,
    BgColorRgb = ffi::CellContentTag::BG_COLOR_RGB,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum CellWide {
    Narrow = ffi::CellWide::NARROW,
    Wide = ffi::CellWide::WIDE,
    SpacerTail = ffi::CellWide::SPACER_TAIL,
    SpacerHead = ffi::CellWide::SPACER_HEAD,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum CellSemanticContent {
    Output = ffi::CellSemanticContent::OUTPUT,
    Input = ffi::CellSemanticContent::INPUT,
    Prompt = ffi::CellSemanticContent::PROMPT,
}
