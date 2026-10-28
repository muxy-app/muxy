//! Format terminal content as plain text, VT sequences, or HTML.
//!
//! A formatter captures a reference to a terminal and formatting options.
//! It can be used repeatedly to produce output that reflects the current
//! terminal state at the time of each format call.
use std::marker::PhantomData;

use crate::{
    alloc::{Allocator, Bytes, Object},
    error::{Error, Result, from_result},
    ffi,
    selection::Selection,
    terminal::Terminal,
};

/// Formatter that formats terminal content.
#[derive(Debug)]
pub struct Formatter<'t, 'alloc: 'cb, 'cb: 't> {
    inner: Object<'alloc, ffi::FormatterImpl>,
    _terminal: PhantomData<&'t Terminal<'alloc, 'cb>>,
}

/// Options for [creating a terminal formatter](Formatter::new).
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
    /// Create a new set of options for [creating a terminal formatter](Formatter::new).
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
    /// Specify the output format to emit.
    #[must_use]
    pub fn with_format(mut self, value: Format) -> Self {
        self.inner.emit = value.into();
        self
    }
    /// Specify whether to unwrap soft-wrapped lines.
    #[must_use]
    pub fn with_unwrap(mut self, value: bool) -> Self {
        self.inner.unwrap = value;
        self
    }
    /// Specify whether to trim trailing whitespace on non-blank lines.
    ///
    /// Trailing blank lines are always trimmed, except in [`Format::Vt`]
    /// output without trimming, where they are kept as part of the layout.
    ///
    /// Defaults to `false`, so [`Format::Vt`] output keeps trailing blank
    /// rows unless this is set.
    #[must_use]
    pub fn with_trim(mut self, value: bool) -> Self {
        self.inner.trim = value;
        self
    }
    /// Specify the selection to restrict output to a range.
    ///
    /// If a selection is not given, the formatter defaults to formatting
    /// the entire screen.
    #[must_use]
    pub fn with_selection(mut self, value: &'s Selection<'t>) -> Self {
        self.inner.selection = &raw const value.inner;
        self
    }

    // --- Extra settings --- //

    /// Specify whether to emit the palette using OSC 4 sequences.
    #[must_use]
    pub fn with_palette(mut self, value: bool) -> Self {
        self.inner.extra.palette = value;
        self
    }
    /// Specify terminal modes that differ from their defaults using CSI h/l.
    #[must_use]
    pub fn with_modes(mut self, value: bool) -> Self {
        self.inner.extra.modes = value;
        self
    }
    /// Specify whether to emit scrolling region state using DECSTBM and DECSLRM sequences.
    #[must_use]
    pub fn with_scrolling_region(mut self, value: bool) -> Self {
        self.inner.extra.scrolling_region = value;
        self
    }
    /// Specify tabstop positions by clearing all tabs and setting each one.
    #[must_use]
    pub fn with_tabstops(mut self, value: bool) -> Self {
        self.inner.extra.tabstops = value;
        self
    }
    /// Specify the present working directory using OSC 7.
    #[must_use]
    pub fn with_pwd(mut self, value: bool) -> Self {
        self.inner.extra.pwd = value;
        self
    }
    /// Specify keyboard modes such as `ModifyOtherKeys`.
    #[must_use]
    pub fn with_keyboard(mut self, value: bool) -> Self {
        self.inner.extra.keyboard = value;
        self
    }

    // --- Screen settings --- //

    /// Specify whether to emit cursor position using CUP (CSI H).
    #[must_use]
    pub fn with_cursor(mut self, value: bool) -> Self {
        self.inner.extra.screen.cursor = value;
        self
    }
    /// Emit current SGR style state based on the cursor's active `style_id`.
    #[must_use]
    pub fn with_style(mut self, value: bool) -> Self {
        self.inner.extra.screen.style = value;
        self
    }
    /// Emit current hyperlink state using OSC 8 sequences.
    #[must_use]
    pub fn with_hyperlink(mut self, value: bool) -> Self {
        self.inner.extra.screen.hyperlink = value;
        self
    }
    /// Emit character protection mode using DECSCA.
    #[must_use]
    pub fn with_protection(mut self, value: bool) -> Self {
        self.inner.extra.screen.protection = value;
        self
    }
    /// Emit Kitty keyboard protocol state using CSI > u and CSI = sequences.
    #[must_use]
    pub fn with_kitty_keyboard(mut self, value: bool) -> Self {
        self.inner.extra.screen.kitty_keyboard = value;
        self
    }
    /// Emit character set designations and invocations.
    #[must_use]
    pub fn with_charsets(mut self, value: bool) -> Self {
        self.inner.extra.screen.charsets = value;
        self
    }
}

