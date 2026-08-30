use super::messages::{MAX_STREAM_CHUNK_BYTES, MAX_STRUCTURED_FRAME_BYTES, ProtocolVersion};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::VecDeque;
use std::fmt::{Display, Formatter};

pub const MAGIC: [u8; 4] = *b"MXS8";
pub const HEADER_BYTES: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FrameKind {
    Hello = 1,
    HelloAccepted = 2,
    VersionMismatch = 3,
    ControlRequest = 10,
    ControlResponse = 11,
    Attach = 20,
    Attached = 21,
    Input = 22,
    Output = 23,
    Resize = 24,
    Exited = 25,
    Failure = 26,
}

impl FrameKind {
    pub fn parse(value: u16) -> Result<Self, CodecError> {
        match value {
            1 => Ok(Self::Hello),
            2 => Ok(Self::HelloAccepted),
            3 => Ok(Self::VersionMismatch),
            10 => Ok(Self::ControlRequest),
            11 => Ok(Self::ControlResponse),
            20 => Ok(Self::Attach),
            21 => Ok(Self::Attached),
            22 => Ok(Self::Input),
            23 => Ok(Self::Output),
            24 => Ok(Self::Resize),
            25 => Ok(Self::Exited),
            26 => Ok(Self::Failure),
            _ => Err(CodecError::UnknownFrameKind(value)),
        }
    }

    pub fn payload_limit(self) -> usize {
        match self {
            Self::Input => MAX_STREAM_CHUNK_BYTES,
            Self::Output => MAX_STREAM_CHUNK_BYTES + 8,
            _ => MAX_STRUCTURED_FRAME_BYTES,
        }
    }

