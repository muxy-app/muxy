//! What unit tests need from each platform.

use std::path::{Path, PathBuf};

/// A shell that runs typed commands: `/bin/sh` on Unix, `cmd.exe` on Windows.
pub(crate) fn shell() -> PathBuf {
    if cfg!(windows) {
        let system = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        Path::new(&system).join(r"System32\cmd.exe")
    } else {
        PathBuf::from("/bin/sh")
    }
}

/// A folder every session can start in.
pub(crate) fn folder() -> PathBuf {
    if cfg!(windows) {
        muxy_core::files::canonicalize(&std::env::temp_dir())
            .unwrap_or_else(|_| std::env::temp_dir())
    } else {
        PathBuf::from("/tmp")
    }
}
