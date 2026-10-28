//! Encode and restore the complete state of a terminal via a binary format.
//!
//! A snapshot is an ordered, CRC-protected record stream. Its READY marker
//! follows enough state to render and resume the terminal, including any
//! unfinished VT parser input. Older scrollback pages follow READY and the
//! FINISH marker terminates the complete snapshot.
//!
//! End-of-file before an operation's required READY or FINISH marker is
//! malformed, truncated snapshot data and returns [`Error::InvalidValue`].
//! [`Error::IoError`] is reserved for reader errors.
//!
//! Decoding is done with the dedicated [`Decoder`] struct; encoding, meanwhile,
//! is supported by methods on [`Terminal`] like [`Terminal::encode_snapshot`].
//!
//! # Format
//!
//! Every integer is unsigned and little-endian.
//! The stream begins with this fixed ten-byte envelope:
//!
//! ```text
//! byte  0               8       10
//!       +---------------+--------+
//!       | "GHOSTSNP"    | version|
//!       | 8-byte magic  | u16    |
//!       +---------------+--------+
//! ```
//!
//! The envelope is followed by independently checksummed records. A record's
//! CRC32C covers its encoded tag and payload length followed by its payload;
//! it does not cover the CRC field itself.
//!
//! ```text
//! byte  0       2             6          10             10 + payload_len
//!       +-------+-------------+-----------+----------------+
//!       | tag   | payload_len | CRC32C    | payload        |
//!       | u16   | u32         | u32       | payload_len B  |
//!       +-------+-------------+-----------+----------------+
//!       \____________________/             \______________/
//!          CRC prefix                         CRC suffix
//! ```
//!
//! Record groups occur in this strict order. SCREEN and HISTORY groups contain
//! one entry for each screen declared by TERMINAL. Each manifest is followed by
//! the number of PAGE records it declares. Active SCREEN pages make the terminal
//! renderable; HISTORY pages are older scrollback ordered newest to oldest so
//! an incremental decoder can prepend them as they arrive.
//!
//! ```text
//!
//! +---------------- TERMINAL ----------------+
//! | terminal-wide state and screen count     |
//! +----------------- SCREEN -----------------+  repeated per screen
//! | active-screen manifest                   |
//! +------------------ PAGE ------------------+  repeated per manifest
//! | active screen rows                       |
//! +------------- CONTINUATION ---------------+
//! | unfinished VT/UTF-8 input, or ground     |
//! +------------------ READY -----------------+
//! | empty renderable-state marker            |  ready() returns here
//! +----------------- HISTORY ----------------+  repeated per screen
//! | scrollback manifest                      |
//! +------------------ PAGE ------------------+  next() consumes one page
//! | older screen rows                        |
//! +------------------ FINISH ----------------+
//! | empty end-of-snapshot marker             |  next() returns NO_VALUE
//! +------------------------------------------+
//! | trailing transport bytes (not consumed) |
//! +------------------------------------------+
//! ```
//!
//! READY separates the renderable prefix through CONTINUATION from history.
//! FINISH terminates the record sequence. Both are empty records protected by
//! CRC32C, like every other record. Declared record counts, tags, and strict
//! decoding enforce the stream's ordering and completeness.
//!
//! Snapshot format version 1 is a work in progress and does not yet carry a
//! binary-compatibility guarantee.
//!
//! ## See also
//!
//! [Snapshot format and Zig codec documentation](https://github.com/ghostty-org/ghostty/blob/main/src/terminal/snapshot/main.zig)
use std::{
    io::{Read, Write},
    marker::PhantomData,
    mem::MaybeUninit,
};

use crate::{
    alloc::{Allocator, Bytes, Object},
    error::{
        Error, Result, from_optional_result, from_optional_result_uninit,
        from_optional_result_with_len, from_result,
    },
    ffi::{self, SnapshotDecoderData as Data, SnapshotDecoderOption as Opt},
    screen::Screen,
    terminal::Terminal,
};

