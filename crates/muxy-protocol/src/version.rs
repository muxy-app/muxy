use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct Version(#[n(0)] pub u16);

/// Builds before V2. Only recognised so they can be told to update.
pub const V1: Version = Version(1);
/// Fields and variants carry numbers, so additive changes keep the version.
pub const V2: Version = Version(2);
/// Bumped only by a breaking change.
pub const CURRENT: Version = V2;
/// During beta a build speaks only its current version.
pub const SUPPORTED: &[Version] = &[CURRENT];

/// Something a server can do that a client checks before offering it.
///
/// Add one only when a client must change its UI for older servers. Numbers
/// are permanent.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Encode, Decode,
)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct Feature(#[n(0)] pub u16);

/// Features this build's server supports.
pub const FEATURES: &[Feature] = &[];
