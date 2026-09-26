use std::convert::Infallible;

use minicbor::decode::Error as DecodeError;
use minicbor::{Decode, Decoder, Encoder};

use crate::wire::{HEADER_LEN, Header, MessageKind, WireError};
use crate::{ChannelId, Message};

/// Every payload except raw input is one CBOR array of the message's fields,
/// so a newer build can append fields that older builds skip.
pub fn encode(
    message: &Message,
    channel: ChannelId,
    output: &mut Vec<u8>,
) -> Result<(), WireError> {
    output.clear();
    if let Message::Input(bytes) = message {
        Header::new(bytes.len(), channel, MessageKind::Input)?;
    }
    output.resize(HEADER_LEN, 0);
    fields(message, &mut Encoder::new(&mut *output))?;
    let header = Header::new(
        output.len() - HEADER_LEN,
        channel,
        MessageKind::from(message),
    )?;
    output[..HEADER_LEN].copy_from_slice(&header.to_bytes());
    Ok(())
}

fn fields(
    message: &Message,
    encoder: &mut Encoder<&mut Vec<u8>>,
) -> Result<(), minicbor::encode::Error<Infallible>> {
    match message {
        Message::Hello { versions } => {
            encoder.array(1)?.encode(versions)?;
        }
        Message::HelloReply {
            versions,
            server,
            features,
        } => {
            encoder
                .array(3)?
                .encode(versions)?
                .encode(server)?
                .encode(features)?;
        }
        Message::VersionUnsupported | Message::ServerRestarting => {
            encoder.array(0)?;
        }
        Message::Request { id, body } => {
            encoder.array(2)?.encode(id)?.encode(body)?;
        }
        Message::Reply { id, body } => {
            encoder.array(2)?.encode(id)?.encode(body)?;
        }
        Message::UnsupportedRequest { id } | Message::UnreadableReply { id } => {
            encoder.array(1)?.encode(id)?;
        }
        Message::FrameAck { channel, seq } => {
            encoder.array(2)?.encode(channel)?.encode(seq)?;
        }
        Message::SessionEnded { session, reason } => {
            encoder.array(2)?.encode(session)?.encode(reason)?;
        }
        Message::Changed { topic, revision } => {
            encoder.array(2)?.encode(topic)?.encode(revision)?;
        }
        Message::GitChanged { project } => {
            encoder.array(1)?.encode(project)?;
        }
        Message::FilesChanged { project, changes } => {
            encoder.array(2)?.encode(project)?.encode(changes)?;
        }
        Message::Progress { session, progress } => {
            encoder.array(2)?.encode(session)?.encode(progress)?;
        }
        Message::SessionMetadata { session, metadata } => {
            encoder.array(2)?.encode(session)?.encode(metadata)?;
        }
        Message::Fatal(error) => {
            encoder.array(1)?.encode(error)?;
        }
        Message::Frame(frame) => {
            encoder.array(1)?.encode(frame)?;
        }
        Message::Metadata(event) => {
            encoder.array(1)?.encode(event)?;
        }
        Message::Mouse(event) => {
            encoder.array(1)?.encode(event)?;
        }
        Message::CellSize(cell) => {
            encoder.array(1)?.encode(cell)?;
        }
        Message::Input(bytes) => {
            encoder.writer_mut().extend_from_slice(bytes);
        }
    }
    Ok(())
}