/// Snapshot-related methods.
impl Terminal<'_, '_> {
    /// Encode a complete terminal snapshot to a writer.
    ///
    /// The terminal's persistent VT stream supplies the continuation bytes
    /// needed to reconstruct unfinished parser state. The caller must prevent
    /// concurrent writes or other terminal mutation for the duration of this
    /// call. The writer callback must not call terminal APIs with the same
    /// terminal handle. A terminal can be encoded with tracking disabled when
    /// its VT parser and UTF-8 decoder are both at ground. If either is
    /// unfinished, tracking must have been enabled before the input that
    /// produced that state was written; otherwise this returns
    /// [`Error::InvalidValue`].
    ///
    /// Encoding begins at the writer's current position. If an error occurs,
    /// the writer may contain a partial snapshot without a valid FINISH
    /// marker. Calls to the writer are synchronous; this function does not
    /// flush or make the caller's destination durable.
    ///
    /// # Errors
    ///
    /// This function returns [`Error::IoError`] if the writer rejects output,
    /// [`Error::LimitExceeded`] if output accounting overflows, or another
    /// error code on failure.
    pub fn encode_snapshot<W: Write>(&mut self, writer: &mut W) -> Result<()> {
        let writer = crate::io::to_writer(writer);
        let result = unsafe { ffi::ghostty_snapshot_encode(self.inner.as_raw(), writer) };
        from_result(result)
    }

    /// Encode a complete terminal snapshot to an allocated buffer.
    ///
    /// The returned buffer is allocated with allocator, or the default
    /// allocator when allocator is `None`.
    ///
    /// A terminal can be encoded with tracking disabled when its VT parser
    /// and UTF-8 decoder are both at ground. If either is unfinished, tracking
    /// must have been enabled before the input that produced that state was
    /// written; otherwise this returns [`Error::InvalidValue`].
    pub fn encode_snapshot_alloc<'a, 'ctx: 'a>(
        &self,
        alloc: Option<&'a Allocator<'ctx>>,
    ) -> Result<Option<Bytes<'a>>> {
        let mut out = std::ptr::null_mut();
        let mut out_len = 0usize;
        let alloc = alloc.map_or(std::ptr::null(), super::alloc::Allocator::to_raw);

        let result = unsafe {
            ffi::ghostty_snapshot_encode_alloc(
                self.inner.as_raw(),
                alloc,
                &raw mut out,
                &raw mut out_len,
            )
        };

        let out = from_optional_result(result, out)?;
        // SAFETY: On success, libghostty hands over `out_len` bytes allocated
        // with `alloc`, or NULL for empty output.
        Ok(out.map(|ptr| unsafe { Bytes::from_raw_parts(ptr, out_len, alloc) }))
    }

    /// Encode a complete terminal snapshot to a caller-provided buffer.
    ///
    /// Pass an empty `buf` to query the required size. A size query returns
    /// [`Error::OutOfSpace`] with the required size, including zero when the
    /// stream is at ground. If a non-empty buffer is too small, the function
    /// has the same result and reports the full required size.
    ///
    /// A terminal can be encoded with tracking disabled when its VT parser
    /// and UTF-8 decoder are both at ground. If either is unfinished, tracking
    /// must have been enabled before the input that produced that state was
    /// written; otherwise this returns [`Error::InvalidValue`].
    pub fn encode_snapshot_buf(&self, buf: &mut [u8]) -> Result<Option<usize>> {
        let mut written = 0usize;

        let result = unsafe {
            ffi::ghostty_snapshot_encode_buf(
                self.inner.as_raw(),
                buf.as_mut_ptr(),
                buf.len(),
                &raw mut written,
            )
        };

        from_optional_result_with_len(result, written)
    }
}

/// Opaque handle to a terminal snapshot decoder.
#[derive(Debug)]
pub struct Decoder<'alloc, 'r> {
    inner: Object<'alloc, ffi::SnapshotDecoderImpl>,
    _phan: PhantomData<&'r mut ffi::Reader>,
}

impl<'alloc, 'r> Decoder<'alloc, 'r> {
    /// Create a snapshot decoder that reads from a caller-provided reader.
    ///
    /// Reads are synchronous and occur only during ready, next, or decode calls.
    /// A zero-byte successful read is permanent end-of-file, not temporary
    /// starvation; nonblocking sources must wait outside the decoder or block
    /// in their callback. Reading zero bytes before a required marker
    /// reports truncated snapshot data as [`Error::InvalidValue`].
    pub fn new<R: Read>(r: &'r mut R) -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_inner(std::ptr::null(), r) }
    }

