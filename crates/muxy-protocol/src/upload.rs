use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::{ErrorCode, OperationId, ServerPath, SessionId};

/// The most one upload chunk carries.
pub const MAX_UPLOAD_CHUNK: usize = 1024 * 1024;
/// The largest file one upload sends.
pub const MAX_UPLOAD_BYTES: u64 = 100 * 1024 * 1024;
/// The longest name an upload gives its file, before the server shortens it.
pub const MAX_UPLOAD_NAME: usize = 1024;

/// One piece of a file sent to the server's computer, such as an image
/// dropped on a terminal there, so programs there can open it by path. Chunks
/// arrive in order. The server keeps the file with the session.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct UploadChunk {
    #[n(0)]
    pub session: SessionId,
    /// The same in every chunk of one file.
    #[n(1)]
    pub upload: OperationId,
    /// The file's name. The server keeps a safe last component of it.
    #[n(2)]
    pub name: String,
    /// Where `bytes` start in the file: the length of the chunks before.
    #[n(3)]
    pub offset: u64,
    #[n(4)]
    #[cbor(with = "crate::wire::cbor::bytes")]
    pub bytes: Vec<u8>,
    /// The file is complete after this chunk.
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

/// An `Uploaded` reply: the file's absolute path once its last chunk arrived.
pub fn validate_uploaded(path: Option<&ServerPath>) -> Result<(), ErrorCode> {
    match path {
        Some(path) if path.0.len() > 4096 || path.0.contains(&0) || !path.0.starts_with(b"/") => {
            Err(ErrorCode::BadPath)
        }
        _ => Ok(()),
    }
}
