use std::marker::PhantomData;

use crate::{
    alloc::{Allocator, Bytes},
    error::{Error, Result, from_optional_result, from_optional_result_with_len, from_result},
    ffi,
    fmt::Format,
    screen::GridRef,
    terminal::{Point, Terminal},
};

pub mod gesture;

#[derive(Clone, Debug)]
pub struct Selection<'t> {
    pub(crate) inner: ffi::Selection,
    _phan: PhantomData<&'t ffi::Terminal>,
}
impl<'t> Selection<'t> {
    #[must_use]
    pub fn new(start: GridRef<'t>, end: GridRef<'t>, rectangle: bool) -> Self {
        unsafe {
            Self::from_raw(ffi::Selection {
                start: start.inner,
                end: end.inner,
                rectangle,
                ..ffi::sized!(ffi::Selection)
            })
        }
    }

    pub(crate) unsafe fn from_raw(value: ffi::Selection) -> Self {
        Self {
            inner: value,
            _phan: PhantomData,
        }
    }

    #[must_use]
    pub fn start(&self) -> GridRef<'t> {
        unsafe { GridRef::from_raw(self.inner.start) }
    }
    #[must_use]
    pub fn end(&self) -> GridRef<'t> {
        unsafe { GridRef::from_raw(self.inner.end) }
    }
    #[must_use]
    pub fn is_rectangle(&self) -> bool {
        self.inner.rectangle
    }

    pub fn adjust(&mut self, terminal: &'t Terminal<'_, '_>, adjustment: Adjustment) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_terminal_selection_adjust(
                terminal.inner.as_raw(),
                &raw mut self.inner,
                adjustment.into(),
            )
        };
        from_result(result)
    }

    pub fn contains(&self, terminal: &'t Terminal<'_, '_>, point: Point) -> Result<bool> {
        let mut contains = false;
        let result = unsafe {
            ffi::ghostty_terminal_selection_contains(
                terminal.inner.as_raw(),
                &raw const self.inner,
                point.into(),
                &raw mut contains,
            )
        };
        from_result(result)?;
        Ok(contains)
    }

    pub fn equals(&self, terminal: &'t Terminal<'_, '_>, other: &Self) -> Result<bool> {
        let mut equal = false;
        let result = unsafe {
            ffi::ghostty_terminal_selection_equal(
                terminal.inner.as_raw(),
                &raw const self.inner,
                &raw const other.inner,
                &raw mut equal,
            )
        };
        from_result(result)?;
        Ok(equal)
    }

    pub fn order(&self, terminal: &'t Terminal<'_, '_>) -> Result<Order> {
        let mut order = ffi::SelectionOrder::FORWARD;

        let result = unsafe {
            ffi::ghostty_terminal_selection_order(
                terminal.inner.as_raw(),
                &raw const self.inner,
                &raw mut order,
            )
        };
        from_result(result)?;
        Order::try_from(order).map_err(|_| Error::InvalidValue)
    }

    pub fn to_ordered(&self, terminal: &'t Terminal<'_, '_>, desired: Order) -> Result<Self> {
        let mut selection = ffi::sized!(ffi::Selection);
        let result = unsafe {
            ffi::ghostty_terminal_selection_ordered(
                terminal.inner.as_raw(),
                &raw const self.inner,
                desired.into(),
                &raw mut selection,
            )
        };
        from_result(result)?;
        Ok(unsafe { Self::from_raw(selection) })
    }
}