    /// Create a new snapshot decoder that reads from a caller-provided reader
    /// with a custom allocator.
    ///
    /// Reads are synchronous and occur only during ready, next, or decode calls.
    /// A zero-byte successful read is permanent end-of-file, not temporary
    /// starvation; nonblocking sources must wait outside the decoder or block
    /// in their callback. The read callback must not call APIs on or drop the
    /// decoder that owns it. Reading zero bytes before a required marker
    /// reports truncated snapshot data as [`Error::InvalidValue`].
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_with_alloc<'ctx: 'alloc, R: Read>(
        alloc: &'alloc Allocator<'ctx>,
        r: &'r mut R,
    ) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_inner(alloc.to_raw(), r) }
    }

    unsafe fn new_inner<R: Read>(alloc: *const ffi::Allocator, r: &'r mut R) -> Result<Self> {
        let reader = crate::io::to_reader(r);
        let mut raw: ffi::SnapshotDecoder = std::ptr::null_mut();
        let result = unsafe { ffi::ghostty_snapshot_decoder_new(alloc, &raw mut raw, reader) };
        from_result(result)?;
        Ok(Self {
            inner: Object::new(raw)?,
            _phan: PhantomData,
        })
    }

    /// Create a snapshot decoder over a borrowed byte buffer.
    ///
    /// The bytes are not copied. Bytes after FINISH are not consumed;
    /// query [`Decoder::source_offset`] to locate them.
    pub fn new_buf(buf: &'r [u8]) -> Result<Self> {
        // SAFETY: A NULL allocator is always valid
        unsafe { Self::new_buf_inner(std::ptr::null(), buf) }
    }

    /// Create a new snapshot decoder over a borrowed byte buffer
    /// with a custom allocator.
    ///
    /// The bytes are not copied. Bytes after FINISH are not consumed;
    /// query [`Decoder::source_offset`] to locate them.
    ///
    /// See the [crate-level documentation](crate#memory-management-and-lifetimes)
    /// regarding custom memory management and lifetimes.
    pub fn new_buf_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        buf: &'r [u8],
    ) -> Result<Self> {
        // SAFETY: Borrow checking should forbid invalid allocators
        unsafe { Self::new_buf_inner(alloc.to_raw(), buf) }
    }

    unsafe fn new_buf_inner(alloc: *const ffi::Allocator, buf: &[u8]) -> Result<Self> {
        let mut raw: ffi::SnapshotDecoder = std::ptr::null_mut();
        let result = unsafe {
            ffi::ghostty_snapshot_decoder_new_buf(alloc, &raw mut raw, buf.as_ptr(), buf.len())
        };
        from_result(result)?;
        Ok(Self {
            inner: Object::new(raw)?,
            _phan: PhantomData,
        })
    }

    /// Decode and validate one complete snapshot.
    ///
    /// This is the one-shot form of READY followed by all history pages
    /// through FINISH. It may only be called before decoding starts. Bytes
    /// following FINISH are left unread. On success this returns a
    /// caller-owned terminal with its persistent VT stream restored.
    /// Continuation tracking on the returned terminal is disabled by default.
    /// When [`Self::set_retain_continuation`] is enabled, the decoder's
    /// maximum continuation size is applied to the terminal, and the terminal
    /// continuation APIs export the exact current continuation when that limit
    /// is nonzero. Tracking remains enabled even if the exported continuation
    /// is empty. Callers that do not need ongoing tracking must call
    /// [`Terminal::set_continuation_max_bytes`] with zero after export and
    /// before writing any post-snapshot bytes, because later input may change
    /// it.
    ///
    /// A decoding, I/O, or allocation error after input consumption begins
    /// poisons the decoder, after which it must be dropped. An invalid
    /// argument or lifecycle error detected before the operation consumes
    /// input does not poison it.    
    pub fn decode<'cb>(self) -> Result<Terminal<'alloc, 'cb>> {
        let mut raw: ffi::Terminal = std::ptr::null_mut();
        let result =
            unsafe { ffi::ghostty_snapshot_decoder_decode(self.inner.as_raw(), &raw mut raw) };
        from_result(result)?;
        unsafe { Terminal::from_raw(raw) }
    }

    /// Decode and validate the renderable snapshot prefix through READY.
    ///
    /// On success, terminal receives a caller-owned terminal with its
    /// persistent VT stream already restored from the snapshot continuation.
    /// The terminal is immediately usable for rendering and live input.
    /// Older scrollback remains to be restored with [`IncrementalDecoder::next`].
    ///
    /// The restored parser state may be unfinished. By default, terminal
    /// continuation tracking is disabled and
    /// [`Terminal::continuation_max_bytes`] returns zero.
    /// When [`Self::set_retain_continuation`] is enabled, the decoder's
    /// maximum continuation size is applied to the terminal, and the terminal
    /// continuation APIs export the exact current continuation when that limit
    /// is nonzero. Tracking remains enabled even if the exported continuation
    /// is empty. Callers that do not need ongoing tracking must call
    /// [`Terminal::set_continuation_max_bytes`] with zero after export and
    /// before writing any post-snapshot bytes, because later input may change
    /// it.
    ///
    /// A decoding, I/O, or allocation error after input consumption begins
    /// poisons the decoder, after which it must be dropped. An invalid
    /// argument or lifecycle error detected before the operation consumes
    /// input does not poison it.    
    pub fn ready<'cb>(self) -> Result<IncrementalDecoder<'alloc, 'r, 'cb>> {
        let mut raw: ffi::Terminal = std::ptr::null_mut();
        let result =
            unsafe { ffi::ghostty_snapshot_decoder_ready(self.inner.as_raw(), &raw mut raw) };
        from_result(result)?;
        let terminal = unsafe { Terminal::from_raw(raw)? };
        Ok(IncrementalDecoder {
            decoder: self,
            ready_terminal: terminal.id,
            finished: false,
            terminal,
        })
    }

    fn get<T>(&self, tag: Data::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_snapshot_decoder_get(self.inner.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_result(result)?;
        // SAFETY: Value should be initialized after successful call.
        Ok(unsafe { value.assume_init() })
    }
    fn get_optional<T>(&self, tag: Data::Type) -> Result<Option<T>> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_snapshot_decoder_get(self.inner.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_optional_result_uninit(result, value)
    }
    fn set<T>(&self, tag: Opt::Type, v: &T) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_snapshot_decoder_set(
                self.inner.as_raw(),
                tag,
                std::ptr::from_ref(v).cast(),
            )
        };
        from_result(result)
    }

    /// Current maximum accepted continuation size.
    ///
    /// This value is available in every non-failed decoder state.
    pub fn max_continuation_bytes(&self) -> Result<usize> {
        self.get(Data::MAX_CONTINUATION_BYTES)
    }

    /// Largest non-ground continuation the decoder will accept.
    ///
    /// A value of zero accepts only snapshots whose VT parser is in the ground
    /// state. The decoder default matches the largest built-in APC protocol
    /// buffer limit, currently 65 MiB.
    ///
    /// This is primarily an input validation limit. When
    /// [`Self::set_retain_continuation`] is enabled, the same value also
    /// becomes the continuation tracking limit on the returned terminal.
    pub fn set_max_continuation_bytes(&mut self, v: usize) -> Result<&mut Self> {
        self.set(Opt::MAX_CONTINUATION_BYTES, &v)?;
        Ok(self)
    }

    /// Whether decoded continuation tracking is retained on returned
    /// terminals.
    ///
    /// This value is available in every non-failed decoder state.
    pub fn retain_continuation(&self) -> Result<bool> {
        self.get(Data::RETAIN_CONTINUATION)
    }

    /// Retain the decoded continuation on the returned terminal.
    ///
    /// When true, terminals returned by [`Self::ready`] and [`Self::decode`]
    /// use [`Self::max_continuation_bytes`] as their continuation tracking
    /// limit. The existing continuation APIs such as
    /// [`Terminal::continuation_buf`] can then export the exact unfinished VT
    /// or UTF-8 input restored from the snapshot.
    ///
    /// This is false by default. A maximum continuation size of zero leaves
    /// tracking disabled. With a nonzero maximum, tracking remains enabled
    /// even when the decoded continuation is empty. Exporting an empty
    /// continuation does not disable it. Callers that do not need ongoing
    /// tracking must still call [`Terminal::set_continuation_max_bytes`] with
    /// zero after export and before writing post-snapshot input.
    pub fn set_retain_continuation(&mut self, value: bool) -> Result<&mut Self> {
        self.set(Opt::RETAIN_CONTINUATION, &value)?;
        Ok(self)
    }

    /// Number of snapshot source bytes consumed so far.
    ///
    /// At FINISH this identifies the first byte after the snapshot. Trailing
    /// bytes are not consumed. This value is unavailable after a decoding
    /// error, because the decoder can no longer guarantee its source position.
    pub fn source_offset(&self) -> Result<usize> {
        self.get(Data::SOURCE_OFFSET)
    }
    /// Advisory complete logical history extent for the primary screen.
    ///
    /// The value counts rows before the active area, including any resident
    /// overlap carried before READY. It becomes available after READY validates.
    pub fn history_rows_primary(&self) -> Result<u64> {
        self.get(Data::HISTORY_ROWS_PRIMARY)
    }
    /// Advisory complete logical history extent for the alternate screen.
    ///
    /// The value has the same semantics and lifetime as [`Decoder::history_rows_primary`]
    /// Querying it returns `Ok(None)` when the snapshot does not declare an
    /// alternate screen.
    pub fn history_rows_alternate(&self) -> Result<Option<u64>> {
        self.get_optional(Data::HISTORY_ROWS_ALTERNATE)
    }
}

