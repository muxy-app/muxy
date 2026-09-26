use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct ServerPath(
    #[n(0)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub Vec<u8>,
);