impl Terminal<'_, '_> {
    pub fn selection(&self) -> Result<Option<Selection<'_>>> {
        self.get_optional::<ffi::Selection>(ffi::TerminalData::SELECTION)
            .map(|v| v.map(|raw| unsafe { Selection::from_raw(raw) }))
    }

    pub fn set_selection(&self, selection: Option<&Selection<'_>>) -> Result<&Self> {
        self.set_optional(ffi::TerminalOption::SELECTION, selection.map(|v| &v.inner))?;
        Ok(self)
    }

    pub fn select_all(&self) -> Result<Option<Selection<'_>>> {
        let mut value = ffi::sized!(ffi::Selection);
        let result =
            unsafe { ffi::ghostty_terminal_select_all(self.inner.as_raw(), &raw mut value) };

        let sel = from_optional_result(result, value)?;
        Ok(sel.map(|v| unsafe {
            Selection::from_raw(v)
        }))
    }
    pub fn select_line(&self, options: SelectLineOptions) -> Result<Option<Selection<'_>>> {
        let mut value = ffi::sized!(ffi::Selection);

        let result = unsafe {
            ffi::ghostty_terminal_select_line(
                self.inner.as_raw(),
                &raw const options.inner,
                &raw mut value,
            )
        };

        let sel = from_optional_result(result, value)?;
        Ok(sel.map(|v| unsafe {
            Selection::from_raw(v)
        }))
    }
    pub fn select_output(&self, grid_ref: GridRef<'_>) -> Result<Option<Selection<'_>>> {
        let mut value = ffi::sized!(ffi::Selection);

        let result = unsafe {
            ffi::ghostty_terminal_select_output(self.inner.as_raw(), grid_ref.inner, &raw mut value)
        };

        let sel = from_optional_result(result, value)?;
        Ok(sel.map(|v| unsafe {
            Selection::from_raw(v)
        }))
    }
    pub fn select_word(&self, options: SelectWordOptions) -> Result<Option<Selection<'_>>> {
        let mut value = ffi::sized!(ffi::Selection);

        let result = unsafe {
            ffi::ghostty_terminal_select_word(
                self.inner.as_raw(),
                &raw const options.inner,
                &raw mut value,
            )
        };

        let sel = from_optional_result(result, value)?;
        Ok(sel.map(|v| unsafe {
            Selection::from_raw(v)
        }))
    }

    pub fn select_word_between(
        &self,
        options: SelectWordBetweenOptions,
    ) -> Result<Option<Selection<'_>>> {
        let mut value = ffi::sized!(ffi::Selection);

        let result = unsafe {
            ffi::ghostty_terminal_select_word_between(
                self.inner.as_raw(),
                &raw const options.inner,
                &raw mut value,
            )
        };

        let sel = from_optional_result(result, value)?;
        Ok(sel.map(|v| unsafe {
            Selection::from_raw(v)
        }))
    }

    pub fn format_selection_alloc<'a, 'ctx: 'a>(
        &self,
        alloc: Option<&'a Allocator<'ctx>>,
        options: FormatOptions,
    ) -> Result<Option<Bytes<'a>>> {
        let mut out = std::ptr::null_mut();
        let mut out_len = 0usize;
        let alloc = alloc.map_or(std::ptr::null(), super::alloc::Allocator::to_raw);

        let result = unsafe {
            ffi::ghostty_terminal_selection_format_alloc(
                self.inner.as_raw(),
                alloc,
                options.inner,
                &raw mut out,
                &raw mut out_len,
            )
        };

        let out = from_optional_result(result, out)?;
        Ok(out.map(|ptr| unsafe { Bytes::from_raw_parts(ptr, out_len, alloc) }))
    }

    pub fn format_selection_buf(
        &self,
        options: FormatOptions,
        buf: &mut [u8],
    ) -> Result<Option<usize>> {
        let mut written = 0usize;

        let result = unsafe {
            ffi::ghostty_terminal_selection_format_buf(
                self.inner.as_raw(),
                options.inner,
                buf.as_mut_ptr(),
                buf.len(),
                &raw mut written,
            )
        };

        from_optional_result_with_len(result, written)
    }
}

#[derive(Clone, Debug)]
pub struct SelectLineOptions<'t, 'ws> {
    inner: ffi::TerminalSelectLineOptions,
    _phan: (PhantomData<&'t ffi::Terminal>, PhantomData<&'ws [char]>),
}
impl<'t, 'ws> SelectLineOptions<'t, 'ws> {
    #[must_use]
    pub fn new(grid_ref: GridRef<'t>) -> Self {
        Self {
            inner: ffi::TerminalSelectLineOptions {
                ref_: grid_ref.inner,
                whitespace: std::ptr::null(),
                whitespace_len: 0,
                semantic_prompt_boundary: false,
                ..ffi::sized!(ffi::TerminalSelectLineOptions)
            },
            _phan: (PhantomData, PhantomData),
        }
    }

    #[must_use]
    pub fn with_whitespace(mut self, value: &'ws [char]) -> Self {
        self.inner.whitespace = value.as_ptr().cast();
        self.inner.whitespace_len = value.len();
        self
    }