impl Drop for Decoder<'_, '_> {
    fn drop(&mut self) {
        unsafe {
            ffi::ghostty_snapshot_decoder_free(self.inner.as_raw());
        }
    }
}

/// A [`Decoder`] that incrementally decodes history and appends it to the
/// terminal, obtained by calling [`Decoder::ready`].
///
/// Call [`IncrementalDecoder::next`] repeatedly until `Ok(None)` is returned
/// to keep decoding history from the snapshot.
///
/// The terminal is accessible for use during the decode process via methods
/// like [`IncrementalDecoder::terminal`] and [`IncrementalDecoder::terminal_mut`],
/// while obtaining ownership of the terminal requires halting the decode
/// process via [`IncrementalDecoder::into_terminal`].
#[derive(Debug)]
pub struct IncrementalDecoder<'alloc, 'r, 'cb> {
    // Drop order is significant here.
    // First drop the decoder, then the terminal.
    decoder: Decoder<'alloc, 'r>,
    terminal: Terminal<'alloc, 'cb>,
    // The identity of the terminal returned by READY. libghostty retains its
    // handle inside the decoder and writes history into it on every `next`
    // call, but `terminal_mut` lets safe code swap `terminal` for another one
    // (e.g. via `std::mem::replace`) and drop the original. `next` checks this
    // against `terminal` so it only lets libghostty touch the handle while we
    // own it.
    //
    // This compares `Terminal::id` rather than the handle: a replacement
    // terminal can be allocated at the freed original's address, and would
    // then receive the rest of the history.
    ready_terminal: u64,
    // Whether FINISH was reached. libghostty doesn't touch the terminal after
    // that, so the check above no longer applies.
    finished: bool,
}

