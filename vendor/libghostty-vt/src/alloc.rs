use std::{
    borrow::Borrow,
    ffi::c_void,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

#[cfg(feature = "allocator_api")]
use allocator_api2::alloc;

use crate::{
    error::{Error, Result},
    ffi,
};

#[derive(Debug)]
pub struct Allocator<'ctx> {
    pub(crate) inner: ffi::Allocator,
    _phan: PhantomData<&'ctx ()>,
}

impl Allocator<'_> {
    pub(crate) fn to_raw(&self) -> *const ffi::Allocator {
        std::ptr::from_ref(&self.inner)
    }
    pub(crate) unsafe fn from_raw(raw: *const ffi::Allocator) -> Self {
        Self {
            inner: unsafe { *raw },
            _phan: PhantomData,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Object<'alloc, T> {
    pub(crate) ptr: NonNull<T>,
    _phan: PhantomData<&'alloc ffi::Allocator>,
}

impl<T> Object<'_, T> {
    pub(crate) fn new(raw: *mut T) -> Result<Self> {
        let ptr = NonNull::new(raw).ok_or(Error::OutOfMemory)?;
        Ok(Self {
            ptr,
            _phan: PhantomData,
        })
    }
    pub(crate) fn as_raw(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

#[derive(Debug)]
pub(crate) struct Ref<'a, T> {
    pub(crate) ptr: NonNull<T>,
    _phan: PhantomData<&'a ()>,
}

impl<T> Ref<'_, T> {
    pub(crate) fn new(raw: *mut T) -> Result<Self> {
        let ptr = NonNull::new(raw).ok_or(Error::OutOfMemory)?;
        Ok(Self {
            ptr,
            _phan: PhantomData,
        })
    }
    pub(crate) fn as_raw(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

#[derive(Debug)]
pub struct Bytes<'alloc> {
    ptr: NonNull<u8>,
    len: usize,
    alloc: *const ffi::Allocator,
    _phan: PhantomData<&'alloc ffi::Allocator>,
}
impl<'alloc> Bytes<'alloc> {
    pub fn new(len: usize) -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null(), len) }
    }

    pub fn new_with_alloc<'ctx: 'alloc>(
        alloc: &'alloc Allocator<'ctx>,
        len: usize,
    ) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw(), len) }
    }

    unsafe fn new_inner(alloc: *const ffi::Allocator, len: usize) -> Result<Self> {
        if len == 0 {
            return Ok(unsafe { Self::from_raw_parts(std::ptr::null_mut(), 0, alloc) });
        }

        let raw = unsafe { ffi::ghostty_alloc(alloc, len) };
        let ptr = NonNull::new(raw).ok_or(Error::OutOfMemory)?;
        unsafe { ptr.as_ptr().write_bytes(0, len) };
        Ok(unsafe { Self::from_raw_parts(ptr.as_ptr(), len, alloc) })
    }

    pub(crate) fn allocator(&self) -> *const ffi::Allocator {
        self.alloc
    }

    pub(crate) unsafe fn from_raw_parts(
        ptr: *mut u8,
        len: usize,
        alloc: *const ffi::Allocator,
    ) -> Self {
        let (ptr, len) = match NonNull::new(ptr) {
            Some(ptr) => (ptr, len),
            None => (NonNull::dangling(), 0),
        };
        Self {
            ptr,
            len,
            alloc,
            _phan: PhantomData,
        }
    }
}
impl Drop for Bytes<'_> {
    fn drop(&mut self) {
        if self.len == 0 {
            return;
        }
        unsafe { ffi::ghostty_free(self.alloc, self.ptr.as_ptr(), self.len) };
    }
}
impl Deref for Bytes<'_> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}
impl DerefMut for Bytes<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}
impl AsRef<[u8]> for Bytes<'_> {
    fn as_ref(&self) -> &[u8] {
        self
    }
}
impl AsMut<[u8]> for Bytes<'_> {
    fn as_mut(&mut self) -> &mut [u8] {
        self
    }
}
impl Borrow<[u8]> for Bytes<'_> {
    fn borrow(&self) -> &[u8] {
        self
    }
}
impl<'a> IntoIterator for &'a Bytes<'_> {
    type Item = &'a u8;
    type IntoIter = std::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.deref().iter()
    }
}


impl Allocator<'static> {
    pub const GLOBAL: Self = Self {
        inner: ffi::Allocator {
            ctx: std::ptr::null_mut(),
            vtable: &ffi::AllocatorVtable {
                alloc: Some(_global_alloc),
                free: Some(_global_free),
                resize: Some(_global_resize),
                remap: Some(_global_remap),
            },
        },
        _phan: PhantomData,
    };
}

