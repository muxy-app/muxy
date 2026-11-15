use std::{convert::Into, marker::PhantomData, mem::MaybeUninit};

use crate::{
    alloc::{Allocator, Object},
    error::{Error, Result, from_optional_result, from_optional_result_uninit, from_result},
    ffi,
    screen::{Cell, Row},
    style::{RgbColor, Style},
    terminal::Terminal,
};

pub use ffi::RenderStateRowSelection as RowSelection;

pub use ffi::RenderStateOverscan as Overscan;

#[derive(Debug)]
pub struct RenderState<'alloc>(Object<'alloc, ffi::RenderStateImpl>);

#[derive(Debug)]
pub struct Snapshot<'alloc, 's>(&'s mut RenderState<'alloc>);

#[derive(Debug)]
pub struct Update<'alloc, 's> {
    state: Option<&'s mut RenderState<'alloc>>,
}

#[derive(Debug)]
pub struct RowIterator<'alloc>(Object<'alloc, ffi::RenderStateRowIteratorImpl>);

#[derive(Debug)]
pub struct RowIteration<'alloc, 's> {
    iter: &'s mut RowIterator<'alloc>,
    _phan: PhantomData<&'s Snapshot<'alloc, 's>>,
}

#[derive(Debug)]
pub struct CellIterator<'alloc>(Object<'alloc, ffi::RenderStateRowCellsImpl>);

#[derive(Debug)]
pub struct CellIteration<'alloc, 's> {
    iter: &'s mut CellIterator<'alloc>,
    _phan: PhantomData<&'s RowIteration<'alloc, 's>>,
}

#[derive(Debug)]
pub struct ImageIterator<'alloc>(Object<'alloc, ffi::RenderStateImageIteratorImpl>);

#[derive(Debug)]
pub struct ImageIteration<'alloc, 's> {
    iter: &'s mut ImageIterator<'alloc>,
    _phan: PhantomData<&'s Snapshot<'alloc, 's>>,
}


impl<'alloc> RenderState<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::RenderState = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_render_state_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    pub fn update<'cb>(
        &mut self,
        terminal: &Terminal<'alloc, 'cb>,
    ) -> Result<Snapshot<'alloc, '_>> {
        let result =
            unsafe { ffi::ghostty_render_state_update(self.0.as_raw(), terminal.inner.as_raw()) };
        from_result(result)?;
        Ok(Snapshot(self))
    }

    pub fn begin_update<'cb>(
        &mut self,
        terminal: &Terminal<'alloc, 'cb>,
    ) -> Result<Update<'alloc, '_>> {
        let result = unsafe {
            ffi::ghostty_render_state_begin_update(self.0.as_raw(), terminal.inner.as_raw())
        };
        from_result(result)?;
        Ok(Update { state: Some(self) })
    }

    fn get<T>(&self, tag: ffi::RenderStateData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_get(self.0.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    fn set<T>(&self, tag: ffi::RenderStateOption::Type, value: &T) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_render_state_set(self.0.as_raw(), tag, std::ptr::from_ref(value).cast())
        };
        from_result(result)
    }

    pub fn set_overscan(&mut self, request: Overscan) -> Result<&mut Self> {
        self.set(ffi::RenderStateOption::OVERSCAN, &request)?;
        Ok(self)
    }

    pub fn overscan_request(&self) -> Result<Overscan> {
        self.get(ffi::RenderStateData::OVERSCAN_REQUEST)
    }
}

impl Drop for RenderState<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_render_state_free(self.0.as_raw()) }
    }
}

impl<'alloc, 's> Update<'alloc, 's> {
    pub fn end(mut self) -> Result<Snapshot<'alloc, 's>> {
        let Some(state) = self.state.take() else {
            return Err(Error::InvalidValue);
        };
        let result = unsafe { ffi::ghostty_render_state_end_update(state.0.as_raw()) };
        from_result(result)?;
        Ok(Snapshot(state))
    }
}

impl Drop for Update<'_, '_> {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            let _ = unsafe { ffi::ghostty_render_state_end_update(state.0.as_raw()) };
        }
    }
}

impl Snapshot<'_, '_> {
    fn get<T>(&self, tag: ffi::RenderStateData::Type) -> Result<T> {
        self.0.get(tag)
    }

    fn set<T>(&self, tag: ffi::RenderStateOption::Type, value: &T) -> Result<()> {
        self.0.set(tag, value)
    }

