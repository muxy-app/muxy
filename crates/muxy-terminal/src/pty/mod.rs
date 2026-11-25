#[cfg(windows)]
#[allow(unsafe_code)]
mod conpty;
mod error;
mod process;
mod reader;

pub use error::{PtyError, PtyStep};
pub use process::{ExitStatus, Pty, PtySize, SpawnRequest};
pub use reader::{PtyEvent, PtyOutput, ReaderHandle};
