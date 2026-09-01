use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

use super::message::{
    AttachExisting, AttachRequest, Attached, CommandActivityQuery, CommandActivityResult, Empty,
    ExitStatus, MAX_FRAME_PAYLOAD_BYTES, MessageKind, ProtocolFailure, QueryResult, Resize,
    SessionDescriptor, SessionMessage, SessionQuery, TerminateResult,
};

pub const SESSION_MAGIC: [u8; 4] = *b"MXS2";
pub const SESSION_PROTOCOL_VERSION: u16 = 2;
pub const SESSION_PAYLOAD_VERSION: u16 = 1;
pub const HEADER_BYTES: usize = 12;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SessionCodecError {
    #[error("session frame magic is invalid")]
    InvalidMagic,
    #[error("unsupported session protocol version {0}")]
    UnsupportedProtocolVersion(u16),
    #[error("unsupported session payload version {0}")]
    UnsupportedPayloadVersion(u16),
    #[error("unknown session message kind {0}")]
    UnknownMessageKind(u16),
    #[error("session payload exceeds {maximum} bytes: {actual}")]
    PayloadTooLarge { maximum: usize, actual: usize },
    #[error("truncated session frame: expected {expected} bytes, received {actual}")]
    Truncated { expected: usize, actual: usize },
    #[error("session frame length is inconsistent: expected {expected} bytes, received {actual}")]
    InconsistentLength { expected: usize, actual: usize },
    #[error("invalid session JSON payload: {0}")]
    InvalidJson(String),
    #[error("session field {field} is not a canonical uppercase UUID: {value}")]
    InvalidUuid { field: &'static str, value: String },
    #[error("session field {field} contains a NUL byte")]
    NulByte { field: &'static str },
    #[error("session field {field} exceeds {maximum} bytes: {actual}")]
    FieldTooLong {
        field: &'static str,
        maximum: usize,
        actual: usize,
    },
    #[error("session field {field} exceeds {maximum} items: {actual}")]
    TooManyItems {
        field: &'static str,
        maximum: usize,
        actual: usize,
    },
    #[error("terminal size must contain nonzero rows and columns")]
    InvalidTerminalSize,
    #[error("exit status must contain exactly one of code or signal")]
    InvalidExitStatus,
    #[error("session descriptor requires a nonzero shell pid and tty device")]
    InvalidDescriptor,
}

#[derive(Serialize)]
struct VersionedPayload<'a, T> {
    version: u16,
    body: &'a T,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedVersionedPayload<T> {
    version: u16,
    body: T,
}

pub struct SessionCodec;

impl SessionCodec {
    pub fn encode(message: &SessionMessage) -> Result<Vec<u8>, SessionCodecError> {
        message.validate()?;
        let payload = encode_payload(message)?;
        if payload.len() > MAX_FRAME_PAYLOAD_BYTES {
            return Err(SessionCodecError::PayloadTooLarge {
                maximum: MAX_FRAME_PAYLOAD_BYTES,
                actual: payload.len(),
            });
        }
        let payload_len =
            u32::try_from(payload.len()).map_err(|_| SessionCodecError::PayloadTooLarge {
                maximum: MAX_FRAME_PAYLOAD_BYTES,
                actual: payload.len(),
            })?;
        let mut frame = Vec::with_capacity(HEADER_BYTES + payload.len());
        frame.extend_from_slice(&SESSION_MAGIC);
        frame.extend_from_slice(&SESSION_PROTOCOL_VERSION.to_be_bytes());
        frame.extend_from_slice(&(message.kind() as u16).to_be_bytes());
        frame.extend_from_slice(&payload_len.to_be_bytes());
        frame.extend_from_slice(&payload);
        Ok(frame)
    }

    pub fn decode(frame: &[u8]) -> Result<SessionMessage, SessionCodecError> {
        let header = parse_header(frame)?;
        let expected = HEADER_BYTES + header.payload_len;
        if frame.len() < expected {
            return Err(SessionCodecError::Truncated {
                expected,
                actual: frame.len(),
            });
        }
        if frame.len() != expected {
            return Err(SessionCodecError::InconsistentLength {
                expected,
                actual: frame.len(),
            });
        }
        decode_payload(header.kind, &frame[HEADER_BYTES..])
    }
}

#[derive(Default)]
pub struct SessionDecoder {
    frame: Vec<u8>,
    expected: Option<usize>,
}

impl SessionDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, mut bytes: &[u8]) -> Result<Vec<SessionMessage>, SessionCodecError> {
        let mut messages = Vec::new();
        while !bytes.is_empty() {
            if self.frame.len() < HEADER_BYTES {
                let take = (HEADER_BYTES - self.frame.len()).min(bytes.len());
                self.frame.extend_from_slice(&bytes[..take]);
                bytes = &bytes[take..];
                if self.frame.len() < HEADER_BYTES {
                    continue;
                }
                let header = match parse_header(&self.frame) {
                    Ok(header) => header,
                    Err(error) => {
                        self.reset();
                        return Err(error);
                    }
                };
                self.expected = Some(HEADER_BYTES + header.payload_len);
            }
            let expected = self
                .expected
                .expect("complete header has an expected length");
            if self.frame.len() == expected {
                let result = SessionCodec::decode(&self.frame);
                self.reset();
                messages.push(result?);
                continue;
            }
            let take = (expected - self.frame.len()).min(bytes.len());
            self.frame.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
            if self.frame.len() == expected {
                let result = SessionCodec::decode(&self.frame);
                self.reset();
                messages.push(result?);
            }
        }
        Ok(messages)
    }

    pub fn finish(&mut self) -> Result<(), SessionCodecError> {
        if self.frame.is_empty() {
            return Ok(());
        }
        let expected = self.expected.unwrap_or(HEADER_BYTES);
        let actual = self.frame.len();
        self.reset();
        Err(SessionCodecError::Truncated { expected, actual })
    }

    pub fn buffered_len(&self) -> usize {
        self.frame.len()
    }

    fn reset(&mut self) {
        self.frame.clear();
        self.expected = None;
    }
}

