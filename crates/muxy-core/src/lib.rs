//! Narrow primitives that are genuinely shared across Muxy boundaries.
//!
//! Wire-visible types belong in `muxy-protocol`; app and server concepts stay
//! in their respective domain crates. The shared shortcut catalog, the app
//! language, and the bounded worker primitive are independent of GPUI and
//! server execution.

pub mod bundle;
pub mod dirs;
pub mod executable;
pub mod file_lock;
pub mod l10n;
pub mod shortcuts;
pub mod worker;

pub mod quick_terminal;
