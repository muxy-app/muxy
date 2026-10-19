use std::path::{Path, PathBuf};

use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::ErrorCode;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct ServerPath(
    #[n(0)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub Vec<u8>,
);

impl ServerPath {
    /// The path as it travels: its raw bytes on Unix, and UTF-8 on Windows,
    /// where a path that isn't valid Unicode is refused rather than
    /// converted lossily.
    pub fn from_path(path: &Path) -> Result<Self, ErrorCode> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Ok(Self(path.as_os_str().as_bytes().to_vec()))
        }
        #[cfg(windows)]
        {
            path.to_str()
                .map(|text| Self(text.as_bytes().to_vec()))
                .ok_or(ErrorCode::BadPath)
        }
    }

    /// The path on this computer. On Windows, bytes that aren't UTF-8 are
    /// refused.
    pub fn to_path_buf(&self) -> Result<PathBuf, ErrorCode> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Ok(Path::new(std::ffi::OsStr::from_bytes(&self.0)).to_path_buf())
        }
        #[cfg(windows)]
        {
            std::str::from_utf8(&self.0)
                .map(PathBuf::from)
                .map_err(|_| ErrorCode::BadPath)
        }
    }

    /// Whether the path is absolute on a Unix or a Windows server, whichever
    /// this build is: `/…`, `X:\…`, `X:/…`, or `\\server\share…`. A server
    /// also checks that it is absolute on its own system.
    pub fn is_absolute(&self) -> bool {
        match self.0.as_slice() {
            [b'/', ..] => true,
            [drive, b':', b'\\' | b'/', ..] => drive.is_ascii_alphabetic(),
            [b'\\', b'\\', unc @ ..] => {
                let mut parts = unc.split(|byte| *byte == b'\\');
                matches!(
                    (parts.next(), parts.next()),
                    (Some(server), Some(share)) if !server.is_empty() && !share.is_empty()
                )
            }
            _ => false,
        }
    }
}