struct Header {
    kind: MessageKind,
    payload_len: usize,
}

pub fn validated_payload_length(header: &[u8]) -> Result<usize, SessionCodecError> {
    parse_header(header).map(|header| header.payload_len)
}

fn parse_header(frame: &[u8]) -> Result<Header, SessionCodecError> {
    if frame.len() < HEADER_BYTES {
        return Err(SessionCodecError::Truncated {
            expected: HEADER_BYTES,
            actual: frame.len(),
        });
    }
    if frame[..4] != SESSION_MAGIC {
        return Err(SessionCodecError::InvalidMagic);
    }
    let version = u16::from_be_bytes([frame[4], frame[5]]);
    if version != SESSION_PROTOCOL_VERSION {
        return Err(SessionCodecError::UnsupportedProtocolVersion(version));
    }
    let raw_kind = u16::from_be_bytes([frame[6], frame[7]]);
    let kind =
        MessageKind::from_raw(raw_kind).ok_or(SessionCodecError::UnknownMessageKind(raw_kind))?;
    let payload_len = usize::try_from(u32::from_be_bytes([
        frame[8], frame[9], frame[10], frame[11],
    ]))
    .expect("u32 always fits usize on supported targets");
    if payload_len > MAX_FRAME_PAYLOAD_BYTES {
        return Err(SessionCodecError::PayloadTooLarge {
            maximum: MAX_FRAME_PAYLOAD_BYTES,
            actual: payload_len,
        });
    }
    Ok(Header { kind, payload_len })
}

fn encode_payload(message: &SessionMessage) -> Result<Vec<u8>, SessionCodecError> {
    match message {
        SessionMessage::Replay(bytes)
        | SessionMessage::Output(bytes)
        | SessionMessage::Input(bytes) => Ok(bytes.clone()),
        SessionMessage::AttachCreateOrAttach(value) => encode_json(value),
        SessionMessage::AttachExisting(value) => encode_json(value),
        SessionMessage::Attached(value) => encode_json(value),
        SessionMessage::Resize(value) => encode_json(value),
        SessionMessage::Exited(value) => encode_json(value),
        SessionMessage::ProtocolError(value) => encode_json(value),
        SessionMessage::Query(value) => encode_json(value),
        SessionMessage::QueryResult(value) => encode_json(value),
        SessionMessage::Recover(value) => encode_json(value),
        SessionMessage::Recovered(value) => encode_json(value),
        SessionMessage::CommandActivity(value) => encode_json(value),
        SessionMessage::CommandActivityResult(value) => encode_json(value),
        SessionMessage::TerminateOne(value) => encode_json(value),
        SessionMessage::TerminateAll(value) => encode_json(value),
        SessionMessage::TerminateResult(value) => encode_json(value),
    }
}

fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, SessionCodecError> {
    serde_json::to_vec(&VersionedPayload {
        version: SESSION_PAYLOAD_VERSION,
        body: value,
    })
    .map_err(|error| SessionCodecError::InvalidJson(error.to_string()))
}

fn decode_payload(kind: MessageKind, payload: &[u8]) -> Result<SessionMessage, SessionCodecError> {
    let message = match kind {
        MessageKind::Replay => SessionMessage::Replay(payload.to_vec()),
        MessageKind::Output => SessionMessage::Output(payload.to_vec()),
        MessageKind::Input => SessionMessage::Input(payload.to_vec()),
        MessageKind::AttachCreateOrAttach => {
            SessionMessage::AttachCreateOrAttach(decode_json::<AttachRequest>(payload)?)
        }
        MessageKind::AttachExisting => {
            SessionMessage::AttachExisting(decode_json::<AttachExisting>(payload)?)
        }
        MessageKind::Attached => SessionMessage::Attached(decode_json::<Attached>(payload)?),
        MessageKind::Resize => SessionMessage::Resize(decode_json::<Resize>(payload)?),
        MessageKind::Exited => SessionMessage::Exited(decode_json::<ExitStatus>(payload)?),
        MessageKind::ProtocolError => {
            SessionMessage::ProtocolError(decode_json::<ProtocolFailure>(payload)?)
        }
        MessageKind::Query => SessionMessage::Query(decode_json::<SessionQuery>(payload)?),
        MessageKind::QueryResult => {
            SessionMessage::QueryResult(decode_json::<QueryResult>(payload)?)
        }
        MessageKind::Recover => SessionMessage::Recover(decode_json::<Empty>(payload)?),
        MessageKind::Recovered => {
            SessionMessage::Recovered(decode_json::<Vec<SessionDescriptor>>(payload)?)
        }
        MessageKind::CommandActivity => {
            SessionMessage::CommandActivity(decode_json::<CommandActivityQuery>(payload)?)
        }
        MessageKind::CommandActivityResult => {
            SessionMessage::CommandActivityResult(decode_json::<CommandActivityResult>(payload)?)
        }
        MessageKind::TerminateOne => {
            SessionMessage::TerminateOne(decode_json::<SessionQuery>(payload)?)
        }
        MessageKind::TerminateAll => SessionMessage::TerminateAll(decode_json::<Empty>(payload)?),
        MessageKind::TerminateResult => {
            SessionMessage::TerminateResult(decode_json::<TerminateResult>(payload)?)
        }
    };
    message.validate()?;
    Ok(message)
}

