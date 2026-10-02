//! Utilities for validating paste data safety.
//!
//! # Example
//!
//! ## Safety Check
//!
//! ```rust
//! use libghostty_vt::paste;
//!
//! let safe_data = "hello world";
//! let unsafe_data = "rm -rf /\n";
//!
//! if paste::is_safe(safe_data) {
//!     println!("Safe to paste");
//! }
//!
//! if !paste::is_safe(unsafe_data) {
//!     println!("Unsafe! Contains newline");
//! }
//! ```
//!
//! ## Encoding
//!
//! ```rust
//! use libghostty_vt::paste;
//!
//! let mut data = *b"hello\nworld";
//! let mut buf = [0u8; 64];
//!
//! if let Ok(len) = paste::encode(&mut data, true, &mut buf) {
//!     println!("Encoded {len} bytes: {}", buf[..len].escape_ascii());
//! }
//! ```

use crate::{
    error::{Result, from_result_with_len},
    ffi,
};

/// Check if paste data is safe to paste into the terminal.
///
/// Data is considered unsafe if it contains:
///   * Newlines (`\n`) which can inject commands
///   * The bracketed paste end sequence (`\x1b[201~`) which can be used to exit bracketed paste
///     mode and inject commands
///
/// This check is conservative and considers data unsafe regardless of current terminal state.
#[must_use]
pub fn is_safe(data: &str) -> bool {
    unsafe { ffi::ghostty_paste_is_safe(data.as_ptr().cast(), data.len()) }
}

/// Encode paste data for writing to the terminal pty.
///
/// This function prepares paste data for terminal input by:
///
/// - Stripping unsafe control bytes (NUL, ESC, DEL, etc.) by replacing them
///   with spaces
/// - Wrapping the data in bracketed paste sequences if `bracketed` is true
/// - Replacing newlines with carriage returns if `bracketed` is false
///
/// The input `data` buffer is modified in place during encoding. The encoded
/// result (potentially with bracketed paste prefix/suffix) is written to the
/// output buffer.
///
/// If the output buffer is too small, the function returns
/// `Err(Error::OutOfSpace { required })` where `required` is the required
/// The caller can then retry with a sufficiently sized buffer.
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
