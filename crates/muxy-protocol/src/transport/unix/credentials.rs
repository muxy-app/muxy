use std::io;

use super::LocalStream;

pub(super) fn peer_uid(stream: &LocalStream) -> io::Result<u32> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        Ok(rustix::net::sockopt::socket_peercred(stream)?.uid.as_raw())
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        use std::os::fd::AsRawFd;
        let (mut uid, mut gid) = (0, 0);
        if unsafe { libc::getpeereid(stream.as_raw_fd(), &raw mut uid, &raw mut gid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(uid)
    }
}

pub(super) fn own_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}
