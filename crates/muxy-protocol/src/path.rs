use std::ffi::OsString;
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

    pub fn as_path(&self) -> &Path {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Path::new(std::ffi::OsStr::from_bytes(&self.0))
        }
        #[cfg(windows)]
        {
            std::str::from_utf8(&self.0).map_or(Path::new(""), Path::new)
        }
    }
}

impl AsRef<Path> for ServerPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl From<&Path> for ServerPath {
    fn from(path: &Path) -> Self {
        Self(path.as_os_str().as_encoded_bytes().to_vec())
    }
}

impl From<&PathBuf> for ServerPath {
    fn from(path: &PathBuf) -> Self {
        path.as_path().into()
    }
}

impl From<PathBuf> for ServerPath {
    fn from(path: PathBuf) -> Self {
        path.into_os_string().into()
    }
}

impl From<OsString> for ServerPath {
    fn from(path: OsString) -> Self {
        Self(path.into_encoded_bytes())
    }
}

impl From<&str> for ServerPath {
    fn from(path: &str) -> Self {
        Self(path.as_bytes().to_vec())
    }
}

impl From<String> for ServerPath {
    fn from(path: String) -> Self {
        Self(path.into_bytes())
    }
}

impl From<ServerPath> for PathBuf {
    fn from(path: ServerPath) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(path.0).into()
        }
        #[cfg(windows)]
        {
            String::from_utf8(path.0).map_or_else(|_| Self::new(), Self::from)
        }
    }
}

impl<'a> TryFrom<&'a ServerPath> for &'a str {
    type Error = std::str::Utf8Error;

    fn try_from(path: &'a ServerPath) -> Result<Self, Self::Error> {
        std::str::from_utf8(&path.0)
    }
}
