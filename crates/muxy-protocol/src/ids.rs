use std::num::NonZeroU64;

use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct SessionId(#[n(0)] NonZeroU64);

impl SessionId {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl From<NonZeroU64> for SessionId {
    fn from(value: NonZeroU64) -> Self {
        Self(value)
    }
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct RequestId(#[n(0)] pub u32);

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct ChannelId(#[n(0)] pub u32);

pub const CONTROL: ChannelId = ChannelId(0);
