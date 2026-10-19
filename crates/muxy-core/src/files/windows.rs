//! Win32 security and file identity calls behind the helpers in `files`.

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_SUCCESS, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW, SDDL_REVISION_1,
    SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetTokenInformation,
    PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Memory that Win32 allocated with `LocalAlloc`.
struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: the pointer came from a Win32 call that hands ownership to the caller.
        unsafe { LocalFree(self.0) };
    }
}

/// Gives `directory` a protected DACL for the user and SYSTEM that
/// everything below it inherits. Setting it walks every file below, so a
/// directory that already has it is left alone.
pub(super) fn protect_directory(directory: &Path) -> io::Result<()> {
    let wanted = private_descriptor()?;
    if same_protected_entries(&dacl(directory)?, &sddl(wanted.0)?) {
        return Ok(());
    }
    let (mut present, mut defaulted, mut acl) = (0, 0, ptr::null_mut());
    // SAFETY: `wanted` is a valid descriptor and the out pointers are writable.
    if unsafe {
        GetSecurityDescriptorDacl(wanted.0, &raw mut present, &raw mut acl, &raw mut defaulted)
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // A missing DACL would be set as a NULL DACL, which grants everyone access.
    if present == 0 || acl.is_null() {
        return Err(io::Error::other(
            "The private security descriptor has no DACL",
        ));
    }
    let path = wide(directory)?;
    // SAFETY: `path` is NUL-terminated and `acl` lives inside `wanted`.
    let status = unsafe {
        SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            acl,
            ptr::null(),
        )
    };
    win32_result(status)
}

fn private_descriptor() -> io::Result<Local> {
    security_descriptor(&format!(
        "D:P(A;OICI;FA;;;{})(A;OICI;FA;;;SY)",
        current_user_sid()?
    ))
}

/// Whether `current` is protected and grants exactly what `wanted` grants.
fn same_protected_entries(current: &str, wanted: &str) -> bool {
    matches!(
        (flags_and_entries(current), flags_and_entries(wanted)),
        (Some((flags, entries)), Some((_, wanted))) if flags.contains('P') && entries == wanted
    )
}

/// Splits a DACL in SDDL, such as `D:PAI(A;…)`, into its flags and its entries.
fn flags_and_entries(sddl: &str) -> Option<(&str, &str)> {
    let rest = sddl.strip_prefix("D:")?;
    Some(rest.split_at(rest.find('(')?))
}

/// The DACL of `path` in SDDL.
pub(super) fn dacl(path: &Path) -> io::Result<String> {
    let path = wide(path)?;
    let mut descriptor = ptr::null_mut();
    // SAFETY: `path` is NUL-terminated; only the descriptor is requested.
    let status = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &raw mut descriptor,
        )
    };
    win32_result(status)?;
    let descriptor = Local(descriptor);
    sddl(descriptor.0)
}

pub(super) fn current_user_sid() -> io::Result<String> {
    let mut token = ptr::null_mut();
    // SAFETY: the pseudo handle needs no closing and `token` is writable.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `OpenProcessToken` returned a handle we now own.
    let token = unsafe { OwnedHandle::from_raw_handle(token) };
    let mut length = 0;
    // SAFETY: a null buffer of length 0 only asks for the required length.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            ptr::null_mut(),
            0,
            &raw mut length,
        );
    }
    // `u64`s keep the buffer aligned for `TOKEN_USER`.
    let mut buffer = vec![0_u64; (length as usize).div_ceil(8)];
    // SAFETY: `buffer` holds at least `length` writable bytes.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &raw mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call filled `buffer` with a `TOKEN_USER` whose SID points into it.
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut text = ptr::null_mut();
    // SAFETY: `sid` is valid while `buffer` lives and `text` is writable.
    if unsafe { ConvertSidToStringSidW(sid, &raw mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call returned a NUL-terminated string we now own.
    Ok(unsafe { take_wide(text) })
}

/// `sid` as SDDL writes it, which is an alias such as `LA` for some accounts.
#[cfg(test)]
pub(super) fn sddl_trustee(sid: &str) -> io::Result<String> {
    let entry = sddl(security_descriptor(&format!("D:(A;;FA;;;{sid})"))?.0)?;
    entry
        .strip_prefix("D:(A;;FA;;;")
        .and_then(|trustee| trustee.strip_suffix(')'))
        .map(str::to_owned)
        .ok_or_else(|| io::Error::other(format!("Unexpected SDDL {entry}")))
}

pub(super) fn file_id(file: &File) -> io::Result<(u64, u64)> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle is open for the borrow and `information` is writable.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &raw mut information) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((
        u64::from(information.dwVolumeSerialNumber),
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    ))
}

fn security_descriptor(sddl: &str) -> io::Result<Local> {
    let sddl: Vec<u16> = sddl.encode_utf16().chain(iter::once(0)).collect();
    let mut descriptor = ptr::null_mut();
    // SAFETY: `sddl` is NUL-terminated and `descriptor` is writable.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &raw mut descriptor,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(Local(descriptor))
}

fn sddl(descriptor: *mut c_void) -> io::Result<String> {
    let mut text = ptr::null_mut();
    // SAFETY: `descriptor` is valid for the call and `text` is writable.
    if unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &raw mut text,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call returned a NUL-terminated string we now own.
    Ok(unsafe { take_wide(text) })
}

/// Reads and frees a NUL-terminated string from `LocalAlloc`.
///
/// # Safety
///
/// `text` must be a NUL-terminated string allocated with `LocalAlloc`.
unsafe fn take_wide(text: *mut u16) -> String {
    let _owned = Local(text.cast());
    let mut length = 0;
    // SAFETY: the caller promises a NUL terminator, so reads stop inside the string.
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: `length` characters before the terminator are readable.
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) })
}

fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    if wide[..wide.len() - 1].contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Paths must not contain NUL characters",
        ));
    }
    Ok(wide)
}

fn win32_result(status: u32) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status.cast_signed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_protected_dacl_with_the_same_entries_matches() {
        let wanted = "D:P(A;OICI;FA;;;S-1-5-21-1)(A;OICI;FA;;;SY)";
        assert!(same_protected_entries(wanted, wanted));
        assert!(same_protected_entries(
            "D:PAI(A;OICI;FA;;;S-1-5-21-1)(A;OICI;FA;;;SY)",
            wanted
        ));
        assert!(!same_protected_entries(
            "D:AI(A;OICI;FA;;;S-1-5-21-1)(A;OICI;FA;;;SY)",
            wanted
        ));
        assert!(!same_protected_entries(
            "D:P(A;OICI;FA;;;S-1-5-21-1)(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)",
            wanted
        ));
        assert!(!same_protected_entries("D:P", wanted));
    }

    #[test]
    fn trustees_are_written_as_sddl_writes_them() -> io::Result<()> {
        assert_eq!(sddl_trustee("S-1-5-32-544")?, "BA");
        assert_eq!(sddl_trustee("S-1-5-18")?, "SY");
        assert_eq!(sddl_trustee("S-1-5-21-1-2-3-1001")?, "S-1-5-21-1-2-3-1001");
        Ok(())
    }

    #[test]
    fn a_protected_directory_is_recognized_and_left_alone() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let wanted = sddl(private_descriptor()?.0)?;
        assert!(!same_protected_entries(&dacl(directory.path())?, &wanted));
        protect_directory(directory.path())?;
        assert!(same_protected_entries(&dacl(directory.path())?, &wanted));
        Ok(())
    }
}
