//! Idiomatic, safe Rust bindings for
//! [`libghostty-vt`](https://libghostty.tip.ghostty.org/),
//! a terminal emulation library extracted from the
//! [Ghostty](https://ghostty.org) terminal emulator.
//!
//! `libghostty-vt` contains the logic for handling the core parts of a
//! terminal emulator: parsing terminal escape sequences, maintaining terminal
//! state, encoding input events, etc. It can handle scrollback, line wrapping,
//! reflow on resize, and more.
//!
//! <div class="warning">
//!
//! This library is currently in development and the API is not yet stable.
//! Breaking changes are expected in future versions.
//! Use with caution in production code.
//!
//! </div>
//!
//! The core type of `libghostty-vt` is the [`Terminal`] — start there to get a
//! better sense of how everything is structured. You can also check out
//! a list of all top-level modules and their functions below.
//!
//! # Examples
//!
//! [`ghostling-rs`](https://github.com/Uzaaft/libghostty-rs/blob/master/example/ghostling_rs/src/main.rs)
//! is a minimal yet functional terminal emulator built with `libghostty-vt`,
//! written as a single file with only around 1000 lines of heavily commented,
//! easily digestible code. It is based on the original [Ghostling](https://github.com/ghostty-org/ghostling)
//! which is built on the same underlying C API.
//!
//! Other examples are currently work-in-progress — if you have any ideas for
//! examples, feel free to share them with us as an issue or pull request.
//!
//! # Memory management and lifetimes
//!
//! When creating the terminal and various other objects, you can control their
//! memory management via a **custom allocator**, usually specified with
//! methods like [`Terminal::new_with_alloc`]. Objects that accept allocators
//! are also bound by the `'alloc` lifetime, since they internally contain
//! a reference to the allocator. If you do not use a custom allocator,
//! feel free to always set the lifetime to `'static`.
//!
//! ## Using the unstable `Allocator` API
//!
//! You can adapt the existing, unstable `Allocator` API into a
//! [libghostty-friendly allocator](alloc::Allocator) via its `From`
//! implementation. Note that the `'alloc` lifetime must at least
//! live as long as the `Allocator` instance itself.
//!
//! # Thread safety
//!
//! The entire `libghostty-vt` library is **not** thread-safe unless otherwise
//! noted. This is because the underlying C API is not designed with thread
//! safety in mind, and we as binding authors must rather conservatively
//! avoid making any assumptions beyond what is presently guaranteed by the
//! C API.
//!
//! In particular, all `libghostty-vt` types are `!Send`, meaning they cannot
//! be *transferred* across threads, since the C API is allowed to use
//! thread-local state; they are also `!Sync`, meaning they cannot be *shared*
//! across threads, since data races may occur as the C API is not guarded with
//! mutexes or other synchronization mechanisms.
//! We currently do not expect to lift these limitations unless the C API starts
//! to make stronger guarantees regarding thread safety.
//!
//! Note that this does *not* mean that `libghostty-vt` can only be used on
//! the main thread. On the contrary, in a complex program we encourage you to
//! create the terminal on a separate thread (or task in async programming),
//! and use [channels](std::sync::mpsc::channel) to communicate between the
//! terminal emulation thread/task and the main program. Under sufficient load, it is
//! generally more efficient to offload terminal emulation to its own operating
//! system-level thread, in order to reduce competition with other business logic.
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/ghostty-org/ghostty/2d0fb81751def478e2f8a5f7e2ee91fa9cbf9bff/images/icons/icon_128@2x.png"
)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]
#![warn(clippy::allow_attributes)]
#![warn(clippy::allow_attributes_without_reason)]
#![allow(
    clippy::missing_errors_doc,
    reason = "underlying C API may return any error outside of expected and
    mitigated situations, and it is not feasible to document them all"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use libghostty_vt_sys as ffi;

// Make sure that `Terminal`'s own impl blocks (i.e. core functions)
// are placed *before* any extra impl blocks from other modules,
// e.g. Kitty Graphics extensions, Selection APIs
pub mod terminal;

pub mod alloc;
pub mod build_info;
pub mod error;
pub mod fmt;
pub mod focus;
pub mod key;
pub mod kitty;
pub mod log;
pub mod mouse;
pub mod osc;
pub mod paste;
pub mod render;
pub mod screen;
pub mod selection;
pub mod sgr;
pub mod style;
pub mod unicode;

#[doc(inline)]
pub use crate::{
    error::Error,
    log::{Logger, set_logger},
    render::RenderState,
    terminal::{Options as TerminalOptions, Terminal},
};

pub(crate) fn sys_set<T>(opt: ffi::SysOption::Type, val: *const T) -> error::Result<()> {
    let result = unsafe { ffi::ghostty_sys_set(opt, val.cast()) };
    error::from_result(result)
}
