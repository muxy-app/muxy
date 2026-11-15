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
    Link,
    #[cfg_attr(windows, allow(dead_code))]
    Other,
}

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
