//! File calls relative to an open directory, so a path can't be redirected
//! between checking it and using it: `openat` and friends on Unix, and
//! `NtCreateFile` with a root directory on Windows.

#[cfg(unix)]
mod unix;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

#[cfg(unix)]
pub(super) use unix::*;
#[cfg(windows)]
pub(super) use windows::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    File,
    Directory,
    /// A symbolic link, or on Windows also a junction or mount point.
    Link,
    /// A device, socket or FIFO. Windows reports those as files.
    #[cfg_attr(windows, allow(dead_code))]
    Other,
}

/// What [`stat`] reads about a name without following it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Entry {
    pub(super) kind: Kind,
    pub(super) size: u64,
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    pub(super) device: u64,
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    pub(super) index: u64,
    #[cfg(unix)]
    pub(super) mode: u32,
}
