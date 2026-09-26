//! Messages from a newer build: this build skips what it doesn't know and
//! keeps the connection.

use std::error::Error;

use minicbor::Encoder;
use muxy_protocol::wire::{
    Decoder, Encoder as FrameEncoder, HEADER_LEN, Header, MessageKind, WireError, decode, encode,
    legacy_version_unsupported,
};
use muxy_protocol::{
    CONTROL, ChannelId, ErrorCode, ExitReason, Message, RequestBody, RequestId, SessionId, Topic,
};

/// A frame holding `payload`, as a newer build would write it.
fn frame(kind: u8, channel: ChannelId, payload: &[u8]) -> Result<Vec<u8>, WireError> {
    let mut header = Header::new(payload.len(), channel, MessageKind::Hello)?;
    header.kind = kind;
    let mut bytes = header.to_bytes().to_vec();
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

fn cbor(
    write: impl FnOnce(&mut Encoder<&mut Vec<u8>>) -> Result<(), Box<dyn Error>>,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::new();
    write(&mut Encoder::new(&mut bytes))?;
    Ok(bytes)
}

fn decode_one(bytes: &[u8]) -> Result<Option<(ChannelId, Message)>, WireError> {
    let header = Header::from_bytes(
        bytes[..HEADER_LEN]
            .try_into()
            .map_err(|_| WireError::Closed)?,
    )?;
    decode(header, &bytes[HEADER_LEN..])
}

#[test]
fn unknown_kinds_are_skipped_and_the_stream_continues() -> Result<(), Box<dyn Error>> {
    let mut bytes = frame(
        40,
        CONTROL,
        &cbor(|e| {
            e.array(1)?.str("from the future")?;
            Ok(())
        })?,
    )?;
    let ping = Message::Request {
        id: RequestId(1),
        body: RequestBody::Ping,
    };
    FrameEncoder::new(&mut bytes).send(CONTROL, &ping)?;
    assert_eq!(Decoder::new(bytes.as_slice()).next()?, (CONTROL, ping));
    Ok(())
}

#[test]
fn fields_appended_by_a_newer_build_are_skipped() -> Result<(), Box<dyn Error>> {
    let payload = cbor(|e| {
        e.array(3)?
            .encode(Topic::Catalog)?
            .u64(4)?
            .str("newer field")?;
        Ok(())
    })?;
    assert_eq!(
        decode_one(&frame(MessageKind::Changed as u8, CONTROL, &payload)?)?,
        Some((
            CONTROL,
            Message::Changed {
                topic: Topic::Catalog,
                revision: 4
            }
        ))
    );
    Ok(())
}

#[test]
fn unknown_values_of_open_enums_are_kept_by_number() -> Result<(), Box<dyn Error>> {
    let changed = cbor(|e| {
        e.array(2)?.u32(99)?.u64(1)?;
        Ok(())
    })?;
    assert_eq!(
        decode_one(&frame(MessageKind::Changed as u8, CONTROL, &changed)?)?,
        Some((
            CONTROL,
            Message::Changed {
                topic: Topic::Unrecognized(99),
                revision: 1
            }
        ))
    );
    let session = SessionId::new(7).ok_or("zero ID")?;
    let ended = cbor(|e| {
        e.array(2)?.encode(session)?;
        e.array(2)?.u32(8)?.str("a newer exit reason")?;
        Ok(())
    })?;
    assert_eq!(
        decode_one(&frame(MessageKind::SessionEnded as u8, CONTROL, &ended)?)?,
        Some((
            CONTROL,
            Message::SessionEnded {
                session,
                reason: ExitReason::Unrecognized(8)
            }
        ))
    );
    let error = cbor(|e| {
        e.array(1)?.array(2)?.u32(99)?.str("newer error")?;
        Ok(())
    })?;
    assert!(matches!(
        decode_one(&frame(MessageKind::Fatal as u8, CONTROL, &error)?)?,
        Some((_, Message::Fatal(reply))) if reply.code == ErrorCode::Unrecognized(99)
    ));
    Ok(())
}

#[test]
fn requests_for_unknown_methods_are_correlated() -> Result<(), Box<dyn Error>> {
    let payload = cbor(|e| {
        e.array(2)?.encode(RequestId(9))?;
        e.array(2)?.u32(900)?.array(1)?.str("argument")?;
        Ok(())
    })?;
    let bytes = frame(MessageKind::Request as u8, CONTROL, &payload)?;
    assert_eq!(
        decode_one(&bytes)?,
        Some((CONTROL, Message::UnsupportedRequest { id: RequestId(9) }))
    );
    Ok(())
}

#[test]
fn unreadable_reply_bodies_are_correlated() -> Result<(), Box<dyn Error>> {
    let payload = cbor(|e| {
        e.array(2)?.encode(RequestId(4))?;
        e.array(2)?.u32(900)?.array(0)?;
        Ok(())
    })?;
    let bytes = frame(MessageKind::Reply as u8, CONTROL, &payload)?;
    assert_eq!(
        decode_one(&bytes)?,
        Some((CONTROL, Message::UnreadableReply { id: RequestId(4) }))
    );
    Ok(())
}

#[test]
fn correlated_failures_round_trip() -> Result<(), WireError> {
    for message in [
        Message::UnsupportedRequest { id: RequestId(3) },
        Message::UnreadableReply { id: RequestId(3) },
    ] {
        let mut bytes = Vec::new();
        encode(&message, CONTROL, &mut bytes)?;
        assert_eq!(Decoder::new(bytes.as_slice()).next()?, (CONTROL, message));
    }
    Ok(())
}

#[test]
fn malformed_request_bodies_are_still_rejected() -> Result<(), Box<dyn Error>> {
    // A body that claims more items than the payload holds is broken, not newer.
    let payload = [0x82, 0x01, 0x82, 0x18];
    let bytes = frame(MessageKind::Request as u8, CONTROL, &payload)?;
    assert!(matches!(decode_one(&bytes), Err(WireError::Decode(_))));
    Ok(())
}

#[test]
fn events_naming_unknown_variants_are_skipped() -> Result<(), Box<dyn Error>> {
    let payload = cbor(|e| {
        e.array(1)?.array(2)?.u32(99)?.array(0)?;
        Ok(())
    })?;
    let bytes = frame(MessageKind::Metadata as u8, ChannelId(1), &payload)?;
    assert_eq!(decode_one(&bytes)?, None);
    Ok(())
}

#[test]
fn builds_before_v2_are_told_to_update_in_their_own_framing() {
    // length 7, version 1, control channel, VersionUnsupported, empty payload.
    assert_eq!(
        legacy_version_unsupported(),
        [7, 0, 0, 0, 1, 0, 0, 0, 0, 0, 5]
    );
}

#[test]
fn mouse_events_naming_newer_buttons_stay_valid() -> Result<(), WireError> {
    use muxy_protocol::{Modifiers, MouseAction, MouseEvent, ScrollDirection};

    let event = Message::Mouse(MouseEvent {
        action: MouseAction::Scroll,
        button: None,
        column: 1,
        row: 1,
        scroll: Some(ScrollDirection::Unrecognized(9)),
        modifiers: Modifiers::default(),
    });
    let mut bytes = Vec::new();
    encode(&event, ChannelId(1), &mut bytes)?;
    let (_, decoded) = Decoder::new(bytes.as_slice()).next()?;
    assert_eq!(decoded, event);
    assert_eq!(decoded.validate(), Ok(()));
    Ok(())
}
