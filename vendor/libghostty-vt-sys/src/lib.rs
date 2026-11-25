#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(clippy::all)]
#![allow(rustdoc::all)]

mod bindings;

use std::ops::Deref;

pub use bindings::*;

#[macro_export]
macro_rules! sized {
    ($ty:ty) => {{
        let mut t = <$ty as ::std::default::Default>::default();
        t.size = ::std::mem::size_of::<$ty>();
        t
    }};
}

impl<S> From<S> for bindings::String
where
    S: Deref<Target = str>,
{
    fn from(value: S) -> Self {
        Self {
            ptr: value.as_ptr(),
            len: value.len(),
        }
    }
}

impl bindings::String {
    pub unsafe fn to_bytes<'a>(self) -> &'a [u8] {
        if self.ptr.is_null() {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    pub unsafe fn to_str<'a>(self) -> &'a str {
        unsafe { std::str::from_utf8_unchecked(self.to_bytes()) }
    }
}

#[cfg(test)]
mod tests {
    use super::bindings;

    #[test]
    fn sized_macro_sets_the_size_field() {
        let colors = crate::sized!(bindings::RenderStateColors);
        assert_eq!(
            colors.size,
            std::mem::size_of::<bindings::RenderStateColors>()
        );
    }

    #[test]
    fn ffi_string_round_trips_static_str() {
        let raw = bindings::String::from("ghostty");
        assert_eq!(raw.len, "ghostty".len());
        assert_eq!(unsafe { raw.to_str() }, "ghostty");
    }

    #[test]
    fn ffi_string_round_trips_borrowed_string() {
        let owned = String::from("terminal");
        let raw = bindings::String::from(owned.as_str());
        assert_eq!(unsafe { raw.to_str() }, "terminal");
    }
}