fn decode_json<T: DeserializeOwned>(payload: &[u8]) -> Result<T, SessionCodecError> {
    let payload: OwnedVersionedPayload<T> = serde_json::from_slice(payload)
        .map_err(|error| SessionCodecError::InvalidJson(error.to_string()))?;
    if payload.version != SESSION_PAYLOAD_VERSION {
        return Err(SessionCodecError::UnsupportedPayloadVersion(
            payload.version,
        ));
    }
    Ok(payload.body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{
        CommandActivity, EnvironmentEntry, LaunchSpecification, OwnerMetadata, TerminationOutcome,
    };

    const SESSION_ID: &str = "11111111-2222-4A33-8B44-555555555555";
    const PROJECT_ID: &str = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE";
    const WORKTREE_ID: &str = "99999999-8888-4777-8666-555555555555";

    fn size() -> Resize {
        Resize {
            columns: 80,
            rows: 24,
            width_px: 800,
            height_px: 480,
        }
    }

    fn owner() -> OwnerMetadata {
        OwnerMetadata {
            project_id: PROJECT_ID.to_owned(),
            worktree_id: Some(WORKTREE_ID.to_owned()),
            title: "Terminal".to_owned(),
        }
    }

    fn launch() -> LaunchSpecification {
        LaunchSpecification {
            shell: "/bin/zsh".to_owned(),
            resources_directory: "/Applications/App.app/Contents/Resources/shell".to_owned(),
            working_directory: "/tmp/project".to_owned(),
            startup_command: Some("printf ready".to_owned()),
            environment: vec![EnvironmentEntry {
                key: "TERM".to_owned(),
                value: "xterm-256color".to_owned(),
            }],
        }
    }

    fn descriptor() -> SessionDescriptor {
        SessionDescriptor {
            session_id: SESSION_ID.to_owned(),
            owner: owner(),
            working_directory: "/tmp/project".to_owned(),
            shell_pid: 42,
            tty_device: 7,
            command_activity: CommandActivity::Idle,
        }
    }

    fn structured_messages() -> Vec<SessionMessage> {
        vec![
            SessionMessage::AttachCreateOrAttach(AttachRequest {
                session_id: SESSION_ID.to_owned(),
                owner: owner(),
                launch: launch(),
                size: size(),
            }),
            SessionMessage::AttachExisting(AttachExisting {
                session_id: SESSION_ID.to_owned(),
                size: size(),
            }),
            SessionMessage::Attached(Attached {
                created: true,
                descriptor: descriptor(),
            }),
            SessionMessage::Resize(size()),
            SessionMessage::Exited(ExitStatus {
                code: Some(0),
                signal: None,
            }),
            SessionMessage::ProtocolError(ProtocolFailure {
                code: "invalidRequest".to_owned(),
                message: "invalid request".to_owned(),
            }),
            SessionMessage::Query(SessionQuery {
                session_id: SESSION_ID.to_owned(),
            }),
            SessionMessage::QueryResult(QueryResult::Found(descriptor())),
            SessionMessage::QueryResult(QueryResult::Missing),
            SessionMessage::Recover(Empty {}),
            SessionMessage::Recovered(vec![descriptor()]),
            SessionMessage::CommandActivity(CommandActivityQuery {
                session_id: SESSION_ID.to_owned(),
            }),
            SessionMessage::CommandActivityResult(CommandActivityResult {
                session_id: SESSION_ID.to_owned(),
                activity: CommandActivity::Running,
            }),
            SessionMessage::TerminateOne(SessionQuery {
                session_id: SESSION_ID.to_owned(),
            }),
            SessionMessage::TerminateAll(Empty {}),
            SessionMessage::TerminateResult(TerminateResult {
                outcome: TerminationOutcome::Terminated,
            }),
            SessionMessage::TerminateResult(TerminateResult {
                outcome: TerminationOutcome::NoSessions,
            }),
        ]
    }

    #[test]
    fn every_structured_message_round_trips() {
        for message in structured_messages() {
            let encoded = SessionCodec::encode(&message).unwrap();
            assert_eq!(&encoded[..4], b"MXS2");
            assert_eq!(SessionCodec::decode(&encoded), Ok(message));
        }
    }

    #[test]
    fn raw_terminal_bytes_round_trip_without_json() {
        for message in [
            SessionMessage::Replay(vec![0, 0xFF, b'\n']),
            SessionMessage::Output(vec![0x1B, b'[', b'm']),
            SessionMessage::Input(vec![0, b'a', 0xFE]),
        ] {
            let frame = SessionCodec::encode(&message).unwrap();
            assert_eq!(
                &frame[HEADER_BYTES..],
                match &message {
                    SessionMessage::Replay(bytes)
                    | SessionMessage::Output(bytes)
                    | SessionMessage::Input(bytes) => bytes,
                    _ => unreachable!(),
                }
            );
            assert_eq!(SessionCodec::decode(&frame), Ok(message));
        }
    }

    #[test]
    fn empty_raw_frame_finishes_without_waiting_for_more_input() {
        let message = SessionMessage::Output(Vec::new());
        let frame = SessionCodec::encode(&message).unwrap();
        let mut decoder = SessionDecoder::new();
        assert_eq!(decoder.feed(&frame), Ok(vec![message]));
        assert_eq!(decoder.finish(), Ok(()));
    }

    #[test]
    fn fragmented_and_coalesced_frames_decode_in_order() {
        let first = SessionMessage::Input(vec![1, 2, 3]);
        let second = SessionMessage::QueryResult(QueryResult::Missing);
        let mut bytes = SessionCodec::encode(&first).unwrap();
        bytes.extend(SessionCodec::encode(&second).unwrap());
        let mut decoder = SessionDecoder::new();
        let mut messages = Vec::new();
        for byte in bytes {
            messages.extend(decoder.feed(&[byte]).unwrap());
            assert!(decoder.buffered_len() <= HEADER_BYTES + MAX_FRAME_PAYLOAD_BYTES);
        }
        assert_eq!(messages, [first, second]);
        assert_eq!(decoder.finish(), Ok(()));
    }

    #[test]
    fn payload_boundaries_are_enforced_before_allocation() {
        let maximum = SessionMessage::Output(vec![1; MAX_FRAME_PAYLOAD_BYTES]);
        assert_eq!(
            SessionCodec::decode(&SessionCodec::encode(&maximum).unwrap()),
            Ok(maximum)
        );
        assert_eq!(
            SessionCodec::encode(&SessionMessage::Output(vec![
                1;
                MAX_FRAME_PAYLOAD_BYTES + 1
            ])),
            Err(SessionCodecError::PayloadTooLarge {
                maximum: MAX_FRAME_PAYLOAD_BYTES,
                actual: MAX_FRAME_PAYLOAD_BYTES + 1,
            })
        );
        let mut header = Vec::from(SESSION_MAGIC);
        header.extend_from_slice(&SESSION_PROTOCOL_VERSION.to_be_bytes());
        header.extend_from_slice(&(MessageKind::Output as u16).to_be_bytes());
        header.extend_from_slice(&((MAX_FRAME_PAYLOAD_BYTES + 1) as u32).to_be_bytes());
        let mut decoder = SessionDecoder::new();
        assert_eq!(
            decoder.feed(&header),
            Err(SessionCodecError::PayloadTooLarge {
                maximum: MAX_FRAME_PAYLOAD_BYTES,
                actual: MAX_FRAME_PAYLOAD_BYTES + 1,
            })
        );
        assert_eq!(decoder.buffered_len(), 0);
    }

    #[test]
    fn malformed_headers_and_lengths_are_rejected() {
        let valid = SessionCodec::encode(&SessionMessage::Input(vec![1, 2])).unwrap();
        let mut bad_magic = valid.clone();
        bad_magic[0] = b'X';
        assert_eq!(
            SessionCodec::decode(&bad_magic),
            Err(SessionCodecError::InvalidMagic)
        );
        let mut bad_version = valid.clone();
        bad_version[4..6].copy_from_slice(&99u16.to_be_bytes());
        assert_eq!(
            SessionCodec::decode(&bad_version),
            Err(SessionCodecError::UnsupportedProtocolVersion(99))
        );
        let mut bad_kind = valid.clone();
        bad_kind[6..8].copy_from_slice(&99u16.to_be_bytes());
        assert_eq!(
            SessionCodec::decode(&bad_kind),
            Err(SessionCodecError::UnknownMessageKind(99))
        );
        assert_eq!(
            SessionCodec::decode(&valid[..HEADER_BYTES - 1]),
            Err(SessionCodecError::Truncated {
                expected: HEADER_BYTES,
                actual: HEADER_BYTES - 1,
            })
        );
        assert!(matches!(
            SessionCodec::decode(&valid[..valid.len() - 1]),
            Err(SessionCodecError::Truncated { .. })
        ));
        let mut extra = valid;
        extra.push(0);
        assert!(matches!(
            SessionCodec::decode(&extra),
            Err(SessionCodecError::InconsistentLength { .. })
        ));
    }

    #[test]
    fn truncated_streams_report_the_expected_frame_size() {
        let frame = SessionCodec::encode(&SessionMessage::Output(vec![1, 2, 3])).unwrap();
        let mut decoder = SessionDecoder::new();
        decoder.feed(&frame[..HEADER_BYTES + 1]).unwrap();
        assert_eq!(
            decoder.finish(),
            Err(SessionCodecError::Truncated {
                expected: HEADER_BYTES + 3,
                actual: HEADER_BYTES + 1,
            })
        );
        assert_eq!(decoder.finish(), Ok(()));
    }

    #[test]
    fn invalid_payload_versions_and_json_are_rejected() {
        let mut frame = SessionCodec::encode(&SessionMessage::Recover(Empty {})).unwrap();
        let payload = br#"{"version":99,"body":{}}"#;
        frame.truncate(HEADER_BYTES);
        frame[8..12].copy_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(payload);
        assert_eq!(
            SessionCodec::decode(&frame),
            Err(SessionCodecError::UnsupportedPayloadVersion(99))
        );
        frame.truncate(HEADER_BYTES);
        frame[8..12].copy_from_slice(&1u32.to_be_bytes());
        frame.push(b'{');
        assert!(matches!(
            SessionCodec::decode(&frame),
            Err(SessionCodecError::InvalidJson(_))
        ));
    }

    #[test]
    fn canonical_ids_nul_values_and_terminal_invariants_are_rejected() {
        let mut request = AttachRequest {
            session_id: SESSION_ID.to_lowercase(),
            owner: owner(),
            launch: launch(),
            size: size(),
        };
        assert!(matches!(
            SessionCodec::encode(&SessionMessage::AttachCreateOrAttach(request.clone())),
            Err(SessionCodecError::InvalidUuid {
                field: "session_id",
                ..
            })
        ));
        request.session_id = "not-a-uuid".to_owned();
        assert!(matches!(
            SessionCodec::encode(&SessionMessage::AttachCreateOrAttach(request.clone())),
            Err(SessionCodecError::InvalidUuid { .. })
        ));
        request.session_id = SESSION_ID.to_owned();
        request.launch.environment[0].value = "bad\0value".to_owned();
        assert_eq!(
            SessionCodec::encode(&SessionMessage::AttachCreateOrAttach(request)),
            Err(SessionCodecError::NulByte {
                field: "environment.value"
            })
        );
        assert_eq!(
            SessionCodec::encode(&SessionMessage::Resize(Resize {
                columns: 0,
                ..size()
            })),
            Err(SessionCodecError::InvalidTerminalSize)
        );
        assert_eq!(
            SessionCodec::encode(&SessionMessage::Exited(ExitStatus {
                code: None,
                signal: None,
            })),
            Err(SessionCodecError::InvalidExitStatus)
        );
    }

    #[test]
    fn structured_field_and_count_limits_are_enforced() {
        let mut request = AttachRequest {
            session_id: SESSION_ID.to_owned(),
            owner: owner(),
            launch: launch(),
            size: size(),
        };
        request.owner.title = "x".repeat(crate::session::MAX_TITLE_BYTES + 1);
        assert!(matches!(
            SessionCodec::encode(&SessionMessage::AttachCreateOrAttach(request.clone())),
            Err(SessionCodecError::FieldTooLong { field: "title", .. })
        ));
        request.owner.title = "Terminal".to_owned();
        request.launch.environment = (0..=crate::session::MAX_ENVIRONMENT_ENTRIES)
            .map(|index| EnvironmentEntry {
                key: format!("K{index}"),
                value: "value".to_owned(),
            })
            .collect();
        assert!(matches!(
            SessionCodec::encode(&SessionMessage::AttachCreateOrAttach(request)),
            Err(SessionCodecError::TooManyItems {
                field: "environment",
                ..
            })
        ));
    }

    #[test]
    fn a_decoder_can_accept_a_new_connection_after_rejection() {
        let mut decoder = SessionDecoder::new();
        let mut invalid = SessionCodec::encode(&SessionMessage::Input(vec![1])).unwrap();
        invalid[0] = 0;
        assert_eq!(decoder.feed(&invalid), Err(SessionCodecError::InvalidMagic));
        let valid = SessionMessage::QueryResult(QueryResult::Missing);
        assert_eq!(
            decoder.feed(&SessionCodec::encode(&valid).unwrap()),
            Ok(vec![valid])
        );
    }
}