impl<'t, 'alloc: 'cb, 'cb: 't> Formatter<'t, 'alloc, 'cb> {
    /// Create a formatter for a terminal's active screen.
    pub fn new(
        terminal: &'t Terminal<'alloc, 'cb>,
        opts: FormatterOptions<'t, '_>,
    ) -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null(), terminal, opts) }
    }

    /// Create a formatter for a terminal's active screen.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        terminal: &'t Terminal<'alloc, 'cb>,
        opts: FormatterOptions,
    ) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
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

    /// Run the formatter and stream output to a writer.
    ///
    /// Each call formats the current terminal state and invokes the writer
    /// synchronously as output becomes available. The writer may be called
    /// more than once.
    ///
    /// If an error occurs, the writer may already contain a partial formatted
    /// output. The operation cannot be resumed from that partial output. This
    /// function does not flush or make the writer's destination durable.
    ///
    /// The writer runs inside a call into libghostty, so a panic in it aborts
    /// the process.
    ///
    /// # Safety
    ///
    /// The writer must not call formatter or terminal APIs using this
    /// formatter or its terminal. The borrow checker can't rule that out,
    /// since the formatter only borrows its terminal immutably, and some
    /// terminal methods that take `&self` still change libghostty state (e.g.
    /// [`Terminal::track_grid_ref`](crate::Terminal::track_grid_ref)).
    ///
    /// # Errors
    ///
    /// Returns [`Error::IoError`] if the writer rejects output, and
    /// [`Error::LimitExceeded`] if output accounting overflows.
    pub unsafe fn format_write<W: std::io::Write>(&mut self, writer: &mut W) -> Result<()> {
        let writer = crate::io::to_writer(writer);
        // SAFETY: The writer outlives this synchronous call, and the caller
        // promises it doesn't reach this formatter or its terminal.
        from_result(unsafe { ffi::ghostty_formatter_format(self.inner.as_raw(), writer) })
    }

    /// Run the formatter and return an allocated buffer with the output.
    ///
    /// Each call formats the current terminal state. The buffer is allocated
    /// using the provided allocator (or the default allocator if `None`).
    /// Empty output returns an empty buffer.
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

        // SAFETY: On success, libghostty hands over `len` bytes allocated
        // with `alloc`, or NULL for empty output.
        Ok(unsafe { Bytes::from_raw_parts(bytes, len, alloc) })
    }

    /// Run the formatter and produce output into the caller-provided buffer.
    ///
    /// Each call formats the current terminal state. If the buffer is too small,
    /// returns `Err(Error::OutOfSpace { required })` where `required` is the
    /// required size. The caller can then retry with a larger buffer.
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

    /// Query the required buffer size for the formatted output.
    ///
    /// The result can be used to create a sufficiently large buffer
    /// for [`Formatter::format_buf`].
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
        // This should always fail with OutOfSpace.
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

/// Output format.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, int_enum::IntEnum)]
pub enum Format {
    /// Plain text (no escape sequences).
    Plain = ffi::FormatterFormat::PLAIN,
    /// VT sequences preserving colors, styles, URLs, etc.
    Vt = ffi::FormatterFormat::VT,
    /// HTML with inline styles.
    Html = ffi::FormatterFormat::HTML,
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;

    /// A writer that accepts at most one byte per write, counting calls.
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

    /// A writer that rejects everything.
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
        // SAFETY: None of the test writers touch the terminal.
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

    /// Mirrors upstream's `PageList VT without trim keeps trailing blank
    /// rows` test in `src/terminal/formatter.zig`.
    #[test]
    fn only_untrimmed_vt_keeps_trailing_blank_rows() {
        let mut terminal = Terminal::new(10, 6).unwrap();
        terminal.vt_write(b"one\r\ntwo");
        let format = |format, trim| {
            let options = FormatterOptions::new().with_format(format).with_trim(trim);
            format_buf(&mut Formatter::new(&terminal, options).unwrap())
        };

        // Six rows, two with text, so five newlines.
        assert_eq!(format(Format::Vt, false), b"one\r\ntwo\r\n\r\n\r\n\r\n");
        // Trimmed output and other formats still drop them.
        assert_eq!(format(Format::Vt, true), b"one\r\ntwo");
        assert_eq!(format(Format::Plain, false), b"one\ntwo");
    }

    /// The one-shot selection API goes through the same upstream
    /// `PageListFormatter`, so the selection's trailing blank rows are kept
    /// too, and only up to the selection's last row.
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
        // Rows 0 to 3: two with text, then two blank ones.
        let selection = Selection::new(at(0, 0), at(9, 3), false);
        let format = |format, trim| {
            let options = FormatOptions::new()
                .with_emit_format(format)
                .with_trim(trim)
                .with_selection(&selection);
            let bytes = terminal.format_selection_alloc(None, options).unwrap();
            bytes.unwrap().to_vec()
        };
        // Unlike `Formatter`, the selection API always emits upstream's
        // `.styles` extras around VT content: the whole 256-color palette as
        // OSC 4 sequences (each ending in ST) before it, and the SGR reset
        // after it. Strip them one by one, unwrapping at each step, so that
        // anything else (stray output, a missing entry) fails the test
        // instead of being silently cut off along with them.
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

        // Four rows, so three newlines: one between the text rows and one
        // for each blank row. The blank rows below the selection don't count.
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
        // The formatter stays usable afterwards.
        let mut output = Vec::new();
        format_write(&mut formatter, &mut output).unwrap();
        assert_eq!(output, b"hello");
    }

    /// A writer that accepts `chunks` writes, then rejects everything.
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
        // Far more output than libghostty buffers before handing it over.
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

        // A writer that fails after the first chunk fails the whole call,
        // having received only part of the output.
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