    pub fn requires_request_id(self) -> bool {
        matches!(self, Self::ControlRequest | Self::ControlResponse)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameHeader {
    pub version: ProtocolVersion,
    pub kind: FrameKind,
    pub flags: u16,
    pub request_id: u64,
    pub payload_len: u32,
}

impl FrameHeader {
    pub fn new(kind: FrameKind, request_id: u64, payload_len: usize) -> Result<Self, CodecError> {
        let payload_len = u32::try_from(payload_len).map_err(|_| CodecError::PayloadTooLarge {
            kind,
            length: payload_len,
        })?;
        let header = Self {
            version: ProtocolVersion::CURRENT,
            kind,
            flags: 0,
            request_id,
            payload_len,
        };
        header.validate()?;
        Ok(header)
    }

    pub fn encode(self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0u8; HEADER_BYTES];
        bytes[0..4].copy_from_slice(&MAGIC);
        bytes[4..6].copy_from_slice(&self.version.major.to_be_bytes());
        bytes[6..8].copy_from_slice(&self.version.minor.to_be_bytes());
        bytes[8..10].copy_from_slice(&(self.kind as u16).to_be_bytes());
        bytes[10..12].copy_from_slice(&self.flags.to_be_bytes());
        bytes[12..20].copy_from_slice(&self.request_id.to_be_bytes());
        bytes[20..24].copy_from_slice(&self.payload_len.to_be_bytes());
        bytes
    }

    pub fn validate(self) -> Result<(), CodecError> {
        let length = self.payload_len as usize;
        if length > self.kind.payload_limit() {
            return Err(CodecError::PayloadTooLarge {
                kind: self.kind,
                length,
            });
        }
        if self.kind.requires_request_id() && self.request_id == 0 {
            return Err(CodecError::MissingRequestId(self.kind));
        }
        if self.flags != 0 {
            return Err(CodecError::UnsupportedFlags(self.flags));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedFrame<'a> {
    pub header: FrameHeader,
    pub payload: &'a [u8],
}

impl DecodedFrame<'_> {
    pub fn correlates_with(self, request: FrameHeader) -> bool {
        request.kind == FrameKind::ControlRequest
            && self.header.kind == FrameKind::ControlResponse
            && request.request_id != 0
            && self.header.request_id == request.request_id
    }
}

pub fn decode_header(bytes: &[u8]) -> Result<FrameHeader, CodecError> {
    if bytes.len() < HEADER_BYTES {
        return Err(CodecError::TruncatedHeader(bytes.len()));
    }
    if bytes[0..4] != MAGIC {
        return Err(CodecError::InvalidMagic);
    }
    let major = u16::from_be_bytes([bytes[4], bytes[5]]);
    let minor = u16::from_be_bytes([bytes[6], bytes[7]]);
    let kind = FrameKind::parse(u16::from_be_bytes([bytes[8], bytes[9]]))?;
    let flags = u16::from_be_bytes([bytes[10], bytes[11]]);
    let request_id = u64::from_be_bytes(
        bytes[12..20]
            .try_into()
            .map_err(|_| CodecError::InvalidHeader)?,
    );
    let payload_len = u32::from_be_bytes(
        bytes[20..24]
            .try_into()
            .map_err(|_| CodecError::InvalidHeader)?,
    );
    let header = FrameHeader {
        version: ProtocolVersion { major, minor },
        kind,
        flags,
        request_id,
        payload_len,
    };
    header.validate()?;
    Ok(header)
}

pub fn decode_frame(bytes: &[u8]) -> Result<DecodedFrame<'_>, CodecError> {
    let header = decode_header(bytes)?;
    let expected = HEADER_BYTES
        .checked_add(header.payload_len as usize)
        .ok_or(CodecError::InvalidHeader)?;
    if bytes.len() < expected {
        return Err(CodecError::TruncatedPayload {
            expected: header.payload_len as usize,
            received: bytes.len().saturating_sub(HEADER_BYTES),
        });
    }
    if bytes.len() != expected {
        return Err(CodecError::TrailingBytes(bytes.len() - expected));
    }
    Ok(DecodedFrame {
        header,
        payload: &bytes[HEADER_BYTES..expected],
    })
}

pub fn encode_frame(
    kind: FrameKind,
    request_id: u64,
    payload: &[u8],
) -> Result<Vec<u8>, CodecError> {
    let header = FrameHeader::new(kind, request_id, payload.len())?;
    let mut bytes = Vec::with_capacity(HEADER_BYTES + payload.len());
    bytes.extend_from_slice(&header.encode());
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

pub fn encode_structured<T: Serialize>(value: &T) -> Result<Vec<u8>, CodecError> {
    let bytes = serde_json::to_vec(value).map_err(|error| CodecError::Json(error.to_string()))?;
    if bytes.len() > MAX_STRUCTURED_FRAME_BYTES {
        return Err(CodecError::StructuredPayloadTooLarge(bytes.len()));
    }
    Ok(bytes)
}

pub fn decode_structured<T: DeserializeOwned>(payload: &[u8]) -> Result<T, CodecError> {
    if payload.len() > MAX_STRUCTURED_FRAME_BYTES {
        return Err(CodecError::StructuredPayloadTooLarge(payload.len()));
    }
    serde_json::from_slice(payload).map_err(|error| CodecError::Json(error.to_string()))
}

pub fn encode_output(sequence: u64, bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    if bytes.len() > MAX_STREAM_CHUNK_BYTES {
        return Err(CodecError::StreamChunkTooLarge(bytes.len()));
    }
    let mut payload = Vec::with_capacity(8 + bytes.len());
    payload.extend_from_slice(&sequence.to_be_bytes());
    payload.extend_from_slice(bytes);
    Ok(payload)
}

pub fn decode_output(payload: &[u8]) -> Result<(u64, &[u8]), CodecError> {
    if payload.len() < 8 {
        return Err(CodecError::TruncatedOutput(payload.len()));
    }
    if payload.len() - 8 > MAX_STREAM_CHUNK_BYTES {
        return Err(CodecError::StreamChunkTooLarge(payload.len() - 8));
    }
    let sequence = u64::from_be_bytes(
        payload[..8]
            .try_into()
            .map_err(|_| CodecError::InvalidHeader)?,
    );
    Ok((sequence, &payload[8..]))
}

pub struct FrameDecoder {
    header_bytes: [u8; HEADER_BYTES],
    header_len: usize,
    pending_header: Option<FrameHeader>,
    payload: Vec<u8>,
    frames: VecDeque<(FrameHeader, Vec<u8>)>,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self {
            header_bytes: [0; HEADER_BYTES],
            header_len: 0,
            pending_header: None,
            payload: Vec::new(),
            frames: VecDeque::new(),
        }
    }
}

impl FrameDecoder {
    pub fn buffered_len(&self) -> usize {
        self.header_len
            + self.payload.len()
            + self
                .frames
                .iter()
                .map(|(_, payload)| HEADER_BYTES + payload.len())
                .sum::<usize>()
    }

    pub fn push(&mut self, mut bytes: &[u8]) -> Result<(), CodecError> {
        while !bytes.is_empty() {
            if self.pending_header.is_none() {
                let take = (HEADER_BYTES - self.header_len).min(bytes.len());
                self.header_bytes[self.header_len..self.header_len + take]
                    .copy_from_slice(&bytes[..take]);
                self.header_len += take;
                bytes = &bytes[take..];
                if self.header_len < HEADER_BYTES {
                    break;
                }
                let header = decode_header(&self.header_bytes);
                self.header_len = 0;
                let header = header?;
                self.payload = Vec::with_capacity(header.payload_len as usize);
                if header.payload_len == 0 {
                    self.frames.push_back((header, Vec::new()));
                    continue;
                }
                self.pending_header = Some(header);
            }

            let header = self.pending_header.ok_or(CodecError::InvalidHeader)?;
            let remaining = header.payload_len as usize - self.payload.len();
            let take = remaining.min(bytes.len());
            self.payload.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
            if self.payload.len() == header.payload_len as usize {
                self.frames
                    .push_back((header, std::mem::take(&mut self.payload)));
                self.pending_header = None;
            }
        }
        Ok(())
    }

