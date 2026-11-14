//! Which user holds the other end of a local socket on Unix, as the kernel
//! recorded it when the socket connected.

use std::io;

use super::LocalStream;

/// The effective user ID of the process at the other end of `stream`.
pub(super) fn peer_uid(stream: &LocalStream) -> io::Result<u32> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        Ok(rustix::net::sockopt::socket_peercred(stream)?.uid.as_raw())
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        use std::os::fd::AsRawFd;
        let (mut uid, mut gid) = (0, 0);
        // SAFETY: the descriptor stays open while `stream` is borrowed, and
        // both out-pointers refer to live values.
        if unsafe { libc::getpeereid(stream.as_raw_fd(), &raw mut uid, &raw mut gid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(uid)
    }
}

/// The effective user ID of this process.
pub(super) fn own_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}
