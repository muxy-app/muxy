use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// Read only by updaters from before protocol V2, which keep a running server
/// only when this matches. Frozen.
pub const COMPATIBILITY: u64 = 21;

/// Update metadata, stable across protocol versions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct BuildInfo {
    #[n(0)]
    pub version: String,
    #[n(1)]
    pub compatibility: u64,
    /// Protocol versions this build speaks.
    #[n(2)]
    #[serde(default)]
    pub protocol: Vec<crate::Version>,
}

impl BuildInfo {
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").into(),
            compatibility: COMPATIBILITY,
            protocol: crate::SUPPORTED.to_vec(),
        }
    }

    /// Whether this build and `other` share a protocol version, so they can talk.
    pub fn shares_protocol_with(&self, other: &Self) -> bool {
        self.protocol
            .iter()
            .any(|version| other.protocol.contains(version))
    }
}

/// Identifies the running process, rather than the binary currently installed on disk.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ServerInfo {
    #[n(0)]
    pub build: BuildInfo,
    #[n(1)]
    pub instance: u64,
    /// Saved files this process couldn't read when it started. Older servers
    /// never send it.
    #[n(2)]
    #[cbor(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<UnreadableFile>,
}

/// At most this many unreadable files are reported.
pub const MAX_UNREADABLE_FILES: usize = 8;
/// At most this many bytes of an unreadable file's error are reported.
pub const MAX_UNREADABLE_ERROR: usize = 1024;

/// A saved file the server moved aside because it couldn't read it, and
/// started with defaults instead.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct UnreadableFile {
    /// Where the file is kept for recovery.
    #[n(0)]
    pub kept_as: crate::ServerPath,
    #[n(1)]
    pub error: String,
}

impl ServerInfo {
    pub fn current() -> Self {
        static INSTANCE: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
        let instance = *INSTANCE.get_or_init(|| {
            let elapsed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default();
            u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX) ^ u64::from(std::process::id())
        });
        Self {
            build: BuildInfo::current(),
            instance,
            unreadable: Vec::new(),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), crate::ErrorCode> {
        let valid = self.unreadable.len() <= MAX_UNREADABLE_FILES
            && self.unreadable.iter().all(|file| {
                crate::validate_absolute_path(&file.kept_as).is_ok()
                    && file.error.len() <= MAX_UNREADABLE_ERROR
            });
        if valid {
            Ok(())
        } else {
            Err(crate::ErrorCode::BadRequest)
        }
    }
}
