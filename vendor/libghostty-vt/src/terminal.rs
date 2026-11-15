use std::{io::Write, marker::PhantomData, mem::MaybeUninit, num::NonZeroU16, ptr::NonNull};

use crate::{
    alloc::{Allocator, Bytes, Object},
    error::{
        Error, Result, from_optional_result, from_optional_result_uninit,
        from_optional_result_with_len, from_result, from_result_with_len,
    },
    ffi::{self, TerminalData as Data, TerminalOption as Opt},
    key, mouse, osc,
    screen::{GridRef, Screen, TrackedGridRef},
    style::{self, Palette, RawPalette, RgbColor},
};

#[doc(inline)]
pub use ffi::{SizeReportSize, TerminalScrollbar as Scrollbar};

#[derive(Debug)]
pub struct Terminal<'alloc: 'cb, 'cb> {
    pub(crate) inner: Object<'alloc, ffi::TerminalImpl>,
    vtable: *mut VTable<'alloc, 'cb>,
    pub(crate) id: u64,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
pub enum CursorStyle {
    Bar = ffi::TerminalCursorStyle::BAR,
    Block = ffi::TerminalCursorStyle::BLOCK,
    Underline = ffi::TerminalCursorStyle::UNDERLINE,
    BlockHollow = ffi::TerminalCursorStyle::BLOCK_HOLLOW,
}

impl<'alloc: 'cb, 'cb> Terminal<'alloc, 'cb> {
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null(), cols, rows) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        cols: u16,
        rows: u16,
    ) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw(), cols, rows) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator, cols: u16, rows: u16) -> Result<Self> {
        let mut raw: ffi::Terminal = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_terminal_new(alloc, &raw mut raw, cols, rows) };
        from_result(result)?;
        unsafe { Self::from_raw(raw) }
    }

    pub(crate) unsafe fn from_raw(raw: ffi::Terminal) -> Result<Self> {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Ok(Self {
            inner: Object::new(raw)?,
            vtable: Box::into_raw(Box::new(VTable::default())),
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        })
    }

    pub fn vt_write(&mut self, data: &[u8]) {
        unsafe { ffi::ghostty_terminal_vt_write(self.inner.as_raw(), data.as_ptr(), data.len()) }
    }

    pub fn vt_write_until_ground(&mut self, data: &[u8]) -> Result<Option<usize>> {
        let mut stop = ffi::sized!(ffi::TerminalWriteStop);
        let result = unsafe {
            ffi::ghostty_terminal_vt_write_until(
                self.inner.as_raw(),
                data.as_ptr(),
                data.len(),
                ffi::TerminalBoundary::GROUND as u32,
                &raw mut stop,
            )
        };
        from_optional_result_with_len(result, stop.consumed)
    }

    pub fn is_vt_ground(&self) -> Result<bool> {
        self.get(Data::VT_GROUND)
    }

    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_terminal_resize(
                self.inner.as_raw(),
                cols,
                rows,
                cell_width_px,
                cell_height_px,
            )
        };
        from_result(result)
    }

    pub fn reset(&mut self) {
        unsafe { ffi::ghostty_terminal_reset(self.inner.as_raw()) }
    }

    pub fn clear_screen(&mut self) -> bool {
        unsafe { ffi::ghostty_terminal_clear_screen(self.inner.as_raw()) }
    }

    pub fn scroll_viewport(&mut self, scroll: ScrollViewport) {
        unsafe { ffi::ghostty_terminal_scroll_viewport(self.inner.as_raw(), scroll.into()) }
    }

    pub fn grid_ref(&self, point: Point) -> Result<GridRef<'_>> {
        let mut grid_ref = ffi::sized!(ffi::GridRef);
        let result = unsafe {
            ffi::ghostty_terminal_grid_ref(self.inner.as_raw(), point.into(), &raw mut grid_ref)
        };
        from_result(result)?;
        Ok(unsafe { GridRef::from_raw(grid_ref) })
    }

    pub fn track_grid_ref(&self, point: Point) -> Result<TrackedGridRef> {
        let mut raw: ffi::TrackedGridRef = std::ptr::null_mut();
        let result = unsafe {
            ffi::ghostty_terminal_grid_ref_track(self.inner.as_raw(), point.into(), &raw mut raw)
        };
        from_result(result)?;

        let inner = NonNull::new(raw).ok_or(Error::InvalidValue)?;
        Ok(TrackedGridRef::new(inner, self.inner.ptr))
    }

    pub fn point_from_grid_ref(
        &self,
        grid_ref: &GridRef<'_>,
        space: PointSpace,
    ) -> Result<Option<PointCoordinate>> {
        let mut point = MaybeUninit::<ffi::PointCoordinate>::zeroed();
        let result = unsafe {
            ffi::ghostty_terminal_point_from_grid_ref(
                self.inner.as_raw(),
                std::ptr::from_ref(&grid_ref.inner),
                space.into_raw(),
                point.as_mut_ptr(),
            )
        };

        from_optional_result_uninit(result, point).map(|value| value.map(Into::into))
    }

    pub fn mode(&self, mode: Mode) -> Result<bool> {
        let mut mode = ffi::TerminalModeConfig {
            mode: mode.into(),
            value: false,
        };

        let result = unsafe {
            ffi::ghostty_terminal_get(
                self.inner.as_raw(),
                Data::MODE,
                (&raw mut mode).cast::<std::ffi::c_void>(),
            )
        };
        from_result(result)?;
        Ok(mode.value)
    }

    pub fn set_mode(&mut self, mode: Mode, value: bool) -> Result<&mut Self> {
        let mode = ffi::TerminalModeConfig {
            mode: mode.into(),
            value,
        };

        let result = unsafe {
            ffi::ghostty_terminal_set(
                self.inner.as_raw(),
                Opt::MODE,
                (&raw const mode).cast::<std::ffi::c_void>(),
            )
        };
        from_result(result)?;
        Ok(self)
    }

    pub fn set_default_mode(&mut self, mode: Mode, value: bool) -> Result<&mut Self> {
        let mode = ffi::TerminalModeConfig {
            mode: mode.into(),
            value,
        };

        let result = unsafe {
            ffi::ghostty_terminal_set(
                self.inner.as_raw(),
                Opt::MODE_DEFAULT,
                (&raw const mode).cast::<std::ffi::c_void>(),
            )
        };
        from_result(result)?;
        Ok(self)
    }

    pub fn compress(&mut self, mode: CompressionMode) -> Result<CompressionResult> {
        let mut value = ffi::TerminalCompressionResult::UNSUPPORTED;
        let result = unsafe {
            ffi::ghostty_terminal_compress(self.inner.as_raw(), mode.into(), &raw mut value)
        };
        from_result(result)?;
        value.try_into().map_err(|_| Error::InvalidValue)
    }

    pub fn compression_activity(&self) -> Result<CompressionActivity> {
        let mut value = 0;
        let result = unsafe {
            ffi::ghostty_terminal_compression_activity(self.inner.as_raw(), &raw mut value)
        };
        from_result(result)?;
        Ok(CompressionActivity(value))
    }

    pub fn continuation_max_bytes(&self) -> Result<usize> {
        self.get(Data::CONTINUATION_MAX_BYTES)
    }

    pub fn set_continuation_max_bytes(&mut self, v: usize) -> Result<&mut Self> {
        self.set(Opt::CONTINUATION_MAX_BYTES, &v)?;
        Ok(self)
    }

    pub fn continuation_write<W: Write>(&mut self, writer: &mut W) -> Result<()> {
        let writer = crate::io::to_writer(writer);
        let result =
            unsafe { ffi::ghostty_terminal_continuation_write(self.inner.as_raw(), writer) };
        from_result(result)
    }

    pub fn continuation_alloc<'a, 'ctx: 'a>(
        &self,
        alloc: Option<&'a Allocator<'ctx>>,
    ) -> Result<Option<Bytes<'a>>> {
        let mut out = std::ptr::null_mut();
        let mut out_len = 0usize;
        let alloc = alloc.map_or(std::ptr::null(), super::alloc::Allocator::to_raw);

        let result = unsafe {
            ffi::ghostty_terminal_continuation_alloc(
                self.inner.as_raw(),
                alloc,
                &raw mut out,
                &raw mut out_len,
            )
        };

        let out = from_optional_result(result, out)?;
        Ok(out.map(|ptr| unsafe { Bytes::from_raw_parts(ptr, out_len, alloc) }))
    }

    pub fn continuation_buf(&self, buf: &mut [u8]) -> Result<Option<usize>> {
        let mut written = 0usize;
        let buf_ptr = if buf.is_empty() {
            std::ptr::null_mut()
        } else {
            buf.as_mut_ptr()
        };

        let result = unsafe {
            ffi::ghostty_terminal_continuation_buf(
                self.inner.as_raw(),
                buf_ptr,
                buf.len(),
                &raw mut written,
            )
        };

        from_optional_result_with_len(result, written)
    }

    pub(crate) fn get<T>(&self, tag: ffi::TerminalData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_terminal_get(self.inner.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }
    pub(crate) fn get_optional<T>(&self, tag: ffi::TerminalData::Type) -> Result<Option<T>> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_terminal_get(self.inner.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_optional_result_uninit(result, value)
    }
    pub(crate) fn set<T>(&self, tag: ffi::TerminalOption::Type, v: &T) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_terminal_set(self.inner.as_raw(), tag, std::ptr::from_ref(v).cast())
        };
        from_result(result)
    }
    pub(crate) fn set_ptr(
        &self,
        tag: ffi::TerminalOption::Type,
        ptr: *const std::ffi::c_void,
    ) -> Result<()> {
        let result = unsafe { ffi::ghostty_terminal_set(self.inner.as_raw(), tag, ptr) };
        from_result(result)
    }
    pub(crate) fn set_optional<T>(
        &self,
        tag: ffi::TerminalOption::Type,
        v: Option<&T>,
    ) -> Result<()> {
        let ptr = if let Some(v) = v {
            std::ptr::from_ref(v)
        } else {
            std::ptr::null()
        };

        let result = unsafe { ffi::ghostty_terminal_set(self.inner.as_raw(), tag, ptr.cast()) };
        from_result(result)
    }

    pub fn cols(&self) -> Result<u16> {
        self.get(Data::COLS)
    }
    pub fn rows(&self) -> Result<u16> {
        self.get(Data::ROWS)
    }
    pub fn width_px(&self) -> Result<u32> {
        self.get(Data::WIDTH_PX)
    }
    pub fn height_px(&self) -> Result<u32> {
        self.get(Data::HEIGHT_PX)
    }

    pub fn scrollback_max_bytes(&self) -> Result<Option<usize>> {
        self.get_optional(Data::SCROLLBACK_MAX_BYTES)
    }

    pub fn set_scrollback_max_bytes(&mut self, v: Option<usize>) -> Result<&mut Self> {
        self.set_optional(Opt::SCROLLBACK_MAX_BYTES, v.as_ref())?;
        Ok(self)
    }

    pub fn scrollback_max_lines(&self) -> Result<Option<usize>> {
        self.get_optional(Data::SCROLLBACK_MAX_LINES)
    }

    pub fn set_scrollback_max_lines(&mut self, v: Option<usize>) -> Result<&mut Self> {
        self.set_optional(Opt::SCROLLBACK_MAX_LINES, v.as_ref())?;
        Ok(self)
    }

    pub fn cursor_x(&self) -> Result<u16> {
        self.get(Data::CURSOR_X)
    }
    pub fn cursor_y(&self) -> Result<u16> {
        self.get(Data::CURSOR_Y)
    }
    pub fn is_cursor_at_prompt(&self) -> Result<bool> {
        self.get(Data::CURSOR_AT_PROMPT)
    }
    pub fn is_cursor_pending_wrap(&self) -> Result<bool> {
        self.get(Data::CURSOR_PENDING_WRAP)
    }
    pub fn is_cursor_visible(&self) -> Result<bool> {
        self.get(Data::CURSOR_VISIBLE)
    }
    pub fn cursor_style(&self) -> Result<style::Style> {
        self.get::<ffi::Style>(Data::CURSOR_STYLE)
            .and_then(std::convert::TryInto::try_into)
    }
    pub fn mouse_shape(&self) -> Result<mouse::Shape> {
        self.get::<ffi::MouseShape::Type>(Data::MOUSE_SHAPE)?
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }
    pub fn kitty_keyboard_flags(&self) -> Result<key::KittyKeyFlags> {
        self.get::<ffi::KittyKeyFlags>(Data::KITTY_KEYBOARD_FLAGS)
            .map(key::KittyKeyFlags::from_bits_retain)
    }

    pub fn scrollbar(&self) -> Result<Scrollbar> {
        self.get(Data::SCROLLBAR)
    }
    pub fn active_screen(&self) -> Result<Screen> {
        self.get::<ffi::TerminalScreen::Type>(Data::ACTIVE_SCREEN)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    pub fn viewport_active(&self) -> Result<bool> {
        self.get(Data::VIEWPORT_ACTIVE)
    }
    pub fn is_mouse_tracking(&self) -> Result<bool> {
        self.get(Data::MOUSE_TRACKING)
    }
    pub fn vt_processing_error(&self) -> Result<bool> {
        self.get(Data::VT_PROCESSING_ERROR)
    }
    pub fn title(&self) -> Result<&[u8]> {
        let str = self.get::<ffi::String>(Data::TITLE)?;
        Ok(unsafe { str.to_bytes() })
    }

    pub fn pwd(&self) -> Result<&[u8]> {
        let str = self.get::<ffi::String>(Data::PWD)?;
        Ok(unsafe { str.to_bytes() })
    }

    pub fn set_title(&mut self, title: impl AsRef<[u8]>) -> Result<&mut Self> {
        let title = title.as_ref();
        let raw = ffi::String {
            ptr: title.as_ptr(),
            len: title.len(),
        };
        self.set(Opt::TITLE, &raw)?;
        Ok(self)
    }

    pub fn set_pwd(&mut self, pwd: impl AsRef<[u8]>) -> Result<&mut Self> {
        let pwd = pwd.as_ref();
        let raw = ffi::String {
            ptr: pwd.as_ptr(),
            len: pwd.len(),
        };
        self.set(Opt::PWD, &raw)?;
        Ok(self)
    }
    pub fn total_rows(&self) -> Result<usize> {
        self.get(Data::TOTAL_ROWS)
    }
    pub fn scrollback_rows(&self) -> Result<usize> {
        self.get(Data::SCROLLBACK_ROWS)
    }

    pub fn fg_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_FOREGROUND)
            .map(|v| v.map(Into::into))
    }
    pub fn default_fg_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_FOREGROUND_DEFAULT)
            .map(|v| v.map(Into::into))
    }
    pub fn set_default_fg_color(&mut self, v: Option<RgbColor>) -> Result<&mut Self> {
        self.set_optional(Opt::COLOR_FOREGROUND, v.map(ffi::ColorRgb::from).as_ref())?;
        Ok(self)
    }

    pub fn bg_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_BACKGROUND)
            .map(|v| v.map(Into::into))
    }
    pub fn default_bg_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_BACKGROUND_DEFAULT)
            .map(|v| v.map(Into::into))
    }
    pub fn set_default_bg_color(&mut self, v: Option<RgbColor>) -> Result<&mut Self> {
        self.set_optional(Opt::COLOR_BACKGROUND, v.map(ffi::ColorRgb::from).as_ref())?;
        Ok(self)
    }

    pub fn cursor_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_CURSOR)
            .map(|v| v.map(Into::into))
    }
    pub fn default_cursor_color(&self) -> Result<Option<RgbColor>> {
        self.get_optional::<ffi::ColorRgb>(Data::COLOR_CURSOR_DEFAULT)
            .map(|v| v.map(Into::into))
    }
    pub fn set_default_cursor_color(&mut self, v: Option<RgbColor>) -> Result<&mut Self> {
        self.set_optional(Opt::COLOR_CURSOR, v.map(ffi::ColorRgb::from).as_ref())?;
        Ok(self)
    }

    pub fn set_default_cursor_style(&mut self, v: Option<CursorStyle>) -> Result<&mut Self> {
        self.set_optional(Opt::DEFAULT_CURSOR_STYLE, v.as_ref())?;
        Ok(self)
    }

    pub fn set_default_cursor_blink(&mut self, v: Option<bool>) -> Result<&mut Self> {
        self.set_optional(Opt::DEFAULT_CURSOR_BLINK, v.as_ref())?;
        Ok(self)
    }

    pub fn set_resize_pull_scrollback(&mut self, v: Option<bool>) -> Result<&mut Self> {
        self.set_optional(Opt::RESIZE_PULL_SCROLLBACK, v.as_ref())?;
        Ok(self)
    }

    pub fn color_palette(&self) -> Result<Palette> {
        self.get::<RawPalette>(Data::COLOR_PALETTE)
            .map(Palette::from)
    }
    pub fn default_color_palette(&self) -> Result<Palette> {
        self.get::<RawPalette>(Data::COLOR_PALETTE_DEFAULT)
            .map(Palette::from)
    }
    pub fn set_default_color_palette(&mut self, v: Option<Palette>) -> Result<&mut Self> {
        self.set_optional::<RawPalette>(
            Opt::COLOR_PALETTE,
            v.map(std::convert::Into::into).as_ref(),
        )?;
        Ok(self)
    }

    pub fn set_apc_max_bytes(&mut self, max: Option<usize>) -> Result<&mut Self> {
        self.set_optional(Opt::APC_MAX_BYTES, max.as_ref())?;
        Ok(self)
    }

    pub fn set_unknown_max_bytes(&mut self, max: usize) -> Result<&mut Self> {
        self.set(Opt::UNKNOWN_MAX_BYTES, &max)?;
        Ok(self)
    }

    pub fn set_terminfo_name(&mut self, name: &str) -> Result<&mut Self> {
        self.set(Opt::TERMINFO_NAME, &ffi::String::from(name))?;
        Ok(self)
    }

    pub fn clipboard_write_max_bytes(&self) -> Result<usize> {
        self.get(Data::CLIPBOARD_WRITE_MAX_BYTES)
    }

    pub fn set_clipboard_write_max_bytes(&mut self, limit: Option<usize>) -> Result<&mut Self> {
        self.set_optional(Opt::CLIPBOARD_WRITE_MAX_BYTES, limit.as_ref())?;
        Ok(self)
    }

    pub fn set_glyph_protocol_enabled(&mut self, enabled: bool) -> Result<&mut Self> {
        self.set(Opt::GLYPH_PROTOCOL, &enabled)?;
        Ok(self)
    }

    pub fn set_title_report_enabled(&mut self, enabled: bool) -> Result<&mut Self> {
        self.set(Opt::TITLE_REPORT, &enabled)?;
        Ok(self)
    }

    pub fn set_xt_checksum_report_enabled(&mut self, enabled: bool) -> Result<&mut Self> {
        self.set(Opt::XT_CHECKSUM_REPORT, &enabled)?;
        Ok(self)
    }

    pub fn set_xt_checksum_extension(&mut self, ext: XtChecksumExtension) -> Result<&mut Self> {
        self.set(Opt::XT_CHECKSUM_EXTENSION, &ext.bits())?;
        Ok(self)
    }
}
impl Drop for Terminal<'_, '_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_terminal_free(self.inner.as_raw()) }
        unsafe { drop(Box::from_raw(self.vtable)) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    Active(PointCoordinate),
    Viewport(PointCoordinate),
    Screen(PointCoordinate),
    History(PointCoordinate),
}

