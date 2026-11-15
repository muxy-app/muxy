use std::{ffi::c_void, io::Write};

use crate::{
    Terminal,
    error::{Result, from_result, from_result_with_len},
    ffi,
    terminal::{ClipboardLocation, ClipboardMime},
};

#[must_use]
pub fn is_safe(data: &str) -> bool {
    unsafe { ffi::ghostty_paste_is_safe(data.as_ptr().cast(), data.len()) }
}

pub fn encode(data: &mut [u8], bracketed: bool, buf: &mut [u8]) -> Result<usize> {
    let mut written = 0usize;
    let result = unsafe {
        ffi::ghostty_paste_encode(
            data.as_mut_ptr().cast(),
            data.len(),
            bracketed,
            buf.as_mut_ptr().cast(),
            buf.len(),
            &raw mut written,
        )
    };
    from_result_with_len(result, written)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
#[non_exhaustive]
pub enum Source {
    #[default]
    Clipboard = ffi::PasteSource::CLIPBOARD,
    Text = ffi::PasteSource::TEXT,
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    inner: ffi::Paste,
}

impl Options {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: ffi::Paste {
                location: ClipboardLocation::Standard.into(),
                source: Source::Clipboard.into(),
                ..ffi::sized!(ffi::Paste)
            },
        }
    }

    #[must_use]
    pub fn with_location(mut self, location: ClipboardLocation) -> Self {
        self.inner.location = location.into();
        self
    }

    #[must_use]
    pub fn with_source(mut self, source: Source) -> Self {
        self.inner.source = source.into();
        self
    }

    #[must_use]
    pub fn with_allow_unsafe(mut self, allow: bool) -> Self {
        self.inner.allow_unsafe = allow;
        self
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

impl Terminal<'_, '_> {
    pub fn paste<F>(
        &mut self,
        options: Options,
        mimes: &[ClipboardMime<'_>],
        reader: F,
    ) -> Result<bool>
    where
        F: FnOnce(&str, &mut dyn Write) -> std::io::Result<()>,
    {
        unsafe extern "C" fn read<F>(
            userdata: *mut c_void,
            mime: ffi::String,
            writer: ffi::Writer,
        ) -> bool
        where
            F: FnOnce(&str, &mut dyn Write) -> std::io::Result<()>,
        {
            let reader = unsafe { &mut *userdata.cast::<Option<F>>() };
            let Some(reader) = reader.take() else {
                return false;
            };
            let mime = unsafe { mime.to_str() };
            let mut writer = PasteWriter {
                writer,
                refused: false,
            };
            reader(mime, &mut writer).is_ok()
        }

        let mut reader = Some(reader);

        let raw = ffi::Paste {
            mimes: mimes.as_ptr().cast(),
            mimes_len: mimes.len(),
            reader: ffi::MimeReader {
                read: Some(read::<F>),
                userdata: (&raw mut reader).cast(),
            },
            ..options.inner
        };
        let mut written = false;
        from_result(unsafe {
            ffi::ghostty_terminal_paste(self.inner.as_raw(), &raw const raw, &raw mut written)
        })?;
        Ok(written)
    }
}

struct PasteWriter {
    writer: ffi::Writer,
    refused: bool,
}

impl Write for PasteWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let write = self
            .writer
            .write
            .expect("libghostty passed a writer without a write function - this is a bug!");
        if !self.refused && unsafe { write(self.writer.userdata, bytes.as_ptr(), bytes.len()) } {
            Ok(bytes.len())
        } else {
            self.refused = true;
            Err(std::io::Error::other("paste writer rejected output"))
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;
    use crate::{
        Error,
        terminal::{ClipboardReadError, Mode},
    };
    use std::cell::{Cell, RefCell};

    const TEXT: [ClipboardMime<'static>; 1] = [ClipboardMime::new("text/plain")];

    fn paste(
        terminal: &mut Terminal<'_, '_>,
        output: &RefCell<Vec<u8>>,
        options: Options,
        mimes: &[ClipboardMime<'_>],
        data: &[u8],
    ) -> (Result<bool>, Vec<String>) {
        output.borrow_mut().clear();
        let mut reads = Vec::new();
        let result = terminal.paste(options, mimes, |mime, out| {
            reads.push(mime.to_owned());
            out.write_all(data)
        });
        (result, reads)
    }

    #[test]
    fn text_paste_follows_bracketed_paste_mode() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).unwrap();
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .unwrap();
        let mimes = [
            ClipboardMime::new("image/png"),
            ClipboardMime::new("text/plain"),
        ];

        let (result, reads) = paste(&mut terminal, &output, Options::new(), &mimes, b"a\0b");
        assert!(result.unwrap());
        assert_eq!(reads, ["text/plain"]);
        assert_eq!(*output.borrow(), b"a b");

        let mimes = [
            ClipboardMime::new("image/png"),
            ClipboardMime::new("UTF8_STRING"),
            ClipboardMime::new("text/plain"),
        ];
        let (result, reads) = paste(&mut terminal, &output, Options::new(), &mimes, b"hi");
        assert!(result.unwrap());
        assert_eq!(reads, ["UTF8_STRING"]);
        assert_eq!(*output.borrow(), b"hi");

        let mime = String::from("text/plain");
        let mut same = false;
        terminal
            .paste(Options::new(), &[ClipboardMime::new(&mime)], |read, out| {
                same = std::ptr::eq(read, mime.as_str());
                out.write_all(b"hi")
            })
            .unwrap();
        assert!(same);

        let (result, _) = paste(&mut terminal, &output, Options::new(), &TEXT, b"a\nb");
        assert!(matches!(result, Err(Error::Rejected)));
        assert!(output.borrow().is_empty());
        let options = Options::new().with_allow_unsafe(true);
        let (result, _) = paste(&mut terminal, &output, options, &TEXT, b"a\nb");
        assert!(result.unwrap());
        assert_eq!(*output.borrow(), b"a\rb");

        assert!(!is_safe("a\rb"));
        let (result, _) = paste(&mut terminal, &output, Options::new(), &TEXT, b"a\rb");
        assert!(matches!(result, Err(Error::Rejected)));
        assert!(output.borrow().is_empty());
        let (result, _) = paste(&mut terminal, &output, options, &TEXT, b"a\rb");
        assert!(result.unwrap());
        assert_eq!(*output.borrow(), b"a\rb");

        terminal.set_mode(Mode::BRACKETED_PASTE, true).unwrap();
        let (result, _) = paste(&mut terminal, &output, Options::new(), &TEXT, b"a\nb");
        assert!(result.unwrap());
        assert_eq!(*output.borrow(), b"\x1b[200~a\nb\x1b[201~");
        let (result, _) = paste(&mut terminal, &output, Options::new(), &TEXT, b"a\rb");
        assert!(result.unwrap());
        assert_eq!(*output.borrow(), b"\x1b[200~a\rb\x1b[201~");
        let (result, _) = paste(
            &mut terminal,
            &output,
            Options::new(),
            &TEXT,
            b"a\x1b[201~b",
        );
        assert!(matches!(result, Err(Error::Rejected)));
        assert!(output.borrow().is_empty());
    }

    #[test]
    fn nothing_to_paste_and_failures_write_nothing() {
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new(80, 24).unwrap();

        let (result, reads) = paste(&mut terminal, &output, Options::new(), &TEXT, b"hi");
        assert!(matches!(result, Err(Error::InvalidValue)));
        assert!(reads.is_empty());

        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .unwrap();

        let (result, reads) = paste(&mut terminal, &output, Options::new(), &[], b"");
        assert!(!result.unwrap());
        assert!(reads.is_empty());
        let png = [ClipboardMime::new("image/png")];
        let (result, reads) = paste(&mut terminal, &output, Options::new(), &png, b"png");
        assert!(!result.unwrap());
        assert!(reads.is_empty());
        let (result, _) = paste(&mut terminal, &output, Options::new(), &TEXT, b"");
        assert!(!result.unwrap());

        output.borrow_mut().clear();
        let result = terminal.paste(Options::new(), &TEXT, |_mime, out| {
            out.write_all(b"partial")?;
            Err(std::io::Error::other("clipboard went away"))
        });
        assert!(matches!(result, Err(Error::IoError)));
        assert!(output.borrow().is_empty());
    }

    #[test]
    fn refused_writes_fail_the_paste_even_if_ignored() {
        let capped = crate::alloc::testing::Capped::new(1024 * 1024);
        let alloc = capped.allocator();
        let output = RefCell::new(Vec::new());
        let mut terminal = Terminal::new_with_alloc(&alloc, 80, 24).unwrap();
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .unwrap();

        let after_refused = Cell::new(false);
        let result = terminal.paste(Options::new(), &TEXT, |_mime, out| {
            out.write_all(b"before ")?;
            let _ = out.write_all(&vec![b'x'; 4 * 1024 * 1024]);
            after_refused.set(out.write_all(b"after").is_err());
            Ok(())
        });
        assert!(after_refused.get());
        assert!(matches!(result, Err(Error::OutOfMemory)));
        assert!(output.borrow().is_empty());
    }

    fn event_password(output: &[u8]) -> String {
        let output = std::str::from_utf8(output).unwrap();
        let start = output.find(":pw=").expect("paste event carries a password") + 4;
        let len = output[start..].find(['\x1b', ';', ':']).unwrap();
        output[start..start + len].to_owned()
    }

    #[test]
    fn kitty_paste_events_replace_user_pastes() {
        let output = RefCell::new(Vec::new());
        let granted = RefCell::new(Vec::new());
        let reads = Cell::new(0);
        let mut terminal = Terminal::new(80, 24).unwrap();
        terminal
            .on_pty_write(|_term, bytes| output.borrow_mut().extend_from_slice(bytes))
            .unwrap();

        terminal.vt_write(b"\x1b[?5522h");
        assert!(terminal.mode(Mode::PASTE_EVENTS).unwrap());
        let (result, _) = paste(&mut terminal, &output, Options::new(), &TEXT, b"hi");
        assert!(result.unwrap());
        assert_eq!(*output.borrow(), b"hi");

        terminal
            .on_clipboard_read(|_term, request| {
                granted.borrow_mut().push(request.granted());
                request.reply(Err(ClipboardReadError::Denied), &[], false);
            })
            .unwrap();
        let mimes = [
            ClipboardMime::new("text/plain"),
            ClipboardMime::new("image/png"),
        ];
        output.borrow_mut().clear();
        let result = terminal.paste(Options::new(), &mimes, |_mime, _out| {
            reads.set(reads.get() + 1);
            Ok(())
        });
        assert!(result.unwrap());
        assert_eq!(reads.get(), 0);
        let event = output.borrow().clone();
        let pw = event_password(&event);
        assert_eq!(
            event,
            format!(
                concat!(
                    "\x1b]5522;type=read:status=OK:pw={pw}\x1b\\",
                    "\x1b]5522;type=read:status=DATA:mime=Lg==:pw={pw};dGV4dC9wbGFpbiBpbWFnZS9wbmcK\x1b\\",
                    "\x1b]5522;type=read:status=DONE:pw={pw}\x1b\\",
                ),
                pw = pw
            )
            .into_bytes()
        );

        let read = |name: &str, pw: &str| {
            format!("\x1b]5522;type=read:id=r{name}:pw={pw};dGV4dC9wbGFpbg==\x1b\\")
        };
        terminal.vt_write(read("", &pw).as_bytes());
        terminal.vt_write(read(":name=YXBw", "b3RoZXI=").as_bytes());
        terminal.vt_write(read(":name=YXBw", &pw).as_bytes());
        terminal.vt_write(read(":name=YXBw", &pw).as_bytes());
        assert_eq!(*granted.borrow(), [false, false, true, false]);

        let (result, reads) = paste(&mut terminal, &output, Options::new(), &TEXT, b"a\n");
        assert!(result.unwrap());
        assert!(reads.is_empty());
        assert!(
            output
                .borrow()
                .starts_with(b"\x1b]5522;type=read:status=OK:pw=")
        );

        let options = Options::new().with_location(ClipboardLocation::Selection);
        let (result, _) = paste(&mut terminal, &output, options, &TEXT, b"hi");
        assert!(result.unwrap());
        assert!(
            output
                .borrow()
                .starts_with(b"\x1b]5522;type=read:status=OK:loc=primary:")
        );

        let options = Options::new().with_source(Source::Text);
        let (result, reads) = paste(&mut terminal, &output, options, &TEXT, b"hi");
        assert!(result.unwrap());
        assert_eq!(reads, ["text/plain"]);
        assert_eq!(*output.borrow(), b"hi");
    }
}
