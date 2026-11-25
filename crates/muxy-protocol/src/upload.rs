use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ErrorCode, OperationId, ServerPath, SessionId, validate_absolute_path};

pub const MAX_UPLOAD_CHUNK: usize = 1024 * 1024;
pub const MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_UPLOAD_NAME: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct UploadChunk {
    #[n(0)]
    pub session: SessionId,
    #[n(1)]
    pub upload: OperationId,
    #[n(2)]
    pub name: String,
    #[n(3)]
    pub offset: u64,
    #[n(4)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub bytes: Vec<u8>,
    #[n(5)]
    pub last: bool,
}

impl UploadChunk {
    pub fn validate(&self) -> Result<(), ErrorCode> {
        let end = u64::try_from(self.bytes.len())
            .ok()
            .and_then(|length| self.offset.checked_add(length));
        if self.bytes.len() > MAX_UPLOAD_CHUNK
            || self.name.len() > MAX_UPLOAD_NAME
            || end.is_none_or(|end| end > MAX_UPLOAD_BYTES)
        {
            return Err(ErrorCode::BadRequest);
        }
        Ok(())
    }
}

pub fn validate_uploaded(path: Option<&ServerPath>) -> Result<(), ErrorCode> {
    path.map_or(Ok(()), validate_absolute_path)
}