/// Returns `None` for a message from a newer build that this build ignores:
/// an unknown kind, or an event naming a variant this build doesn't know.
pub fn decode(header: Header, payload: &[u8]) -> Result<Option<(ChannelId, Message)>, WireError> {
    header.validate()?;
    if payload.len() != header.payload_len()? {
        return Err(DecodeError::message("payload length differs from its header").into());
    }
    let Some(kind) = MessageKind::from_u8(header.kind)? else {
        return Ok(None);
    };
    let channel = ChannelId(header.channel);
    if kind == MessageKind::Input {
        return Ok(Some((channel, Message::Input(payload.to_vec()))));
    }
    let mut decoder = Decoder::new(payload);
    let mut fields = Fields::new(&mut decoder)?;
    let message = match message(kind, &mut fields) {
        Ok(message) => message,
        // A skipped frame would never be acknowledged, so frames stay strict.
        Err(error) if error.is_unknown_variant() && kind != MessageKind::Frame => {
            // Ignored events must still be well formed.
            let mut whole = Decoder::new(payload);
            whole.skip()?;
            consumed(&whole, payload)?;
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    fields.finish()?;
    consumed(&decoder, payload)?;
    Ok(Some((channel, message)))
}

fn consumed(decoder: &Decoder<'_>, payload: &[u8]) -> Result<(), WireError> {
    if decoder.position() == payload.len() {
        Ok(())
    } else {
        Err(DecodeError::message("trailing bytes after the payload").into())
    }
}

fn message(kind: MessageKind, fields: &mut Fields<'_, '_>) -> Result<Message, DecodeError> {
    Ok(match kind {
        MessageKind::Request => {
            let id = fields.next()?;
            match fields.readable()? {
                Some(body) => Message::Request { id, body },
                None => Message::UnsupportedRequest { id },
            }
        }
        MessageKind::Reply => {
            let id = fields.next()?;
            match fields.readable()? {
                Some(body) => Message::Reply { id, body },
                None => Message::UnreadableReply { id },
            }
        }
        MessageKind::Hello => Message::Hello {
            versions: fields.next()?,
        },
        MessageKind::HelloReply => Message::HelloReply {
            versions: fields.next()?,
            server: fields.next()?,
            features: fields.next()?,
        },
        MessageKind::VersionUnsupported => Message::VersionUnsupported,
        MessageKind::ServerRestarting => Message::ServerRestarting,
        MessageKind::FrameAck => Message::FrameAck {
            channel: fields.next()?,
            seq: fields.next()?,
        },
        MessageKind::SessionEnded => Message::SessionEnded {
            session: fields.next()?,
            reason: fields.next()?,
        },
        MessageKind::Changed => Message::Changed {
            topic: fields.next()?,
            revision: fields.next()?,
        },
        MessageKind::GitChanged => Message::GitChanged {
            project: fields.next()?,
        },
        MessageKind::FilesChanged => Message::FilesChanged {
            project: fields.next()?,
            changes: fields.next()?,
        },
        MessageKind::Progress => Message::Progress {
            session: fields.next()?,
            progress: fields.next()?,
        },
        MessageKind::SessionMetadata => Message::SessionMetadata {
            session: fields.next()?,
            metadata: fields.next()?,
        },
        MessageKind::Fatal => Message::Fatal(fields.next()?),
        MessageKind::Frame => Message::Frame(fields.next()?),
        MessageKind::Metadata => Message::Metadata(fields.next()?),
        MessageKind::Mouse => Message::Mouse(fields.next()?),
        MessageKind::CellSize => Message::CellSize(fields.next()?),
        MessageKind::Input => return Err(DecodeError::message("input is raw bytes, not CBOR")),
    })
}

/// A payload's field array. Fields a newer build appended are skipped.
struct Fields<'a, 'b> {
    decoder: &'a mut Decoder<'b>,
    remaining: u64,
}

impl<'a, 'b> Fields<'a, 'b> {
    fn new(decoder: &'a mut Decoder<'b>) -> Result<Self, DecodeError> {
        let position = decoder.position();
        let remaining = decoder
            .array()?
            .ok_or_else(|| DecodeError::message("expected a field array").at(position))?;
        Ok(Self { decoder, remaining })
    }

    fn next<T: Decode<'b, ()>>(&mut self) -> Result<T, DecodeError> {
        if self.remaining == 0 {
            return Err(DecodeError::message("missing field").at(self.decoder.position()));
        }
        self.remaining -= 1;
        self.decoder.decode()
    }

    /// Decodes the next field, or skips it when it names a variant this build
    /// doesn't know, such as a newer method. Invalid values stay errors.
    fn readable<T: Decode<'b, ()>>(&mut self) -> Result<Option<T>, DecodeError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        let start = self.decoder.position();
        match self.decoder.decode() {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.is_unknown_variant() => {
                self.decoder.set_position(start);
                self.decoder.skip()?;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn finish(self) -> Result<(), DecodeError> {
        for _ in 0..self.remaining {
            self.decoder.skip()?;
        }
        Ok(())
    }
}
