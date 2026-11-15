use std::io::{Read, Write};

use crate::ffi;

pub fn to_writer<W: Write>(w: &mut W) -> ffi::Writer {
    unsafe extern "C" fn trampoline<W: Write>(
        userdata: *mut ::std::os::raw::c_void,
        data: *const u8,
        len: usize,
    ) -> bool {
        let w: &mut W = unsafe { &mut *userdata.cast::<W>() };

        let data = unsafe { std::slice::from_raw_parts(data, len) };

        w.write_all(data).is_ok()
    }

    ffi::Writer {
        userdata: std::ptr::from_mut(w).cast(),
        write: Some(trampoline::<W>),
    }
}

pub fn to_reader<R: Read>(r: &mut R) -> ffi::Reader {
    unsafe extern "C" fn trampoline<R: Read>(
        userdata: *mut ::std::os::raw::c_void,
        buffer: *mut u8,
        capacity: usize,
        out_read: *mut usize,
    ) -> bool {
        let r: &mut R = unsafe { &mut *userdata.cast::<R>() };

        let buf = unsafe {
            buffer.write_bytes(0, capacity);
            std::slice::from_raw_parts_mut(buffer, capacity)
        };

        match r.read(buf) {
            Ok(len) => unsafe {
                *out_read = len;
                true
            },
            Err(_) => false,
        }
    }

    ffi::Reader {
        userdata: std::ptr::from_mut(r).cast(),
        read: Some(trampoline::<R>),
    }
}

#[cfg(all(test, miri))]
mod miri_soundness {
    use super::*;
    use std::mem::MaybeUninit;

    #[test]
    fn reader_can_inspect_the_entire_destination() {
        struct InspectingReader;
        impl Read for InspectingReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                assert!(buffer.iter().all(|&byte| byte == 0));
                buffer[..2].copy_from_slice(&[0x31, 0x8b]);
                Ok(2)
            }
        }

        let mut reader = InspectingReader;
        let callback = to_reader(&mut reader);
        let mut storage = MaybeUninit::<[u8; 9]>::uninit();
        for _ in 0..2 {
            let mut read = usize::MAX;
            assert!(unsafe {
                callback.read.unwrap()(
                    callback.userdata,
                    storage.as_mut_ptr().cast(),
                    9,
                    &raw mut read,
                )
            });
            assert_eq!(read, 2);
            assert_eq!(
                unsafe { storage.assume_init_ref() },
                &[0x31, 0x8b, 0, 0, 0, 0, 0, 0, 0]
            );
            storage.write([0xa5; 9]);
        }
    }
}
