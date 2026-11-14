//! The shell's own handlers: the default app for a file or address, and
//! File Explorer with a file selected.

use std::ffi::OsStr;
use std::io;
use std::path::Path;

use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, w};

/// Shell handlers may be COM objects, so each call runs with COM started on
/// its thread. It is stopped again only if this started it.
struct Com(bool);

impl Com {
    fn start() -> Self {
        // SAFETY: a successful call is balanced by `CoUninitialize` when
        // dropped, on this thread.
        let started =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Self(started.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: matches the successful `CoInitializeEx` in `start`.
            unsafe { CoUninitialize() };
        }
    }
}

/// Opens `target`, a file or an address, with what the user chose for it.
pub(super) fn open(target: &OsStr) -> io::Result<()> {
    let _com = Com::start();
    // SAFETY: the strings are valid and NUL-terminated for the call.
    let instance = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &HSTRING::from(target),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    // Values up to 32 are errors, as the documentation says.
    if instance.0 as usize <= 32 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Shows `file` selected in its folder.
pub(super) fn select(file: &Path) -> io::Result<()> {
    let _com = Com::start();
    let mut item: *mut ITEMIDLIST = std::ptr::null_mut();
    // SAFETY: `item` receives an ID list the shell allocates, freed below.
    unsafe { SHParseDisplayName(&HSTRING::from(file), None, &raw mut item, 0, None) }
        .map_err(io::Error::other)?;
    // SAFETY: with no items listed, `item` itself is selected in its folder.
    let shown = unsafe { SHOpenFolderAndSelectItems(item, None, 0) };
    // SAFETY: `item` came from `SHParseDisplayName` and is not used again.
    unsafe { CoTaskMemFree(Some(item.cast_const().cast())) };
    shown.map_err(io::Error::other)
}
