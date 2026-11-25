use std::sync::{PoisonError, RwLock};

use crate::{error::Result, ffi};

pub type SecureRandomSource = fn(&mut [u8]) -> bool;

static RANDOM_SOURCE: RwLock<Option<SecureRandomSource>> = RwLock::new(None);

pub unsafe fn set_secure_random_source(source: Option<SecureRandomSource>) -> Result<()> {
    unsafe extern "C" fn callback(_: *mut std::ffi::c_void, ptr: *mut u8, len: usize) -> bool {
        let Some(source) = *RANDOM_SOURCE.read().unwrap_or_else(PoisonError::into_inner) else {
            return false;
        };
        let bytes: &mut [u8] = if len == 0 {
            &mut []
        } else {
            unsafe {
                ptr.write_bytes(0, len);
                std::slice::from_raw_parts_mut(ptr, len)
            }
        };
        source(bytes)
    }

    let callback: ffi::SysRandomSecureFn = source.map(|_| callback as _);
    *RANDOM_SOURCE
        .write()
        .unwrap_or_else(PoisonError::into_inner) = source;
    crate::sys_set(
        ffi::SysOption::RANDOM_SECURE,
        callback.map_or(std::ptr::null(), |f| f as *const std::ffi::c_void),
    )
}
