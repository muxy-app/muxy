//! Encoding focus gained/lost events into terminal escape sequences
//! (CSI I / CSI O) for focus reporting mode (mode 1004).
//!
//! # Basic Usage
//!
//! Use [`Event::encode`] to encode a focus event into a caller-provided
//! buffer. If the buffer is too small, the method returns
//! `Err(Error::OutOfSpace { required })` where `required` is the required size.
//!
//! # Example
//!
//! ```rust
//! use libghostty_vt::focus::Event;
//! let mut buf = [0u8; 8];
//! if let Ok(written) = Event::Gained.encode(&mut buf) {
//!     println!("Encoded {written} bytes: {:?}", &buf[..written]);
//! }
//! ```

use crate::{
    error::{Result, from_result_with_len},
    ffi,
};

/// Event type for focus reporting mode (mode 1004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Terminal window gained focus.
    Gained,
    /// Terminal window lost focus.
    Lost,
}

impl Event {
    /// Encode a focus event into a terminal escape sequence.
    ///
    /// Encodes a focus gained (CSI I) or focus lost (CSI O)
    /// report into the provided buffer.
    ///
    /// If the buffer is too small, the method returns
    /// `Err(Error::OutOfSpace { required })` where `required` is the required size.
    /// The caller can then retry with a sufficiently sized buffer.
    pub fn encode(self, buf: &mut [u8]) -> Result<usize> {
        let mut written: usize = 0;
        let result = unsafe {
            ffi::ghostty_focus_encode(
                self.into(),
                buf.as_mut_ptr().cast(),
                buf.len(),
                &raw mut written,
            )
        };
        from_result_with_len(result, written)
    }
}

impl From<Event> for ffi::FocusEvent::Type {
    fn from(value: Event) -> Self {
        match value {
            Event::Gained => ffi::FocusEvent::GAINED,
            Event::Lost => ffi::FocusEvent::LOST,
        }
    }
}
