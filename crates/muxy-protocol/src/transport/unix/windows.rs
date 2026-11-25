use std::fs::OpenOptions;
use std::io;
use std::mem;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, AsRawSocket, AsSocket, BorrowedSocket};
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

use super::{LocalListener, LocalStream};

#[derive(Debug)]
pub(super) struct Pollable(LocalListener);

impl From<LocalListener> for Pollable {
    fn from(listener: LocalListener) -> Self {
        Self(listener)
    }
}

impl Pollable {
    pub(super) fn accept(&self) -> io::Result<(LocalStream, uds_windows::SocketAddr)> {
        self.0.accept()
    }
}

impl AsSocket for Pollable {
    fn as_socket(&self) -> BorrowedSocket<'_> {
        unsafe { BorrowedSocket::borrow_raw(self.0.as_raw_socket()) }
    }
}

pub(super) fn peer_pid(socket: &impl AsRawSocket) -> io::Result<u32> {
    let socket = SOCKET::try_from(socket.as_raw_socket()).map_err(io::Error::other)?;
    let mut pid = 0_u32;
    let mut returned = 0_u32;
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
        return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
    }
    Ok(pid)
}

pub(super) fn is_socket(path: &Path) -> io::Result<bool> {
    let file = OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let mut info: FILE_ATTRIBUTE_TAG_INFO = unsafe { mem::zeroed() };
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

pub(super) fn interrupt(socket: &impl AsRawSocket) {
    let Ok(handle) = usize::try_from(socket.as_raw_socket()) else {
        return;
    };
    unsafe {
        CancelIoEx(handle as _, ptr::null());
    }
}
