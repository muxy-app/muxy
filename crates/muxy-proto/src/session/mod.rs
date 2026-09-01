mod codec;
mod message;

pub use codec::{
    HEADER_BYTES, SESSION_MAGIC, SESSION_PAYLOAD_VERSION, SESSION_PROTOCOL_VERSION, SessionCodec,
    SessionCodecError, SessionDecoder,
};
pub use message::{
    AttachExisting, AttachRequest, Attached, CommandActivity, CommandActivityQuery,
    CommandActivityResult, Empty, EnvironmentEntry, ExitStatus, LaunchSpecification,
    MAX_ENVIRONMENT_BYTES, MAX_ENVIRONMENT_ENTRIES, MAX_FIELD_BYTES, MAX_FRAME_PAYLOAD_BYTES,
    MAX_PATH_BYTES, MAX_STARTUP_COMMAND_BYTES, MAX_TITLE_BYTES, MessageKind, OwnerMetadata,
    ProtocolFailure, QueryResult, Resize, SessionDescriptor, SessionMessage, SessionQuery,
    TerminateResult, TerminationOutcome,
};