impl<'alloc, 'r, 'cb> IncrementalDecoder<'alloc, 'r, 'cb> {
    /// Decode one history page into the terminal returned by READY.
    ///
    /// Each `Ok(Some(progress))` result consumes and validates one PAGE
    /// record. Query the values on the returned `progress` before
    /// calling [`IncrementalDecoder::next`] again.
    ///
    /// `Ok(None)` means FINISH was validated; repeated calls after FINISH
    /// also return `Ok(None)`.
    ///
    /// The terminal may be rendered, resized, and fed live PTY input between
    /// calls. If a history page can no longer be applied safely, it is still
    /// consumed and validated and progress reports zero rows. The decoder
    /// applies history to the terminal produced by its READY operation.
    ///
    /// If that terminal has been replaced through
    /// [`IncrementalDecoder::terminal_mut`] (e.g. with [`std::mem::replace`]),
    /// this returns [`Error::InvalidValue`] without consuming input. Unlike a
    /// decoding error, this doesn't invalidate the decoder: putting the READY
    /// terminal back allows decoding to continue. After FINISH, the terminal
    /// may be replaced freely.
    ///
    /// A decoding error invalidates the decoder's source position. The terminal
    /// remains usable with its already-restored history, but the decoder can
    /// only be dropped.
    pub fn next<'d>(&'d mut self) -> Result<Option<Progress<'alloc, 'r, 'd>>> {
        if self.finished {
            return Ok(None);
        }
        // libghostty applies history to the handle it retained at READY, not to
        // whatever terminal we hold now. If we no longer hold the READY
        // terminal, it may already have been freed, so calling into
        // libghostty would be a use-after-free.
        if self.terminal.id != self.ready_terminal {
            return Err(Error::InvalidValue);
        }
        let result = unsafe { ffi::ghostty_snapshot_decoder_next(self.decoder.inner.as_raw()) };
        self.finished = result == ffi::Result::NO_VALUE;
        from_optional_result(
            result,
            Progress {
                decoder: &self.decoder,
            },
        )
    }

    /// Return a shared reference to the terminal being decoded.
    #[must_use]
    pub fn terminal(&self) -> &Terminal<'alloc, 'cb> {
        &self.terminal
    }
    /// Return an exclusive reference to the terminal being decoded.
    ///
    /// Replacing the terminal behind this reference makes
    /// [`IncrementalDecoder::next`] fail until the original is put back.
    pub fn terminal_mut(&mut self) -> &mut Terminal<'alloc, 'cb> {
        &mut self.terminal
    }
    /// Stop decoding and obtain the final, fully decoded terminal.
    #[must_use]
    pub fn into_terminal(self) -> Terminal<'alloc, 'cb> {
        self.terminal
    }
}

