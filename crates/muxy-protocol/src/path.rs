use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// A Unix path on the server, as its bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct ServerPath(
    #[n(0)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub Vec<u8>,
);

impl ServerPath {
    pub fn as_path(&self) -> &Path {
        Path::new(OsStr::from_bytes(&self.0))
    }
}

impl AsRef<Path> for ServerPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl From<&Path> for ServerPath {
    fn from(path: &Path) -> Self {
        Self(path.as_os_str().as_bytes().to_vec())
    }
}

impl From<&PathBuf> for ServerPath {
    fn from(path: &PathBuf) -> Self {
        path.as_path().into()
    }
}

impl From<PathBuf> for ServerPath {
    fn from(path: PathBuf) -> Self {
        Self(path.into_os_string().into_vec())
    }
}

impl From<OsString> for ServerPath {
    fn from(path: OsString) -> Self {
        Self(path.into_vec())
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
        OsString::from_vec(path.0).into()
    }
}

/// The path as text, when it is UTF-8.
impl<'a> TryFrom<&'a ServerPath> for &'a str {
    type Error = std::str::Utf8Error;

    fn try_from(path: &'a ServerPath) -> Result<Self, Self::Error> {
        std::str::from_utf8(&path.0)
    }
}
