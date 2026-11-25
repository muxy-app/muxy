use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct Version(#[n(0)] pub u16);

pub const V1: Version = Version(1);
pub const V2: Version = Version(2);
pub const CURRENT: Version = V2;
pub const SUPPORTED: &[Version] = &[CURRENT];

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct Feature(#[n(0)] pub u16);

impl Feature {
    pub const TERMINAL_INPUT: Self = Self(1);
    pub const SESSION_ENVIRONMENT: Self = Self(2);
    pub const PAIRED_COMPUTERS: Self = Self(3);
    pub const SESSION_MOVE: Self = Self(4);
    pub const PUSH_NOTIFICATIONS: Self = Self(5);
}

pub const FEATURES: &[Feature] = &[
    Feature::TERMINAL_INPUT,
    Feature::SESSION_ENVIRONMENT,
    Feature::PAIRED_COMPUTERS,
    Feature::SESSION_MOVE,
    Feature::PUSH_NOTIFICATIONS,
];