    pub fn dirty(&self) -> Result<Dirty> {
        self.get::<ffi::RenderStateDirty::Type>(ffi::RenderStateData::DIRTY)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    pub fn cols(&self) -> Result<u16> {
        self.get(ffi::RenderStateData::COLS)
    }

    pub fn rows(&self) -> Result<u16> {
        self.get(ffi::RenderStateData::ROWS)
    }

    pub fn overscan(&self) -> Result<Overscan> {
        self.get(ffi::RenderStateData::OVERSCAN)
    }

    pub fn cursor_color(&self) -> Result<Option<RgbColor>> {
        let has_value = self.get(ffi::RenderStateData::COLOR_CURSOR_HAS_VALUE)?;
        if has_value {
            let color = self.get(ffi::RenderStateData::COLOR_CURSOR)?;
            Ok(Some(color))
        } else {
            Ok(None)
        }
    }

    pub fn cursor_visible(&self) -> Result<bool> {
        self.get(ffi::RenderStateData::CURSOR_VISIBLE)
    }

    pub fn cursor_blinking(&self) -> Result<bool> {
        self.get(ffi::RenderStateData::CURSOR_BLINKING)
    }

    pub fn cursor_password_input(&self) -> Result<bool> {
        self.get(ffi::RenderStateData::CURSOR_PASSWORD_INPUT)
    }

    pub fn cursor_visual_style(&self) -> Result<CursorVisualStyle> {
        self.get::<ffi::RenderStateCursorVisualStyle::Type>(
            ffi::RenderStateData::CURSOR_VISUAL_STYLE,
        )
        .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    pub fn cursor_viewport(&self) -> Result<Option<CursorViewport>> {
        let has_value = self.get(ffi::RenderStateData::CURSOR_VIEWPORT_HAS_VALUE)?;
        if has_value {
            let x = self.get(ffi::RenderStateData::CURSOR_VIEWPORT_X)?;
            let y = self.get(ffi::RenderStateData::CURSOR_VIEWPORT_Y)?;
            let at_wide_tail = self.get(ffi::RenderStateData::CURSOR_VIEWPORT_WIDE_TAIL)?;
            Ok(Some(CursorViewport { x, y, at_wide_tail }))
        } else {
            Ok(None)
        }
    }

    pub fn cursor(&self) -> Result<Cursor> {
        let mut raw = ffi::sized!(ffi::RenderStateCursor);
        from_result(unsafe {
            ffi::ghostty_render_state_get(
                self.0.0.as_raw(),
                ffi::RenderStateData::CURSOR,
                (&raw mut raw).cast(),
            )
        })?;
        Ok(Cursor {
            viewport: raw.viewport_has_value.then_some(CursorViewport {
                x: raw.viewport_x,
                y: raw.viewport_y,
                at_wide_tail: raw.wide_tail,
            }),
            visible: raw.visible,
            blinking: raw.blinking,
            password_input: raw.password_input,
            visual_style: raw
                .visual_style
                .try_into()
                .map_err(|_| Error::InvalidValue)?,
        })
    }

    pub fn clean(&self) -> Result<()> {
        from_result(unsafe { ffi::ghostty_render_state_clean(self.0.0.as_raw()) })
    }

    pub fn colors(&self) -> Result<Colors> {
        let mut colors = ffi::sized!(ffi::RenderStateColors);
        let result = unsafe {
            ffi::ghostty_render_state_get(
                self.0.0.as_raw(),
                ffi::RenderStateData::COLORS,
                (&raw mut colors).cast(),
            )
        };
        from_result(result)?;

        Ok(Colors {
            background: colors.background.into(),
            foreground: colors.foreground.into(),
            cursor: if colors.cursor_has_value {
                Some(colors.cursor.into())
            } else {
                None
            },
            palette: colors.palette.map(Into::into),
        })
    }

    pub fn image_placements_generation(&self) -> Result<u64> {
        self.get(ffi::RenderStateData::IMAGE_PLACEMENTS_GENERATION)
    }

    pub fn set_dirty(&self, dirty: Dirty) -> Result<()> {
        self.set(
            ffi::RenderStateOption::DIRTY,
            &(dirty as ffi::RenderStateDirty::Type),
        )
    }
}

impl<'alloc> RowIterator<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::RenderStateRowIterator = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_render_state_row_iterator_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    pub fn update<'s>(
        &'s mut self,
        snapshot: &'s Snapshot<'alloc, '_>,
    ) -> Result<RowIteration<'alloc, 's>> {
        let result = unsafe {
            ffi::ghostty_render_state_get(
                snapshot.0.0.as_raw(),
                ffi::RenderStateData::ROW_ITERATOR,
                std::ptr::from_mut(&mut self.0.ptr).cast(),
            )
        };
        from_result(result)?;

        Ok(RowIteration {
            iter: self,
            _phan: PhantomData,
        })
    }
}

impl Drop for RowIterator<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_render_state_row_iterator_free(self.0.as_raw()) }
    }
}

impl<'s> RowIteration<'_, 's> {
    pub fn cells_raw(&self) -> Result<impl ExactSizeIterator<Item = Cell> + use<'s>> {
        let view: ffi::CellsView = self.get(ffi::RenderStateRowData::CELLS_RAW)?;
        let cells: &'s [ffi::Cell] = if view.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(view.ptr, view.len) }
        };
        Ok(cells.iter().copied().map(Cell))
    }
}

impl RowIteration<'_, '_> {
    pub fn next_dirty(&mut self) -> Option<(u16, &Self)> {
        let mut y = 0;
        unsafe {
            ffi::ghostty_render_state_row_iterator_next_dirty(self.iter.0.as_raw(), &raw mut y)
        }
        .then_some((y, self))
    }

