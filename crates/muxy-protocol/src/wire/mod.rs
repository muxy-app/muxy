//! Physical framing and serialization for `muxy-protocol` messages.
//!
//! This boundary validates bytes and codecs, not peer policy or runtime
//! behavior.

pub mod cbor;
mod codec;
mod error;
mod header;
mod kind;
mod stream;

pub use codec::{decode, encode};
pub use error::WireError;
pub use header::{HEADER_LEN, Header, MAX_FRAME, legacy_version_unsupported};
pub use kind::MessageKind;
pub use stream::{Decoder, Encoder};
