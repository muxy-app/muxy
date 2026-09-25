use std::io::{Read, Write};

use muxy_protocol::wire::{Decoder, Encoder, WireError};
use muxy_protocol::{CONTROL, ErrorCode, ErrorReply, Message, SUPPORTED, Version};

pub(super) fn accept(
    decoder: &mut Decoder<impl Read>,
    encoder: &mut Encoder<impl Write>,
) -> Result<Option<Version>, WireError> {
    negotiate(decoder, encoder, |message| {
        log::error!("fatal protocol error: {message}");
    })
}

/// Unauthenticated network peers are reported at a limited rate instead.
pub(super) fn accept_remote(
    decoder: &mut Decoder<impl Read>,
    encoder: &mut Encoder<impl Write>,
) -> Result<Option<Version>, WireError> {
    negotiate(decoder, encoder, super::auth::report)
}

fn negotiate(
    decoder: &mut Decoder<impl Read>,
    encoder: &mut Encoder<impl Write>,
    report: impl Fn(&str),
) -> Result<Option<Version>, WireError> {
    let reject = |message: String| {
        report(&message);
        rejection(message)
    };
    let message = match decoder.next() {
        Ok((
            CONTROL,
            Message::Hello {
                versions,
                compatibility,
            },
        )) if !versions.is_empty() => {
            if let Some(version) = versions
                .into_iter()
                .filter(|version| {
                    SUPPORTED.contains(version) && compatibility == muxy_protocol::COMPATIBILITY
                })
                .max()
            {
                encoder.send(
                    CONTROL,
                    &Message::HelloReply {
                        versions: SUPPORTED.to_vec(),
                        server: muxy_protocol::ServerInfo::current(),
                    },
                )?;
                return Ok(Some(version));
            }
            Message::VersionUnsupported
        }
        Ok(_) => reject("expected Hello on control".into()),
        Err(WireError::Closed) => return Ok(None),
        Err(error @ WireError::Io(_)) => return Err(error),
        Err(error) => reject(error.to_string()),
    };
    encoder.send(CONTROL, &message)?;
    Ok(None)
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