    #[must_use]
    pub fn with_semantic_prompt_boundary(mut self, value: bool) -> Self {
        self.inner.semantic_prompt_boundary = value;
        self
    }
}

#[derive(Clone, Debug)]
pub struct SelectWordOptions<'t, 'bc> {
    inner: ffi::TerminalSelectWordOptions,
    _phan: (PhantomData<&'t ffi::Terminal>, PhantomData<&'bc [char]>),
}
impl<'t, 'bc> SelectWordOptions<'t, 'bc> {
    #[must_use]
    pub fn new(grid_ref: GridRef<'t>) -> Self {
        Self {
            inner: ffi::TerminalSelectWordOptions {
                ref_: grid_ref.inner,
                ..ffi::sized!(ffi::TerminalSelectWordOptions)
            },
            _phan: (PhantomData, PhantomData),
        }
    }

    #[must_use]
    pub fn with_boundary_codepoints(mut self, value: &'bc [char]) -> Self {
        self.inner.boundary_codepoints = value.as_ptr().cast();
        self.inner.boundary_codepoints_len = value.len();
        self
    }
}

#[derive(Debug)]
pub struct SelectWordBetweenOptions<'t, 'bc> {
    inner: ffi::TerminalSelectWordBetweenOptions,
    _phan: (PhantomData<&'t ffi::Terminal>, PhantomData<&'bc [char]>),
}
impl<'t, 'bc> SelectWordBetweenOptions<'t, 'bc> {
    #[must_use]
    pub fn new(start: GridRef<'t>, end: GridRef<'t>) -> Self {
        Self {
            inner: ffi::TerminalSelectWordBetweenOptions {
                start: start.inner,
                end: end.inner,
                ..ffi::sized!(ffi::TerminalSelectWordBetweenOptions)
            },
            _phan: (PhantomData, PhantomData),
        }
    }

    #[must_use]
    pub fn with_boundary_codepoints(mut self, value: &'bc [char]) -> Self {
        self.inner.boundary_codepoints = value.as_ptr().cast();
        self.inner.boundary_codepoints_len = value.len();
        self
    }
}

#[derive(Debug)]
pub struct FormatOptions<'t, 's> {
    inner: ffi::TerminalSelectionFormatOptions,
    _phan: PhantomData<&'s Selection<'t>>,
}
impl<'t, 's> FormatOptions<'t, 's> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: ffi::TerminalSelectionFormatOptions {
                ..ffi::sized!(ffi::TerminalSelectionFormatOptions)
            },
            _phan: PhantomData,
        }
    }
    #[must_use]
    pub fn with_emit_format(mut self, value: Format) -> Self {
        self.inner.emit = value.into();
        self
    }
    #[must_use]
    pub fn with_unwrap(mut self, value: bool) -> Self {
        self.inner.unwrap = value;
        self
    }
    #[must_use]
    pub fn with_trim(mut self, value: bool) -> Self {
        self.inner.trim = value;
        self
    }
    #[must_use]
    pub fn with_selection(mut self, value: &'s Selection<'t>) -> Self {
        self.inner.selection = &raw const value.inner;
        self
    }
}
impl Default for FormatOptions<'_, '_> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum Adjustment {
    Left = ffi::SelectionAdjust::LEFT,
    Right = ffi::SelectionAdjust::RIGHT,
    Up = ffi::SelectionAdjust::UP,
    Down = ffi::SelectionAdjust::DOWN,
    Home = ffi::SelectionAdjust::HOME,
    End = ffi::SelectionAdjust::END,
    PageUp = ffi::SelectionAdjust::PAGE_UP,
    PageDown = ffi::SelectionAdjust::PAGE_DOWN,
    BeginningOfLine = ffi::SelectionAdjust::BEGINNING_OF_LINE,
    EndOfLine = ffi::SelectionAdjust::END_OF_LINE,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum Order {
    Forward = ffi::SelectionOrder::FORWARD,
    Reverse = ffi::SelectionOrder::REVERSE,
    MirroredForward = ffi::SelectionOrder::MIRRORED_FORWARD,
    MirroredReverse = ffi::SelectionOrder::MIRRORED_REVERSE,
}
