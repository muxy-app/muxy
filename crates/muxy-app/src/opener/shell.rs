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

struct Com(bool);

impl Com {
    fn start() -> Self {
        let started =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Self(started.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub(super) fn open(target: &OsStr) -> io::Result<()> {
    let _com = Com::start();
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
    if instance.0 as usize <= 32 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn select(file: &Path) -> io::Result<()> {
    let _com = Com::start();
    let mut item: *mut ITEMIDLIST = std::ptr::null_mut();
    unsafe { SHParseDisplayName(&HSTRING::from(file), None, &raw mut item, 0, None) }
        .map_err(io::Error::other)?;
    let shown = unsafe { SHOpenFolderAndSelectItems(item, None, 0) };
    unsafe { CoTaskMemFree(Some(item.cast_const().cast())) };
    shown.map_err(io::Error::other)
}