impl From<Point> for ffi::Point {
    fn from(value: Point) -> Self {
        match value {
            Point::Active(coord) => Self {
                tag: ffi::PointTag::ACTIVE,
                value: ffi::PointValue {
                    coordinate: coord.into(),
                },
            },
            Point::Viewport(coord) => Self {
                tag: ffi::PointTag::VIEWPORT,
                value: ffi::PointValue {
                    coordinate: coord.into(),
                },
            },
            Point::Screen(coord) => Self {
                tag: ffi::PointTag::SCREEN,
                value: ffi::PointValue {
                    coordinate: coord.into(),
                },
            },
            Point::History(coord) => Self {
                tag: ffi::PointTag::HISTORY,
                value: ffi::PointValue {
                    coordinate: coord.into(),
                },
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointSpace {
    Active,
    Viewport,
    Screen,
    History,
}

impl PointSpace {
    pub(crate) fn into_raw(self) -> ffi::PointTag::Type {
        match self {
            Self::Active => ffi::PointTag::ACTIVE,
            Self::Viewport => ffi::PointTag::VIEWPORT,
            Self::Screen => ffi::PointTag::SCREEN,
            Self::History => ffi::PointTag::HISTORY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointCoordinate {
    pub x: u16,
    pub y: u32,
}
impl From<PointCoordinate> for ffi::PointCoordinate {
    fn from(value: PointCoordinate) -> Self {
        let PointCoordinate { x, y } = value;
        Self { x, y }
    }
}
impl From<ffi::PointCoordinate> for PointCoordinate {
    fn from(value: ffi::PointCoordinate) -> Self {
        let ffi::PointCoordinate { x, y } = value;
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollViewport {
    Top,
    Bottom,
    Delta(isize),
    Row(usize),
    DeltaPrompt(isize),
}
impl From<ScrollViewport> for ffi::TerminalScrollViewport {
    fn from(value: ScrollViewport) -> Self {
        match value {
            ScrollViewport::Top => Self {
                tag: ffi::TerminalScrollViewportTag::TOP,
                value: ffi::TerminalScrollViewportValue::default(),
            },
            ScrollViewport::Bottom => Self {
                tag: ffi::TerminalScrollViewportTag::BOTTOM,
                value: ffi::TerminalScrollViewportValue::default(),
            },
            ScrollViewport::Delta(delta) => Self {
                tag: ffi::TerminalScrollViewportTag::DELTA,
                value: {
                    let mut v = ffi::TerminalScrollViewportValue::default();
                    v.delta = delta;
                    v
                },
            },
            ScrollViewport::Row(row) => Self {
                tag: ffi::TerminalScrollViewportTag::ROW,
                value: {
                    let mut v = ffi::TerminalScrollViewportValue::default();
                    v.row = row;
                    v
                },
            },
            ScrollViewport::DeltaPrompt(delta_prompt) => Self {
                tag: ffi::TerminalScrollViewportTag::DELTA_PROMPT,
                value: {
                    let mut v = ffi::TerminalScrollViewportValue::default();
                    v.delta_prompt = delta_prompt;
                    v
                },
            },
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mode(pub ffi::Mode);

impl Mode {
    const ANSI_BIT: u16 = 1 << 15;

    #[must_use]
    pub const fn new(v: u16, kind: ModeKind) -> Self {
        match kind {
            ModeKind::Ansi => Self(v | Self::ANSI_BIT),
            ModeKind::Dec => Self(v),
        }
    }

    #[must_use]
    pub const fn value(self) -> u16 {
        (self.0) & 0x7fff
    }

    #[must_use]
    pub const fn kind(self) -> ModeKind {
        if (self.0) & Self::ANSI_BIT > 0 {
            ModeKind::Ansi
        } else {
            ModeKind::Dec
        }
    }

    pub const KAM: Self = Self::new(2, ModeKind::Ansi);
    pub const INSERT: Self = Self::new(4, ModeKind::Ansi);
    pub const SRM: Self = Self::new(12, ModeKind::Ansi);
    pub const LINEFEED: Self = Self::new(20, ModeKind::Ansi);

    pub const DECCKM: Self = Self::new(1, ModeKind::Dec);
    pub const _132_COLUMN: Self = Self::new(3, ModeKind::Dec);
    pub const SLOW_SCROLL: Self = Self::new(4, ModeKind::Dec);
    pub const REVERSE_COLORS: Self = Self::new(5, ModeKind::Dec);
    pub const ORIGIN: Self = Self::new(6, ModeKind::Dec);
    pub const WRAPAROUND: Self = Self::new(7, ModeKind::Dec);
    pub const AUTOREPEAT: Self = Self::new(8, ModeKind::Dec);
    pub const X10_MOUSE: Self = Self::new(9, ModeKind::Dec);
    pub const CURSOR_BLINKING: Self = Self::new(12, ModeKind::Dec);
    pub const CURSOR_VISIBLE: Self = Self::new(25, ModeKind::Dec);
    pub const ENABLE_MODE3: Self = Self::new(40, ModeKind::Dec);
    pub const REVERSE_WRAP: Self = Self::new(45, ModeKind::Dec);
    pub const ALT_SCREEN_LEGACY: Self = Self::new(47, ModeKind::Dec);
    pub const KEYPAD_KEYS: Self = Self::new(66, ModeKind::Dec);
    pub const LEFT_RIGHT_MARGIN: Self = Self::new(69, ModeKind::Dec);
    pub const NORMAL_MOUSE: Self = Self::new(1000, ModeKind::Dec);
    pub const BUTTON_MOUSE: Self = Self::new(1002, ModeKind::Dec);
    pub const ANY_MOUSE: Self = Self::new(1003, ModeKind::Dec);
    pub const FOCUS_EVENT: Self = Self::new(1004, ModeKind::Dec);
    pub const UTF8_MOUSE: Self = Self::new(1005, ModeKind::Dec);
    pub const SGR_MOUSE: Self = Self::new(1006, ModeKind::Dec);
    pub const ALT_SCROLL: Self = Self::new(1007, ModeKind::Dec);
    pub const URXVT_MOUSE: Self = Self::new(1015, ModeKind::Dec);
    pub const SGR_PIXELS_MOUSE: Self = Self::new(1016, ModeKind::Dec);
    pub const NUMLOCK_KEYPAD: Self = Self::new(1035, ModeKind::Dec);
    pub const ALT_ESC_PREFIX: Self = Self::new(1036, ModeKind::Dec);
    pub const ALT_SENDS_ESC: Self = Self::new(1039, ModeKind::Dec);
    pub const REVERSE_WRAP_EXT: Self = Self::new(1045, ModeKind::Dec);
    pub const ALT_SCREEN: Self = Self::new(1047, ModeKind::Dec);
    pub const SAVE_CURSOR: Self = Self::new(1048, ModeKind::Dec);
    pub const ALT_SCREEN_SAVE: Self = Self::new(1049, ModeKind::Dec);
    pub const BRACKETED_PASTE: Self = Self::new(2004, ModeKind::Dec);
    pub const SYNC_OUTPUT: Self = Self::new(2026, ModeKind::Dec);
    pub const GRAPHEME_CLUSTER: Self = Self::new(2027, ModeKind::Dec);
    pub const COLOR_SCHEME_REPORT: Self = Self::new(2031, ModeKind::Dec);
    pub const VISIBILITY_REPORT: Self = Self::new(2033, ModeKind::Dec);
    pub const IN_BAND_RESIZE: Self = Self::new(2048, ModeKind::Dec);
    pub const PASTE_EVENTS: Self = Self::new(5522, ModeKind::Dec);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModeKind {
    Dec,
    Ansi,
}

impl From<Mode> for ffi::Mode {
    fn from(value: Mode) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceAttributes {
    pub primary: PrimaryDeviceAttributes,
    pub secondary: SecondaryDeviceAttributes,
    pub tertiary: TertiaryDeviceAttributes,
}

impl From<DeviceAttributes> for ffi::DeviceAttributes {
    fn from(value: DeviceAttributes) -> Self {
        Self {
            primary: value.primary.into(),
            secondary: value.secondary.into(),
            tertiary: value.tertiary.into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrimaryDeviceAttributes(ffi::DeviceAttributesPrimary);

impl PrimaryDeviceAttributes {
    #[must_use]
    pub const fn new(
        conformance_level: ConformanceLevel,
        features: &[DeviceAttributeFeature],
    ) -> Self {
        assert!(features.len() <= 64);

        let mut f = [0u16; 64];
        let mut i = 0;
        while i < features.len() {
            f[i] = features[i].0;
            i += 1;
        }

        Self(ffi::DeviceAttributesPrimary {
            conformance_level: conformance_level.0,
            features: f,
            num_features: features.len(),
        })
    }
}

impl From<PrimaryDeviceAttributes> for ffi::DeviceAttributesPrimary {
    fn from(value: PrimaryDeviceAttributes) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConformanceLevel(pub u16);

impl ConformanceLevel {
    pub const VT100: Self = Self(ffi::DA_CONFORMANCE_VT100);
    pub const VT101: Self = Self(ffi::DA_CONFORMANCE_VT101);
    pub const VT102: Self = Self(ffi::DA_CONFORMANCE_VT102);
    pub const VT125: Self = Self(ffi::DA_CONFORMANCE_VT125);
    pub const VT131: Self = Self(ffi::DA_CONFORMANCE_VT131);
    pub const VT132: Self = Self(ffi::DA_CONFORMANCE_VT132);
    pub const VT220: Self = Self(ffi::DA_CONFORMANCE_VT220);
    pub const VT240: Self = Self(ffi::DA_CONFORMANCE_VT240);
    pub const VT320: Self = Self(ffi::DA_CONFORMANCE_VT320);
    pub const VT340: Self = Self(ffi::DA_CONFORMANCE_VT340);
    pub const VT420: Self = Self(ffi::DA_CONFORMANCE_VT420);
    pub const VT510: Self = Self(ffi::DA_CONFORMANCE_VT510);
    pub const VT520: Self = Self(ffi::DA_CONFORMANCE_VT520);
    pub const VT525: Self = Self(ffi::DA_CONFORMANCE_VT525);
    pub const LEVEL_2: Self = Self(ffi::DA_CONFORMANCE_LEVEL_2);
    pub const LEVEL_3: Self = Self(ffi::DA_CONFORMANCE_LEVEL_3);
    pub const LEVEL_4: Self = Self(ffi::DA_CONFORMANCE_LEVEL_4);
    pub const LEVEL_5: Self = Self(ffi::DA_CONFORMANCE_LEVEL_5);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceAttributeFeature(pub u16);

impl DeviceAttributeFeature {
    pub const COLUMNS_132: Self = Self(ffi::DA_FEATURE_COLUMNS_132);
    pub const PRINTER: Self = Self(ffi::DA_FEATURE_PRINTER);
    pub const REGIS: Self = Self(ffi::DA_FEATURE_REGIS);
    pub const SIXEL: Self = Self(ffi::DA_FEATURE_SIXEL);
    pub const SELECTIVE_ERASE: Self = Self(ffi::DA_FEATURE_SELECTIVE_ERASE);
    pub const USER_DEFINED_KEYS: Self = Self(ffi::DA_FEATURE_USER_DEFINED_KEYS);
    pub const NATIONAL_REPLACEMENT: Self = Self(ffi::DA_FEATURE_NATIONAL_REPLACEMENT);
    pub const TECHNICAL_CHARACTERS: Self = Self(ffi::DA_FEATURE_TECHNICAL_CHARACTERS);
    pub const LOCATOR: Self = Self(ffi::DA_FEATURE_LOCATOR);
    pub const TERMINAL_STATE: Self = Self(ffi::DA_FEATURE_TERMINAL_STATE);
    pub const WINDOWING: Self = Self(ffi::DA_FEATURE_WINDOWING);
    pub const HORIZONTAL_SCROLLING: Self = Self(ffi::DA_FEATURE_HORIZONTAL_SCROLLING);
    pub const ANSI_COLOR: Self = Self(ffi::DA_FEATURE_ANSI_COLOR);
    pub const RECTANGULAR_EDITING: Self = Self(ffi::DA_FEATURE_RECTANGULAR_EDITING);
    pub const ANSI_TEXT_LOCATOR: Self = Self(ffi::DA_FEATURE_ANSI_TEXT_LOCATOR);
    pub const CLIPBOARD: Self = Self(ffi::DA_FEATURE_CLIPBOARD);
}

#[derive(Debug, Copy, Clone)]
pub struct SecondaryDeviceAttributes {
    pub device_type: DeviceType,
    pub firmware_version: u16,
    pub rom_cartridge: u16,
}

impl From<SecondaryDeviceAttributes> for ffi::DeviceAttributesSecondary {
    fn from(value: SecondaryDeviceAttributes) -> Self {
        Self {
            device_type: value.device_type.0,
            firmware_version: value.firmware_version,
            rom_cartridge: value.rom_cartridge,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceType(pub u16);

impl DeviceType {
    pub const VT100: Self = Self(ffi::DA_DEVICE_TYPE_VT100);
    pub const VT220: Self = Self(ffi::DA_DEVICE_TYPE_VT220);
    pub const VT240: Self = Self(ffi::DA_DEVICE_TYPE_VT240);
    pub const VT330: Self = Self(ffi::DA_DEVICE_TYPE_VT330);
    pub const VT340: Self = Self(ffi::DA_DEVICE_TYPE_VT340);
    pub const VT320: Self = Self(ffi::DA_DEVICE_TYPE_VT320);
    pub const VT382: Self = Self(ffi::DA_DEVICE_TYPE_VT382);
    pub const VT420: Self = Self(ffi::DA_DEVICE_TYPE_VT420);
    pub const VT510: Self = Self(ffi::DA_DEVICE_TYPE_VT510);
    pub const VT520: Self = Self(ffi::DA_DEVICE_TYPE_VT520);
    pub const VT525: Self = Self(ffi::DA_DEVICE_TYPE_VT525);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TertiaryDeviceAttributes {
    pub unit_id: u32,
}

impl From<TertiaryDeviceAttributes> for ffi::DeviceAttributesTertiary {
    fn from(value: TertiaryDeviceAttributes) -> Self {
        Self {
            unit_id: value.unit_id,
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct XtChecksumExtension: u8 {
        const NO_NEGATE = 1;
        const NO_ATTRIBUTES = 2;
        const NO_OMIT_BLANKS = 4;
        const UNWRITTEN_AS_SPACES = 8;
        const FULL_CODEPOINTS = 16;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum ColorScheme {
    Light = ffi::ColorScheme::LIGHT,
    Dark = ffi::ColorScheme::DARK,
}

impl ColorScheme {
    pub fn encode_report(self, buf: &mut [u8]) -> Result<usize> {
        let mut written = 0;
        let result = unsafe {
            ffi::ghostty_color_scheme_report_encode(
                self.into(),
                buf.as_mut_ptr().cast(),
                buf.len(),
                &raw mut written,
            )
        };
        from_result_with_len(result, written)
    }
}

impl From<ColorScheme> for ffi::ColorScheme::Type {
    fn from(value: ColorScheme) -> Self {
        match value {
            ColorScheme::Light => ffi::ColorScheme::LIGHT,
            ColorScheme::Dark => ffi::ColorScheme::DARK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum CompressionMode {
    Incremental = ffi::TerminalCompressionMode::INCREMENTAL,
    Full = ffi::TerminalCompressionMode::FULL,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum CompressionResult {
    Unsupported = ffi::TerminalCompressionResult::UNSUPPORTED,
    Pending = ffi::TerminalCompressionResult::PENDING,
    Complete = ffi::TerminalCompressionResult::COMPLETE,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompressionActivity(u64);

#[derive(Debug)]
pub struct ClipboardWrite<'t> {
    ptr: *const ffi::ClipboardWrite,
    _phan: PhantomData<&'t ()>,
}

impl<'t> ClipboardWrite<'t> {
    #[must_use]
    pub fn name(&self) -> &'t [u8] {
        unsafe {
            crate::sized_field!(self.ptr, ffi::ClipboardWrite, name).map_or(&[], |n| n.to_bytes())
        }
    }

    #[must_use]
    pub fn granted(&self) -> bool {
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardWrite, granted) }.unwrap_or(false)
    }

    #[must_use]
    pub fn can_remember(&self) -> bool {
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardWrite, can_remember) }.unwrap_or(false)
    }

    pub fn reply(self, result: std::result::Result<(), ClipboardWriteError>, remember: bool) {
        let reply = ffi::ClipboardWriteReply {
            result: result.map_or_else(Into::into, |()| ffi::ClipboardWriteResult::SUCCESS),
            remember,
            ..ffi::sized!(ffi::ClipboardWriteReply)
        };
        if let Some(callback) =
            unsafe { crate::sized_field!(self.ptr, ffi::ClipboardWrite, reply) }.flatten()
        {
            unsafe { callback(self.ptr, &raw const reply) };
        }
    }

    unsafe fn from_raw(ptr: *const ffi::ClipboardWrite) -> Self {
        Self {
            ptr,
            _phan: PhantomData,
        }
    }

    #[must_use]
    pub fn location(&self) -> ClipboardLocation {
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardWrite, location) }
            .and_then(|location| location.try_into().ok())
            .unwrap_or(ClipboardLocation::Standard)
    }
    #[must_use]
    pub fn contents(&self) -> ClipboardContents<'t> {
        let (ptr, len) = unsafe {
            (
                crate::sized_field!(self.ptr, ffi::ClipboardWrite, contents),
                crate::sized_field!(self.ptr, ffi::ClipboardWrite, contents_len),
            )
        };
        let contents: &'t [ffi::ClipboardContent] = match (ptr, len) {
            (Some(ptr), Some(len)) if !ptr.is_null() => {
                unsafe { std::slice::from_raw_parts(ptr, len) }
            }
            _ => &[],
        };
        ClipboardContents(contents.iter())
    }
}

#[derive(Clone, Debug)]
pub struct ClipboardContents<'t>(std::slice::Iter<'t, ffi::ClipboardContent>);

impl<'t> Iterator for ClipboardContents<'t> {
    type Item = ClipboardContent<'t>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|v| unsafe { ClipboardContent::from_raw(v) })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl DoubleEndedIterator for ClipboardContents<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|v| unsafe { ClipboardContent::from_raw(v) })
    }
}
impl ExactSizeIterator for ClipboardContents<'_> {}
impl std::iter::FusedIterator for ClipboardContents<'_> {}

#[derive(Clone, Copy, Debug)]
pub struct ClipboardContent<'t> {
    pub mime: &'t [u8],
    pub data: &'t [u8],
}
impl ClipboardContent<'_> {
    unsafe fn from_raw(value: &ffi::ClipboardContent) -> Self {
        unsafe {
            Self {
                mime: value.mime.to_bytes(),
                data: value.data.to_bytes(),
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum ClipboardLocation {
    Standard = ffi::ClipboardLocation::STANDARD,
    Selection = ffi::ClipboardLocation::SELECTION,
    Primary = ffi::ClipboardLocation::PRIMARY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum ClipboardWriteError {
    Denied = ffi::ClipboardWriteResult::DENIED,
    Unsupported = ffi::ClipboardWriteResult::UNSUPPORTED,
    Busy = ffi::ClipboardWriteResult::BUSY,
    InvalidData = ffi::ClipboardWriteResult::INVALID_DATA,
    IoError = ffi::ClipboardWriteResult::IO_ERROR,
}

#[derive(Debug)]
pub struct ClipboardRead<'t> {
    ptr: *const ffi::ClipboardRead,
    _phan: PhantomData<&'t ()>,
}

impl<'t> ClipboardRead<'t> {
    unsafe fn from_raw(ptr: *const ffi::ClipboardRead) -> Self {
        Self {
            ptr,
            _phan: PhantomData,
        }
    }

    #[must_use]
    pub fn location(&self) -> ClipboardLocation {
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, location) }
            .and_then(|location| location.try_into().ok())
            .unwrap_or(ClipboardLocation::Standard)
    }

    #[must_use]
    pub fn mimes(&self) -> impl ExactSizeIterator<Item = &'t [u8]> + use<'t> {
        let (ptr, len) = unsafe {
            (
                crate::sized_field!(self.ptr, ffi::ClipboardRead, mimes),
                crate::sized_field!(self.ptr, ffi::ClipboardRead, mimes_len),
            )
        };
        let mimes: &'t [ffi::String] = match (ptr, len) {
            (Some(ptr), Some(len)) if !ptr.is_null() && len > 0 => {
                unsafe { std::slice::from_raw_parts(ptr, len) }
            }
            _ => &[],
        };
        mimes.iter().map(|mime| unsafe { mime.to_bytes() })
    }

    #[must_use]
    pub fn list(&self) -> bool {
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, list) }.unwrap_or(false)
    }

    #[must_use]
    pub fn name(&self) -> &'t [u8] {
        unsafe {
            crate::sized_field!(self.ptr, ffi::ClipboardRead, name).map_or(&[], |n| n.to_bytes())
        }
    }

    #[must_use]
    pub fn granted(&self) -> bool {
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, granted) }.unwrap_or(false)
    }

    #[must_use]
    pub fn can_remember(&self) -> bool {
        unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, can_remember) }.unwrap_or(false)
    }

    pub fn reply(
        self,
        result: std::result::Result<&[ClipboardReplyContent<'_>], ClipboardReadError>,
        available: &[ClipboardMime<'_>],
        remember: bool,
    ) {
        let (result, contents) = match result {
            Ok(contents) => (ffi::ClipboardReadResult::SUCCESS, contents),
            Err(error) => (error.into(), &[][..]),
        };
        let reply = ffi::ClipboardReadReply {
            result,
            contents: contents.as_ptr().cast(),
            contents_len: contents.len(),
            available: available.as_ptr().cast(),
            available_len: available.len(),
            remember,
            ..ffi::sized!(ffi::ClipboardReadReply)
        };
        if let Some(callback) =
            unsafe { crate::sized_field!(self.ptr, ffi::ClipboardRead, reply) }.flatten()
        {
            unsafe { callback(self.ptr, &raw const reply) };
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ClipboardReplyContent<'a> {
    raw: ffi::ClipboardContent,
    _phan: PhantomData<&'a [u8]>,
}

impl<'a> ClipboardReplyContent<'a> {
    #[must_use]
    pub const fn new(mime: &'a str, data: &'a [u8]) -> Self {
        Self {
            raw: ffi::ClipboardContent {
                mime: ffi::String {
                    ptr: mime.as_ptr(),
                    len: mime.len(),
                },
                data: ffi::String {
                    ptr: data.as_ptr(),
                    len: data.len(),
                },
            },
            _phan: PhantomData,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct ClipboardMime<'a> {
    raw: ffi::String,
    _phan: PhantomData<&'a str>,
}

impl<'a> ClipboardMime<'a> {
    #[must_use]
    pub const fn new(mime: &'a str) -> Self {
        Self {
            raw: ffi::String {
                ptr: mime.as_ptr(),
                len: mime.len(),
            },
            _phan: PhantomData,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum ClipboardReadError {
    Denied = ffi::ClipboardReadResult::DENIED,
    Unsupported = ffi::ClipboardReadResult::UNSUPPORTED,
    Busy = ffi::ClipboardReadResult::BUSY,
    IoError = ffi::ClipboardReadResult::IO_ERROR,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnknownSequence<'t> {
    #[non_exhaustive]
    Apc {
        content: &'t [u8],
        truncated: bool,
    },
    #[non_exhaustive]
    Osc {
        content: &'t [u8],
        truncated: bool,
        terminator: osc::Terminator,
    },
}

impl UnknownSequence<'_> {
    unsafe fn from_raw<'t>(
        raw: *const ffi::TerminalUnknownSequence,
    ) -> Option<UnknownSequence<'t>> {
        match unsafe { (*raw).tag } {
            ffi::TerminalUnknownSequenceTag::APC => {
                let apc = unsafe { (*raw).value.apc };
                Some(UnknownSequence::Apc {
                    content: unsafe { apc.content.to_bytes() },
                    truncated: apc.truncated,
                })
            }
            ffi::TerminalUnknownSequenceTag::OSC => {
                let osc = unsafe { (*raw).value.osc };
                let terminator = osc.terminator.try_into().ok()?;
                Some(UnknownSequence::Osc {
                    content: unsafe { osc.content.to_bytes() },
                    truncated: osc.truncated,
                    terminator,
                })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DesktopNotification<'t> {
    ptr: *const ffi::TerminalDesktopNotification,
    _phan: PhantomData<&'t ()>,
}

impl<'t> DesktopNotification<'t> {
    unsafe fn from_raw(raw: *const ffi::TerminalDesktopNotification) -> Self {
        Self {
            ptr: raw,
            _phan: PhantomData,
        }
    }

    #[must_use]
    pub fn title(self) -> &'t [u8] {
        unsafe {
            crate::sized_field!(self.ptr, ffi::TerminalDesktopNotification, title)
                .map_or(&[], |title| title.to_bytes())
        }
    }

    #[must_use]
    pub fn body(self) -> &'t [u8] {
        unsafe {
            crate::sized_field!(self.ptr, ffi::TerminalDesktopNotification, body)
                .map_or(&[], |body| body.to_bytes())
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ProgressReport<'t> {
    ptr: *const ffi::TerminalProgressReport,
    _phan: PhantomData<&'t ()>,
}

impl ProgressReport<'_> {
    unsafe fn from_raw(raw: *const ffi::TerminalProgressReport) -> Self {
        Self {
            ptr: raw,
            _phan: PhantomData,
        }
    }

    pub fn state(self) -> Result<ProgressState> {
        unsafe { *self.ptr }
            .state
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }

    #[must_use]
    pub fn progress(self) -> Option<u8> {
        u8::try_from(unsafe { *self.ptr }.progress).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum ProgressState {
    Remove = ffi::TerminalProgressState::REMOVE,
    Set = ffi::TerminalProgressState::SET,
    Error = ffi::TerminalProgressState::ERROR,
    Indeterminate = ffi::TerminalProgressState::INDETERMINATE,
    Pause = ffi::TerminalProgressState::PAUSE,
}

#[derive(Debug, Copy, Clone)]
pub struct ProgramStatus<'t> {
    ptr: *const ffi::TerminalProgramStatus,
    _phan: PhantomData<&'t ()>,
}

impl<'t> ProgramStatus<'t> {
    unsafe fn from_raw(raw: *const ffi::TerminalProgramStatus) -> Self {
        Self {
            ptr: raw,
            _phan: PhantomData,
        }
    }

    fn raw(self) -> &'t ffi::TerminalProgramStatus {
        unsafe { &*self.ptr }
    }

    pub fn state(self) -> Result<ProgramStatusState> {
        self.raw().state.try_into().map_err(|_| Error::InvalidValue)
    }

    pub fn kind(self) -> Result<ProgramStatusKind> {
        self.raw().kind.try_into().map_err(|_| Error::InvalidValue)
    }

    #[must_use]
    pub fn progress(self) -> Option<u8> {
        u8::try_from(self.raw().progress).ok()
    }

    #[must_use]
    pub fn id(self) -> &'t [u8] {
        unsafe { self.raw().id.to_bytes() }
    }

    #[must_use]
    pub fn app(self) -> &'t [u8] {
        unsafe { self.raw().app.to_bytes() }
    }

    #[must_use]
    pub fn title(self) -> &'t [u8] {
        unsafe { self.raw().title.to_bytes() }
    }

    #[must_use]
    pub fn message(self) -> &'t [u8] {
        unsafe { self.raw().message.to_bytes() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum ProgramStatusState {
    Idle = ffi::ProgramStatusState::IDLE,
    Working = ffi::ProgramStatusState::WORKING,
    Done = ffi::ProgramStatusState::DONE,
    Blocked = ffi::ProgramStatusState::BLOCKED,
    Error = ffi::ProgramStatusState::ERROR,
    Clear = ffi::ProgramStatusState::CLEAR,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum ProgramStatusKind {
    None = ffi::ProgramStatusKind::NONE,
    Permission = ffi::ProgramStatusKind::PERMISSION,
    Question = ffi::ProgramStatusKind::QUESTION,
    Auth = ffi::ProgramStatusKind::AUTH,
}

#[derive(Debug, Copy, Clone)]
pub struct SemanticPrompt<'t> {
    ptr: *const ffi::TerminalSemanticPrompt,
    _phan: PhantomData<&'t ()>,
}

impl<'t> SemanticPrompt<'t> {
    unsafe fn from_raw(raw: *const ffi::TerminalSemanticPrompt) -> Self {
        Self {
            ptr: raw,
            _phan: PhantomData,
        }
    }

    fn raw(self) -> &'t ffi::TerminalSemanticPrompt {
        unsafe { &*self.ptr }
    }

    pub fn kind(self) -> Result<SemanticPromptKind> {
        self.raw().kind.try_into().map_err(|_| Error::InvalidValue)
    }

    pub fn prompt_kind(self) -> Result<PromptKind> {
        self.raw()
            .prompt_kind
            .try_into()
            .map_err(|_| Error::InvalidValue)
    }

    #[must_use]
    pub fn exit_code(self) -> Option<i32> {
        let raw = self.raw();
        raw.has_exit_code.then_some(raw.exit_code)
    }

    #[must_use]
    pub fn command(self) -> &'t [u8] {
        unsafe { self.raw().command.to_bytes() }
    }

    #[must_use]
    pub fn error(self) -> &'t [u8] {
        unsafe { self.raw().error.to_bytes() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum SemanticPromptKind {
    PromptStart = ffi::SemanticPromptKind::PROMPT_START,
    InputStart = ffi::SemanticPromptKind::INPUT_START,
    OutputStart = ffi::SemanticPromptKind::OUTPUT_START,
    CommandEnd = ffi::SemanticPromptKind::COMMAND_END,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum PromptKind {
    Primary = ffi::SemanticPromptPromptKind::PRIMARY,
    Right = ffi::SemanticPromptPromptKind::RIGHT,
    Continuation = ffi::SemanticPromptPromptKind::CONTINUATION,
    Secondary = ffi::SemanticPromptPromptKind::SECONDARY,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResizeWindow {
    Grid {
        rows: Option<NonZeroU16>,
        columns: Option<NonZeroU16>,
    },
}

impl ResizeWindow {
    unsafe fn from_raw(raw: *const ffi::TerminalResizeWindow) -> Option<Self> {
        match unsafe { (*raw).tag } {
            ffi::TerminalResizeWindowTag::GRID => {
                let ffi::TerminalResizeWindowGrid { rows, columns } = unsafe { (*raw).value.grid };
                Some(Self::Grid {
                    rows: NonZeroU16::new(rows),
                    columns: NonZeroU16::new(columns),
                })
            }
            _ => None,
        }
    }
}


macro_rules! handlers {
    {
        $(
            $(#[$fmeta:meta])*
            $vis:vis fn $name:ident(
                &mut self,
                tag = $tag:ident,
                from = $rawfnty:ident( $($rfname:ident: $rfty:ty),*$(,)? ) $(-> $rawrty:ty)?,
                $(#[$tmeta:meta])*
                to = $(<$lf:lifetime>)? $fnty:ident( $($fty:ty),*$(,)? ) $(-> $rty:ty)?,
            ) |$t:ident, $func:ident| $block:block
        )*
    } => {
        impl<'alloc, 'cb> $crate::terminal::Terminal<'alloc, 'cb> {$(
            $(#[$fmeta])*
            $vis fn $name(&mut self, f: impl $fnty<'alloc, 'cb>) -> $crate::error::Result<&mut Self> {
                unsafe extern "C" fn callback(
                    t: $crate::ffi::Terminal,
                    ud: *mut std::ffi::c_void,
                    $($rfname: $rfty),*
                ) $(-> $rawrty)? {
                    let vtable = unsafe { &mut *ud.cast::<VTable<'_, '_>>() };

                    let obj = $crate::alloc::Object::new(t).expect("received null terminal ptr in callback - this is a bug!");
                    let term = ::core::mem::ManuallyDrop::new($crate::terminal::Terminal::<'_, '_> {
                        inner: obj,
                        vtable: ud.cast(),
                        id: 0,
                    });
                    let $t: &$crate::terminal::Terminal = &term;
                    let $func = vtable.$name.as_deref_mut()
                        .expect("no handler set but callback is still called - this is a bug!");
                    $block
                }

                unsafe { (*self.vtable).$name = Some(::std::boxed::Box::new(f)); }

                let userdata = self.vtable.cast::<::std::ffi::c_void>().cast_const();
                self.set_ptr($crate::ffi::TerminalOption::USERDATA, userdata)?;

                let _: $crate::ffi::$rawfnty = Some(callback);

                let callback_ptr: unsafe extern "C" fn(
                    $crate::ffi::Terminal,
                    *mut ::std::ffi::c_void,
                    $($rfty),*
                ) $(-> $rawrty)? = callback;

                let result = unsafe {
                    $crate::ffi::ghostty_terminal_set(
                        self.inner.as_raw(),
                        $crate::ffi::TerminalOption::$tag,
                        callback_ptr as *const ::std::ffi::c_void
                    )
                };
                $crate::error::from_result(result)?;
                Ok(self)
            }
        )*}
        $(
            $(#[$tmeta])*
            pub trait $fnty<'alloc, 'cb>:
                $(for<$lf>)? FnMut(
                    &$($lf)? $crate::terminal::Terminal<'alloc, 'cb>,
                    $($fty),*
                ) $(-> $rty)? + 'cb {}

            impl<'alloc, 'cb, F> $fnty<'alloc, 'cb> for F
            where
                F: $(for<$lf>)? FnMut(
                    &$($lf)? $crate::terminal::Terminal<'alloc, 'cb>,
                    $($fty),*
                ) $(-> $rty)? + 'cb
            {}
        )*

        struct VTable<'alloc, 'cb> {
            $($name: Option<::std::boxed::Box<dyn $fnty<'alloc, 'cb>>>),*
        }

        impl ::core::fmt::Debug for VTable<'_, '_> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                f.write_str("VTable {..}")
            }
        }

        impl ::core::default::Default for VTable<'_, '_> {
            fn default() -> Self {
                Self {
                    $($name: None),*
                }
            }
        }
    };
}

handlers! {
    pub fn on_pty_write(
        &mut self,
        tag = WRITE_PTY,
        from = TerminalWritePtyFn(ptr: *const u8, len: usize),
        to = <'t>PtyWriteFn(&'t [u8]),
    ) |term, func| {
        let data = unsafe { std::slice::from_raw_parts(ptr, len) };
        func(term, data);
    }

    pub fn on_bell(
        &mut self,
        tag = BELL,
        from = TerminalBellFn(),
        to = BellFn(),
    ) |term, func| {
        func(term);
    }

    pub fn on_render_hold(
        &mut self,
        tag = RENDER_HOLD,
        from = TerminalRenderHoldFn(held: bool),
        to = RenderHoldFn(bool),
    ) |term, func| {
        func(term, held);
    }

    pub fn on_enquiry(
        &mut self,
        tag = ENQUIRY,
        from = TerminalEnquiryFn() -> ffi::String,
        to = <'t>EnquiryFn() -> Option<&'t str>,
    ) |term, func| {
        func(term).unwrap_or("").into()
    }

    pub fn on_xtversion(
        &mut self,
        tag = XTVERSION,
        from = TerminalXtversionFn() -> ffi::String,
        to = <'t>XtversionFn() -> Option<&'t str>,
    ) |term, func| {
        func(term).unwrap_or("").into()
    }

    pub fn on_title_changed(
        &mut self,
        tag = TITLE_CHANGED,
        from = TerminalTitleChangedFn(),
        to = TitleChangedFn(),
    ) |term, func| {
        func(term);
    }

    pub fn on_pwd_changed(
        &mut self,
        tag = PWD_CHANGED,
        from = TerminalPwdChangedFn(),
        to = PwdChangedFn(),
    ) |term, func| {
        func(term);
    }

    pub fn on_size(
        &mut self,
        tag = SIZE,
        from = TerminalSizeFn(out: *mut ffi::SizeReportSize) -> bool,
        to = SizeFn() -> Option<SizeReportSize>,
    ) |term, func| {
        if let Some(size) = func(term) {
            unsafe { *out = size };
            true
        } else {
            false
        }
    }

    pub fn on_color_scheme(
        &mut self,
        tag = COLOR_SCHEME,
        from = TerminalColorSchemeFn(out: *mut ffi::ColorScheme::Type) -> bool,
        to = ColorSchemeFn() -> Option<ColorScheme>,
    ) |term, func| {
        if let Some(size) = func(term) {
            unsafe { *out = size.into() };
            true
        } else {
            false
        }
    }

    pub fn on_device_attributes(
        &mut self,
        tag = DEVICE_ATTRIBUTES,
        from = TerminalDeviceAttributesFn(out: *mut ffi::DeviceAttributes) -> bool,
        to = DeviceAttributesFn() -> Option<DeviceAttributes>,
    ) |term, func| {
        if let Some(size) = func(term) {
            unsafe { *out = size.into() };
            true
        } else {
            false
        }
    }

    pub fn on_clipboard_write(
        &mut self,
        tag = CLIPBOARD_WRITE,
        from = TerminalClipboardWriteFn(
            write: *const ffi::ClipboardWrite
        ),
        to = <'t>ClipboardWriteFn(ClipboardWrite<'t>),
    ) |term, func| {
        func(term, unsafe { ClipboardWrite::from_raw(write) });
    }

    pub fn on_clipboard_read(
        &mut self,
        tag = CLIPBOARD_READ,
        from = TerminalClipboardReadFn(read: *const ffi::ClipboardRead),
        to = <'t>ClipboardReadFn(ClipboardRead<'t>),
    ) |term, func| {
        func(term, unsafe { ClipboardRead::from_raw(read) });
    }

    pub fn on_desktop_notification(
        &mut self,
        tag = DESKTOP_NOTIFICATION,
        from = TerminalDesktopNotificationFn(
            notif: *const ffi::TerminalDesktopNotification
        ),
        to = <'t>DesktopNotificationFn(DesktopNotification<'t>),
    ) |term, func| {
        func(term, unsafe { DesktopNotification::from_raw(notif) });
    }

    pub fn on_progress_report(
        &mut self,
        tag = PROGRESS_REPORT,
        from = TerminalProgressReportFn(
            progress: *const ffi::TerminalProgressReport
        ),
        to = <'t>ProgressReportFn(ProgressReport<'t>),
    ) |term, func| {
        func(term, unsafe { ProgressReport::from_raw(progress) });
    }

    pub fn on_program_status(
        &mut self,
        tag = PROGRAM_STATUS,
        from = TerminalProgramStatusFn(
            report: *const ffi::TerminalProgramStatus
        ),
        to = <'t>ProgramStatusFn(ProgramStatus<'t>),
    ) |term, func| {
        func(term, unsafe { ProgramStatus::from_raw(report) });
    }

    pub fn on_semantic_prompt(
        &mut self,
        tag = SEMANTIC_PROMPT,
        from = TerminalSemanticPromptFn(
            event: *const ffi::TerminalSemanticPrompt
        ),
        to = <'t>SemanticPromptFn(SemanticPrompt<'t>),
    ) |term, func| {
        func(term, unsafe { SemanticPrompt::from_raw(event) });
    }

    pub fn on_resize_window(
        &mut self,
        tag = RESIZE_WINDOW,
        from = TerminalResizeWindowFn(request: *const ffi::TerminalResizeWindow),
        to = ResizeWindowFn(ResizeWindow),
    ) |term, func| {
        let request = unsafe { ResizeWindow::from_raw(request) };
        if let Some(request) = request {
            func(term, request);
        }
    }

    pub fn on_unknown_sequence(
        &mut self,
        tag = UNKNOWN_SEQUENCE,
        from = TerminalUnknownSequenceFn(sequence: *const ffi::TerminalUnknownSequence),
        to = <'t>UnknownSequenceFn(UnknownSequence<'t>),
    ) |term, func| {
        if let Some(sequence) = unsafe { UnknownSequence::from_raw(sequence) } {
            func(term, sequence);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderState;
    use crate::render::CursorVisualStyle;
    use std::cell::{Cell, RefCell};
    use std::mem::ManuallyDrop;

    #[derive(Debug, Default, PartialEq, Eq)]
    struct SeenRead {
        location: Option<ClipboardLocation>,
        mimes: Vec<Vec<u8>>,
        list: bool,
        name: Vec<u8>,
        granted: bool,
        can_remember: bool,
    }

    fn clipboard_read(
        input: &[u8],
        answer: impl Fn(ClipboardRead<'_>),
    ) -> (Vec<SeenRead>, Vec<u8>) {
        let seen = RefCell::new(Vec::new());
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal
            .on_clipboard_read(|_term, request| {
                seen.borrow_mut().push(SeenRead {
                    location: Some(request.location()),
                    mimes: request.mimes().map(<[u8]>::to_vec).collect(),
                    list: request.list(),
                    name: request.name().to_vec(),
                    granted: request.granted(),
                    can_remember: request.can_remember(),
                });
                answer(request);
            })
            .expect("callback should register");
        terminal.vt_write(input);
        drop(terminal);
        (seen.into_inner(), output.into_inner())
    }

    #[test]
    fn scroll_viewport_delta_prompt() {
        let mut terminal = Terminal::new(80, 2).expect("terminal should initialize");
        terminal.vt_write(
            b"\x1b]133;P;k=i\x07p1\r\n\
              \x1b]133;B\x07cmd1\r\n\
              \x1b]133;C\x07out1\r\n\
              \x1b]133;P;k=i\x07p2\r\n\
              \x1b]133;B\x07cmd2\r\n\
              \x1b]133;C\x07out2\r\n\
              \x1b]133;P;k=i\x07p3\r\n\
              \x1b]133;B\x07cmd3\r\n\
              \x1b]133;C\x07out3\r\n\
              \x1b]133;P;k=i\x07p4",
        );
        let mut scroll = |scroll| {
            terminal.scroll_viewport(scroll);
            terminal.scrollbar().expect("scrollbar should exist").offset
        };

        assert_eq!(scroll(ScrollViewport::DeltaPrompt(-1)), 6);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(-1)), 3);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(1)), 6);

        assert_eq!(scroll(ScrollViewport::DeltaPrompt(isize::MIN)), 0);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(isize::MAX)), 8);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(0)), 8);

        assert_eq!(scroll(ScrollViewport::DeltaPrompt(-1)), 6);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(0)), 6);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(-100)), 0);
        assert_eq!(scroll(ScrollViewport::DeltaPrompt(100)), 8);
    }

    #[test]
    fn osc52_clipboard_read() {
        let (seen, output) = clipboard_read(b"\x1b]52;c;?\x1b\\", |request| {
            request.reply(
                Ok(&[ClipboardReplyContent::new("text/plain", b"hello")]),
                &[],
                false,
            );
        });
        assert_eq!(
            seen,
            [SeenRead {
                location: Some(ClipboardLocation::Standard),
                mimes: vec![b"text/plain".to_vec()],
                ..SeenRead::default()
            }]
        );
        assert_eq!(output, b"\x1b]52;c;aGVsbG8=\x1b\\");

        let (_, output) = clipboard_read(b"\x1b]52;c;?\x1b\\", |request| {
            request.reply(Err(ClipboardReadError::Denied), &[], false);
        });
        assert_eq!(output, b"\x1b]52;c;\x1b\\");
        let (_, output) = clipboard_read(b"\x1b]52;c;?\x1b\\", |_request| {});
        assert_eq!(output, b"\x1b]52;c;\x1b\\");
    }

    #[test]
    fn osc5522_clipboard_read() {
        let (seen, output) = clipboard_read(
            b"\x1b]5522;type=read:id=r1:name=YXBw;dGV4dC9wbGFpbiAu\x1b\\",
            |request| {
                request.reply(
                    Ok(&[
                        ClipboardReplyContent::new("text/plain", b"hello"),
                        ClipboardReplyContent::new("image/png", b"png"),
                    ]),
                    &[
                        ClipboardMime::new("text/plain"),
                        ClipboardMime::new("image/png"),
                    ],
                    false,
                );
            },
        );
        assert_eq!(
            seen,
            [SeenRead {
                location: Some(ClipboardLocation::Standard),
                mimes: vec![b"text/plain".to_vec()],
                list: true,
                name: b"app".to_vec(),
                ..SeenRead::default()
            }]
        );
        assert_eq!(
            output,
            concat!(
                "\x1b]5522;type=read:status=OK:id=r1\x1b\\",
                "\x1b]5522;type=read:status=DATA:id=r1:mime=Lg==;dGV4dC9wbGFpbiBpbWFnZS9wbmcK\x1b\\",
                "\x1b]5522;type=read:status=DATA:id=r1:mime=dGV4dC9wbGFpbg==;aGVsbG8=\x1b\\",
                "\x1b]5522;type=read:status=DONE:id=r1\x1b\\",
            )
            .as_bytes()
        );

        let (_, output) = clipboard_read(
            b"\x1b]5522;type=read:id=r2;dGV4dC9wbGFpbg==\x1b\\",
            |request| request.reply(Err(ClipboardReadError::Busy), &[], false),
        );
        assert_eq!(output, b"\x1b]5522;type=read:status=EBUSY:id=r2\x1b\\");
        let (_, output) = clipboard_read(
            b"\x1b]5522;type=read:id=r3;dGV4dC9wbGFpbg==\x1b\\",
            |_request| {},
        );
        assert_eq!(output, b"\x1b]5522;type=read:status=EPERM:id=r3\x1b\\");
    }

    #[test]
    fn osc5522_clipboard_read_remembers_password_grants() {
        let read = "\x1b]5522;type=read:id=a:name=YXBw:pw=c2VjcmV0;dGV4dC9wbGFpbg==\x1b\\";
        let (seen, _) = clipboard_read(read.repeat(2).as_bytes(), |request| {
            request.reply(
                Ok(&[ClipboardReplyContent::new("text/plain", b"hi")]),
                &[],
                true,
            );
        });
        let grants: Vec<_> = seen.iter().map(|s| (s.can_remember, s.granted)).collect();
        assert_eq!(grants, [(true, false), (true, true)]);
    }

    fn pty_output(setup: impl FnOnce(&mut Terminal<'_, '_>), input: &[u8]) -> Vec<u8> {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        setup(&mut terminal);
        terminal.vt_write(input);
        drop(terminal);
        output.into_inner()
    }

    #[test]
    fn vt_write_until_ground_stops_at_ground() {
        let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
        assert!(terminal.is_vt_ground().unwrap());
        assert_eq!(terminal.vt_write_until_ground(b"hello").unwrap(), Some(0));
        assert_eq!(terminal.cursor_x().unwrap(), 0);

        terminal.vt_write(b"\x1b[");
        assert!(!terminal.is_vt_ground().unwrap());
        assert_eq!(terminal.vt_write_until_ground(b"31").unwrap(), None);
        assert_eq!(terminal.vt_write_until_ground(b"mhello").unwrap(), Some(1));
        assert!(terminal.is_vt_ground().unwrap());
        assert_eq!(terminal.cursor_x().unwrap(), 0);

        terminal.vt_write(b"\xe2");
        assert!(!terminal.is_vt_ground().unwrap());
        assert_eq!(
            terminal.vt_write_until_ground(b"\x82\xac!").unwrap(),
            Some(2)
        );
        assert_eq!(terminal.cursor_x().unwrap(), 1);
    }

    #[test]
    fn cursor_at_prompt_follows_semantic_prompts() {
        let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
        assert!(!terminal.is_cursor_at_prompt().unwrap());
        terminal.vt_write(b"\x1b]133;A\x1b\\");
        assert!(terminal.is_cursor_at_prompt().unwrap());
        terminal.vt_write(b"\x1b[?1049h");
        assert!(!terminal.is_cursor_at_prompt().unwrap());
    }

    #[test]
    fn terminfo_name_answers_xtgettcap() {
        let query = b"\x1bP+q544e\x1b\\";
        assert_eq!(pty_output(|_| {}, query), b"");
        assert_eq!(
            pty_output(
                |terminal| {
                    terminal.set_terminfo_name("xterm-256color").unwrap();
                },
                query
            ),
            b"\x1bP1+r544E=787465726D2D323536636F6C6F72\x1b\\"
        );
        assert_eq!(
            pty_output(
                |terminal| {
                    terminal.set_terminfo_name("xterm-256color").unwrap();
                    terminal.set_terminfo_name("").unwrap();
                },
                query
            ),
            b""
        );

        let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
        assert!(matches!(
            terminal.set_terminfo_name(&"x".repeat(129)),
            Err(Error::InvalidValue)
        ));
    }

    #[test]
    fn clipboard_write_max_bytes_bounds_kitty_writes() {
        let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
        let default = terminal.clipboard_write_max_bytes().unwrap();
        assert_eq!(default, 64 * 1024 * 1024);
        terminal.set_clipboard_write_max_bytes(Some(4)).unwrap();
        assert_eq!(terminal.clipboard_write_max_bytes().unwrap(), 4);
        terminal.set_clipboard_write_max_bytes(None).unwrap();
        assert_eq!(terminal.clipboard_write_max_bytes().unwrap(), default);

        let write = concat!(
            "\x1b]5522;type=write:id=c1\x1b\\",
            "\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;SGVsbA==\x1b\\",
            "\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;bw==\x1b\\",
            "\x1b]5522;type=wdata\x1b\\",
        );
        let run = |limit| {
            let written = RefCell::new(Vec::new());
            let output = RefCell::new(Vec::new());
            let mut terminal = Terminal::new(8, 2).expect("terminal should initialize");
            terminal
                .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
                .unwrap()
                .on_clipboard_write(|_term, request| {
                    written
                        .borrow_mut()
                        .extend(request.contents().map(|c| c.data.to_vec()));
                    request.reply(Ok(()), false);
                })
                .unwrap()
                .set_clipboard_write_max_bytes(Some(limit))
                .unwrap();
            terminal.vt_write(write.as_bytes());
            drop(terminal);
            (output.into_inner(), written.into_inner())
        };

        let (output, written) = run(4);
        assert_eq!(output, b"\x1b]5522;type=write:status=EFBIG:id=c1\x1b\\");
        assert!(written.is_empty());

        let (output, written) = run(5);
        assert_eq!(output, b"\x1b]5522;type=write:status=DONE:id=c1\x1b\\");
        assert_eq!(written, [b"Hello".to_vec()]);
    }

    #[test]
    fn unknown_sequences_are_reported() {
        use osc::Terminator::{Bel, St};

        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal
            .on_unknown_sequence(|_term, sequence| {
                seen.borrow_mut().push(match sequence {
                    UnknownSequence::Apc {
                        content, truncated, ..
                    } => (content.to_vec(), truncated, None),
                    UnknownSequence::Osc {
                        content,
                        truncated,
                        terminator,
                        ..
                    } => (content.to_vec(), truncated, Some(terminator)),
                });
            })
            .unwrap();
        terminal.vt_write(b"\x1b_unknown\x1b\\");
        terminal.set_unknown_max_bytes(4).unwrap();
        terminal.vt_write(b"\x1b_unknown\x1b\\");
        terminal.vt_write(b"\x1b]7400;status=busy\x07");
        terminal.set_unknown_max_bytes(64).unwrap();
        terminal.vt_write(b"\x1b_unknown\x1b\\");
        terminal.vt_write(b"\x1b]7400;status=busy\x1b\\");
        terminal.vt_write(b"\x1b]7501;state=working\x1b\\");
        terminal.vt_write(b"\x1b]2;title\x07");
        drop(terminal);
        assert_eq!(
            seen.into_inner(),
            [
                (b"unkn".to_vec(), true, None),
                (b"7400".to_vec(), true, Some(Bel)),
                (b"unknown".to_vec(), false, None),
                (b"7400;status=busy".to_vec(), false, Some(St)),
            ]
        );
    }

    #[test]
    fn osc5522_clipboard_read_targets_only() {
        let remember = "\x1b]5522;type=read:id=a:name=YXBw:pw=c2VjcmV0;dGV4dC9wbGFpbg==\x1b\\";
        let list = "\x1b]5522;type=read:id=l:name=YXBw:pw=c2VjcmV0;Lg==\x1b\\";
        let input = [remember, list, remember].concat();
        let (seen, output) = clipboard_read(input.as_bytes(), |request| {
            request.reply(Ok(&[]), &[], true);
        });
        let observed: Vec<_> = seen
            .iter()
            .map(|s| (s.mimes.len(), s.list, s.granted))
            .collect();
        assert_eq!(
            observed,
            [(1, false, false), (0, true, false), (1, false, true)]
        );

        let listing = concat!(
            "\x1b]5522;type=read:status=OK:id=l\x1b\\",
            "\x1b]5522;type=read:status=DATA:id=l:mime=Lg==\x1b\\",
            "\x1b]5522;type=read:status=DONE:id=l\x1b\\",
        );
        let output = String::from_utf8(output).expect("responses are ASCII");
        assert!(output.contains(listing), "{output:?}");
    }

    #[test]
    fn clipboard_read_empty_reply_contents() {
        let (_, output) = clipboard_read(
            b"\x1b]5522;type=read:id=e;dGV4dC9wbGFpbg==\x1b\\",
            |request| {
                request.reply(
                    Ok(&[
                        ClipboardReplyContent::new("", b""),
                        ClipboardReplyContent::new("text/plain", b""),
                    ]),
                    &[ClipboardMime::new("")],
                    false,
                );
            },
        );
        assert_eq!(
            output,
            concat!(
                "\x1b]5522;type=read:status=OK:id=e\x1b\\",
                "\x1b]5522;type=read:status=DONE:id=e\x1b\\",
            )
            .as_bytes()
        );

        let (_, output) = clipboard_read(b"\x1b]52;c;?\x1b\\", |request| {
            request.reply(Ok(&[ClipboardReplyContent::new("", b"")]), &[], false);
        });
        assert_eq!(output, b"\x1b]52;c;\x1b\\");
    }

    #[test]
    fn osc5522_clipboard_read_errors() {
        for (error, status) in [
            (ClipboardReadError::Denied, "EPERM"),
            (ClipboardReadError::Unsupported, "ENOSYS"),
            (ClipboardReadError::Busy, "EBUSY"),
            (ClipboardReadError::IoError, "EIO"),
        ] {
            let (_, output) = clipboard_read(
                b"\x1b]5522;type=read:id=x;dGV4dC9wbGFpbg==\x1b\\",
                |request| request.reply(Err(error), &[], false),
            );
            let expected = format!("\x1b]5522;type=read:status={status}:id=x\x1b\\");
            assert_eq!(output, expected.as_bytes(), "{error:?}");
        }
    }

    #[test]
    fn clipboard_read_primary_location() {
        let reply = |request: ClipboardRead<'_>| {
            request.reply(
                Ok(&[ClipboardReplyContent::new("text/plain", b"hello")]),
                &[],
                false,
            );
        };

        let (seen, output) = clipboard_read(b"\x1b]52;p;?\x1b\\", reply);
        assert_eq!(seen[0].location, Some(ClipboardLocation::Primary));
        assert_eq!(output, b"\x1b]52;p;aGVsbG8=\x1b\\");

        let (seen, output) = clipboard_read(
            b"\x1b]5522;type=read:loc=primary:id=p;dGV4dC9wbGFpbg==\x1b\\",
            reply,
        );
        assert_eq!(seen[0].location, Some(ClipboardLocation::Primary));
        assert_eq!(
            output,
            concat!(
                "\x1b]5522;type=read:status=OK:loc=primary:id=p\x1b\\",
                "\x1b]5522;type=read:status=DATA:id=p:mime=dGV4dC9wbGFpbg==;aGVsbG8=\x1b\\",
                "\x1b]5522;type=read:status=DONE:id=p\x1b\\",
            )
            .as_bytes()
        );
    }

    #[test]
    fn resize_pull_scrollback_controls_growing_rows() {
        fn cursor_row_after_growing(
            configure: impl FnOnce(&mut Terminal<'static, 'static>),
        ) -> u16 {
            let mut terminal = Terminal::new(10, 5).expect("terminal should initialize");
            configure(&mut terminal);
            terminal.vt_write(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8");
            assert_eq!(terminal.cursor_y().unwrap(), 4);
            terminal
                .resize(10, 8, 8, 16)
                .expect("resize should succeed");
            terminal.cursor_y().unwrap()
        }
        fn set(terminal: &mut Terminal<'static, 'static>, pull: Option<bool>) {
            terminal
                .set_resize_pull_scrollback(pull)
                .expect("option should be settable");
        }

        assert_eq!(cursor_row_after_growing(|_| {}), 7);
        assert_eq!(cursor_row_after_growing(|t| set(t, Some(true))), 7);
        assert_eq!(cursor_row_after_growing(|t| set(t, Some(false))), 4);
        assert_eq!(
            cursor_row_after_growing(|t| {
                set(t, Some(false));
                set(t, None);
            }),
            7
        );
        assert_eq!(
            cursor_row_after_growing(|t| {
                set(t, Some(false));
                t.vt_write(b"\x1bc");
            }),
            4
        );
        assert_eq!(
            cursor_row_after_growing(|t| {
                set(t, Some(false));
                t.reset();
            }),
            4
        );
    }

    #[test]
    fn render_hold_reports_start_and_end_in_pairs() {
        let events = RefCell::new(Vec::new());
        let take = || std::mem::take(&mut *events.borrow_mut());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_render_hold(|_term, held| events.borrow_mut().push(held))
            .expect("callback should register");

        terminal.vt_write(b"\x1b[?2026h\x1b[?2026hA\x1b[?2026l\x1b[?2026l");
        assert_eq!(take(), [true, false]);

        terminal.reset();
        terminal
            .resize(100, 30, 8, 16)
            .expect("resize should succeed");
        assert_eq!(take(), []);

        terminal.vt_write(b"\x1b[?2026h");
        terminal.reset();
        terminal.reset();
        terminal.vt_write(b"\x1b[?2026h");
        terminal
            .resize(80, 24, 8, 16)
            .expect("resize should succeed");
        terminal.vt_write(b"\x1b[?2026h");
        terminal
            .resize(80, 24, 8, 16)
            .expect("resize should succeed");
        assert_eq!(take(), [true, false, true, false, true, false]);
        assert!(!terminal.mode(Mode::SYNC_OUTPUT).unwrap());

        terminal.vt_write(b"\x1b[?2026h");
        assert!(terminal.resize(0, 24, 8, 16).is_err());
        assert!(terminal.mode(Mode::SYNC_OUTPUT).unwrap());
        terminal.vt_write(b"\x1b[?2026l");
        assert_eq!(take(), [true, false]);

        terminal.vt_write(b"\x1b[?2026h");
        terminal
            .set_mode(Mode::SYNC_OUTPUT, false)
            .expect("mode should be settable");
        assert!(!terminal.mode(Mode::SYNC_OUTPUT).unwrap());
        terminal
            .set_mode(Mode::SYNC_OUTPUT, true)
            .expect("mode should be settable")
            .set_mode(Mode::SYNC_OUTPUT, false)
            .expect("mode should be settable");
        assert_eq!(take(), [true]);
        terminal.vt_write(b"\x1b[?2026h");
        assert_eq!(take(), [true]);
    }

    fn first_row_text(snapshot: &crate::render::Snapshot<'_, '_>) -> Result<String> {
        let mut rows = crate::render::RowIterator::new()?;
        let mut cells = crate::render::CellIterator::new()?;
        let mut row_iter = rows.update(snapshot)?;
        let Some(row) = row_iter.next() else {
            return Ok(String::new());
        };
        let mut cell_iter = cells.update(row)?;
        let mut text = String::new();
        while let Some(cell) = cell_iter.next() {
            text.extend(cell.graphemes()?);
        }
        Ok(text)
    }

    #[test]
    fn render_hold_captures_frame_before_hold() {
        let render_state =
            RefCell::new(RenderState::new().expect("render state should initialize"));
        let frames = RefCell::new(Vec::new());
        let take = || std::mem::take(&mut *frames.borrow_mut());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_render_hold(|term, held| {
                let text = render_state.try_borrow_mut().ok().and_then(|mut state| {
                    let snapshot = state.update(term).ok()?;
                    first_row_text(&snapshot).ok()
                });
                frames.borrow_mut().push((held, text));
            })
            .expect("callback should register");

        terminal.vt_write(b"A\x1b[?2026hB");
        assert_eq!(take(), [(true, Some("A".to_owned()))]);
        {
            let mut state = render_state.borrow_mut();
            let snapshot = state.update(&terminal).expect("render state should update");
            assert_eq!(first_row_text(&snapshot).unwrap(), "AB");
        }
        terminal.vt_write(b"\x1b[?2026l");
        assert_eq!(take(), [(false, Some("AB".to_owned()))]);

        terminal.vt_write(b"\x1b[?2026hC");
        terminal
            .resize(100, 30, 8, 16)
            .expect("resize should succeed");
        assert_eq!(
            take(),
            [
                (true, Some("AB".to_owned())),
                (false, Some("ABC".to_owned()))
            ]
        );
        terminal.vt_write(b"\x1b[?2026hD");
        terminal.reset();
        assert_eq!(
            take(),
            [(true, Some("ABC".to_owned())), (false, Some(String::new()))]
        );
    }

    #[inline(never)]
    fn build_terminal(callback_count: &RefCell<usize>) -> Terminal<'static, '_> {
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");

        terminal
            .on_device_attributes(move |_term| {
                *callback_count.borrow_mut() += 1;
                Some(DeviceAttributes {
                    primary: PrimaryDeviceAttributes::new(
                        ConformanceLevel::VT220,
                        &[DeviceAttributeFeature::ANSI_COLOR],
                    ),
                    secondary: SecondaryDeviceAttributes {
                        device_type: DeviceType::VT220,
                        firmware_version: 1,
                        rom_cartridge: 0,
                    },
                    tertiary: TertiaryDeviceAttributes { unit_id: 0 },
                })
            })
            .expect("callback should register");

        terminal
    }

    fn relocate_into_new_box<T>(value: T) -> (Box<T>, usize, usize) {
        let src = Box::new(ManuallyDrop::new(value));
        let src_addr = std::ptr::from_ref(&**src).cast::<T>() as usize;

        unsafe {
            let dst_layout = std::alloc::Layout::new::<T>();
            let dst_ptr = std::alloc::alloc(dst_layout).cast::<T>();
            if dst_ptr.is_null() {
                std::alloc::handle_alloc_error(dst_layout);
            }

            let dst_addr = dst_ptr as usize;
            assert_ne!(
                src_addr, dst_addr,
                "test setup failed: source and destination storage unexpectedly match"
            );

            std::ptr::copy_nonoverlapping(std::ptr::from_ref(&**src).cast::<T>(), dst_ptr, 1);

            std::alloc::dealloc(
                Box::into_raw(src).cast::<u8>(),
                std::alloc::Layout::new::<ManuallyDrop<T>>(),
            );

            (Box::from_raw(dst_ptr), src_addr, dst_addr)
        }
    }

    #[test]
    fn title_changed_callback_returns_correct_title() {
        let captured_title: RefCell<Vec<u8>> = RefCell::new(Vec::new());
        let callback_count: Cell<usize> = Cell::new(0);

        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");

        terminal
            .on_title_changed(|term| {
                callback_count.set(callback_count.get() + 1);
                let title = term
                    .title()
                    .expect("title() should succeed inside callback");
                *captured_title.borrow_mut() = title.to_owned();
            })
            .expect("callback should register");

        terminal.vt_write(b"\x1b]2;Hello Effects\x1b\\");
        assert_eq!(callback_count.get(), 1);
        assert_eq!(*captured_title.borrow(), b"Hello Effects");

        terminal.vt_write(b"\x1b]2;Second Title\x1b\\");
        assert_eq!(callback_count.get(), 2);
        assert_eq!(*captured_title.borrow(), b"Second Title");
    }

    #[test]
    fn pwd_changed_callback_returns_correct_pwd() {
        let captured_pwd: RefCell<Vec<u8>> = RefCell::new(Vec::new());
        let callback_count: Cell<usize> = Cell::new(0);

        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");

        terminal
            .on_pwd_changed(|term| {
                callback_count.set(callback_count.get() + 1);
                let pwd = term.pwd().expect("pwd() should succeed inside callback");
                *captured_pwd.borrow_mut() = pwd.to_owned();
            })
            .expect("callback should register");

        terminal.vt_write(b"\x1b]7;file://localhost/tmp/project\x1b\\");
        assert_eq!(callback_count.get(), 1);
        assert_eq!(*captured_pwd.borrow(), b"file://localhost/tmp/project");

        terminal.vt_write(b"\x1b]7;file://localhost/tmp/other\x1b\\");
        assert_eq!(callback_count.get(), 2);
        assert_eq!(*captured_pwd.borrow(), b"file://localhost/tmp/other");

        terminal.vt_write(b"\x1b]7;file://localhost/tmp/\xff\x1b\\");
        assert_eq!(callback_count.get(), 3);
        assert_eq!(*captured_pwd.borrow(), b"file://localhost/tmp/\xff");
    }

    #[test]
    fn default_cursor_reset_uses_configured_style_and_blink() {
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        let mut render_state = RenderState::new().expect("render state should initialize");

        terminal
            .set_default_cursor_style(Some(CursorStyle::Underline))
            .expect("default cursor style should update")
            .set_default_cursor_blink(Some(true))
            .expect("default cursor blink should update");

        terminal.vt_write(b"\x1b[0 q");
        let snapshot = render_state
            .update(&terminal)
            .expect("render state should update");

        assert_eq!(
            snapshot
                .cursor_visual_style()
                .expect("cursor style should be readable"),
            CursorVisualStyle::Underline
        );
        assert!(
            snapshot
                .cursor_blinking()
                .expect("cursor blink should be readable")
        );
    }

    #[test]
    fn glyph_protocol_enabled_setting_updates() {
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");

        terminal
            .set_glyph_protocol_enabled(false)
            .expect("glyph protocol should disable")
            .set_glyph_protocol_enabled(true)
            .expect("glyph protocol should enable");
    }

    #[test]
    fn callbacks_survive_explicit_relocation() {
        let callback_count = RefCell::new(0usize);
        let terminal = build_terminal(&callback_count);
        let (mut terminal, addr_before, addr_after) = relocate_into_new_box(terminal);
        assert_ne!(addr_before, addr_after);

        terminal.vt_write(b"\x1b[c");
        assert_eq!(*callback_count.borrow(), 1);
    }


    #[test]
    fn clipboard_write_decodes_base64() {
        let written = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_clipboard_write(|_, request| {
                for content in request.contents() {
                    written.borrow_mut().push(content.data.to_vec());
                }
                request.reply(Ok(()), false);
            })
            .unwrap();

        terminal.vt_write(b"\x1b]52;c;//4=\x1b\\");
        assert_eq!(*written.borrow(), [vec![0xff, 0xfe]]);
    }

    #[test]
    fn multibyte_utf8_is_decoded() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal.vt_write(b"\xc3\xa9");
        let codepoint = terminal
            .grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
            .unwrap()
            .cell()
            .unwrap()
            .codepoint()
            .unwrap();
        assert_eq!(codepoint, 0xe9);
    }

    #[test]
    fn mouse_shape_follows_osc_22() {
        let mut terminal = Terminal::new(8, 3).unwrap();
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::Text);
        terminal.vt_write(b"\x1b]22;pointer\x07");
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::Pointer);
        terminal.vt_write(b"\x1b]22;nwse-resize\x1b\\");
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::NwseResize);
        terminal.vt_write(b"\x1b]22;not-a-shape\x07");
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::NwseResize);
        terminal.vt_write(b"\x1b]22;\x1b\\");
        assert_eq!(terminal.mouse_shape().unwrap(), mouse::Shape::Text);
    }

    #[test]
    fn desktop_notifications_are_not_assumed_to_be_utf8() {
        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_desktop_notification(|_, notification| {
                seen.borrow_mut()
                    .push((notification.title().to_vec(), notification.body().to_vec()));
            })
            .unwrap();
        terminal.vt_write(b"\x1b]9;\xff\xfe\x07");
        terminal.vt_write(b"\x1b]777;notify;\xc3\x28;ok\x07");
        assert_eq!(
            *seen.borrow(),
            [
                (b"".to_vec(), b"\xff\xfe".to_vec()),
                (b"\xc3\x28".to_vec(), b"ok".to_vec()),
            ]
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SeenStatus {
        state: Option<ProgramStatusState>,
        kind: Option<ProgramStatusKind>,
        progress: Option<u8>,
        id: Vec<u8>,
        app: Vec<u8>,
        title: Vec<u8>,
        message: Vec<u8>,
    }

    impl SeenStatus {
        fn bare(state: ProgramStatusState) -> Self {
            Self {
                state: Some(state),
                kind: Some(ProgramStatusKind::None),
                progress: None,
                id: Vec::new(),
                app: Vec::new(),
                title: Vec::new(),
                message: Vec::new(),
            }
        }
    }

    fn program_status(input: &[u8]) -> (Vec<SeenStatus>, Vec<u8>) {
        let seen = RefCell::new(Vec::new());
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_pty_write(|_, bytes| output.borrow_mut().extend_from_slice(bytes))
            .unwrap()
            .on_program_status(|_, report| {
                seen.borrow_mut().push(SeenStatus {
                    state: report.state().ok(),
                    kind: report.kind().ok(),
                    progress: report.progress(),
                    id: report.id().to_vec(),
                    app: report.app().to_vec(),
                    title: report.title().to_vec(),
                    message: report.message().to_vec(),
                });
            })
            .unwrap();
        terminal.vt_write(input);
        drop(terminal);
        (seen.into_inner(), output.into_inner())
    }

    #[test]
    fn program_status_delivers_every_field() {
        let (seen, output) = program_status(
            b"\x1b]7501;state=blocked:kind=permission:progress=40:id=a/b\
              :app=terraform:title=UGxhbg==:msg=QXBwbHk/\x07",
        );
        assert_eq!(
            seen,
            [SeenStatus {
                state: Some(ProgramStatusState::Blocked),
                kind: Some(ProgramStatusKind::Permission),
                progress: Some(40),
                id: b"a/b".to_vec(),
                app: b"terraform".to_vec(),
                title: b"Plan".to_vec(),
                message: b"Apply?".to_vec(),
            }]
        );
        assert!(output.is_empty());
    }

    #[test]
    fn program_status_leaves_unsent_values_empty() {
        let (seen, _) = program_status(
            b"\x1b]7501;state=blocked:kind=permission:app=terraform:msg=QXBwbHk/\x1b\\",
        );
        assert_eq!(
            seen,
            [SeenStatus {
                kind: Some(ProgramStatusKind::Permission),
                app: b"terraform".to_vec(),
                message: b"Apply?".to_vec(),
                ..SeenStatus::bare(ProgramStatusState::Blocked)
            }]
        );

        let (seen, _) = program_status(b"\x1b]7501;state=done\x1b\\");
        assert_eq!(seen, [SeenStatus::bare(ProgramStatusState::Done)]);
    }

    #[test]
    fn program_status_progress_and_kind_follow_state() {
        let (seen, _) = program_status(
            &[
                &b"\x1b]7501;state=working:progress=0\x07"[..],
                b"\x1b]7501;state=working:progress=100\x07",
                b"\x1b]7501;state=working:progress=101\x07",
                b"\x1b]7501;state=idle:progress=50:kind=question\x07",
                b"\x1b]7501;state=blocked:kind=auth\x07",
                b"\x1b]7501;state=blocked:kind=telepathy\x07",
            ]
            .concat(),
        );
        assert_eq!(
            seen,
            [
                SeenStatus {
                    progress: Some(0),
                    ..SeenStatus::bare(ProgramStatusState::Working)
                },
                SeenStatus {
                    progress: Some(100),
                    ..SeenStatus::bare(ProgramStatusState::Working)
                },
                SeenStatus::bare(ProgramStatusState::Working),
                SeenStatus::bare(ProgramStatusState::Idle),
                SeenStatus {
                    kind: Some(ProgramStatusKind::Auth),
                    ..SeenStatus::bare(ProgramStatusState::Blocked)
                },
                SeenStatus::bare(ProgramStatusState::Blocked),
            ]
        );
    }

    #[test]
    fn program_status_drops_invalid_reports() {
        let (seen, output) =
            program_status(b"\x1b]7501;state=sleeping\x07\x1b]7501;state=idle:id=a//b\x07");
        assert!(seen.is_empty());
        assert!(output.is_empty());
    }

    #[test]
    fn program_status_clears_every_record_on_full_reset() {
        let (seen, _) = program_status(b"\x1b]7501;state=error:id=x\x07\x1bc");
        assert_eq!(
            seen,
            [
                SeenStatus {
                    id: b"x".to_vec(),
                    ..SeenStatus::bare(ProgramStatusState::Error)
                },
                SeenStatus::bare(ProgramStatusState::Clear),
            ]
        );
    }

    #[test]
    fn program_status_is_not_cleared_by_reset() {
        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_program_status(|_, report| {
                seen.borrow_mut()
                    .push((report.state().ok(), report.id().to_vec()));
            })
            .unwrap();

        terminal.vt_write(b"\x1b]7501;state=error:id=x\x07");
        assert_eq!(
            seen.take(),
            [(Some(ProgramStatusState::Error), b"x".to_vec())]
        );

        terminal.reset();
        assert!(seen.borrow().is_empty());

        terminal.vt_write(b"\x1bc");
        assert_eq!(seen.take(), [(Some(ProgramStatusState::Clear), Vec::new())]);
    }

    #[test]
    fn program_status_query_is_answered_only_with_a_callback() {
        const REPLY: &[u8] =
            b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth";

        let (seen, output) = program_status(b"\x1b]7501;?\x1b\\\x1b]7501;?\x07");
        assert!(seen.is_empty());
        assert_eq!(output, [REPLY, b"\x1b\\", REPLY, b"\x07"].concat());

        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_pty_write(|_, bytes| output.borrow_mut().extend_from_slice(bytes))
            .unwrap();
        terminal.vt_write(b"\x1b]7501;?\x1b\\");
        drop(terminal);
        assert!(output.into_inner().is_empty());
    }

    #[derive(Debug, PartialEq, Eq)]
    struct SeenPrompt {
        kind: Option<SemanticPromptKind>,
        prompt_kind: Option<PromptKind>,
        exit_code: Option<i32>,
        command: Vec<u8>,
        error: Vec<u8>,
    }

    impl SeenPrompt {
        fn bare(kind: SemanticPromptKind) -> Self {
            Self {
                kind: Some(kind),
                prompt_kind: Some(PromptKind::Primary),
                exit_code: None,
                command: Vec::new(),
                error: Vec::new(),
            }
        }
    }

    fn semantic_prompt(input: &[u8]) -> Vec<SeenPrompt> {
        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 3).unwrap();
        terminal
            .on_semantic_prompt(|_, event| {
                seen.borrow_mut().push(SeenPrompt {
                    kind: event.kind().ok(),
                    prompt_kind: event.prompt_kind().ok(),
                    exit_code: event.exit_code(),
                    command: event.command().to_vec(),
                    error: event.error().to_vec(),
                });
            })
            .unwrap();
        terminal.vt_write(input);
        drop(terminal);
        seen.into_inner()
    }

    #[test]
    fn semantic_prompt_reports_each_command_step() {
        let seen = semantic_prompt(
            b"\x1b]133;A\x07\
              \x1b]133;P;k=r\x07\
              \x1b]133;P;k=c\x07\
              \x1b]133;P;k=s\x07\
              \x1b]133;P;k=x\x07\
              \x1b]133;N\x07\
              \x1b]133;B\x07\
              \x1b]133;I\x07\
              \x1b]133;C;cmdline_url=ls%20-la\x07\
              \x1b]133;D;0\x07\
              \x1b]133;D;-1;err=boom\x07\
              \x1b]133;D\x07",
        );
        assert_eq!(
            seen,
            [
                SeenPrompt::bare(SemanticPromptKind::PromptStart),
                SeenPrompt {
                    prompt_kind: Some(PromptKind::Right),
                    ..SeenPrompt::bare(SemanticPromptKind::PromptStart)
                },
                SeenPrompt {
                    prompt_kind: Some(PromptKind::Continuation),
                    ..SeenPrompt::bare(SemanticPromptKind::PromptStart)
                },
                SeenPrompt {
                    prompt_kind: Some(PromptKind::Secondary),
                    ..SeenPrompt::bare(SemanticPromptKind::PromptStart)
                },
                SeenPrompt::bare(SemanticPromptKind::PromptStart),
                SeenPrompt::bare(SemanticPromptKind::PromptStart),
                SeenPrompt::bare(SemanticPromptKind::InputStart),
                SeenPrompt::bare(SemanticPromptKind::InputStart),
                SeenPrompt {
                    command: b"ls -la".to_vec(),
                    ..SeenPrompt::bare(SemanticPromptKind::OutputStart)
                },
                SeenPrompt {
                    exit_code: Some(0),
                    ..SeenPrompt::bare(SemanticPromptKind::CommandEnd)
                },
                SeenPrompt {
                    exit_code: Some(-1),
                    error: b"boom".to_vec(),
                    ..SeenPrompt::bare(SemanticPromptKind::CommandEnd)
                },
                SeenPrompt::bare(SemanticPromptKind::CommandEnd),
            ]
        );
    }

    #[test]
    fn semantic_prompt_command_is_raw_bytes() {
        let seen = semantic_prompt(
            b"\x1b]133;C;cmdline_url=%ff%fe\x07\
              \x1b]133;C;cmdline='bad\x07\
              \x1b]133;L\x07",
        );
        assert_eq!(
            seen,
            [
                SeenPrompt {
                    command: b"\xff\xfe".to_vec(),
                    ..SeenPrompt::bare(SemanticPromptKind::OutputStart)
                },
                SeenPrompt::bare(SemanticPromptKind::OutputStart),
            ]
        );
    }

    fn resize_window(input: &[u8]) -> (Vec<ResizeWindow>, (u16, u16)) {
        let seen = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).unwrap();
        terminal
            .on_resize_window(|_, request| seen.borrow_mut().push(request))
            .unwrap();
        terminal.vt_write(input);
        let size = (terminal.cols().unwrap(), terminal.rows().unwrap());
        drop(terminal);
        (seen.into_inner(), size)
    }

    #[test]
    fn resize_window_delivers_rows_then_columns() {
        let (seen, size) = resize_window(b"\x1b[8;30;100t");
        assert_eq!(
            seen,
            [ResizeWindow::Grid {
                rows: NonZeroU16::new(30),
                columns: NonZeroU16::new(100),
            }]
        );
        assert_eq!(size, (80, 24));
    }

    #[test]
    fn resize_window_reports_omitted_and_zero_params_as_none() {
        let (seen, size) = resize_window(b"\x1b[8;;120t\x1b[8;30t\x1b[8t\x1b[8;0;0t");
        assert_eq!(
            seen,
            [
                ResizeWindow::Grid {
                    rows: None,
                    columns: NonZeroU16::new(120),
                },
                ResizeWindow::Grid {
                    rows: NonZeroU16::new(30),
                    columns: None,
                },
                ResizeWindow::Grid {
                    rows: None,
                    columns: None,
                },
                ResizeWindow::Grid {
                    rows: None,
                    columns: None,
                },
            ]
        );
        assert_eq!(size, (80, 24));
    }

    #[test]
    fn resize_window_from_raw_skips_unknown_tags() {
        let mut raw = ffi::TerminalResizeWindow {
            tag: ffi::TerminalResizeWindowTag::GRID,
            ..Default::default()
        };
        raw.value.grid = ffi::TerminalResizeWindowGrid {
            rows: 30,
            columns: 100,
        };
        assert_eq!(
            unsafe { ResizeWindow::from_raw(&raw const raw) },
            Some(ResizeWindow::Grid {
                rows: NonZeroU16::new(30),
                columns: NonZeroU16::new(100),
            })
        );

        raw.tag = ffi::TerminalResizeWindowTag::GRID + 1;
        assert_eq!(unsafe { ResizeWindow::from_raw(&raw const raw) }, None);
    }

    #[test]
    fn unknown_sequence_from_raw_skips_unknown_tags_and_terminators() {
        let apc_bytes = *b"Gf=100";
        let osc_bytes = *b"7400;status=busy";
        let mut raw = ffi::TerminalUnknownSequence {
            tag: ffi::TerminalUnknownSequenceTag::APC,
            ..Default::default()
        };
        raw.value.apc = ffi::TerminalUnknownStringSequence {
            truncated: true,
            content: ffi::String {
                ptr: apc_bytes.as_ptr(),
                len: apc_bytes.len(),
            },
        };
        assert_eq!(
            unsafe { UnknownSequence::from_raw(&raw const raw) },
            Some(UnknownSequence::Apc {
                content: b"Gf=100",
                truncated: true,
            })
        );

        raw.tag = ffi::TerminalUnknownSequenceTag::OSC;
        raw.value.osc = ffi::TerminalUnknownOscSequence {
            truncated: false,
            content: ffi::String {
                ptr: osc_bytes.as_ptr(),
                len: osc_bytes.len(),
            },
            terminator: ffi::OscTerminator::ST,
        };
        assert_eq!(
            unsafe { UnknownSequence::from_raw(&raw const raw) },
            Some(UnknownSequence::Osc {
                content: b"7400;status=busy",
                truncated: false,
                terminator: osc::Terminator::St,
            })
        );

        raw.value.osc.terminator = ffi::OscTerminator::BEL + 1;
        assert_eq!(unsafe { UnknownSequence::from_raw(&raw const raw) }, None);

        raw.tag = ffi::TerminalUnknownSequenceTag::OSC + 1;
        assert_eq!(unsafe { UnknownSequence::from_raw(&raw const raw) }, None);
    }

    fn tiny_terminal() -> Terminal<'static, 'static> {
        Terminal::new(8, 3).expect("terminal should initialize")
    }

    fn codepoint_at_tracked_ref(terminal: &Terminal<'_, '_>, tracked: &TrackedGridRef) -> u32 {
        let snapshot = tracked
            .snapshot(terminal)
            .expect("tracked snapshot should not fail")
            .expect("tracked ref should have a value");
        snapshot
            .cell()
            .expect("tracked snapshot should resolve to a cell")
            .codepoint()
            .expect("tracked snapshot cell should expose a codepoint")
    }

    #[test]
    fn tracked_grid_ref_follows_scroll() {
        let mut terminal = tiny_terminal();
        terminal.vt_write(b"alpha\r\nbravo\r\ncharlie");

        let tracked = terminal
            .track_grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
            .expect("tracked grid ref should initialize");

        terminal.vt_write(b"\r\ndelta");

        assert!(tracked.has_value());
        assert_eq!(
            codepoint_at_tracked_ref(&terminal, &tracked),
            u32::from('a')
        );
        assert_eq!(
            tracked
                .point(PointSpace::Screen)
                .expect("tracked point should resolve")
                .expect("tracked point should have a value")
                .x,
            0
        );
    }

    #[test]
    fn tracked_grid_ref_reports_loss_and_can_set_point() {
        let mut terminal = tiny_terminal();
        terminal.vt_write(b"alpha\r\nbravo\r\ncharlie");

        let mut tracked = terminal
            .track_grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
            .expect("tracked grid ref should initialize");

        terminal.reset();

        assert!(!tracked.has_value());
        assert!(
            tracked
                .snapshot(&terminal)
                .expect("missing tracked snapshot should not fail")
                .is_none()
        );
        assert!(
            tracked
                .point(PointSpace::Screen)
                .expect("missing tracked point should not fail")
                .is_none()
        );

        terminal.vt_write(b"echo");
        tracked
            .set(&mut terminal, Point::Active(PointCoordinate { x: 0, y: 0 }))
            .expect("tracked grid ref should set to a new point");

        assert!(tracked.has_value());
        assert_eq!(
            codepoint_at_tracked_ref(&terminal, &tracked),
            u32::from('e')
        );
    }

    #[test]
    fn tracked_grid_ref_survives_terminal_drop() {
        let tracked = {
            let mut terminal = tiny_terminal();
            terminal.vt_write(b"alpha");
            terminal
                .track_grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
                .expect("tracked grid ref should initialize")
        };

        assert!(!tracked.has_value());
        assert!(
            tracked
                .point(PointSpace::Screen)
                .expect("detached tracked point should not fail")
                .is_none()
        );
    }

    #[test]
    fn tracked_grid_ref_rejects_different_terminal() {
        let mut first = tiny_terminal();
        first.vt_write(b"alpha");
        let mut second = tiny_terminal();
        second.vt_write(b"bravo");

        let mut tracked = first
            .track_grid_ref(Point::Active(PointCoordinate { x: 0, y: 0 }))
            .expect("tracked grid ref should initialize");

        assert!(matches!(
            tracked.snapshot(&second),
            Err(Error::InvalidValue)
        ));
        assert!(matches!(
            tracked.set(&mut second, Point::Active(PointCoordinate { x: 0, y: 0 })),
            Err(Error::InvalidValue)
        ));
    }

    #[test]
    fn grid_ref_converts_back_to_point() {
        let mut terminal = tiny_terminal();
        terminal.vt_write(b"alpha");

        let original = PointCoordinate { x: 1, y: 0 };
        let grid_ref = terminal
            .grid_ref(Point::Active(original))
            .expect("grid ref should resolve");

        assert_eq!(
            terminal
                .point_from_grid_ref(&grid_ref, PointSpace::Active)
                .expect("grid ref point conversion should not fail")
                .expect("grid ref should be representable in active space"),
            original
        );
    }

    #[test]
    fn clipboard_write_reads_only_fields_within_its_size() {
        unsafe extern "C" fn reply(
            _: *const ffi::ClipboardWrite,
            _: *const ffi::ClipboardWriteReply,
        ) {
            panic!("a reply function past the request's size was called");
        }
        let data = b"hi";
        let content = ffi::ClipboardContent {
            mime: ffi::String::from("text/plain"),
            data: ffi::String {
                ptr: data.as_ptr(),
                len: data.len(),
            },
        };
        let raw = ffi::ClipboardWrite {
            size: std::mem::offset_of!(ffi::ClipboardWrite, name),
            location: ffi::ClipboardLocation::PRIMARY,
            contents: &raw const content,
            contents_len: 1,
            name: ffi::String::from("program"),
            granted: true,
            can_remember: true,
            reply: Some(reply),
            ..ffi::sized!(ffi::ClipboardWrite)
        };
        let write = unsafe { ClipboardWrite::from_raw(&raw const raw) };
        assert_eq!(write.location(), ClipboardLocation::Primary);
        assert_eq!(write.contents().next().unwrap().data, b"hi");
        assert!(write.name().is_empty());
        assert!(!write.granted());
        assert!(!write.can_remember());
        write.reply(Ok(()), false);
    }

    #[test]
    fn clipboard_read_reads_only_fields_within_its_size() {
        unsafe extern "C" fn reply(
            _: *const ffi::ClipboardRead,
            _: *const ffi::ClipboardReadReply,
        ) {
            panic!("a reply function past the request's size was called");
        }
        let mime = ffi::String::from("text/plain");
        let raw = ffi::ClipboardRead {
            size: std::mem::offset_of!(ffi::ClipboardRead, name),
            location: ffi::ClipboardLocation::SELECTION,
            mimes: &raw const mime,
            mimes_len: 1,
            list: true,
            name: ffi::String::from("program"),
            granted: true,
            can_remember: true,
            reply: Some(reply),
            ..ffi::sized!(ffi::ClipboardRead)
        };
        let read = unsafe { ClipboardRead::from_raw(&raw const raw) };
        assert_eq!(read.location(), ClipboardLocation::Selection);
        assert_eq!(read.mimes().collect::<Vec<_>>(), [b"text/plain"]);
        assert!(read.name().is_empty());
        assert!(!read.granted());
        assert!(!read.can_remember());
        read.reply(Ok(&[]), &[], false);
    }

    const CHECKSUM_SCREEN: &[u8] = b"\x1b[1mA\x1b[0mB C";

    const DECRQCRA: &[u8] = b"\x1b[7;1;1;1;1;4*y";

    fn request_checksum(terminal: &mut Terminal<'_, '_>, output: &RefCell<Vec<u8>>) -> Vec<u8> {
        output.borrow_mut().clear();
        terminal.vt_write(DECRQCRA);
        output.borrow().clone()
    }

    #[test]
    fn xt_checksum_report_requires_opt_in() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal.vt_write(CHECKSUM_SCREEN);

        assert_eq!(request_checksum(&mut terminal, &output), b"");

        terminal
            .set_xt_checksum_report_enabled(true)
            .expect("enabling checksum reports should succeed");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FEBA\x1b\\"
        );

