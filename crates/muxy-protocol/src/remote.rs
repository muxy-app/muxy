//! Network access for paired devices: settings, pairing, and credentials.

use std::fmt::{self, Write};

use serde::{Deserialize, Serialize};

use crate::{DeviceId, ErrorCode, ServerIdentity};

pub const DEFAULT_REMOTE_PORT: u16 = 7419;
pub const MAX_DEVICES: usize = 64;
pub const MAX_DEVICE_NAME: usize = 64;
pub const MAX_PAIRING_HOSTS: usize = 8;
const MAX_HOST: usize = 253;
const MAX_SERVER_NAME: usize = 255;
const MAX_STATUS: usize = 1024;
const MAX_LINK: usize = 4096;
const LINK_PREFIX: &str = "muxy://pair?";
const LINK_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RemoteAccessSettings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for RemoteAccessSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_REMOTE_PORT,
        }
    }
}

impl RemoteAccessSettings {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.port >= 1024 {
            Ok(())
        } else {
            Err(ErrorCode::BadRequest)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ListenerStatus {
    Disabled,
    Listening,
    Failed(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: DeviceId,
    pub name: String,
    /// Unix seconds.
    pub paired_at: u64,
    /// Unix seconds of the latest successful authentication.
    pub last_seen: Option<u64>,
    pub connected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RemoteAccessState {
    pub revision: u64,
    pub settings: RemoteAccessSettings,
    pub status: ListenerStatus,
    /// Unix seconds at which the pending pairing offer expires.
    pub pairing_expires_at: Option<u64>,
    pub devices: Vec<PairedDevice>,
}

impl RemoteAccessState {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        self.settings.validate()?;
        if self.devices.len() > MAX_DEVICES
            || matches!(&self.status, ListenerStatus::Failed(message) if message.len() > MAX_STATUS)
        {
            return Err(ErrorCode::BadRequest);
        }
        for device in &self.devices {
            validate_device_name(&device.name)?;
        }
        Ok(())
    }
}

/// Everything a phone needs to reach and pin a server, carried by the pairing link.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PairingInvite {
    pub hosts: Vec<String>,
    pub port: u16,
    /// SHA-256 of the server's TLS certificate.
    pub fingerprint: [u8; 32],
    pub secret: [u8; 16],
}

impl fmt::Debug for PairingInvite {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingInvite")
            .field("hosts", &self.hosts)
            .field("port", &self.port)
            .field("fingerprint", &hex(&self.fingerprint))
            .finish_non_exhaustive()
    }
}

impl PairingInvite {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.hosts.is_empty()
            || self.hosts.len() > MAX_PAIRING_HOSTS
            || self.port < 1024
            || !self.hosts.iter().all(|host| is_host(host))
        {
            Err(ErrorCode::BadRequest)
        } else {
            Ok(())
        }
    }

    pub fn to_link(&self) -> String {
        let mut link = format!("{LINK_PREFIX}v={LINK_VERSION}");
        for host in &self.hosts {
            link.push_str("&h=");
            link.push_str(host);
        }
        let _ = write!(
            link,
            "&p={}&f={}&s={}",
            self.port,
            hex(&self.fingerprint),
            hex(&self.secret)
        );
        link
    }

    pub fn parse_link(link: &str) -> Result<Self, ErrorCode> {
        let query = link
            .strip_prefix(LINK_PREFIX)
            .filter(|_| link.len() <= MAX_LINK)
            .ok_or(ErrorCode::BadRequest)?;
        let mut version = None;
        let mut hosts = Vec::new();
        let mut port = None;
        let mut fingerprint = None;
        let mut secret = None;
        for field in query.split('&') {
            let (key, value) = field.split_once('=').ok_or(ErrorCode::BadRequest)?;
            let repeated = match key {
                "v" => version.replace(value).is_some(),
                "h" => {
                    hosts.push(value.to_owned());
                    false
                }
                "p" => port.replace(parse_port(value)?).is_some(),
                "f" => fingerprint.replace(unhex(value)?).is_some(),
                "s" => secret.replace(unhex(value)?).is_some(),
                _ => return Err(ErrorCode::BadRequest),
            };
            if repeated {
                return Err(ErrorCode::BadRequest);
            }
        }
        if version != Some(LINK_VERSION) {
            return Err(ErrorCode::BadRequest);
        }
        let invite = Self {
            hosts,
            port: port.ok_or(ErrorCode::BadRequest)?,
            fingerprint: fingerprint.ok_or(ErrorCode::BadRequest)?,
            secret: secret.ok_or(ErrorCode::BadRequest)?,
        };
        invite.validate()?;
        Ok(invite)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PairingOffer {
    pub invite: PairingInvite,
    /// Unix seconds.
    pub expires_at: u64,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PairRequest {
    pub secret: [u8; 16],
    pub name: String,
}

impl fmt::Debug for PairRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairRequest")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl PairRequest {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        validate_device_name(&self.name)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeviceCredential {
    pub device: DeviceId,
    pub token: [u8; 32],
}

impl fmt::Debug for DeviceCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceCredential")
            .field("device", &self.device)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Paired {
    pub credential: DeviceCredential,
    pub server: ServerIdentity,
    pub name: String,
}

impl Paired {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        if self.name.trim().is_empty()
            || self.name.len() > MAX_SERVER_NAME
            || self.name.chars().any(char::is_control)
        {
            Err(ErrorCode::BadRequest)
        } else {
            Ok(())
        }
    }
}

pub fn validate_device_name(name: &str) -> Result<(), ErrorCode> {
    if name.trim().is_empty() || name.len() > MAX_DEVICE_NAME || name.chars().any(char::is_control)
    {
        Err(ErrorCode::BadRequest)
    } else {
        Ok(())
    }
}

fn is_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= MAX_HOST
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

fn parse_port(value: &str) -> Result<u16, ErrorCode> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ErrorCode::BadRequest);
    }
    value.parse().map_err(|_| ErrorCode::BadRequest)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn unhex<const N: usize>(value: &str) -> Result<[u8; N], ErrorCode> {
    let digits = value.as_bytes();
    if digits.len() != N * 2 {
        return Err(ErrorCode::BadRequest);
    }
    let mut bytes = [0; N];
    for (byte, pair) in bytes.iter_mut().zip(digits.chunks_exact(2)) {
        let high = char::from(pair[0]).to_digit(16);
        let low = char::from(pair[1]).to_digit(16);
        let (Some(high), Some(low)) = (high, low) else {
            return Err(ErrorCode::BadRequest);
        };
        *byte = u8::try_from(high * 16 + low).map_err(|_| ErrorCode::BadRequest)?;
    }
    Ok(bytes)
}
