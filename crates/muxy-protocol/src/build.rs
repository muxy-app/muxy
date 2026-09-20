use serde::{Deserialize, Serialize};

/// Bump when mixed beta builds would be unsafe, including behavior or shared storage changes.
pub const COMPATIBILITY: u64 = 16;

/// Stable update metadata, independent of the mutable terminal schema.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BuildInfo {
    pub version: String,
    pub compatibility: u64,
}

impl BuildInfo {
    pub fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").into(),
            compatibility: COMPATIBILITY,
        }
    }
}

/// Identifies the running process, rather than the binary currently installed on disk.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerInfo {
    pub build: BuildInfo,
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