    pub fn next_frame(&mut self) -> Result<Option<(FrameHeader, Vec<u8>)>, CodecError> {
        Ok(self.frames.pop_front())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodecError {
    TruncatedHeader(usize),
    InvalidMagic,
    InvalidHeader,
    UnknownFrameKind(u16),
    UnsupportedFlags(u16),
    MissingRequestId(FrameKind),
    PayloadTooLarge { kind: FrameKind, length: usize },
    StructuredPayloadTooLarge(usize),
    StreamChunkTooLarge(usize),
    TruncatedPayload { expected: usize, received: usize },
    TruncatedOutput(usize),
    TrailingBytes(usize),
    Json(String),
}

impl Display for CodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TruncatedHeader(length) => write!(formatter, "truncated frame header: {length}"),
            Self::InvalidMagic => formatter.write_str("invalid frame magic"),
            Self::InvalidHeader => formatter.write_str("invalid frame header"),
            Self::UnknownFrameKind(kind) => write!(formatter, "unknown frame kind: {kind}"),
            Self::UnsupportedFlags(flags) => write!(formatter, "unsupported frame flags: {flags}"),
            Self::MissingRequestId(kind) => write!(formatter, "missing request ID for {kind:?}"),
            Self::PayloadTooLarge { kind, length } => {
                write!(formatter, "payload for {kind:?} is too large: {length}")
            }
            Self::StructuredPayloadTooLarge(length) => {
                write!(formatter, "structured payload is too large: {length}")
            }
            Self::StreamChunkTooLarge(length) => {
                write!(formatter, "stream chunk is too large: {length}")
            }
            Self::TruncatedPayload { expected, received } => {
                write!(
                    formatter,
                    "truncated payload: expected {expected}, received {received}"
                )
            }
            Self::TruncatedOutput(length) => {
                write!(formatter, "truncated output payload: {length}")
            }
            Self::TrailingBytes(length) => write!(formatter, "frame has trailing bytes: {length}"),
            Self::Json(message) => write!(formatter, "invalid structured payload: {message}"),
        }
    }
}

