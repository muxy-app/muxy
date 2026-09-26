use std::io::{Read, Write};

use muxy_protocol::wire::{Decoder, Encoder, WireError};
use muxy_protocol::{CONTROL, ErrorCode, ErrorReply, FEATURES, Message, SUPPORTED, ServerInfo, V1};

/// Returns whether the client shares a protocol version and may continue.
pub(super) fn accept(
    decoder: &mut Decoder<impl Read>,
    encoder: &mut Encoder<impl Write>,
) -> Result<bool, WireError> {
    negotiate(decoder, encoder, |message| {
        log::error!("fatal protocol error: {message}");
    })
}

/// Unauthenticated network peers are reported at a limited rate instead.
pub(super) fn accept_remote(
    decoder: &mut Decoder<impl Read>,
    encoder: &mut Encoder<impl Write>,
) -> Result<bool, WireError> {
    negotiate(decoder, encoder, super::auth::report)
}

fn negotiate(
    decoder: &mut Decoder<impl Read>,
    encoder: &mut Encoder<impl Write>,
    report: impl Fn(&str),
) -> Result<bool, WireError> {
    let reject = |message: String| {
        report(&message);
        rejection(message)
    };
    let message = match decoder.next() {
        Ok((CONTROL, Message::Hello { versions })) if !versions.is_empty() => {
            if versions.iter().any(|version| SUPPORTED.contains(version)) {
                encoder.send(
                    CONTROL,
                    &Message::HelloReply {
                        versions: SUPPORTED.to_vec(),
                        server: ServerInfo::current(),
                        features: FEATURES.to_vec(),
                    },
                )?;
                return Ok(true);
            }
            Message::VersionUnsupported
        }
        Ok(_) => reject("expected Hello on control".into()),
        Err(WireError::Closed) => return Ok(false),
        Err(error @ WireError::Io(_)) => return Err(error),
        Err(WireError::UnsupportedVersion(version)) if version == V1.0 => {
            encoder.reject_legacy_peer()?;
            return Ok(false);
        }
        Err(WireError::UnsupportedVersion(_)) => Message::VersionUnsupported,
        Err(error) => reject(error.to_string()),
    };
    encoder.send(CONTROL, &message)?;
    Ok(false)
}

pub(super) fn fatal(message: impl Into<String>) -> Message {
    let message = message.into();
    log::error!("fatal protocol error: {message}");
    rejection(message)
}

pub(super) fn rejection(message: impl Into<String>) -> Message {
    Message::Fatal(ErrorReply {
        code: ErrorCode::BadRequest,
        message: message.into(),
    })
}