unsafe extern "C" fn _global_alloc(
    _allocator: *mut c_void,
    len: usize,
    alignment: u8,
    _ret_addr: usize,
) -> *mut c_void {
    let Ok(layout) = std::alloc::Layout::from_size_align(len, 1 << alignment) else {
        return std::ptr::null_mut();
    };
    unsafe { std::alloc::alloc(layout).cast::<c_void>() }
}

unsafe extern "C" fn _global_free(
    _allocator: *mut c_void,
    mem: *mut c_void,
    len: usize,
    alignment: u8,
    _ret_addr: usize,
) {
    let Ok(layout) = std::alloc::Layout::from_size_align(len, 1 << alignment) else {
        return;
    };
    unsafe { std::alloc::dealloc(mem.cast::<u8>(), layout) }
}
unsafe extern "C" fn _global_resize(
    _allocator: *mut c_void,
    _mem: *mut c_void,
    _old_len: usize,
    _alignment: u8,
    _new_len: usize,
    _ret_addr: usize,
) -> bool {
    false
}
unsafe extern "C" fn _global_remap(
    _allocator: *mut c_void,
    mem: *mut c_void,
    old_len: usize,
    alignment: u8,
    new_len: usize,
    _ret_addr: usize,
) -> *mut c_void {
    let Ok(layout) = std::alloc::Layout::from_size_align(old_len, 1 << alignment) else {
        return std::ptr::null_mut();
    };
    unsafe { std::alloc::realloc(mem.cast::<u8>(), layout, new_len).cast::<c_void>() }
}


#[cfg(feature = "allocator_api")]
impl<'ctx, A: alloc::Allocator> From<&'ctx A> for Allocator<'ctx> {
    fn from(value: &'ctx A) -> Self {
        Self {
            inner: ffi::Allocator {
                ctx: std::ptr::from_ref(value)
                    .cast_mut()
                    .cast::<std::ffi::c_void>(),
                vtable: &ffi::AllocatorVtable {
                    alloc: Some(_alloc::<A>),
                    free: Some(_free::<A>),
                    resize: Some(_resize),
                    remap: Some(_remap::<A>),
                },
            },
            _phan: PhantomData,
        }
    }
}

#[cfg(feature = "allocator_api")]
unsafe extern "C" fn _alloc<A: alloc::Allocator>(
    allocator: *mut c_void,
    len: usize,
    alignment: u8,
    _ret_addr: usize,
) -> *mut c_void {
    let layout = alloc::Layout::from_size_align(len, 1 << alignment).ok();

    unsafe { get_allocator::<A>(allocator) }
        .and_then(|alloc| alloc.allocate(layout?).ok())
        .map_or(std::ptr::null_mut(), |p| p.as_ptr().cast::<c_void>())
}

#[cfg(feature = "allocator_api")]
unsafe extern "C" fn _free<A: alloc::Allocator>(
    allocator: *mut c_void,
    mem: *mut c_void,
    len: usize,
    alignment: u8,
    _ret_addr: usize,
) {
    let Some(mem) = NonNull::new(mem.cast::<u8>()) else {
        return;
    };
    let Some(layout) = alloc::Layout::from_size_align(len, 1 << alignment).ok() else {
        return;
    };
    if let Some(alloc) = unsafe { get_allocator::<A>(allocator) } {
        unsafe { alloc.deallocate(mem, layout) };
    }
}

#[cfg(feature = "allocator_api")]
unsafe extern "C" fn _resize(
    _allocator: *mut c_void,
    _mem: *mut c_void,
    _old_len: usize,
    _alignment: u8,
    _new_len: usize,
    _ret_addr: usize,
) -> bool {
    false
}

#[cfg(feature = "allocator_api")]
unsafe extern "C" fn _remap<A: alloc::Allocator>(
    allocator: *mut c_void,
    mem: *mut c_void,
    old_len: usize,
    alignment: u8,
    new_len: usize,
    _ret_addr: usize,
) -> *mut c_void {
    let mem = NonNull::new(mem.cast::<u8>());
    let old_layout = alloc::Layout::from_size_align(old_len, 1 << alignment).ok();
    let new_layout = alloc::Layout::from_size_align(new_len, 1 << alignment).ok();

    unsafe { get_allocator::<A>(allocator) }
        .and_then(|alloc| {
            if new_len < old_len {
                unsafe { alloc.shrink(mem?, old_layout?, new_layout?) }.ok()
            } else {
                unsafe { alloc.grow(mem?, old_layout?, new_layout?) }.ok()
            }
        })
        .map_or(std::ptr::null_mut(), |p| p.as_ptr().cast::<c_void>())
}