impl std::error::Error for CodecError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::messages::{
        ClientKind, ControlRequest, Hello, PROTOCOL_MAJOR, PROTOCOL_MINOR,
    };

    #[test]
    fn header_is_exact_big_endian_24_byte_contract() {
        let header = FrameHeader::new(FrameKind::ControlRequest, 0x0102_0304_0506_0708, 3).unwrap();
        let bytes = header.encode();
        assert_eq!(bytes.len(), 24);
        assert_eq!(&bytes[0..4], b"MXS8");
        assert_eq!(&bytes[4..12], &[0, 1, 0, 0, 0, 10, 0, 0]);
        assert_eq!(&bytes[12..20], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(&bytes[20..24], &[0, 0, 0, 3]);
        assert_eq!(decode_header(&bytes).unwrap(), header);
    }

    #[test]
    fn decoder_rejects_length_before_waiting_for_or_copying_payload() {
        let mut bytes = FrameHeader {
            version: ProtocolVersion::CURRENT,
            kind: FrameKind::Input,
            flags: 0,
            request_id: 0,
            payload_len: (MAX_STREAM_CHUNK_BYTES + 1) as u32,
        }
        .encode();
        assert!(matches!(
            decode_header(&bytes),
            Err(CodecError::PayloadTooLarge {
                kind: FrameKind::Input,
                ..
            })
        ));
        bytes[8..10].copy_from_slice(&999u16.to_be_bytes());
        assert_eq!(
            decode_header(&bytes),
            Err(CodecError::UnknownFrameKind(999))
        );
    }

    #[test]
    fn incremental_decoder_validates_header_before_buffering_payload() {
        let header = FrameHeader {
            version: ProtocolVersion::CURRENT,
            kind: FrameKind::Input,
            flags: 0,
            request_id: 0,
            payload_len: (MAX_STREAM_CHUNK_BYTES + 1) as u32,
        }
        .encode();
        let mut supplied = Vec::with_capacity(HEADER_BYTES + MAX_STRUCTURED_FRAME_BYTES);
        supplied.extend_from_slice(&header);
        supplied.resize(HEADER_BYTES + MAX_STRUCTURED_FRAME_BYTES, 0);
        let mut decoder = FrameDecoder::default();
        assert!(matches!(
            decoder.push(&supplied),
            Err(CodecError::PayloadTooLarge {
                kind: FrameKind::Input,
                ..
            })
        ));
        assert!(decoder.buffered_len() <= HEADER_BYTES);
    }

    #[test]
    fn malformed_truncated_oversized_and_trailing_frames_fail() {
        assert_eq!(decode_header(b"MXS8"), Err(CodecError::TruncatedHeader(4)));
        let mut bad_magic = [0u8; HEADER_BYTES];
        bad_magic[0..4].copy_from_slice(b"BAD!");
        assert_eq!(decode_header(&bad_magic), Err(CodecError::InvalidMagic));
        let frame = encode_frame(FrameKind::Hello, 0, b"abc").unwrap();
        assert!(matches!(
            decode_frame(&frame[..frame.len() - 1]),
            Err(CodecError::TruncatedPayload {
                expected: 3,
                received: 2
            })
        ));
        let mut trailing = frame.clone();
        trailing.push(0);
        assert_eq!(decode_frame(&trailing), Err(CodecError::TrailingBytes(1)));
        assert!(encode_frame(FrameKind::Input, 0, &vec![0; MAX_STREAM_CHUNK_BYTES + 1]).is_err());
    }

    #[test]
    fn request_response_correlation_is_exact() {
        let request = FrameHeader::new(FrameKind::ControlRequest, 42, 0).unwrap();
        let response = FrameHeader::new(FrameKind::ControlResponse, 42, 0).unwrap();
        let other = FrameHeader::new(FrameKind::ControlResponse, 43, 0).unwrap();
        let response_frame = DecodedFrame {
            header: response,
            payload: &[],
        };
        assert!(response_frame.correlates_with(request));
        assert!(
            !DecodedFrame {
                header: other,
                payload: &[]
            }
            .correlates_with(request)
        );
        assert_eq!(
            FrameHeader::new(FrameKind::ControlRequest, 0, 0),
            Err(CodecError::MissingRequestId(FrameKind::ControlRequest))
        );
    }

    #[test]
    fn incremental_decoder_handles_split_and_multiple_frames() {
        let first = encode_frame(FrameKind::Hello, 0, b"one").unwrap();
        let second = encode_frame(FrameKind::Attach, 0, b"two").unwrap();
        let mut decoder = FrameDecoder::default();
        decoder.push(&first[..10]).unwrap();
        assert!(decoder.next_frame().unwrap().is_none());
        decoder.push(&first[10..]).unwrap();
        decoder.push(&second).unwrap();
        assert_eq!(decoder.next_frame().unwrap().unwrap().1, b"one");
        assert_eq!(decoder.next_frame().unwrap().unwrap().1, b"two");
        assert!(decoder.next_frame().unwrap().is_none());

        let payload = vec![b'x'; MAX_STRUCTURED_FRAME_BYTES / 2 + 1];
        let first = encode_frame(FrameKind::Hello, 0, &payload).unwrap();
        let second = encode_frame(FrameKind::Attach, 0, &payload).unwrap();
        let mut combined = first;
        combined.extend_from_slice(&second);
        decoder.push(&combined).unwrap();
        assert_eq!(
            decoder.next_frame().unwrap().unwrap().1.len(),
            payload.len()
        );
        assert_eq!(
            decoder.next_frame().unwrap().unwrap().1.len(),
            payload.len()
        );
    }

    #[test]
    fn structured_and_output_payloads_round_trip_with_limits() {
        let hello = Hello {
            version: ProtocolVersion::CURRENT,
            client_kind: ClientKind::Control,
            process_id: 12,
            nonce: [7; 32],
        };
        let bytes = encode_structured(&hello).unwrap();
        assert_eq!(decode_structured::<Hello>(&bytes).unwrap(), hello);
        let request = ControlRequest::Ping;
        assert_eq!(
            decode_structured::<ControlRequest>(&encode_structured(&request).unwrap()).unwrap(),
            request
        );
        let output = encode_output(99, b"ready").unwrap();
        assert_eq!(decode_output(&output).unwrap(), (99, b"ready".as_slice()));
        assert!(encode_output(1, &vec![0; MAX_STREAM_CHUNK_BYTES + 1]).is_err());
    }

    #[test]
    fn current_header_version_is_pinned() {
        assert_eq!(PROTOCOL_MAJOR, 1);
        assert_eq!(PROTOCOL_MINOR, 0);
    }
}
