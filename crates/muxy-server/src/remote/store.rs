use std::fs;
use std::io;
use std::path::Path;

use muxy_core::hex;
use muxy_protocol::transport::tls::TlsIdentity;
use muxy_protocol::{
    DeviceId, DeviceKind, MAX_DEVICES, PushEnvironment, PushPlatform, PushStatus, PushToken,
    RemoteAccessSettings, validate_device_name,
};
use serde::{Deserialize, Serialize};

use crate::push::{Credentials, PushKey};

#[derive(Clone, Debug)]
pub(super) struct Device {
    pub(super) id: DeviceId,
    pub(super) name: String,
    /// SHA-256 of the device token; the token itself is never stored.
    pub(super) token_hash: [u8; 32],
    pub(super) paired_at: u64,
    pub(super) last_seen: Option<u64>,
    /// Only kinds this build knows; anything else is kept as a phone.
    pub(super) kind: DeviceKind,
    pub(super) push: Option<PushRecord>,
}

/// A phone's push registration: its token, the key its notifications are
/// encrypted with, and its registration at the relay once there is one.
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct PushRecord {
    pub(crate) token: PushToken,
    pub(crate) key: PushKey,
    pub(crate) relay: Option<Credentials>,
    /// `Active`, `Pending`, or `Refused`.
    pub(crate) status: PushStatus,
}

impl std::fmt::Debug for PushRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PushRecord")
            .field("token", &self.token)
            .field("relay", &self.relay)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub(super) struct Stored {
    pub(super) settings: RemoteAccessSettings,
    pub(super) identity: Option<TlsIdentity>,
    pub(super) devices: Vec<Device>,
    pub(super) push_notifications: bool,
}

