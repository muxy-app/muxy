use std::fmt;

use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::ErrorCode;
use crate::wire::cbor::open_enum;

pub const PUSH_KEY_BYTES: usize = 32;
const MIN_APNS_TOKEN: usize = 64;
const MAX_APNS_TOKEN: usize = 200;
const MIN_FCM_TOKEN: usize = 32;
const MAX_FCM_TOKEN: usize = 4096;

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum PushPlatform {
        Apns = 0,
        Fcm = 1,
    }
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum PushEnvironment {
        Sandbox = 0,
        Production = 1,
    }
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum PushStatus {
        Active = 0,
        Pending = 1,
        Refused = 2,
        Disabled = 3,
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct PushToken {
    #[n(0)]
    pub platform: PushPlatform,
    #[n(1)]
    pub token: String,
    #[n(2)]
    pub environment: Option<PushEnvironment>,
}

impl fmt::Debug for PushToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PushToken")
            .field("platform", &self.platform)
            .field("environment", &self.environment)
            .finish_non_exhaustive()
    }
}

impl PushToken {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let valid = match (self.platform, self.environment) {
            (PushPlatform::Apns, Some(PushEnvironment::Sandbox | PushEnvironment::Production)) => {
                (MIN_APNS_TOKEN..=MAX_APNS_TOKEN).contains(&self.token.len())
                    && self.token.bytes().all(|byte| byte.is_ascii_hexdigit())
            }
            (PushPlatform::Fcm, None) => {
                (MIN_FCM_TOKEN..=MAX_FCM_TOKEN).contains(&self.token.len())
                    && self.token.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':' | b'.' | b'-')
                    })
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(ErrorCode::BadRequest)
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct PushRegistration {
    #[n(0)]
    pub status: PushStatus,
    #[n(1)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub key: Option<[u8; PUSH_KEY_BYTES]>,
}

impl fmt::Debug for PushRegistration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PushRegistration")
            .field("status", &self.status)
            .field("key", &self.key.map(|_| "…"))
            .finish()
    }
}

impl PushRegistration {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        match (self.status, self.key) {
            (PushStatus::Disabled, Some(_))
            | (PushStatus::Active | PushStatus::Pending | PushStatus::Refused, None) => {
                Err(ErrorCode::BadRequest)
            }
            _ => Ok(()),
        }
    }
}
