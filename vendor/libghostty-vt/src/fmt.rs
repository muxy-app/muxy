use std::marker::PhantomData;

use crate::{
    alloc::{Allocator, Bytes, Object},
    error::{Error, Result, from_result},
    ffi,
    selection::Selection,
    terminal::Terminal,
};

#[derive(Debug)]
pub struct Formatter<'t, 'alloc: 'cb, 'cb: 't> {
    inner: Object<'alloc, ffi::FormatterImpl>,
    _terminal: PhantomData<&'t Terminal<'alloc, 'cb>>,
}

#[derive(Debug)]
pub struct FormatterOptions<'t, 's> {
    inner: ffi::FormatterTerminalOptions,
    _phan: PhantomData<&'s Selection<'t>>,
}
impl Default for FormatterOptions<'_, '_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'t, 's> FormatterOptions<'t, 's> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: ffi::FormatterTerminalOptions {
                extra: ffi::FormatterTerminalExtra {
                    screen: ffi::FormatterScreenExtra {
                        ..ffi::sized!(ffi::FormatterScreenExtra)
                    },
                    ..ffi::sized!(ffi::FormatterTerminalExtra)
                },
                ..ffi::sized!(ffi::FormatterTerminalOptions)
            },
            _phan: PhantomData,
        }
    }
    #[must_use]
    pub fn with_format(mut self, value: Format) -> Self {
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


    #[must_use]
    pub fn with_palette(mut self, value: bool) -> Self {
        self.inner.extra.palette = value;
        self
    }
    #[must_use]
    pub fn with_modes(mut self, value: bool) -> Self {
        self.inner.extra.modes = value;
        self
    }
    #[must_use]
    pub fn with_scrolling_region(mut self, value: bool) -> Self {
        self.inner.extra.scrolling_region = value;
        self
    }
    #[must_use]
    pub fn with_tabstops(mut self, value: bool) -> Self {
        self.inner.extra.tabstops = value;
        self
    }
    #[must_use]
    pub fn with_pwd(mut self, value: bool) -> Self {
        self.inner.extra.pwd = value;
        self
    }
    #[must_use]
    pub fn with_keyboard(mut self, value: bool) -> Self {
        self.inner.extra.keyboard = value;
        self
    }


    #[must_use]
    pub fn with_cursor(mut self, value: bool) -> Self {
        self.inner.extra.screen.cursor = value;
        self
    }
    #[must_use]
    pub fn with_style(mut self, value: bool) -> Self {
        self.inner.extra.screen.style = value;
        self
    }
    #[must_use]
    pub fn with_hyperlink(mut self, value: bool) -> Self {
        self.inner.extra.screen.hyperlink = value;
        self
    }
    #[must_use]
    pub fn with_protection(mut self, value: bool) -> Self {
        self.inner.extra.screen.protection = value;
        self
    }
    #[must_use]
    pub fn with_kitty_keyboard(mut self, value: bool) -> Self {
        self.inner.extra.screen.kitty_keyboard = value;
        self
    }
    #[must_use]
    pub fn with_charsets(mut self, value: bool) -> Self {
        self.inner.extra.screen.charsets = value;
        self
    }
}

