use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub const COMPATIBILITY: u64 = 21;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct BuildInfo {
    #[n(0)]
    pub version: String,
    #[n(1)]
    pub compatibility: u64,
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

    pub fn shares_protocol_with(&self, other: &Self) -> bool {
        self.protocol
            .iter()
            .any(|version| other.protocol.contains(version))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ServerInfo {
    #[n(0)]
    pub build: BuildInfo,
    #[n(1)]
    pub instance: u64,
    #[n(2)]
    #[cbor(default)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<UnreadableFile>,
}

pub const CATALOG_FILE: &str = "catalog.json";
pub const UNREADABLE_SUFFIX: &str = ".unreadable-";

pub const MAX_UNREADABLE_FILES: usize = 8;
pub const MAX_UNREADABLE_ERROR: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct UnreadableFile {
    #[n(0)]
    pub kept_as: crate::ServerPath,
    #[n(1)]
    pub error: String,
}

impl UnreadableFile {
    pub fn is_catalog(&self) -> bool {
        self.kept_as
            .as_path()
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(CATALOG_FILE))
            .is_some_and(|rest| rest.starts_with(UNREADABLE_SUFFIX))
    }
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

    pub fn validate(&self) -> Result<(), crate::ErrorCode> {
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
