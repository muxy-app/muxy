//! Win32 pieces the server uses to start, track and stop processes.

#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod windows;
