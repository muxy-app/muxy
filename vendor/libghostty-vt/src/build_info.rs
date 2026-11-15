use std::mem::MaybeUninit;

use crate::{
    error::{Error, Result, from_result},
    ffi::{self, BuildInfo as Info},
};

pub fn supports_simd() -> Result<bool> {
    build_info(Info::SIMD)
}

pub fn supports_kitty_graphics() -> Result<bool> {
    build_info(Info::KITTY_GRAPHICS)
}

pub fn supports_tmux_control_mode() -> Result<bool> {
    build_info(Info::TMUX_CONTROL_MODE)
}

pub fn optimize_mode() -> Result<OptimizeMode> {
    build_info::<ffi::OptimizeMode::Type>(Info::OPTIMIZE)
        .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
}

pub fn version_string() -> Result<&'static [u8]> {
    build_string(Info::VERSION_STRING)
}
pub fn major_version() -> Result<usize> {
    build_info(Info::VERSION_MAJOR)
}
pub fn minor_version() -> Result<usize> {
    build_info(Info::VERSION_MINOR)
}
pub fn patch_version() -> Result<usize> {
    build_info(Info::VERSION_PATCH)
}
pub fn pre_version() -> Result<&'static [u8]> {
    build_string(Info::VERSION_PRE)
}
pub fn build_version() -> Result<&'static [u8]> {
    build_string(Info::VERSION_BUILD)
}

fn build_string(tag: ffi::BuildInfo::Type) -> Result<&'static [u8]> {
    build_info::<ffi::String>(tag)
        .map(|s| unsafe { s.to_bytes() })
}

fn build_info<T>(tag: ffi::BuildInfo::Type) -> Result<T> {
    let mut value = MaybeUninit::zeroed();
    let result = unsafe { ffi::ghostty_build_info(tag, std::ptr::from_mut(&mut value).cast()) };
    from_result(result)?;
    Ok(unsafe { value.assume_init() })
}

#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, int_enum::IntEnum)]
pub enum OptimizeMode {
    Debug = ffi::OptimizeMode::DEBUG,
    ReleaseSafe = ffi::OptimizeMode::RELEASE_SAFE,
    ReleaseSmall = ffi::OptimizeMode::RELEASE_SMALL,
    ReleaseFast = ffi::OptimizeMode::RELEASE_FAST,
}