        terminal
            .set_xt_checksum_report_enabled(false)
            .expect("disabling checksum reports should succeed");
        assert_eq!(request_checksum(&mut terminal, &output), b"");
    }

    #[test]
    fn xt_checksum_xtchecksum_ignored_while_disabled() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal.vt_write(CHECKSUM_SCREEN);

        terminal.vt_write(b"\x1b[1#y");
        terminal
            .set_xt_checksum_report_enabled(true)
            .expect("enabling checksum reports should succeed");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FEBA\x1b\\"
        );

        terminal.vt_write(b"\x1b[1#y");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~0146\x1b\\"
        );
    }

    #[test]
    fn xt_checksum_extension_changes_calculation() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal.vt_write(CHECKSUM_SCREEN);
        terminal
            .set_xt_checksum_report_enabled(true)
            .expect("enabling checksum reports should succeed");

        terminal
            .set_xt_checksum_extension(XtChecksumExtension::NO_ATTRIBUTES)
            .expect("valid extension should be accepted");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FF3A\x1b\\"
        );

        terminal
            .set_xt_checksum_extension(XtChecksumExtension::NO_OMIT_BLANKS)
            .expect("valid extension should be accepted");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FE9A\x1b\\"
        );

        terminal
            .set_xt_checksum_extension(XtChecksumExtension::NO_NEGATE)
            .expect("valid extension should be accepted");
        terminal.vt_write(b"\x1bc");
        terminal.vt_write(CHECKSUM_SCREEN);
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~0146\x1b\\"
        );

        terminal
            .set_xt_checksum_extension(XtChecksumExtension::empty())
            .expect("empty extension should be accepted");
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FEBA\x1b\\"
        );

        let result = terminal.set_xt_checksum_extension(XtChecksumExtension::from_bits_retain(32));
        assert!(matches!(result, Err(Error::InvalidValue)));
        assert_eq!(
            request_checksum(&mut terminal, &output),
            b"\x1bP7!~FEBA\x1b\\"
        );
    }

    #[test]
    fn xt_checksum_extension_unwritten_and_full_codepoints() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).expect("terminal should initialize");
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .expect("callback should register");
        terminal
            .set_xt_checksum_report_enabled(true)
            .expect("enabling checksum reports should succeed");

        terminal.vt_write("\x1b[2;1H\u{1D400}\x1b[1;2H\x1b[1mX".as_bytes());
        let request = |terminal: &mut Terminal<'_, '_>, req: &[u8]| {
            output.borrow_mut().clear();
            terminal.vt_write(req);
            output.borrow().clone()
        };

        let row1 = b"\x1b[7;1;1;1;1;2*y";
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::empty())
            .expect("valid extension should be accepted");
        assert_eq!(request(&mut terminal, row1), b"\x1bP7!~FF28\x1b\\");
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::UNWRITTEN_AS_SPACES)
            .expect("valid extension should be accepted");
        assert_eq!(request(&mut terminal, row1), b"\x1bP7!~FF08\x1b\\");

        let row2 = b"\x1b[7;1;2;1;2;1*y";
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::empty())
            .expect("valid extension should be accepted");
        assert_eq!(request(&mut terminal, row2), b"\x1bP7!~FFE5\x1b\\");
        terminal
            .set_xt_checksum_extension(XtChecksumExtension::FULL_CODEPOINTS)
            .expect("valid extension should be accepted");
        assert_eq!(request(&mut terminal, row2), b"\x1bP7!~2C00\x1b\\");
    }
}