impl<'t, 'alloc: 'cb, 'cb: 't> Formatter<'t, 'alloc, 'cb> {
    pub fn new(
        terminal: &'t Terminal<'alloc, 'cb>,
        opts: FormatterOptions<'t, '_>,
    ) -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null(), terminal, opts) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        terminal: &'t Terminal<'alloc, 'cb>,
        opts: FormatterOptions,
    ) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw(), terminal, opts) }
    }

    unsafe fn new_inner(
        alloc: *const ffi::Allocator,
        terminal: &'t Terminal<'alloc, 'cb>,
        opts: FormatterOptions,
    ) -> Result<Self> {
        let mut raw: ffi::Formatter = std::ptr::null_mut();

        let result = unsafe {
            ffi::ghostty_formatter_terminal_new(
                alloc,
                &raw mut raw,
                terminal.inner.as_raw(),
                opts.inner,
            )
        };
        from_result(result)?;

        Ok(Self {
            inner: Object::new(raw)?,
            _terminal: PhantomData,
        })
    }

    pub unsafe fn format_write<W: std::io::Write>(&mut self, writer: &mut W) -> Result<()> {
        let writer = crate::io::to_writer(writer);
        from_result(unsafe { ffi::ghostty_formatter_format(self.inner.as_raw(), writer) })
    }

    pub fn format_alloc<'a, 'ctx: 'a>(
        &mut self,
        alloc: Option<&'a Allocator<'ctx>>,
    ) -> Result<Bytes<'a>> {
        let alloc = if let Some(alloc) = alloc {
            alloc.to_raw()
        } else {
            std::ptr::null()
        };

        let mut bytes = std::ptr::null_mut();
        let mut len = 0usize;
        let result = unsafe {
            ffi::ghostty_formatter_format_alloc(
                self.inner.as_raw(),
                alloc,
                std::ptr::from_mut(&mut bytes),
                std::ptr::from_mut(&mut len),
            )
        };
        from_result(result)?;

        Ok(unsafe { Bytes::from_raw_parts(bytes, len, alloc) })
    }

    pub fn format_buf(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut len = 0usize;
        let result = unsafe {
            ffi::ghostty_formatter_format_buf(
                self.inner.as_raw(),
                std::ptr::from_mut(buf).cast(),
                buf.len(),
                std::ptr::from_mut(&mut len),
            )
        };
        from_result(result)?;
        Ok(len)
    }

    pub fn format_len(&mut self) -> Result<usize> {
        let mut len = 0usize;
        let result = unsafe {
            ffi::ghostty_formatter_format_buf(
                self.inner.as_raw(),
                std::ptr::null_mut(),
                0,
                std::ptr::from_mut(&mut len),
            )
        };
        match from_result(result) {
            Err(Error::OutOfSpace { .. }) => Ok(len),
            Err(e) => Err(e),
            Ok(()) => Err(Error::InvalidValue),
        }
    }
}

impl Drop for Formatter<'_, '_, '_> {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_formatter_free(self.inner.as_raw()) }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
pub enum Format {
    Plain = ffi::FormatterFormat::PLAIN,
    Vt = ffi::FormatterFormat::VT,
    Html = ffi::FormatterFormat::HTML,
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;

    struct ByteAtATime {
        bytes: Vec<u8>,
        writes: usize,
    }

    impl std::io::Write for ByteAtATime {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.writes += 1;
            self.bytes.extend_from_slice(&buf[..1]);
            Ok(1)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct Reject;

