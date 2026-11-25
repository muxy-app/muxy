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

impl Terminal<'_, '_> {
    pub fn encode_snapshot<W: Write>(&mut self, writer: &mut W) -> Result<()> {
        let writer = crate::io::to_writer(writer);
        let result = unsafe { ffi::ghostty_snapshot_encode(self.inner.as_raw(), writer) };
        from_result(result)
    }

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
        Ok(out.map(|ptr| unsafe { Bytes::from_raw_parts(ptr, out_len, alloc) }))
    }

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

#[derive(Debug)]
pub struct Decoder<'alloc, 'r> {
    inner: Object<'alloc, ffi::SnapshotDecoderImpl>,
    _phan: PhantomData<&'r mut ffi::Reader>,
}

impl<'alloc, 'r> Decoder<'alloc, 'r> {
    pub fn new<R: Read>(r: &'r mut R) -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null(), r) }
    }

    pub fn new_with_alloc<'ctx: 'alloc, R: Read>(
        alloc: &'alloc Allocator<'ctx>,
        r: &'r mut R,
    ) -> Result<Self> {
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

    pub fn new_buf(buf: &'r [u8]) -> Result<Self> {
        unsafe { Self::new_buf_inner(std::ptr::null(), buf) }
    }

    pub fn new_buf_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        buf: &'r [u8],
    ) -> Result<Self> {
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

    pub fn decode<'cb>(self) -> Result<Terminal<'alloc, 'cb>> {
        let mut raw: ffi::Terminal = std::ptr::null_mut();
        let result =
            unsafe { ffi::ghostty_snapshot_decoder_decode(self.inner.as_raw(), &raw mut raw) };
        from_result(result)?;
        unsafe { Terminal::from_raw(raw) }
    }

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

    pub fn max_continuation_bytes(&self) -> Result<usize> {
        self.get(Data::MAX_CONTINUATION_BYTES)
    }

    pub fn set_max_continuation_bytes(&mut self, v: usize) -> Result<&mut Self> {
        self.set(Opt::MAX_CONTINUATION_BYTES, &v)?;
        Ok(self)
    }

    pub fn retain_continuation(&self) -> Result<bool> {
        self.get(Data::RETAIN_CONTINUATION)
    }

    pub fn set_retain_continuation(&mut self, value: bool) -> Result<&mut Self> {
        self.set(Opt::RETAIN_CONTINUATION, &value)?;
        Ok(self)
    }

    pub fn kitty_image_storage_limit(&self) -> Result<u64> {
        self.get(Data::KITTY_IMAGE_STORAGE_LIMIT)
    }

    pub fn set_kitty_image_storage_limit(&mut self, limit: u64) -> Result<&mut Self> {
        self.set(Opt::KITTY_IMAGE_STORAGE_LIMIT, &limit)?;
        Ok(self)
    }

    pub fn source_offset(&self) -> Result<usize> {
        self.get(Data::SOURCE_OFFSET)
    }
    pub fn history_rows_primary(&self) -> Result<u64> {
        self.get(Data::HISTORY_ROWS_PRIMARY)
    }
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

#[derive(Debug)]
pub struct IncrementalDecoder<'alloc, 'r, 'cb> {
    decoder: Decoder<'alloc, 'r>,
    terminal: Terminal<'alloc, 'cb>,
    ready_terminal: u64,
    finished: bool,
}

impl<'alloc, 'r, 'cb> IncrementalDecoder<'alloc, 'r, 'cb> {
    pub fn next<'d>(&'d mut self) -> Result<Option<Progress<'alloc, 'r, 'd>>> {
        if self.finished {
            return Ok(None);
        }
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

    #[must_use]
    pub fn terminal(&self) -> &Terminal<'alloc, 'cb> {
        &self.terminal
    }
    pub fn terminal_mut(&mut self) -> &mut Terminal<'alloc, 'cb> {
        &mut self.terminal
    }
    #[must_use]
    pub fn into_terminal(self) -> Terminal<'alloc, 'cb> {
        self.terminal
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Progress<'alloc, 'r, 'd> {
    decoder: &'d Decoder<'alloc, 'r>,
}

impl<'alloc, 'r, 'd> Progress<'alloc, 'r, 'd> {
    pub fn screen(&self) -> Result<Screen> {
        self.decoder
            .get::<ffi::TerminalScreen::Type>(Data::PROGRESS_SCREEN)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    pub fn rows(&self) -> Result<usize> {
        self.decoder.get(Data::PROGRESS_ROWS)
    }
    pub fn remaining(&self) -> Result<u32> {
        self.decoder.get(Data::PROGRESS_REMAINING)
    }

    #[must_use]
    pub fn as_decoder(self) -> &'d Decoder<'alloc, 'r> {
        self.decoder
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;

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

        *incremental.terminal_mut() = original;
        let mut rows = 0;
        while let Some(progress) = incremental.next().unwrap() {
            rows += progress.rows().unwrap();
        }
        assert!(rows > 0);
    }

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

        assert_eq!(restored.continuation_max_bytes().unwrap(), 1024);
        assert_eq!(continuation(&restored).as_deref(), Some(&b""[..]));

        restored.vt_write(b"\x1b[");
        assert_eq!(continuation(&restored).as_deref(), Some(&b"\x1b["[..]));
    }

    #[test]
    fn zero_limit_leaves_tracking_disabled() {
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

        assert_eq!(&bytes[..8], b"GHOSTSNP");
        assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 1);

        let finish = &bytes[bytes.len() - RECORD_HEADER_LEN..];
        assert_eq!(&finish[2..6], &0u32.to_le_bytes());

        assert!(Decoder::new_buf(&bytes).unwrap().decode().is_ok());
    }

    #[test]
    fn truncated_snapshot_is_invalid_value() {
        let bytes = encoded_snapshot();

        let truncated = &bytes[..bytes.len() - RECORD_HEADER_LEN];
        let result = Decoder::new_buf(truncated).unwrap().decode();
        assert!(matches!(result, Err(Error::InvalidValue)));
    }

    #[test]
    fn corrupted_record_fails_to_decode() {
        let mut bytes = encoded_snapshot();

        let mut finish_crc = bytes.clone();
        let last = finish_crc.len() - 1;
        finish_crc[last] ^= 0x01;
        assert!(Decoder::new_buf(&finish_crc).unwrap().decode().is_err());

        let payload = 10 + RECORD_HEADER_LEN;
        bytes[payload] ^= 0x01;
        assert!(Decoder::new_buf(&bytes).unwrap().decode().is_err());
    }
}
