//! What the integration tests need from each platform.
#![allow(dead_code, reason = "Each test file uses only some helpers")]

use std::path::{Path, PathBuf};

use muxy_protocol::ServerPath;

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

pub(crate) fn server_path(path: &Path) -> Result<ServerPath, String> {
    ServerPath::from_path(path)
        .map_err(|code| format!("{} is not a server path: {code:?}", path.display()))
}

/// Whether a local socket exists at `path`. On Windows it is a reparse point.
pub(crate) fn is_socket(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_socket())
    }
    #[cfg(windows)]
    {
        muxy_core::files::FileId::of_link(path).is_ok()
    }
}

/// A typed command line: the pseudoconsole reads Enter as `\r`.
pub(crate) fn line(command: &str) -> Vec<u8> {
    format!("{command}{}", if cfg!(windows) { "\r" } else { "\n" }).into_bytes()
}
