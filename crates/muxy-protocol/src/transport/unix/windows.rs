//! The Win32 calls behind local sockets on Windows.

use std::fs::OpenOptions;
use std::io;
use std::mem;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, AsRawSocket};
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Networking::WinSock::{
    SIO_AF_UNIX_GETPEERPID, SOCKET, SOCKET_ERROR, WSAGetLastError, WSAIoctl,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FileAttributeTagInfo,
    GetFileInformationByHandleEx,
};
use windows_sys::Win32::System::IO::CancelIoEx;
use windows_sys::Win32::System::SystemServices::IO_REPARSE_TAG_AF_UNIX;

/// The process ID of whoever holds the other end of a connected socket.
pub(super) fn peer_pid(socket: &impl AsRawSocket) -> io::Result<u32> {
    let socket = SOCKET::try_from(socket.as_raw_socket()).map_err(io::Error::other)?;
    let mut pid = 0_u32;
    let mut returned = 0_u32;
    // SAFETY: the output buffer is a live `u32` of the size passed, and the
    // call is synchronous because it has no overlapped structure.
    let result = unsafe {
        WSAIoctl(
            socket,
            SIO_AF_UNIX_GETPEERPID,
            ptr::null(),
            0,
            ptr::from_mut(&mut pid).cast(),
            4,
            &raw mut returned,
            ptr::null_mut(),
            None,
        )
    };
    if result == SOCKET_ERROR {
        // SAFETY: reads this thread's last Winsock error.
        return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
    }
    Ok(pid)
}

/// Whether `path` itself, not a link's target, is an `AF_UNIX` socket. Windows
/// stores one as a reparse point, which std reports as a regular file.
pub(super) fn is_socket(path: &Path) -> io::Result<bool> {
    let file = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    // SAFETY: `FILE_ATTRIBUTE_TAG_INFO` is plain data, valid when zeroed.
    let mut info: FILE_ATTRIBUTE_TAG_INFO = unsafe { mem::zeroed() };
    // SAFETY: the handle is open, and the buffer is a live structure of the
    // size passed for this information class.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileAttributeTagInfo,
            ptr::from_mut(&mut info).cast(),
            u32::try_from(size_of::<FILE_ATTRIBUTE_TAG_INFO>()).map_err(io::Error::other)?,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        && info.ReparseTag == IO_REPARSE_TAG_AF_UNIX)
}

/// Wakes every read and write blocked on the socket, from any thread or
/// clone. Winsock's `shutdown` does not wake them on its own.
pub(super) fn interrupt(socket: &impl AsRawSocket) {
    let Ok(handle) = usize::try_from(socket.as_raw_socket()) else {
        return;
    };
    // SAFETY: cancelling I/O on a socket handle that is still open only
    // completes its pending operations early.
    unsafe {
        CancelIoEx(handle as _, ptr::null());
    }
}
