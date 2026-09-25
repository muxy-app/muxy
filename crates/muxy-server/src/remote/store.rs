use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use muxy_protocol::transport::tls::TlsIdentity;
use muxy_protocol::{DeviceId, MAX_DEVICES, RemoteAccessSettings, validate_device_name};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub(super) struct Device {
    pub(super) id: DeviceId,
    pub(super) name: String,
    /// SHA-256 of the device token; the token itself is never stored.
    pub(super) token_hash: [u8; 32],
    pub(super) paired_at: u64,
    pub(super) last_seen: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Stored {
    pub(super) settings: RemoteAccessSettings,
    pub(super) identity: Option<TlsIdentity>,
    pub(super) devices: Vec<Device>,
}

#[derive(Deserialize, Serialize)]
struct Document {
    enabled: bool,
    port: u16,
    certificate: Option<String>,
    private_key: Option<String>,
    devices: Vec<DocumentDevice>,
}

#[derive(Deserialize, Serialize)]
struct DocumentDevice {
    id: DeviceId,
    name: String,
    token_sha256: String,
    paired_at: u64,
    last_seen: Option<u64>,
}

pub(super) fn load(path: &Path) -> io::Result<Stored> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Stored::default()),
        Err(error) => return Err(error),
    };
    let document: Document = serde_json::from_slice(&bytes)?;
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
            certificate: unhex(&certificate)?,
            private_key: unhex(&private_key)?,
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
                token_hash: unhex(&device.token_sha256)?.try_into().ok()?,
                paired_at: device.paired_at,
                last_seen: device.last_seen,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Stored {
        settings,
        identity,
        devices,
    })
}

pub(super) fn save(path: &Path, stored: &Stored) -> io::Result<()> {
    let document = Document {
        enabled: stored.settings.enabled,
        port: stored.settings.port,
        certificate: stored
            .identity
            .as_ref()
            .map(|identity| hex(&identity.certificate)),
        private_key: stored
            .identity
            .as_ref()
            .map(|identity| hex(&identity.private_key)),
        devices: stored
            .devices
            .iter()
            .map(|device| DocumentDevice {
                id: device.id,
                name: device.name.clone(),
                token_sha256: hex(&device.token_hash),
                paired_at: device.paired_at,
                last_seen: device.last_seen,
            })
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&document)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("remote access store has no parent"))?;
    let temporary = parent.join(format!(".remote-{}.tmp", muxy_protocol::OperationId::new()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            u8::try_from(high * 16 + low).ok()
        })
        .collect()
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
            }],
        };
        save(&path, &stored)?;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        let loaded = load(&path)?;
        assert_eq!(loaded.settings, stored.settings);
        assert_eq!(loaded.identity, stored.identity);
        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices[0].token_hash, [7; 32]);
        assert_eq!(loaded.devices[0].last_seen, Some(11));
        fs::remove_dir_all(directory)
    }

    #[test]
    fn unreadable_or_inconsistent_stores_are_errors() -> io::Result<()> {
        let directory = directory()?;
        let path = directory.join("remote.json");
        for contents in [
            "not json",
            r#"{"enabled":false,"port":80,"certificate":null,"private_key":null,"devices":[]}"#,
            r#"{"enabled":true,"port":7419,"certificate":null,"private_key":null,"devices":[]}"#,
            r#"{"enabled":false,"port":7419,"certificate":"00","private_key":null,"devices":[]}"#,
        ] {
            fs::write(&path, contents)?;
            assert!(load(&path).is_err(), "{contents}");
        }
        fs::remove_dir_all(directory)
    }
}
