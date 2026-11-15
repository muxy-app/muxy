#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ghostty-org/ghostty/2d0fb81751def478e2f8a5f7e2ee91fa9cbf9bff/images/icons/icon_128@2x.png"
)]
#![warn(clippy::pedantic)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]
#![warn(clippy::allow_attributes)]
#![warn(clippy::allow_attributes_without_reason)]
#![expect(
    clippy::needless_pass_by_value,
    reason = "handles such as grid refs and option structs are small, and taken
    by value the same way the C API takes them"
)]
#![allow(
    clippy::missing_errors_doc,
    reason = "underlying C API may return any error outside of expected and
    mitigated situations, and it is not feasible to document them all"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use libghostty_vt_sys as ffi;

macro_rules! sized_field {
    ($ptr:expr, $ty:ty, $field:ident) => {{
        fn field_size<T, F>(_: for<'a> fn(&'a T) -> &'a F) -> usize {
            ::std::mem::size_of::<F>()
        }
        let ptr: *const $ty = $ptr;
        let end = ::std::mem::offset_of!($ty, $field) + field_size::<$ty, _>(|s| &s.$field);
        ((*ptr).size >= end).then(|| (*ptr).$field)
    }};
}
pub(crate) use sized_field;

pub mod terminal;

pub mod alloc;
pub mod build_info;
pub mod error;
pub mod fmt;
pub mod focus;
pub(crate) mod io;
pub mod key;
pub mod kitty;
pub mod log;
pub mod mouse;
pub mod osc;
pub mod paste;
pub mod render;
pub mod screen;
pub mod search;
pub mod selection;
pub mod sgr;
pub mod snapshot;
pub mod style;
pub mod sys;
pub mod unicode;

#[doc(inline)]
pub use crate::{
    error::Error,
    log::{Logger, set_logger},
    render::RenderState,
    terminal::Terminal,
};

pub(crate) fn sys_set<T>(opt: ffi::SysOption::Type, val: *const T) -> error::Result<()> {
    let result = unsafe { ffi::ghostty_sys_set(opt, val.cast()) };
    error::from_result(result)
}