impl Default for Stored {
    fn default() -> Self {
        Self {
            settings: RemoteAccessSettings::default(),
            identity: None,
            devices: Vec::new(),
            push_notifications: true,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct Document {
    enabled: bool,
    port: u16,
    certificate: Option<String>,
    private_key: Option<String>,
    devices: Vec<DocumentDevice>,
    /// Missing in files from before push notifications, which start on.
    #[serde(default = "on")]
    push_notifications: bool,
}

fn on() -> bool {
    true
}

#[derive(Deserialize, Serialize)]
struct DocumentDevice {
    id: DeviceId,
    name: String,
    token_sha256: String,
    paired_at: u64,
    last_seen: Option<u64>,
    /// Missing in files from before computers could pair.
    #[serde(default)]
    kind: StoredKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    push: Option<DocumentPush>,
}

#[derive(Deserialize, Serialize)]
struct DocumentPush {
    platform: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    environment: Option<String>,
    token: String,
    key: String,
    relay_id: Option<String>,
    relay_secret: Option<String>,
    status: String,
}

impl DocumentPush {
    fn from_record(record: &PushRecord) -> Option<Self> {
        Some(Self {
            platform: match record.token.platform {
                PushPlatform::Apns => "apns",
                PushPlatform::Fcm => "fcm",
                PushPlatform::Unrecognized(_) => return None,
            }
            .into(),
            environment: match record.token.environment {
                None => None,
                Some(PushEnvironment::Sandbox) => Some("sandbox".into()),
                Some(PushEnvironment::Production) => Some("production".into()),
                Some(PushEnvironment::Unrecognized(_)) => return None,
            },
            token: record.token.token.clone(),
            key: hex::encode(&record.key),
            relay_id: record.relay.as_ref().map(|relay| relay.id.clone()),
            relay_secret: record.relay.as_ref().map(|relay| relay.secret.clone()),
            status: match record.status {
                PushStatus::Active => "active",
                PushStatus::Refused => "refused",
                PushStatus::Pending | PushStatus::Disabled | PushStatus::Unrecognized(_) => {
                    "pending"
                }
            }
            .into(),
        })
    }

    /// `None` for an entry this build can't use, which is dropped without
    /// losing the device.
    fn record(self) -> Option<PushRecord> {
        let token = PushToken {
            platform: match self.platform.as_str() {
                "apns" => PushPlatform::Apns,
                "fcm" => PushPlatform::Fcm,
                _ => return None,
            },
            token: self.token,
            environment: match self.environment.as_deref() {
                None => None,
                Some("sandbox") => Some(PushEnvironment::Sandbox),
                Some("production") => Some(PushEnvironment::Production),
                Some(_) => return None,
            },
        };
        token.validate().ok()?;
        let relay = match (self.relay_id, self.relay_secret) {
            (Some(id), Some(secret)) => Some(Credentials::new(id, secret)?),
            (None, None) => None,
            _ => return None,
        };
        Some(PushRecord {
            token,
            key: hex::decode_vec(&self.key)?.try_into().ok()?,
            relay,
            status: match self.status.as_str() {
                "active" => PushStatus::Active,
                "refused" => PushStatus::Refused,
                _ => PushStatus::Pending,
            },
        })
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum StoredKind {
    #[default]
    Phone,
    Computer,
}

impl From<StoredKind> for DeviceKind {
    fn from(kind: StoredKind) -> Self {
        match kind {
            StoredKind::Phone => Self::Phone,
            StoredKind::Computer => Self::Computer,
        }
    }
}

impl From<DeviceKind> for StoredKind {
    fn from(kind: DeviceKind) -> Self {
        match kind {
            DeviceKind::Computer => Self::Computer,
            DeviceKind::Phone | DeviceKind::Unrecognized(_) => Self::Phone,
        }
    }
}

#[cfg(test)]
pub(super) fn load(path: &Path) -> io::Result<Stored> {
    read(path)?.map_or_else(|| Ok(Stored::default()), |bytes| parse(path, &bytes))
}

/// `None` when nothing was saved yet.
pub(super) fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn parse(path: &Path, bytes: &[u8]) -> io::Result<Stored> {
    let document: Document = serde_json::from_slice(bytes)?;
    stored(document).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} has invalid contents", path.display()),
        )
    })
}

fn stored(document: Document) -> Option<Stored> {
    let settings = RemoteAccessSettings {
        enabled: document.enabled,
        port: document.port,
    };
    settings.validate().ok()?;
    let identity = match (document.certificate, document.private_key) {
        (Some(certificate), Some(private_key)) => Some(TlsIdentity {
            certificate: hex::decode_vec(&certificate)?,
            private_key: hex::decode_vec(&private_key)?,
        }),
        (None, None) => None,
        _ => return None,
    };
    if document.devices.len() > MAX_DEVICES || (settings.enabled && identity.is_none()) {
        return None;
    }
    let devices = document
        .devices
        .into_iter()
        .map(|device| {
            validate_device_name(&device.name).ok()?;
            Some(Device {
                id: device.id,
                name: device.name,
                token_hash: hex::decode_vec(&device.token_sha256)?.try_into().ok()?,
                paired_at: device.paired_at,
                last_seen: device.last_seen,
                kind: device.kind.into(),
                push: device.push.and_then(DocumentPush::record),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Stored {
        settings,
        identity,
        devices,
        push_notifications: document.push_notifications,
    })
}

pub(super) fn save(path: &Path, stored: &Stored) -> io::Result<()> {
    let document = Document {
        enabled: stored.settings.enabled,
        port: stored.settings.port,
        certificate: stored
            .identity
            .as_ref()
            .map(|identity| hex::encode(&identity.certificate)),
        private_key: stored
            .identity
            .as_ref()
            .map(|identity| hex::encode(&identity.private_key)),
        devices: stored
            .devices
            .iter()
            .map(|device| DocumentDevice {
                id: device.id,
                name: device.name.clone(),
                token_sha256: hex::encode(&device.token_hash),
                paired_at: device.paired_at,
                last_seen: device.last_seen,
                kind: device.kind.into(),
                push: device.push.as_ref().and_then(DocumentPush::from_record),
            })
            .collect(),
        push_notifications: stored.push_notifications,
    };
    let bytes = serde_json::to_vec_pretty(&document)?;
    Ok(muxy_core::fs::write_atomic(path, &bytes, 0o600)?)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use super::*;

    fn directory() -> io::Result<PathBuf> {
        let directory =
            std::env::temp_dir().join(format!("muxy-remote-{}", muxy_protocol::OperationId::new()));
        fs::create_dir(&directory)?;
        Ok(directory)
    }

    #[test]
    fn saved_access_round_trips_with_private_permissions() -> io::Result<()> {
        let directory = directory()?;
        let path = directory.join("remote.json");
        assert!(load(&path)?.devices.is_empty());
        let stored = Stored {
            settings: RemoteAccessSettings {
                enabled: true,
                port: 7419,
            },
            identity: Some(TlsIdentity {
                certificate: vec![1, 2, 3],
                private_key: vec![4, 5, 6],
            }),
            devices: vec![Device {
                id: DeviceId::from_u128(9),
                name: "Phone".into(),
                token_hash: [7; 32],
                paired_at: 10,
                last_seen: Some(11),
                kind: DeviceKind::Computer,
                push: None,
            }],
            push_notifications: false,
        };
        save(&path, &stored)?;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        let loaded = load(&path)?;
        assert_eq!(loaded.settings, stored.settings);
        assert_eq!(loaded.identity, stored.identity);
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices[0].token_hash, [7; 32]);
        assert_eq!(loaded.devices[0].last_seen, Some(11));
        assert_eq!(loaded.devices[0].kind, DeviceKind::Computer);
        assert!(!loaded.push_notifications);
        fs::remove_dir_all(directory)
    }

    fn push_record() -> PushRecord {
        PushRecord {
            token: PushToken {
                platform: PushPlatform::Apns,
                token: "ab".repeat(32),
                environment: Some(PushEnvironment::Sandbox),
            },
            key: [3; 32],
            relay: Credentials::new(
                "01jc8q4m6x2v7s0t3n5r9k1b2d".into(),
                "q5XxAbCdEfGhIjKlMnOpQrStUvWxYz0123456789-_A".into(),
            ),
            status: PushStatus::Refused,
        }
    }

    fn phone(push: Option<PushRecord>) -> Device {
        Device {
            id: DeviceId::from_u128(9),
            name: "Phone".into(),
            token_hash: [7; 32],
            paired_at: 10,
            last_seen: None,
            kind: DeviceKind::Phone,
            push,
        }
    }

    #[test]
    fn push_registrations_round_trip_with_their_relay_secret_and_key() -> io::Result<()> {
        let directory = directory()?;
        let path = directory.join("remote.json");
        let stored = Stored {
            devices: vec![phone(Some(push_record()))],
            ..Stored::default()
        };
        save(&path, &stored)?;
        let text = fs::read_to_string(&path)?;
        for field in [
            "\"platform\": \"apns\"",
            "\"environment\": \"sandbox\"",
            "\"relay_id\": \"01jc8q4m6x2v7s0t3n5r9k1b2d\"",
            "\"status\": \"refused\"",
            "\"push_notifications\": true",
        ] {
            assert!(text.contains(field), "{field} missing from {text}");
        }
        let loaded = load(&path)?;
        assert_eq!(loaded.devices[0].push, Some(push_record()));
        assert!(loaded.push_notifications);
        fs::remove_dir_all(directory)
    }

    #[test]
    fn an_unusable_push_entry_is_dropped_without_unpairing_the_phone() -> io::Result<()> {
        let directory = directory()?;
        let path = directory.join("remote.json");
        save(
            &path,
            &Stored {
                devices: vec![phone(Some(push_record()))],
                ..Stored::default()
            },
        )?;
        let text = fs::read_to_string(&path)?.replace("\"apns\"", "\"carrier-pigeon\"");
        fs::write(&path, text)?;
        let loaded = load(&path)?;
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices[0].push, None);
        fs::remove_dir_all(directory)
    }

    #[test]
    fn devices_saved_before_computers_could_pair_are_phones() -> io::Result<()> {
        let directory = directory()?;
        let path = directory.join("remote.json");
        fs::write(
            &path,
            format!(
                r#"{{"enabled":false,"port":7419,"certificate":null,"private_key":null,
                "devices":[{{"id":"{}","name":"Phone","token_sha256":"{}","paired_at":1,"last_seen":null}}]}}"#,
                DeviceId::from_u128(9),
                "07".repeat(32)
            ),
        )?;
        let loaded = load(&path)?;
        assert_eq!(loaded.devices[0].kind, DeviceKind::Phone);
        assert_eq!(loaded.devices[0].push, None);
        assert!(loaded.push_notifications);
        fs::remove_dir_all(directory)
    }
}