#[cfg(all(test, miri))]
mod miri_soundness {
    use super::*;

    struct MockTerminal {
        userdata: *mut std::ffi::c_void,
        write: ffi::TerminalWritePtyFn,
        read: ffi::TerminalClipboardReadFn,
        resize: ffi::TerminalResizeWindowFn,
        unknown: ffi::TerminalUnknownSequenceFn,
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn ghostty_terminal_new(
        _: *const ffi::Allocator,
        out: *mut ffi::Terminal,
        _: u16,
        _: u16,
    ) -> ffi::Result::Type {
        let terminal = Box::new(MockTerminal {
            userdata: std::ptr::null_mut(),
            write: None,
            read: None,
            resize: None,
            unknown: None,
        });
        unsafe { *out = Box::into_raw(terminal).cast() };
        ffi::Result::SUCCESS
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn ghostty_terminal_free(terminal: ffi::Terminal) {
        unsafe { drop(Box::from_raw(terminal.cast::<MockTerminal>())) };
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn ghostty_terminal_set(
        terminal: ffi::Terminal,
        option: ffi::TerminalOption::Type,
        value: *const std::ffi::c_void,
    ) -> ffi::Result::Type {
        let terminal = unsafe { &mut *terminal.cast::<MockTerminal>() };
        match option {
            ffi::TerminalOption::USERDATA => terminal.userdata = value.cast_mut(),
            ffi::TerminalOption::WRITE_PTY => {
                terminal.write = Some(unsafe { std::mem::transmute(value) });
            }
            ffi::TerminalOption::CLIPBOARD_READ => {
                terminal.read = Some(unsafe { std::mem::transmute(value) });
            }
            ffi::TerminalOption::RESIZE_WINDOW => {
                terminal.resize = Some(unsafe { std::mem::transmute(value) });
            }
            ffi::TerminalOption::UNKNOWN_SEQUENCE => {
                terminal.unknown = Some(unsafe { std::mem::transmute(value) });
            }
            _ => return ffi::Result::INVALID_VALUE,
        }
        ffi::Result::SUCCESS
    }

    unsafe extern "C" fn reply_to_read(
        read: *const ffi::ClipboardRead,
        _: *const ffi::ClipboardReadReply,
    ) {
        let terminal = unsafe { (*read).ctx.cast_mut().cast::<ffi::TerminalImpl>() };
        unsafe { ghostty_terminal_vt_write(terminal, b"reply".as_ptr(), 5) };
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn ghostty_terminal_vt_write(
        terminal: ffi::Terminal,
        data: *const u8,
        len: usize,
    ) {
        let (userdata, write, read, resize, unknown) = {
            let mock = unsafe { &*terminal.cast::<MockTerminal>() };
            (
                mock.userdata,
                mock.write,
                mock.read,
                mock.resize,
                mock.unknown,
            )
        };
        let bytes = unsafe { std::slice::from_raw_parts(data, len) };
        if let Some(kind) = bytes.strip_prefix(b"resize:") {
            let known = kind == b"grid";
            let mut request = MaybeUninit::<ffi::TerminalResizeWindow>::uninit();
            let p = request.as_mut_ptr();
            unsafe {
                if known {
                    (&raw mut (*p).tag).write(ffi::TerminalResizeWindowTag::GRID);
                    (&raw mut (*p).value.grid).write(ffi::TerminalResizeWindowGrid {
                        rows: 30,
                        columns: 100,
                    });
                } else {
                    (&raw mut (*p).tag).write(ffi::TerminalResizeWindowTag::GRID + 1);
                }
            }
            unsafe { resize.unwrap()(terminal, userdata, p.cast_const()) };
        } else if let Some(kind) = bytes.strip_prefix(b"unknown:") {
            let content = b"payload";
            let content = ffi::String {
                ptr: content.as_ptr(),
                len: content.len(),
            };
            let mut sequence = MaybeUninit::<ffi::TerminalUnknownSequence>::uninit();
            let p = sequence.as_mut_ptr();
            unsafe {
                match kind {
                    b"apc" => {
                        (&raw mut (*p).tag).write(ffi::TerminalUnknownSequenceTag::APC);
                        (&raw mut (*p).value.apc).write(ffi::TerminalUnknownStringSequence {
                            truncated: true,
                            content,
                        });
                    }
                    b"osc" | b"osc-badterm" => {
                        let terminator = if kind == b"osc" {
                            ffi::OscTerminator::BEL
                        } else {
                            ffi::OscTerminator::BEL + 1
                        };
                        (&raw mut (*p).tag).write(ffi::TerminalUnknownSequenceTag::OSC);
                        (&raw mut (*p).value.osc).write(ffi::TerminalUnknownOscSequence {
                            truncated: false,
                            content,
                            terminator,
                        });
                    }
                    _ => (&raw mut (*p).tag).write(ffi::TerminalUnknownSequenceTag::OSC + 1),
                }
            }
            unsafe { unknown.unwrap()(terminal, userdata, p.cast_const()) };
        } else if bytes == b"clipboard" {
            let request = ffi::ClipboardRead {
                ctx: terminal.cast(),
                reply: Some(reply_to_read),
                ..ffi::sized!(ffi::ClipboardRead)
            };
            unsafe { read.unwrap()(terminal, userdata, &raw const request) };
        } else {
            unsafe { write.unwrap()(terminal, userdata, data, len) };
        }
    }

    #[test]
    fn moved_terminal_callbacks_and_destruction() {
        use std::cell::Cell;
        struct CountDrop<'a>(&'a Cell<usize>);
        impl Drop for CountDrop<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        fn dispatch<'alloc: 'cb, 'cb>(
            mut terminal: Terminal<'alloc, 'cb>,
        ) -> Terminal<'alloc, 'cb> {
            terminal.vt_write(b"reply");
            terminal
        }

        let drops = Cell::new(0);
        let calls = Cell::new(0);
        let mut terminal = Terminal::new(8, 2).unwrap();
        let first = CountDrop(&drops);
        let observed = &calls;
        terminal
            .on_pty_write(move |_, bytes| {
                let _guard = &first;
                assert_eq!(bytes, b"reply");
                observed.set(observed.get() + 1);
            })
            .unwrap();
        let mut terminal = dispatch(terminal);
        assert_eq!(calls.get(), 1);
        assert_eq!(drops.get(), 0);

        let second = CountDrop(&drops);
        terminal
            .on_pty_write(move |_, bytes| {
                let _guard = &second;
                assert_eq!(bytes, b"reply");
                observed.set(observed.get() + 10);
            })
            .unwrap();
        assert_eq!(drops.get(), 1);
        drop(dispatch(terminal));
        assert_eq!(calls.get(), 11);
        assert_eq!(drops.get(), 2);
        drop(Terminal::new(8, 2).unwrap());
    }

    #[test]
    fn moved_terminal_nested_clipboard_reply() {
        let order = std::cell::RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal
            .on_pty_write(|_, bytes| {
                assert_eq!(bytes, b"reply");
                order.borrow_mut().push(2);
            })
            .unwrap();
        terminal
            .on_clipboard_read(|_, request| {
                order.borrow_mut().push(1);
                request.reply(Ok(&[]), &[], false);
                order.borrow_mut().push(3);
            })
            .unwrap();
        fn dispatch(mut terminal: Terminal<'_, '_>) {
            terminal.vt_write(b"clipboard");
        }
        dispatch(terminal);
        assert_eq!(*order.borrow(), [1, 2, 3]);
    }

    #[test]
    fn resize_window_skips_unknown_tags() {
        let seen = std::cell::RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal
            .on_resize_window(|_, request| seen.borrow_mut().push(request))
            .unwrap();
        terminal.vt_write(b"resize:unknown");
        terminal.vt_write(b"resize:grid");
        drop(terminal);
        assert_eq!(
            seen.into_inner(),
            [ResizeWindow::Grid {
                rows: NonZeroU16::new(30),
                columns: NonZeroU16::new(100),
            }]
        );
    }

    #[test]
    fn unknown_sequence_skips_unknown_tags_and_terminators() {
        let seen = std::cell::RefCell::new(Vec::new());
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal
            .on_unknown_sequence(|_, sequence| {
                seen.borrow_mut().push(match sequence {
                    UnknownSequence::Apc { content, truncated } => {
                        (None, content.to_vec(), truncated)
                    }
                    UnknownSequence::Osc {
                        content,
                        truncated,
                        terminator,
                    } => (Some(terminator), content.to_vec(), truncated),
                });
            })
            .unwrap();
        terminal.vt_write(b"unknown:future");
        terminal.vt_write(b"unknown:osc-badterm");
        terminal.vt_write(b"unknown:apc");
        terminal.vt_write(b"unknown:osc");
        drop(terminal);
        assert_eq!(
            seen.into_inner(),
            [
                (None, b"payload".to_vec(), true),
                (Some(osc::Terminator::Bel), b"payload".to_vec(), false),
            ]
        );
    }

    #[test]
    fn clipboard_write_with_no_representations() {
        let raw = ffi::ClipboardWrite {
            location: ffi::ClipboardLocation::STANDARD,
            contents: std::ptr::null(),
            contents_len: 0,
            ..ffi::sized!(ffi::ClipboardWrite)
        };
        let write = unsafe { ClipboardWrite::from_raw(&raw) };
        assert_eq!(write.contents().count(), 0);
    }

    #[test]
    fn clipboard_content_with_non_utf8_data() {
        let data = [0xFF_u8, 0xFE];
        let raw = ffi::ClipboardContent {
            mime: ffi::String::from("text/plain"),
            data: ffi::String {
                ptr: data.as_ptr(),
                len: data.len(),
            },
        };
        let content = unsafe { ClipboardContent::from_raw(&raw) };
        assert_eq!(content.mime, b"text/plain");
        assert_eq!(content.data, &data);
    }

    #[test]
    fn clipboard_content_with_non_utf8_mime() {
        let mime = [0xFF_u8, 0xFE];
        let data = *b"hello";
        let raw = ffi::ClipboardContent {
            mime: ffi::String {
                ptr: mime.as_ptr(),
                len: mime.len(),
            },
            data: ffi::String {
                ptr: data.as_ptr(),
                len: data.len(),
            },
        };
        let content = unsafe { ClipboardContent::from_raw(&raw) };
        assert_eq!(content.mime, &mime);
        assert_eq!(content.data, b"hello");
    }
}