#[inline]
#[cfg(feature = "allocator_api")]
unsafe fn get_allocator<'a, A: alloc::Allocator>(ptr: *mut c_void) -> Option<&'a A> {
    unsafe { ptr.cast::<A>().as_ref() }
}

#[cfg(all(test, not(miri)))]
pub(crate) mod testing {
    use std::{cell::Cell, ffi::c_void};

    use super::Allocator;
    use crate::ffi;

    fn allocator<'a, T>(ctx: &'a T, vtable: &'static ffi::AllocatorVtable) -> Allocator<'a> {
        let raw = ffi::Allocator {
            ctx: std::ptr::from_ref(ctx).cast_mut().cast(),
            vtable: &raw const *vtable,
        };
        unsafe { Allocator::from_raw(&raw const raw) }
    }

    fn layout(len: usize, alignment: u8) -> std::alloc::Layout {
        std::alloc::Layout::from_size_align(len, 1 << alignment).expect("valid layout")
    }

    unsafe extern "C" fn no_resize(
        _ctx: *mut c_void,
        _mem: *mut c_void,
        _old_len: usize,
        _alignment: u8,
        _new_len: usize,
        _ret_addr: usize,
    ) -> bool {
        false
    }

    unsafe extern "C" fn no_remap(
        _ctx: *mut c_void,
        _mem: *mut c_void,
        _old_len: usize,
        _alignment: u8,
        _new_len: usize,
        _ret_addr: usize,
    ) -> *mut c_void {
        std::ptr::null_mut()
    }

    pub(crate) struct Capped {
        cap: usize,
        largest_request: Cell<usize>,
    }

    impl Capped {
        pub(crate) fn new(cap: usize) -> Self {
            Self {
                cap,
                largest_request: Cell::new(0),
            }
        }

        pub(crate) fn allocator(&self) -> Allocator<'_> {
            static VTABLE: ffi::AllocatorVtable = ffi::AllocatorVtable {
                alloc: Some(capped_alloc),
                resize: Some(no_resize),
                remap: Some(no_remap),
                free: Some(heap_free),
            };
            allocator(self, &VTABLE)
        }