/// The current progress of the decode process.
#[derive(Debug, Clone, Copy)]
pub struct Progress<'alloc, 'r, 'd> {
    decoder: &'d Decoder<'alloc, 'r>,
}

impl<'alloc, 'r, 'd> Progress<'alloc, 'r, 'd> {
    /// Screen associated with the most recently decoded history page.
    pub fn screen(&self) -> Result<Screen> {
        self.decoder
            .get::<ffi::TerminalScreen::Type>(Data::PROGRESS_SCREEN)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    /// Rows prepended by the most recently decoded history page.
    ///
    /// Zero means the page was consumed and validated but could not be
    /// applied to the live terminal.
    pub fn rows(&self) -> Result<usize> {
        self.decoder.get(Data::PROGRESS_ROWS)
    }
    /// Page records remaining in the same screen's HISTORY sequence.
    ///
    /// This is not a count of all pages remaining in the snapshot.
    pub fn remaining(&self) -> Result<u32> {
        self.decoder.get(Data::PROGRESS_REMAINING)
    }

    /// Get a reference to the underlying decoder.
    #[must_use]
    pub fn as_decoder(self) -> &'d Decoder<'alloc, 'r> {
        self.decoder
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;

    /// A snapshot of a terminal stopped in the middle of `ESC [31`.
    fn unfinished_snapshot() -> Vec<u8> {
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal.set_continuation_max_bytes(1024).unwrap();
        terminal.vt_write(b"\x1b[31");
        let mut bytes = Vec::new();
        terminal.encode_snapshot(&mut bytes).unwrap();
        bytes
    }

    fn continuation(terminal: &Terminal<'_, '_>) -> Option<Vec<u8>> {
        let mut buf = [0; 16];
        let len = terminal.continuation_buf(&mut buf).unwrap()?;
        Some(buf[..len].to_vec())
    }

    /// A snapshot of an 80x24 terminal with enough scrollback that some of it
    /// is encoded as HISTORY pages after READY, so that
    /// [`IncrementalDecoder::next`] has something to apply.
    fn snapshot_with_history() -> Vec<u8> {
        let mut terminal = Terminal::new(80, 24).unwrap();
        for i in 0..5000 {
            terminal.vt_write(format!("line {i}\r\n").as_bytes());
        }
        let mut bytes = Vec::new();
        terminal.encode_snapshot(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn next_restores_history() {
        let bytes = snapshot_with_history();
        let mut incremental = Decoder::new_buf(&bytes).unwrap().ready().unwrap();
        let mut rows = 0;
        while let Some(progress) = incremental.next().unwrap() {
            rows += progress.rows().unwrap();
        }
        assert!(rows > 0);
    }

    /// Swapping the READY terminal out of the incremental decoder and dropping
    /// it must not let [`IncrementalDecoder::next`] write into the freed
    /// terminal.
    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn next_after_swapping_out_the_terminal_is_not_a_use_after_free() {
        let bytes = snapshot_with_history();
        let alloc = crate::alloc::testing::Guard::allocator();

        let decoder = Decoder::new_buf_with_alloc(&alloc, &bytes).unwrap();
        let mut incremental = decoder.ready().unwrap();

        let original = std::mem::replace(
            incremental.terminal_mut(),
            Terminal::new_with_alloc(&alloc, 80, 24).unwrap(),
        );
        drop(original);

        // The READY terminal is gone, so libghostty must not be asked to
        // write history into it.
        assert!(matches!(incremental.next(), Err(Error::InvalidValue)));
    }

    #[test]
    fn terminal_may_be_replaced_after_finish() {
        let bytes = snapshot_with_history();
        let mut incremental = Decoder::new_buf(&bytes).unwrap().ready().unwrap();
        while incremental.next().unwrap().is_some() {}

        drop(std::mem::replace(
            incremental.terminal_mut(),
            Terminal::new(80, 24).unwrap(),
        ));
        assert!(incremental.next().unwrap().is_none());
    }

    #[test]
    fn next_resumes_once_the_ready_terminal_is_put_back() {
        let bytes = snapshot_with_history();
        let mut incremental = Decoder::new_buf(&bytes).unwrap().ready().unwrap();

        let original =
            std::mem::replace(incremental.terminal_mut(), Terminal::new(80, 24).unwrap());
        assert!(matches!(incremental.next(), Err(Error::InvalidValue)));

        // The rejected call consumed nothing, so decoding carries on as if the
        // swap never happened.
        *incremental.terminal_mut() = original;
        let mut rows = 0;
        while let Some(progress) = incremental.next().unwrap() {
            rows += progress.rows().unwrap();
        }
        assert!(rows > 0);
    }

    /// The allocator usually hands a terminal created right after the
    /// original was freed the same address. It must still be rejected,
    /// rather than receive the rest of another terminal's history.
    #[test]
    fn next_rejects_a_new_terminal_at_the_old_address() {
        let bytes = snapshot_with_history();
        let mut incremental = Decoder::new_buf(&bytes).unwrap().ready().unwrap();

        let original =
            std::mem::replace(incremental.terminal_mut(), Terminal::new(80, 24).unwrap());
        drop(original);
        *incremental.terminal_mut() = Terminal::new(80, 24).unwrap();
        assert!(matches!(incremental.next(), Err(Error::InvalidValue)));
    }

    #[test]
    fn decoded_continuation_is_not_retained_by_default() {
        let bytes = unfinished_snapshot();
        let decoder = Decoder::new_buf(&bytes).unwrap();
        assert!(!decoder.retain_continuation().unwrap());
        let restored = decoder.decode().unwrap();
        assert_eq!(restored.continuation_max_bytes().unwrap(), 0);
    }

    #[test]
    fn decoded_continuation_can_be_exported_and_resumed() {
        let bytes = unfinished_snapshot();
        let mut decoder = Decoder::new_buf(&bytes).unwrap();
        decoder
            .set_max_continuation_bytes(1024)
            .unwrap()
            .set_retain_continuation(true)
            .unwrap();
        assert!(decoder.retain_continuation().unwrap());

        let mut restored = decoder.decode().unwrap();
        assert_eq!(restored.continuation_max_bytes().unwrap(), 1024);
        assert_eq!(continuation(&restored).as_deref(), Some(&b"\x1b[31"[..]));

        // Tracking isn't needed after the export, so turn it off before
        // writing post-snapshot input. The parser state is still restored.
        restored.set_continuation_max_bytes(0).unwrap();
        restored.vt_write(b"mX");
        assert!(restored.is_vt_ground().unwrap());
        assert_eq!(restored.cursor_x().unwrap(), 1);
    }

    #[test]
    fn ready_retains_continuation_before_history_is_restored() {
        let bytes = unfinished_snapshot();
        let mut decoder = Decoder::new_buf(&bytes).unwrap();
        decoder.set_retain_continuation(true).unwrap();
        // The tracking limit is the decoder's, not the encoder's.
        let limit = decoder.max_continuation_bytes().unwrap();
        let mut incremental = decoder.ready().unwrap();
        assert_eq!(
            incremental.terminal().continuation_max_bytes().unwrap(),
            limit
        );
        assert_eq!(
            continuation(incremental.terminal()).as_deref(),
            Some(&b"\x1b[31"[..])
        );
        while incremental.next().unwrap().is_some() {}
    }

    #[test]
    fn ground_snapshot_keeps_tracking_enabled() {
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal.vt_write(b"hi");
        let mut bytes = Vec::new();
        terminal.encode_snapshot(&mut bytes).unwrap();

        let mut decoder = Decoder::new_buf(&bytes).unwrap();
        decoder
            .set_max_continuation_bytes(1024)
            .unwrap()
            .set_retain_continuation(true)
            .unwrap();
        let mut restored = decoder.decode().unwrap();

        // There was nothing unfinished to restore, but tracking is still on
        // and exports an empty continuation rather than refusing to.
        assert_eq!(restored.continuation_max_bytes().unwrap(), 1024);
        assert_eq!(continuation(&restored).as_deref(), Some(&b""[..]));

        // Exporting doesn't turn tracking off, so later input is still tracked.
        restored.vt_write(b"\x1b[");
        assert_eq!(continuation(&restored).as_deref(), Some(&b"\x1b["[..]));
    }

    #[test]
    fn zero_limit_leaves_tracking_disabled() {
        // Only snapshots at ground are accepted with a zero limit.
        let mut terminal = Terminal::new(8, 2).unwrap();
        terminal.vt_write(b"hi");
        let mut bytes = Vec::new();
        terminal.encode_snapshot(&mut bytes).unwrap();

        let mut decoder = Decoder::new_buf(&bytes).unwrap();
        decoder
            .set_max_continuation_bytes(0)
            .unwrap()
            .set_retain_continuation(true)
            .unwrap();
        let restored = decoder.decode().unwrap();
        assert_eq!(restored.continuation_max_bytes().unwrap(), 0);
    }

    /// Length of a record header: u16 tag, u32 payload length, u32 CRC32C.
    const RECORD_HEADER_LEN: usize = 10;

    fn encoded_snapshot() -> Vec<u8> {
        let mut terminal = Terminal::new(20, 5).expect("terminal should initialize");
        terminal.vt_write(b"hello\r\nworld");
        let bytes = terminal
            .encode_snapshot_alloc(None)
            .expect("snapshot should encode")
            .expect("snapshot should not be empty");
        bytes.to_vec()
    }

    #[test]
    fn finish_is_an_empty_marker_record() {
        let bytes = encoded_snapshot();

        // The envelope is "GHOSTSNP" followed by the u16 format version.
        assert_eq!(&bytes[..8], b"GHOSTSNP");
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 1);

        // FINISH is the final record and carries no payload, so the stream
        // ends with its bare header. Its declared payload length is zero.
        let finish = &bytes[bytes.len() - RECORD_HEADER_LEN..];
        assert_eq!(&finish[2..6], &0u32.to_le_bytes());

        assert!(Decoder::new_buf(&bytes).unwrap().decode().is_ok());
    }

    #[test]
    fn truncated_snapshot_is_invalid_value() {
        let bytes = encoded_snapshot();

        // Dropping FINISH means end-of-file before the required marker, which
        // the snapshot contract documents as `Error::InvalidValue`.
        let truncated = &bytes[..bytes.len() - RECORD_HEADER_LEN];
        let result = Decoder::new_buf(truncated).unwrap().decode();
        assert!(matches!(result, Err(Error::InvalidValue)));
    }

    #[test]
    fn corrupted_record_fails_to_decode() {
        let mut bytes = encoded_snapshot();

        // Every record, including the empty FINISH marker, is protected by
        // CRC32C. Flipping a bit in FINISH's checksum must be detected.
        let mut finish_crc = bytes.clone();
        let last = finish_crc.len() - 1;
        finish_crc[last] ^= 0x01;
        assert!(Decoder::new_buf(&finish_crc).unwrap().decode().is_err());

        // Likewise for a payload byte of the first (TERMINAL) record, which
        // starts right after the ten-byte envelope and its record header.
        let payload = 10 + RECORD_HEADER_LEN;
        bytes[payload] ^= 0x01;
        assert!(Decoder::new_buf(&bytes).unwrap().decode().is_err());
    }
}
