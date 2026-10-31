//! Narrow primitives that are genuinely shared across Muxy boundaries.
//!
//! Wire-visible types belong in `muxy-protocol`, which builds on this crate;
//! app and server concepts stay in their respective domain crates. The shared
//! shortcut catalog, the app language, the bounded worker primitive, and the
//! hex, base64, and SHA-256 helpers are independent of GPUI and server
//! execution.

pub mod base64;
pub mod bundle;
pub mod capability;
pub mod dirs;
pub mod executable;
pub mod file_lock;
pub mod fs;
pub mod hex;
pub mod l10n;
pub mod release;
pub mod sha256;
pub mod shortcuts;
pub mod worker;

pub mod quick_terminal;
