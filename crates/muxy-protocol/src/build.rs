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
        }
    }
}
