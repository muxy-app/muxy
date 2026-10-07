//! Portable pseudo-terminal process and byte-I/O adapters.
//!
//! Session lifecycle remains server-owned.

mod error;
mod process;
mod reader;

pub use error::{PtyError, PtyStep};
pub use process::{ExitStatus, ProcessMonitor, Pty, PtySize, SpawnRequest};
pub use reader::{PtyEvent, ReaderHandle};