    #[expect(
        clippy::should_implement_trait,
        reason = "lending `next` cannot implement trait"
    )]
    pub fn next(&mut self) -> Option<&Self> {
        if unsafe { ffi::ghostty_render_state_row_iterator_next(self.iter.0.as_raw()) } {
            Some(self)
        } else {
            None
        }
    }

    fn get<T>(&self, tag: ffi::RenderStateRowData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_row_get(self.iter.0.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    fn set<T>(&self, tag: ffi::RenderStateRowOption::Type, value: &T) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_render_state_row_set(
                self.iter.0.as_raw(),
                tag,
                std::ptr::from_ref(value).cast(),
            )
        };
        from_result(result)
    }

    pub fn dirty(&self) -> Result<bool> {
        self.get(ffi::RenderStateRowData::DIRTY)
    }

    pub fn id(&self) -> Result<RowId> {
        self.get::<ffi::RenderStateRowId>(ffi::RenderStateRowData::ID)
            .map(|id| RowId(id.bits))
    }

    pub fn viewport_y(&self) -> Result<i32> {
        self.get(ffi::RenderStateRowData::VIEWPORT_Y)
    }

    pub fn raw_row(&self) -> Result<Row> {
        self.get(ffi::RenderStateRowData::RAW).map(Row)
    }

    pub fn set_dirty(&self, dirty: bool) -> Result<()> {
        self.set(ffi::RenderStateRowOption::DIRTY, &dirty)
    }

    pub fn selection(&self) -> Result<Option<RowSelection>> {
        let mut value = ffi::sized!(RowSelection);
        let result = unsafe {
            ffi::ghostty_render_state_row_get(
                self.iter.0.as_raw(),
                ffi::RenderStateRowData::SELECTION,
                std::ptr::from_mut(&mut value).cast(),
            )
        };
        from_optional_result(result, value)
    }
}

impl<'alloc> CellIterator<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::RenderStateRowCells = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_render_state_row_cells_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    pub fn update<'s>(
        &'s mut self,
        row: &'s RowIteration<'alloc, '_>,
    ) -> Result<CellIteration<'alloc, 's>> {
        let result = unsafe {
            ffi::ghostty_render_state_row_get(
                row.iter.0.as_raw(),
                ffi::RenderStateRowData::CELLS,
                std::ptr::from_mut(&mut self.0.ptr).cast(),
            )
        };
        from_result(result)?;

        Ok(CellIteration {
            iter: self,
            _phan: PhantomData,
        })
    }
}

impl Drop for CellIterator<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_render_state_row_cells_free(self.0.as_raw()) }
    }
}

impl CellIteration<'_, '_> {
    #[expect(
        clippy::should_implement_trait,
        reason = "lending `next` cannot implement trait"
    )]
    pub fn next(&mut self) -> Option<&Self> {
        if unsafe { ffi::ghostty_render_state_row_cells_next(self.iter.0.as_raw()) } {
            Some(self)
        } else {
            None
        }
    }

    pub fn select(&mut self, x: u16) -> Result<()> {
        let result = unsafe { ffi::ghostty_render_state_row_cells_select(self.iter.0.as_raw(), x) };
        from_result(result)
    }

    fn get<T>(&self, tag: ffi::RenderStateRowCellsData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_row_cells_get(
                self.iter.0.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    pub fn raw_cell(&self) -> Result<Cell> {
        self.get(ffi::RenderStateRowCellsData::RAW).map(Cell)
    }

    pub fn style(&self) -> Result<Style> {
        let mut value = ffi::sized!(ffi::Style);
        let result = unsafe {
            ffi::ghostty_render_state_row_cells_get(
                self.iter.0.as_raw(),
                ffi::RenderStateRowCellsData::STYLE,
                std::ptr::from_mut(&mut value).cast(),
            )
        };
        from_result(result)?;
        Style::try_from(value)
    }

    pub fn fg_color(&self) -> Result<Option<RgbColor>> {
        let res = self.get::<ffi::ColorRgb>(ffi::RenderStateRowCellsData::FG_COLOR);
        match res {
            Ok(o) => Ok(Some(o.into())),
            Err(Error::InvalidValue) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn bg_color(&self) -> Result<Option<RgbColor>> {
        let res = self.get::<ffi::ColorRgb>(ffi::RenderStateRowCellsData::BG_COLOR);
        match res {
            Ok(o) => Ok(Some(o.into())),
            Err(Error::InvalidValue) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn graphemes(&self) -> Result<Vec<char>> {
        let len = self.graphemes_len()? as usize;
        let mut graphemes = vec!['\0'; len];
        self.graphemes_buf(&mut graphemes)?;
        Ok(graphemes)
    }

    pub fn graphemes_len(&self) -> Result<u32> {
        self.get(ffi::RenderStateRowCellsData::GRAPHEMES_LEN)
    }

    pub fn graphemes_buf(&self, buf: &mut [char]) -> Result<()> {
        let required = self.graphemes_len()? as usize;
        if buf.len() < required {
            return Err(Error::OutOfSpace { required });
        }
        let result = unsafe {
            ffi::ghostty_render_state_row_cells_get(
                self.iter.0.as_raw(),
                ffi::RenderStateRowCellsData::GRAPHEMES_BUF,
                buf.as_mut_ptr().cast(),
            )
        };
        from_result(result)
    }

    pub fn graphemes_utf8(&self, buf: &mut String) -> Result<()> {
        buf.clear();
        let bytes = unsafe { buf.as_mut_vec() };
        loop {
            let mut cbuf = ffi::Buffer {
                ptr: bytes.as_mut_ptr(),
                cap: bytes.capacity(),
                len: 0,
            };
            let result = unsafe {
                ffi::ghostty_render_state_row_cells_get(
                    self.iter.0.as_raw(),
                    ffi::RenderStateRowCellsData::GRAPHEMES_UTF8,
                    std::ptr::from_mut(&mut cbuf).cast(),
                )
            };
            match result {
                ffi::Result::SUCCESS => {
                    unsafe { bytes.set_len(cbuf.len) };
                    return Ok(());
                }
                ffi::Result::OUT_OF_SPACE => bytes.reserve(cbuf.len),
                ffi::Result::OUT_OF_MEMORY => return Err(Error::OutOfMemory),
                _ => return Err(Error::InvalidValue),
            }
        }
    }

    pub fn is_selected(&self) -> Result<bool> {
        self.get(ffi::RenderStateRowCellsData::SELECTED)
    }

    pub fn has_styling(&self) -> Result<bool> {
        self.get(ffi::RenderStateRowCellsData::HAS_STYLING)
    }
}

impl<'alloc> ImageIterator<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut raw: ffi::RenderStateImageIterator = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_render_state_image_iterator_new(alloc, &raw mut raw) };
        from_result(result)?;
        Ok(Self(Object::new(raw)?))
    }

    pub fn update<'s>(
        &'s mut self,
        snapshot: &'s Snapshot<'alloc, '_>,
    ) -> Result<ImageIteration<'alloc, 's>> {
        let result = unsafe {
            ffi::ghostty_render_state_get(
                snapshot.0.0.as_raw(),
                ffi::RenderStateData::IMAGE_ITERATOR,
                std::ptr::from_mut(&mut self.0.ptr).cast(),
            )
        };
        from_result(result)?;

        Ok(ImageIteration {
            iter: self,
            _phan: PhantomData,
        })
    }
}

impl Drop for ImageIterator<'_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_render_state_image_iterator_free(self.0.as_raw()) }
    }
}