        #[cfg(all(feature = "kitty-graphics", feature = "png"))]
        pub(crate) fn largest_request(&self) -> usize {
            self.largest_request.get()
        }
    }

    unsafe extern "C" fn capped_alloc(
        ctx: *mut c_void,
        len: usize,
        alignment: u8,
        _ret_addr: usize,
    ) -> *mut c_void {
        let this = unsafe { &*ctx.cast::<Capped>() };
        this.largest_request
            .set(this.largest_request.get().max(len));
        if len > this.cap {
            return std::ptr::null_mut();
        }
        unsafe { std::alloc::alloc(layout(len, alignment)).cast() }
    }

    unsafe extern "C" fn heap_free(
        _ctx: *mut c_void,
        mem: *mut c_void,
        len: usize,
        alignment: u8,
        _ret_addr: usize,
    ) {
        unsafe { std::alloc::dealloc(mem.cast(), layout(len, alignment)) };
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) struct Guard;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    impl Guard {
        pub(crate) fn allocator() -> Allocator<'static> {
            static VTABLE: ffi::AllocatorVtable = ffi::AllocatorVtable {
                alloc: Some(guard_pages::alloc),
                resize: Some(no_resize),
                remap: Some(no_remap),
                free: Some(guard_pages::free),
            };
            allocator(&(), &VTABLE)
        }
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    mod guard_pages {
        use std::ffi::{c_int, c_void};

        unsafe extern "C" {
            fn mmap(
                addr: *mut c_void,
                len: usize,
                prot: c_int,
                flags: c_int,
                fd: c_int,
                offset: i64,
            ) -> *mut c_void;
        }

        const PROT_NONE: c_int = 0;
        const PROT_READ_WRITE: c_int = 0x1 | 0x2;
        const MAP_PRIVATE: c_int = 0x2;
        const MAP_FIXED: c_int = 0x10;
        #[cfg(target_os = "linux")]
        const MAP_ANON: c_int = 0x20;
        #[cfg(target_os = "macos")]
        const MAP_ANON: c_int = 0x1000;
        const MAP_FAILED: *mut c_void = usize::MAX as *mut c_void;
        const PAGE: usize = 4096;

        pub(super) unsafe extern "C" fn alloc(
            _ctx: *mut c_void,
            len: usize,
            alignment: u8,
            _ret_addr: usize,
        ) -> *mut c_void {
            if 1usize << alignment > PAGE {
                return std::ptr::null_mut();
            }
            let mem = unsafe {
                mmap(
                    std::ptr::null_mut(),
                    len.max(1),
                    PROT_READ_WRITE,
                    MAP_PRIVATE | MAP_ANON,
                    -1,
                    0,
                )
            };
            if mem == MAP_FAILED {
                std::ptr::null_mut()
            } else {
                mem
            }
        }

        pub(super) unsafe extern "C" fn free(
            _ctx: *mut c_void,
            mem: *mut c_void,
            len: usize,
            _alignment: u8,
            _ret_addr: usize,
        ) {
            let result = unsafe {
                mmap(
                    mem,
                    len.max(1),
                    PROT_NONE,
                    MAP_PRIVATE | MAP_ANON | MAP_FIXED,
                    -1,
                    0,
                )
            };
            assert_eq!(result, mem, "remapping freed memory failed");
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::used_underscore_items,
    reason = "the underscore-named vtable functions are what is under test"
)]
mod tests {
    #[cfg(all(feature = "allocator_api", not(miri)))]
    #[test]
    fn allocator_api_allocators_keep_their_state() {
        use std::{alloc::Layout, cell::Cell, ptr::NonNull};

        use allocator_api2::alloc::{AllocError, Global};

        struct Counting(Cell<usize>);
        unsafe impl allocator_api2::alloc::Allocator for Counting {
            fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
                self.0.set(self.0.get() + 1);
                Global.allocate(layout)
            }
            unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
                unsafe { Global.deallocate(ptr, layout) };
            }
        }

        let counting = Counting(Cell::new(0));
        let alloc = super::Allocator::from(&counting);
        let terminal = crate::Terminal::new_with_alloc(&alloc, 80, 24).unwrap();
        drop(terminal);
        assert!(counting.0.get() > 0);
    }

    use std::ptr::NonNull;

    use super::{_global_alloc, _global_free, _global_remap};


    #[test]
    fn global_allocator_respects_requested_alignment() {
        let len = 16usize;
        let alignment_log2 = 4u8;
        let expected_alignment = 1usize << alignment_log2;

        let raw = unsafe { _global_alloc(std::ptr::null_mut(), len, alignment_log2, 0) };
        let mem = NonNull::new(raw.cast::<u8>()).expect("global allocator returned null");

        assert_eq!((mem.as_ptr() as usize) % expected_alignment, 0);

        unsafe {
            _global_free(
                std::ptr::null_mut(),
                mem.as_ptr().cast(),
                len,
                alignment_log2,
                0,
            );
        };
    }

    #[test]
    fn global_allocator_round_trip_preserves_written_bytes() {
        let initial_len = 16usize;
        let new_len = 32usize;
        let alignment_log2 = 3u8;

        let raw = unsafe { _global_alloc(std::ptr::null_mut(), initial_len, alignment_log2, 0) };
        let mem = NonNull::new(raw.cast::<u8>()).expect("global allocator returned null");

        let initial = unsafe { std::slice::from_raw_parts_mut(mem.as_ptr(), initial_len) };
        for (index, byte) in initial.iter_mut().enumerate() {
            *byte = u8::try_from(index).unwrap();
        }

        let raw = unsafe {
            _global_remap(
                std::ptr::null_mut(),
                mem.as_ptr().cast(),
                initial_len,
                alignment_log2,
                new_len,
                0,
            )
        };
        let mem = NonNull::new(raw.cast::<u8>()).expect("global remap returned null");

        let grown = unsafe { std::slice::from_raw_parts(mem.as_ptr(), new_len) };
        for (index, byte) in grown[..initial_len].iter().copied().enumerate() {
            assert_eq!(byte, u8::try_from(index).unwrap());
        }

        unsafe {
            _global_free(
                std::ptr::null_mut(),
                mem.as_ptr().cast(),
                new_len,
                alignment_log2,
                0,
            );
        };
    }

    #[test]
    #[cfg_attr(miri, ignore = "calls into libghostty")]
    fn bytes_are_zero_initialized() {
        for len in [0, 1, 4096] {
            let bytes = super::Bytes::new_with_alloc(&super::Allocator::GLOBAL, len)
                .expect("allocation failed");
            assert_eq!(bytes.len(), len);
            assert!(bytes.iter().all(|&b| b == 0));
        }
    }

    #[test]
    #[cfg_attr(miri, ignore = "calls into libghostty")]
    fn empty_output_is_an_empty_buffer() {
        let terminal = crate::Terminal::new(10, 5).expect("terminal");
        let mut formatter =
            crate::fmt::Formatter::new(&terminal, crate::fmt::FormatterOptions::new())
                .expect("formatter");
        let bytes = formatter
            .format_alloc(None)
            .expect("empty output is not an error");
        assert!(bytes.is_empty());
    }
}
