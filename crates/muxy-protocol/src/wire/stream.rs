use std::io::{Read, Write};

use crate::{ChannelId, Message};

use crate::wire::{HEADER_LEN, Header, MAX_FRAME, WireError, decode, encode};

#[derive(Debug)]
pub struct Encoder<W: Write> {
    writer: W,
    buffer: Vec<u8>,
}

impl<W: Write> Encoder<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            buffer: Vec::new(),
        }
    }

    pub fn send(&mut self, channel: ChannelId, message: &Message) -> Result<(), WireError> {
        encode(message, channel, &mut self.buffer)?;
        self.writer.write_all(&self.buffer)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Decoder<R: Read> {
    reader: R,
    buffer: Vec<u8>,
    payload_limit: usize,
}

impl<R: Read> Decoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            payload_limit: MAX_FRAME,
        }
    }

    /// Rejects larger frames before allocating for them, e.g. from unauthenticated peers.
    pub fn set_payload_limit(&mut self, limit: usize) {
        self.payload_limit = limit;
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<(ChannelId, Message), WireError> {
        let mut bytes = [0; HEADER_LEN];
        self.reader.read_exact(&mut bytes)?;
        let header = Header::from_bytes(bytes)?;
        let length = header.payload_len()?;
        if length > self.payload_limit {
            return Err(WireError::FrameTooLarge);
        }
        self.buffer.resize(length, 0);
        self.reader.read_exact(&mut self.buffer)?;
        decode(header, &self.buffer)
    }
}

#[cfg(test)]
mod tests {
    use crate::ChannelId;

    use super::*;
    use crate::wire::{MAX_FRAME, MessageKind};

    #[test]
    fn oversized_headers_do_not_allocate_a_payload_buffer() {
        for length in [17 * 1024 * 1024, u32::MAX] {
            let bytes = Header {
                length,
                version: 1,
                channel: 1,
                kind: MessageKind::Input as u8,
            }
            .to_bytes();
            let mut reader = bytes.as_slice();
            let mut decoder = Decoder::new(&mut reader);
            assert!(matches!(decoder.next(), Err(WireError::FrameTooLarge)));
            assert_eq!(decoder.buffer.capacity(), 0);
            assert!(reader.is_empty());
        }
        assert_eq!(MAX_FRAME, 16 * 1024 * 1024);
    }

    #[test]
    fn payload_limit_rejects_before_allocating_and_can_be_lifted() -> Result<(), WireError> {
        let message = Message::Input(vec![0xff; 4097]);
        let mut bytes = Vec::new();
        Encoder::new(&mut bytes).send(ChannelId(1), &message)?;
        let mut decoder = Decoder::new(bytes.as_slice());
        decoder.set_payload_limit(4096);
        assert!(matches!(decoder.next(), Err(WireError::FrameTooLarge)));
        assert_eq!(decoder.buffer.capacity(), 0);
        let mut decoder = Decoder::new(bytes.as_slice());
        decoder.set_payload_limit(4096);
        decoder.set_payload_limit(MAX_FRAME);
        assert_eq!(decoder.next()?, (ChannelId(1), message));
        Ok(())
    }

    #[test]
    fn decoder_reuses_its_payload_buffer() -> Result<(), WireError> {
        let large = Message::Input(vec![0xff; 4096]);
        let small = Message::Input(vec![0; 3]);
        let mut bytes = Vec::new();
        let mut encoder = Encoder::new(&mut bytes);
        encoder.send(ChannelId(1), &large)?;
        encoder.send(ChannelId(2), &small)?;
        let mut decoder = Decoder::new(bytes.as_slice());
        assert_eq!(decoder.next()?, (ChannelId(1), large));
        let capacity = decoder.buffer.capacity();
        let pointer = decoder.buffer.as_ptr();
        assert_eq!(decoder.next()?, (ChannelId(2), small));
        assert_eq!(decoder.buffer.capacity(), capacity);
        assert_eq!(decoder.buffer.as_ptr(), pointer);
        Ok(())
    }
}