    impl std::io::Write for Reject {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("rejected"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn format_write<W: std::io::Write>(
        formatter: &mut Formatter<'_, '_, '_>,
        writer: &mut W,
    ) -> Result<()> {
        unsafe { formatter.format_write(writer) }
    }

    fn format_buf(formatter: &mut Formatter<'_, '_, '_>) -> Vec<u8> {
        let mut buf = vec![0; formatter.format_len().unwrap()];
        let len = formatter.format_buf(&mut buf).unwrap();
        buf.truncate(len);
        buf
    }

    #[test]
    fn writer_output_matches_buffer_output_in_every_format() {
        let mut terminal = Terminal::new(20, 3).unwrap();
        terminal.vt_write(b"plain \x1b[1;31mbold red\x1b[0m\r\nsecond line");
        for format in [Format::Plain, Format::Vt, Format::Html] {
            let options = FormatterOptions::new().with_format(format);
            let mut formatter = Formatter::new(&terminal, options).unwrap();
            let expected = format_buf(&mut formatter);
            assert!(!expected.is_empty(), "{format:?}");

            let mut actual = Vec::new();
            format_write(&mut formatter, &mut actual).unwrap();
            assert_eq!(actual, expected, "{format:?}");
        }
    }

    #[test]
    fn only_untrimmed_vt_keeps_trailing_blank_rows() {
        let mut terminal = Terminal::new(10, 6).unwrap();
        terminal.vt_write(b"one\r\ntwo");
        let format = |format, trim| {
            let options = FormatterOptions::new().with_format(format).with_trim(trim);
            format_buf(&mut Formatter::new(&terminal, options).unwrap())
        };

        assert_eq!(format(Format::Vt, false), b"one\r\ntwo\r\n\r\n\r\n\r\n");
        assert_eq!(format(Format::Vt, true), b"one\r\ntwo");
        assert_eq!(format(Format::Plain, false), b"one\ntwo");
    }

    #[test]
    fn only_untrimmed_vt_keeps_trailing_blank_rows_in_selection() {
        use crate::{
            selection::FormatOptions,
            terminal::{Point, PointCoordinate},
        };

        let mut terminal = Terminal::new(10, 6).unwrap();
        terminal.vt_write(b"one\r\ntwo");
        let at = |x, y| {
            terminal
                .grid_ref(Point::Active(PointCoordinate { x, y }))
                .unwrap()
        };
        let selection = Selection::new(at(0, 0), at(9, 3), false);
        let format = |format, trim| {
            let options = FormatOptions::new()
                .with_emit_format(format)
                .with_trim(trim)
                .with_selection(&selection);
            let bytes = terminal.format_selection_alloc(None, options).unwrap();
            bytes.unwrap().to_vec()
        };
        let vt_content = |trim| {
            let output = format(Format::Vt, trim);
            let mut rest = output.as_slice();
            for _ in 0..256 {
                rest = rest.strip_prefix(b"\x1b]4;").unwrap();
                let st = rest.windows(2).position(|w| w == b"\x1b\\").unwrap();
                rest = &rest[st + 2..];
            }
            rest.strip_suffix(b"\x1b[0m").unwrap().to_vec()
        };

        assert_eq!(vt_content(false), b"one\r\ntwo\r\n\r\n");
        assert_eq!(vt_content(true), b"one\r\ntwo");
        assert_eq!(format(Format::Plain, false), b"one\ntwo");
    }

    #[test]
    fn partial_writes_are_retried_until_complete() {
        let mut terminal = Terminal::new(20, 3).unwrap();
        terminal.vt_write(b"hello\r\nworld");
        let mut formatter = Formatter::new(&terminal, FormatterOptions::new()).unwrap();
        let expected = format_buf(&mut formatter);

        let mut writer = ByteAtATime {
            bytes: Vec::new(),
            writes: 0,
        };
        format_write(&mut formatter, &mut writer).unwrap();
        assert_eq!(writer.bytes, expected);
        assert_eq!(writer.writes, expected.len());
    }

    #[test]
    fn writer_failure_is_an_io_error() {
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal.vt_write(b"hello");
        let mut formatter = Formatter::new(&terminal, FormatterOptions::new()).unwrap();
        assert!(matches!(
            format_write(&mut formatter, &mut Reject),
            Err(Error::IoError)
        ));
        let mut output = Vec::new();
        format_write(&mut formatter, &mut output).unwrap();
        assert_eq!(output, b"hello");
    }

    struct FailAfter {
        chunks: usize,
        bytes: Vec<u8>,
    }

    impl std::io::Write for FailAfter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.chunks == 0 {
                return Err(std::io::Error::other("rejected"));
            }
            self.chunks -= 1;
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn large_output_streams_in_several_writes() {
        let mut terminal = Terminal::new(200, 100).unwrap();
        for i in 0..100 {
            terminal.vt_write(format!("{i:0>199}\r\n").as_bytes());
        }
        let mut formatter = Formatter::new(&terminal, FormatterOptions::new()).unwrap();
        let expected = format_buf(&mut formatter);
        assert!(expected.len() > 16 * 1024);

        let mut writer = FailAfter {
            chunks: usize::MAX,
            bytes: Vec::new(),
        };
        format_write(&mut formatter, &mut writer).unwrap();
        assert_eq!(writer.bytes, expected);
        assert!(usize::MAX - writer.chunks > 1, "output came in one write");

        let mut writer = FailAfter {
            chunks: 1,
            bytes: Vec::new(),
        };
        assert!(matches!(
            format_write(&mut formatter, &mut writer),
            Err(Error::IoError)
        ));
        assert!(!writer.bytes.is_empty() && writer.bytes.len() < expected.len());
    }
}
