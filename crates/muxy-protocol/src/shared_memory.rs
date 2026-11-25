use std::io;

const PREFIX: &str = "/mx";
const MAX_NAME: usize = 30;

pub fn is_region_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(PREFIX) else {
        return false;
    };
    let mut parts = rest.split('-');
    name.len() <= MAX_NAME
        && parts.next().is_some_and(is_hex)
        && parts.next().is_some_and(is_hex)
        && parts.next().is_none()
}

fn is_hex(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub const fn supported() -> bool {
    cfg!(any(target_os = "macos", target_os = "linux"))
}

#[derive(Debug)]
pub struct Region {
    name: String,
}

impl Region {
    pub fn create(pixels: &[u8]) -> io::Result<Self> {
        let name = next_name();
        platform::create(&name, pixels)?;
        Ok(Self { name })
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for Region {
    fn drop(&mut self) {
        platform::unlink(&self.name);
    }
}

pub fn take<T>(name: &str, len: usize, read: impl FnOnce(&[u8]) -> T) -> io::Result<T> {
    if !is_region_name(name) || len == 0 {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    platform::take(name, len, read)
}

fn next_name() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{PREFIX}{:x}-{:x}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[allow(unsafe_code, reason = "mapping shared memory")]
mod platform {
    use std::ffi::c_void;
    use std::io;
    use std::os::fd::OwnedFd;

    use rustix::fs::Mode;
    use rustix::mm::{MapFlags, ProtFlags};
    use rustix::shm;

    struct Mapping {
        address: *mut c_void,
        len: usize,
    }

    impl Mapping {
        fn new(fd: &OwnedFd, len: usize, prot: ProtFlags) -> io::Result<Self> {
            let address = unsafe {
                rustix::mm::mmap(std::ptr::null_mut(), len, prot, MapFlags::SHARED, fd, 0)
            }?;
            Ok(Self { address, len })
        }

        fn bytes(&self) -> &[u8] {
            unsafe { std::slice::from_raw_parts(self.address.cast::<u8>(), self.len) }
        }

        fn bytes_mut(&mut self) -> &mut [u8] {
            unsafe { std::slice::from_raw_parts_mut(self.address.cast::<u8>(), self.len) }
        }
    }

    impl Drop for Mapping {
        fn drop(&mut self) {
            let _ = unsafe { rustix::mm::munmap(self.address, self.len) };
        }
    }

    pub(super) fn create(name: &str, pixels: &[u8]) -> io::Result<()> {
        if pixels.is_empty() {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        let flags = shm::OFlags::CREATE | shm::OFlags::EXCL | shm::OFlags::RDWR;
        let mode = Mode::RUSR | Mode::WUSR;
        let fd = match shm::open(name, flags, mode) {
            Err(rustix::io::Errno::EXIST) => {
                unlink(name);
                shm::open(name, flags, mode)?
            }
            opened => opened?,
        };
        let written = (|| {
            reserve(&fd, pixels.len() as u64)?;
            let mut mapping = Mapping::new(&fd, pixels.len(), ProtFlags::READ | ProtFlags::WRITE)?;
            mapping.bytes_mut().copy_from_slice(pixels);
            Ok(())
        })();
        if written.is_err() {
            unlink(name);
        }
        written
    }

    pub(super) fn take<T>(name: &str, len: usize, read: impl FnOnce(&[u8]) -> T) -> io::Result<T> {
        let opened = shm::open(name, shm::OFlags::RDONLY, Mode::empty());
        unlink(name);
        let fd = opened?;
        let size = rustix::fs::fstat(&fd)?.st_size;
        if u64::try_from(size).map_err(io::Error::other)? < len as u64 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        let mapping = Mapping::new(&fd, len, ProtFlags::READ)?;
        Ok(read(mapping.bytes()))
    }

    pub(super) fn unlink(name: &str) {
        let _ = shm::unlink(name);
    }

    #[cfg(target_os = "linux")]
    fn reserve(fd: &OwnedFd, len: u64) -> io::Result<()> {
        rustix::fs::fallocate(fd, rustix::fs::FallocateFlags::empty(), 0, len)?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn reserve(fd: &OwnedFd, len: u64) -> io::Result<()> {
        rustix::fs::ftruncate(fd, len)?;
        Ok(())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use std::io;

    pub(super) fn create(_name: &str, _pixels: &[u8]) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn take<T>(
        _name: &str,
        _len: usize,
        _read: impl FnOnce(&[u8]) -> T,
    ) -> io::Result<T> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn unlink(_name: &str) {}
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use super::*;

    #[test]
    fn a_region_is_read_once_by_name_and_then_gone() -> io::Result<()> {
        let pixels: Vec<u8> = (0..=255).cycle().take(4096 * 3 + 7).collect();
        let region = Region::create(&pixels)?;
        assert!(is_region_name(region.name()));
        let read = take(region.name(), pixels.len(), <[u8]>::to_vec)?;
        assert_eq!(read, pixels);
        assert!(take(region.name(), pixels.len(), <[u8]>::len).is_err());
        Ok(())
    }

    #[test]
    fn a_region_dropped_before_it_is_read_is_gone() -> io::Result<()> {
        let region = Region::create(&[1, 2, 3, 4])?;
        let name = region.name().to_owned();
        drop(region);
        assert!(take(&name, 4, <[u8]>::len).is_err());
        Ok(())
    }

    #[test]
    fn a_reader_cannot_map_past_the_end_of_a_region() -> io::Result<()> {
        let region = Region::create(&[1, 2, 3, 4])?;
        assert!(take(region.name(), 1 << 20, <[u8]>::len).is_err());
        Ok(())
    }

    #[test]
    fn only_names_this_build_creates_are_opened() {
        for name in [
            "/etc/passwd",
            "/mx",
            "/mx1-",
            "/mx-1",
            "/mx1-2-3",
            "/mxg-1",
            "mx1-2",
        ] {
            assert!(!is_region_name(name), "{name}");
            assert!(take(name, 4, <[u8]>::len).is_err(), "{name}");
        }
        assert!(is_region_name("/mx1a-2b"));
    }
}