impl ImageIteration<'_, '_> {
    #[expect(
        clippy::should_implement_trait,
        reason = "lending `next` cannot implement trait"
    )]
    pub fn next(&mut self) -> Option<&Self> {
        if unsafe { ffi::ghostty_render_state_image_iterator_next(self.iter.0.as_raw()) } {
            Some(self)
        } else {
            None
        }
    }

    fn get<T>(&self, tag: ffi::RenderStateImageData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_image_get(
                self.iter.0.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    fn get_optional<T>(&self, tag: ffi::RenderStateImageData::Type) -> Result<Option<T>> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_render_state_image_get(
                self.iter.0.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        from_optional_result_uninit(result, value)
    }

    pub fn protocol(&self) -> Result<ImageProtocol> {
        self.get::<ffi::ImageProtocol::Type>(ffi::RenderStateImageData::PROTOCOL)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    pub fn image_id(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::IMAGE_ID)
    }

    pub fn image_generation(&self) -> Result<u64> {
        self.get(ffi::RenderStateImageData::IMAGE_GENERATION)
    }

    pub fn layer(&self) -> Result<ImageLayer> {
        self.get::<ffi::RenderStateImageLayer::Type>(ffi::RenderStateImageData::LAYER)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    pub fn z(&self) -> Result<i32> {
        self.get(ffi::RenderStateImageData::Z)
    }

    pub fn viewport_col(&self) -> Result<i32> {
        self.get(ffi::RenderStateImageData::VIEWPORT_COL)
    }

    pub fn viewport_row(&self) -> Result<i32> {
        self.get(ffi::RenderStateImageData::VIEWPORT_ROW)
    }

    pub fn grid_cols(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::GRID_COLS)
    }

    pub fn grid_rows(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::GRID_ROWS)
    }

    pub fn cell_offset_x(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::CELL_OFFSET_X)
    }

    pub fn cell_offset_y(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::CELL_OFFSET_Y)
    }

    pub fn pixel_width(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::PIXEL_WIDTH)
    }

    pub fn pixel_height(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::PIXEL_HEIGHT)
    }

    pub fn source_x(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::SOURCE_X)
    }

    pub fn source_y(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::SOURCE_Y)
    }

    pub fn source_width(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::SOURCE_WIDTH)
    }

    pub fn source_height(&self) -> Result<u32> {
        self.get(ffi::RenderStateImageData::SOURCE_HEIGHT)
    }

    pub fn kitty_placement_id(&self) -> Result<Option<u32>> {
        self.get_optional(ffi::RenderStateImageData::KITTY_PLACEMENT_ID)
    }

    pub fn kitty_kind(&self) -> Result<Option<KittyPlacementKind>> {
        self.get_optional::<ffi::RenderStateKittyPlacementKind::Type>(
            ffi::RenderStateImageData::KITTY_KIND,
        )?
        .map(|v| v.try_into().map_err(|_| Error::InvalidValue))
        .transpose()
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorViewport {
    pub x: u16,
    pub y: u16,
    pub at_wide_tail: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursor {
    pub viewport: Option<CursorViewport>,
    pub visible: bool,
    pub blinking: bool,
    pub password_input: bool,
    pub visual_style: CursorVisualStyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RowId([u64; 2]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colors {
    pub background: RgbColor,
    pub foreground: RgbColor,
    pub cursor: Option<RgbColor>,
    pub palette: [RgbColor; 256],
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum Dirty {
    Clean = ffi::RenderStateDirty::FALSE,
    Partial = ffi::RenderStateDirty::PARTIAL,
    Full = ffi::RenderStateDirty::FULL,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum CursorVisualStyle {
    Bar = ffi::RenderStateCursorVisualStyle::BAR,
    Block = ffi::RenderStateCursorVisualStyle::BLOCK,
    Underline = ffi::RenderStateCursorVisualStyle::UNDERLINE,
    BlockHollow = ffi::RenderStateCursorVisualStyle::BLOCK_HOLLOW,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, int_enum::IntEnum)]
#[non_exhaustive]
pub enum ImageProtocol {
    Kitty = ffi::ImageProtocol::KITTY,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, int_enum::IntEnum)]
pub enum ImageLayer {
    BelowBackground = ffi::RenderStateImageLayer::BELOW_BACKGROUND,
    BelowText = ffi::RenderStateImageLayer::BELOW_TEXT,
    AboveText = ffi::RenderStateImageLayer::ABOVE_TEXT,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, int_enum::IntEnum)]
#[non_exhaustive]
pub enum KittyPlacementKind {
    Virtual = ffi::RenderStateKittyPlacementKind::VIRTUAL,
    Relative = ffi::RenderStateKittyPlacementKind::RELATIVE,
    Fixed = ffi::RenderStateKittyPlacementKind::FIXED,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::Terminal;

    #[test]
    fn dirty_decodes_after_set_dirty_then_update() {
        let terminal = Terminal::new(8, 3).unwrap();
        let mut state = RenderState::new().unwrap();

        state
            .update(&terminal)
            .unwrap()
            .set_dirty(Dirty::Clean)
            .unwrap();

        assert!(state.update(&terminal).unwrap().dirty().is_ok());
    }

    fn cursor_from_getters(snapshot: &Snapshot<'_, '_>) -> Cursor {
        Cursor {
            viewport: snapshot.cursor_viewport().unwrap(),
            visible: snapshot.cursor_visible().unwrap(),
            blinking: snapshot.cursor_blinking().unwrap(),
            password_input: snapshot.cursor_password_input().unwrap(),
            visual_style: snapshot.cursor_visual_style().unwrap(),
        }
    }

    #[test]
    fn bulk_cursor_matches_individual_getters() {
        let mut terminal = Terminal::new(8, 2).unwrap();
        let mut state = RenderState::new().unwrap();

        terminal.vt_write(b"hi");
        let snapshot = state.update(&terminal).unwrap();
        let cursor = snapshot.cursor().unwrap();
        assert_eq!(cursor, cursor_from_getters(&snapshot));
        assert_eq!(
            cursor.viewport,
            Some(CursorViewport {
                x: 2,
                y: 0,
                at_wide_tail: false
            })
        );

        terminal.vt_write("\x1b[?25l\x1b[5 q\r\n中\x1b[2G".as_bytes());
        let snapshot = state.update(&terminal).unwrap();
        let cursor = snapshot.cursor().unwrap();
        assert_eq!(cursor, cursor_from_getters(&snapshot));
        assert!(!cursor.visible);
        assert_eq!(cursor.visual_style, CursorVisualStyle::Bar);
        assert_eq!(
            cursor.viewport,
            Some(CursorViewport {
                x: 1,
                y: 1,
                at_wide_tail: true
            })
        );

        terminal.vt_write(b"\r\n\r\n\r\n");
        terminal.scroll_viewport(crate::terminal::ScrollViewport::Top);
        let snapshot = state.update(&terminal).unwrap();
        let cursor = snapshot.cursor().unwrap();
        assert_eq!(cursor, cursor_from_getters(&snapshot));
        assert_eq!(cursor.viewport, None);
    }

    fn dirty_rows<'alloc>(
        rows: &mut RowIterator<'alloc>,
        snapshot: &Snapshot<'alloc, '_>,
    ) -> Vec<u16> {
        let mut iteration = rows.update(snapshot).unwrap();
        let mut ys = Vec::new();
        while let Some((y, _)) = iteration.next_dirty() {
            ys.push(y);
        }
        ys
    }

    #[test]
    fn next_dirty_follows_global_and_row_dirty_state() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal.vt_write(b"\x1b[2;1H");
        let mut state = RenderState::new().unwrap();
        let mut rows = RowIterator::new().unwrap();

        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Full);
        assert_eq!(dirty_rows(&mut rows, &snapshot), [0, 1, 2]);

        snapshot.clean().unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Clean);
        assert_eq!(dirty_rows(&mut rows, &snapshot), [] as [u16; 0]);
        snapshot.clean().unwrap();

        terminal.vt_write(b"x");
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.dirty().unwrap(), Dirty::Partial);
        assert_eq!(dirty_rows(&mut rows, &snapshot), [1]);
    }

    fn row_positions<'alloc>(
        rows: &mut RowIterator<'alloc>,
        snapshot: &Snapshot<'alloc, '_>,
    ) -> (Vec<i32>, Vec<u16>) {
        let mut viewport_ys = Vec::new();
        let mut iteration = rows.update(snapshot).unwrap();
        while let Some(row) = iteration.next() {
            viewport_ys.push(row.viewport_y().unwrap());
        }
        (viewport_ys, dirty_rows(rows, snapshot))
    }

    fn sides(overscan: Overscan) -> (u16, u16) {
        (overscan.above, overscan.below)
    }

    #[test]
    fn overscan_request_defaults_to_none_and_round_trips() {
        let mut state = RenderState::new().unwrap();
        assert_eq!(sides(state.overscan_request().unwrap()), (0, 0));

        let request = Overscan { above: 2, below: 1 };
        state.set_overscan(request).unwrap();
        assert_eq!(sides(state.overscan_request().unwrap()), (2, 1));
    }

    #[test]
    fn overscan_captures_only_rows_that_exist() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        for _ in 0..10 {
            terminal.vt_write(b"x\r\n");
        }
        let mut state = RenderState::new().unwrap();
        let mut rows = RowIterator::new().unwrap();

        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(sides(snapshot.overscan().unwrap()), (0, 0));
        assert_eq!(
            row_positions(&mut rows, &snapshot),
            (vec![0, 1, 2], vec![0, 1, 2])
        );

        state.set_overscan(Overscan { above: 1, below: 1 }).unwrap();
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.rows().unwrap(), 3);
        assert_eq!(sides(snapshot.overscan().unwrap()), (1, 0));
        assert_eq!(
            row_positions(&mut rows, &snapshot),
            (vec![-1, 0, 1, 2], vec![0, 1, 2, 3])
        );

        terminal.scroll_viewport(crate::terminal::ScrollViewport::Top);
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(sides(snapshot.overscan().unwrap()), (0, 1));
        assert_eq!(
            row_positions(&mut rows, &snapshot),
            (vec![0, 1, 2, 3], vec![0, 1, 2, 3])
        );
    }

    fn row_ids<'alloc>(
        rows: &mut RowIterator<'alloc>,
        snapshot: &Snapshot<'alloc, '_>,
    ) -> Vec<RowId> {
        let mut ids = Vec::new();
        let mut iteration = rows.update(snapshot).unwrap();
        while let Some(row) = iteration.next() {
            ids.push(row.id().unwrap());
        }
        ids
    }

    #[test]
    fn row_ids_follow_rows_as_the_viewport_scrolls() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        for _ in 0..10 {
            terminal.vt_write(b"x\r\n");
        }
        let mut state = RenderState::new().unwrap();
        let mut rows = RowIterator::new().unwrap();

        let before = row_ids(&mut rows, &state.update(&terminal).unwrap());
        assert!(before[0] != before[1] && before[1] != before[2] && before[0] != before[2]);
        assert_eq!(
            row_ids(&mut rows, &state.update(&terminal).unwrap()),
            before
        );

        terminal.scroll_viewport(crate::terminal::ScrollViewport::Delta(-1));
        let after = row_ids(&mut rows, &state.update(&terminal).unwrap());
        assert_eq!(after[1..], before[..2]);
        assert!(!before.contains(&after[0]));

        state.set_overscan(Overscan { above: 0, below: 1 }).unwrap();
        let overscanned = row_ids(&mut rows, &state.update(&terminal).unwrap());
        assert_eq!(overscanned[..3], after);
        assert_eq!(overscanned[3], before[2]);
    }

    #[test]
    fn cells_raw_matches_cell_iteration_and_outlives_the_row() {
        let mut terminal = Terminal::new(4, 2).unwrap();
        terminal.vt_write(b"ab\r\ncd");
        let mut state = RenderState::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut rows = RowIterator::new().unwrap();
        let mut cells = CellIterator::new().unwrap();
        let mut iteration = rows.update(&snapshot).unwrap();

        let row = iteration.next().unwrap();
        let first_row = row.cells_raw().unwrap();
        assert_eq!(first_row.len(), 4);
        let mut expected = Vec::new();
        let mut cell_iteration = cells.update(row).unwrap();
        while let Some(cell) = cell_iteration.next() {
            expected.push(cell.raw_cell().unwrap());
        }

        let second_row = iteration.next().unwrap().cells_raw().unwrap();
        let first_row: Vec<_> = first_row.collect();
        assert_eq!(first_row, expected);
        let codepoints =
            |cells: &[Cell]| -> Vec<u32> { cells.iter().map(|c| c.codepoint().unwrap()).collect() };
        assert_eq!(codepoints(&first_row), [0x61, 0x62, 0, 0]);
        assert_eq!(
            codepoints(&second_row.collect::<Vec<_>>()),
            [0x63, 0x64, 0, 0]
        );
    }

    #[test]
    fn graphemes_utf8_replaces_the_buffer() {
        let mut terminal = Terminal::new(4, 1).unwrap();
        terminal.vt_write("e\u{301}".as_bytes());
        let mut state = RenderState::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut rows = RowIterator::new().unwrap();
        let mut cells = CellIterator::new().unwrap();
        let mut iteration = rows.update(&snapshot).unwrap();
        let mut cell_iteration = cells.update(iteration.next().unwrap()).unwrap();

        let mut text = String::from("x");
        cell_iteration.next().unwrap();
        cell_iteration.graphemes_utf8(&mut text).unwrap();
        assert_eq!(text, "e\u{301}");
        cell_iteration.next().unwrap();
        cell_iteration.graphemes_utf8(&mut text).unwrap();
        assert_eq!(text, "");
    }

    #[test]
    fn graphemes_buf_rejects_short_buffer() {
        let mut terminal = Terminal::new(4, 1).unwrap();
        terminal.vt_write("e\u{301}".as_bytes());
        let mut state = RenderState::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut rows = RowIterator::new().unwrap();
        let mut cells = CellIterator::new().unwrap();
        let mut iteration = rows.update(&snapshot).unwrap();
        let mut cell_iteration = cells.update(iteration.next().unwrap()).unwrap();
        cell_iteration.next().unwrap();

        let mut short = ['x'];
        assert!(matches!(
            cell_iteration.graphemes_buf(&mut short),
            Err(Error::OutOfSpace { required: 2 })
        ));
        assert_eq!(short, ['x']);

        let mut exact = ['x'; 2];
        cell_iteration.graphemes_buf(&mut exact).unwrap();
        assert_eq!(exact, ['e', '\u{301}']);
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Placement {
        protocol: ImageProtocol,
        image_id: u32,
        layer: ImageLayer,
        z: i32,
        viewport: (i32, i32),
        grid: (u32, u32),
        cell_offset: (u32, u32),
        pixel_size: (u32, u32),
        source: (u32, u32, u32, u32),
        kitty_placement_id: Option<u32>,
        kitty_kind: Option<KittyPlacementKind>,
    }

    fn placements<'alloc>(
        images: &mut ImageIterator<'alloc>,
        snapshot: &Snapshot<'alloc, '_>,
    ) -> Vec<(Placement, u64)> {
        let mut out = Vec::new();
        let mut iteration = images.update(snapshot).unwrap();
        while let Some(p) = iteration.next() {
            out.push((
                Placement {
                    protocol: p.protocol().unwrap(),
                    image_id: p.image_id().unwrap(),
                    layer: p.layer().unwrap(),
                    z: p.z().unwrap(),
                    viewport: (p.viewport_col().unwrap(), p.viewport_row().unwrap()),
                    grid: (p.grid_cols().unwrap(), p.grid_rows().unwrap()),
                    cell_offset: (p.cell_offset_x().unwrap(), p.cell_offset_y().unwrap()),
                    pixel_size: (p.pixel_width().unwrap(), p.pixel_height().unwrap()),
                    source: (
                        p.source_x().unwrap(),
                        p.source_y().unwrap(),
                        p.source_width().unwrap(),
                        p.source_height().unwrap(),
                    ),
                    kitty_placement_id: p.kitty_placement_id().unwrap(),
                    kitty_kind: p.kitty_kind().unwrap(),
                },
                p.image_generation().unwrap(),
            ));
        }
        out
    }

    #[test]
    fn image_iteration_is_empty_without_a_cell_size() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();

        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1,q=2;////\x1b\\");
        #[cfg(feature = "kitty-graphics")]
        assert!(terminal.kitty_graphics().unwrap().image(1).is_some());
        let snapshot = state.update(&terminal).unwrap();
        let generation = snapshot.image_placements_generation().unwrap();
        {
            let mut iteration = images.update(&snapshot).unwrap();
            assert!(matches!(iteration.image_id(), Err(Error::InvalidValue)));
            assert!(iteration.next().is_none());
            assert!(matches!(iteration.image_id(), Err(Error::InvalidValue)));
        }

        terminal.resize(10, 5, 10, 20).unwrap();
        let snapshot = state.update(&terminal).unwrap();
        assert_ne!(snapshot.image_placements_generation().unwrap(), generation);
        let found = placements(&mut images, &snapshot);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.image_id, 1);
    }

    #[test]
    fn image_iteration_reports_the_source_rectangle() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 20).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=2,v=2,i=7,x=1,y=0,w=1,h=2,q=2;////////////////\x1b\\");
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=2,v=2,i=8,x=0,y=1,w=2,h=1,q=2;////////////////\x1b\\");

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut found = placements(&mut images, &snapshot)
            .into_iter()
            .map(|(p, _)| (p.image_id, p.source))
            .collect::<Vec<_>>();
        found.sort_unstable();
        assert_eq!(found, [(7, (1, 0, 1, 2)), (8, (0, 1, 2, 1))]);
    }

    #[test]
    fn image_iteration_reports_a_fixed_placement() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 20).unwrap();
        terminal.vt_write(
            b"\x1b[2;3H\
              \x1b_Ga=T,f=24,s=2,v=2,i=7,p=3,c=4,r=2,X=3,Y=4,z=5,q=2;////////////////\x1b\\",
        );

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let found = placements(&mut images, &snapshot);
        assert_eq!(
            found.iter().map(|(p, _)| p).collect::<Vec<_>>(),
            [&Placement {
                protocol: ImageProtocol::Kitty,
                image_id: 7,
                layer: ImageLayer::AboveText,
                z: 5,
                viewport: (2, 1),
                grid: (4, 2),
                cell_offset: (3, 4),
                pixel_size: (37, 36),
                source: (0, 0, 2, 2),
                kitty_placement_id: Some(3),
                kitty_kind: Some(KittyPlacementKind::Fixed),
            }],
        );

        #[cfg(feature = "kitty-graphics")]
        {
            let image = terminal.kitty_graphics().unwrap().image(7).unwrap();
            assert_eq!(found[0].1, image.generation().unwrap());
        }

        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=8,P=7,Q=3,H=1,V=2,q=2;////\x1b\\");
        let snapshot = state.update(&terminal).unwrap();
        let relative = placements(&mut images, &snapshot)
            .into_iter()
            .map(|(p, _)| p)
            .find(|p| p.image_id == 8)
            .unwrap();
        assert_eq!(relative.viewport, (3, 3));
        assert_eq!(relative.kitty_placement_id, Some(0));
        assert_eq!(relative.kitty_kind, Some(KittyPlacementKind::Relative));
    }

    #[test]
    fn image_iteration_reports_a_virtual_placement() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,t=d,f=24,i=1,p=1,s=1,v=2,U=1,c=2,r=1,q=2;////////\x1b\\");
        terminal.vt_write("\x1b[2;4H\x1b[38;5;1;58;5;1m\u{10EEEE}\u{10EEEE}".as_bytes());

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let found = placements(&mut images, &snapshot);
        assert_eq!(
            found.iter().map(|(p, _)| p).collect::<Vec<_>>(),
            [&Placement {
                protocol: ImageProtocol::Kitty,
                image_id: 1,
                layer: ImageLayer::BelowText,
                z: -1,
                viewport: (3, 1),
                grid: (2, 1),
                cell_offset: (8, 0),
                pixel_size: (5, 10),
                source: (0, 0, 1, 2),
                kitty_placement_id: Some(1),
                kitty_kind: Some(KittyPlacementKind::Virtual),
            }],
        );
    }

    #[test]
    fn image_placements_follow_scrolling_and_track_changes() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1,c=2,r=4,q=2;////\x1b\\");

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let generation = snapshot.image_placements_generation().unwrap();
        let found = placements(&mut images, &snapshot);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].0.viewport, found[0].0.grid), ((0, 0), (2, 4)));

        terminal.vt_write(b"\x1b[5;5Hhi");
        let snapshot = state.update(&terminal).unwrap();
        assert_eq!(snapshot.image_placements_generation().unwrap(), generation);

        terminal.vt_write(b"\n\n");
        let snapshot = state.update(&terminal).unwrap();
        assert_ne!(snapshot.image_placements_generation().unwrap(), generation);
        let found = placements(&mut images, &snapshot);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].0.viewport, found[0].0.grid), ((0, -2), (2, 4)));
    }

    #[test]
    fn image_layer_follows_the_z_index() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        for (id, z) in [(1, -1_073_741_825), (2, -1_073_741_824), (3, -1), (4, 0)] {
            terminal
                .vt_write(format!("\x1b_Ga=T,f=24,s=1,v=1,i={id},z={z},q=2;////\x1b\\").as_bytes());
        }

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut found = placements(&mut images, &snapshot)
            .into_iter()
            .map(|(p, _)| (p.image_id, p.z, p.layer))
            .collect::<Vec<_>>();
        found.sort_unstable_by_key(|&(id, ..)| id);
        assert_eq!(
            found,
            [
                (1, -1_073_741_825, ImageLayer::BelowBackground),
                (2, -1_073_741_824, ImageLayer::BelowText),
                (3, -1, ImageLayer::BelowText),
                (4, 0, ImageLayer::AboveText),
            ],
        );
    }

    #[test]
    fn image_iteration_stays_exhausted() {
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1,q=2;////\x1b\\");

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new().unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let mut iteration = images.update(&snapshot).unwrap();
        assert!(iteration.next().is_some());
        assert!(iteration.next().is_none());
        assert!(iteration.next().is_none());
        assert!(matches!(iteration.image_id(), Err(Error::InvalidValue)));
    }

    #[test]
    #[cfg(not(miri))]
    fn image_iterator_allocation_failure_is_reported() {
        let capped = crate::alloc::testing::Capped::new(0);
        assert!(matches!(
            ImageIterator::new_with_alloc(&capped.allocator()),
            Err(Error::OutOfMemory)
        ));
    }

    #[test]
    #[cfg(all(not(miri), any(target_os = "linux", target_os = "macos")))]
    fn image_iterator_uses_its_allocator() {
        let alloc = crate::alloc::testing::Guard::allocator();
        let mut terminal = Terminal::new(10, 5).unwrap();
        terminal.resize(10, 5, 10, 10).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=1,q=2;////\x1b\\");

        let mut state = RenderState::new().unwrap();
        let mut images = ImageIterator::new_with_alloc(&alloc).unwrap();
        let snapshot = state.update(&terminal).unwrap();
        let found = placements(&mut images, &snapshot);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.image_id, 1);

        terminal.vt_write(b"\x1b_Ga=T,f=24,s=1,v=1,i=2,q=2;////\x1b\\");
        let snapshot = state.update(&terminal).unwrap();
        let mut ids = placements(&mut images, &snapshot)
            .into_iter()
            .map(|(p, _)| p.image_id)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, [1, 2]);
        drop(images);
    }
}
