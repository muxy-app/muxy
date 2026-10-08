use std::io::{self, Read, Write};

use muxy_protocol::wire::{
    Decoder, Encoder, HEADER_LEN, Header, MAX_FRAME, MessageKind, WireError, decode, encode,
};
use muxy_protocol::{CONTROL, ChannelId, ChannelKind, Message, MetadataEvent, V2};

#[test]
fn every_incomplete_sample_and_empty_stream_are_closed() -> Result<(), WireError> {
    for message in Message::samples() {
        let mut bytes = Vec::new();
        encode(&message, channel(&message), &mut bytes)?;
        for end in 0..bytes.len() {
            let mut decoder = Decoder::new(&bytes[..end]);
            assert!(
                matches!(decoder.next(), Err(WireError::Closed)),
                "{message:?} truncated at {end}"
            );
        }
    }
    Ok(())
}

#[test]
fn unsupported_versions_and_lengths_shorter_than_the_header_are_rejected() {
    let header = Header {
        length: 7,
        version: V2.0,
        channel: 0,
        kind: MessageKind::VersionUnsupported as u8,
    };
    for version in [0, 1, V2.0 + 1, u16::MAX] {
        let header = Header { version, ..header };
        assert!(matches!(
            Header::from_bytes(header.to_bytes()),
            Err(WireError::UnsupportedVersion(value)) if value == version
        ));
        assert!(matches!(
            decode(header, &[]),
            Err(WireError::UnsupportedVersion(_))
        ));
    }
    for length in 0..7 {
        let bytes = Header { length, ..header }.to_bytes();
        assert!(matches!(
            Decoder::new(bytes.as_slice()).next(),
            Err(WireError::Decode(_))
        ));
    }
}

#[test]
fn malformed_payloads_mismatched_lengths_and_trailing_bytes_are_rejected() -> Result<(), WireError>
{
    let hello = Header::new(1, CONTROL, MessageKind::Hello)?;
    assert!(matches!(decode(hello, &[0x80]), Err(WireError::Decode(_))));
    let input = Header::new(1, ChannelId(1), MessageKind::Input)?;
    for payload in [&[][..], &[1, 2][..]] {
        assert!(matches!(decode(input, payload), Err(WireError::Decode(_))));
    }
    for message in Message::samples() {
        if matches!(message, Message::Input(_)) {
            continue;
        }
        let mut bytes = Vec::new();
        encode(&message, channel(&message), &mut bytes)?;
        bytes.push(0);
        let header = Header::new(
            bytes.len() - HEADER_LEN,
            channel(&message),
            MessageKind::from(&message),
        )?;
        assert!(matches!(
            decode(header, &bytes[HEADER_LEN..]),
            Err(WireError::Decode(_))
        ));
    }
    Ok(())
}

#[test]
fn the_frame_cap_includes_the_entire_header() -> Result<(), WireError> {
    let message = Message::Input(vec![0xff; MAX_FRAME - HEADER_LEN]);
    let mut bytes = Vec::new();
    encode(&message, ChannelId(1), &mut bytes)?;
    assert_eq!(bytes.len(), MAX_FRAME);
    assert_eq!(
        Decoder::new(bytes.as_slice()).next()?,
        (ChannelId(1), message)
    );
    let mut header = Header::new(MAX_FRAME - HEADER_LEN, ChannelId(1), MessageKind::Input)?;
    header.length += 1;
    assert!(matches!(
        Header::from_bytes(header.to_bytes()),
        Err(WireError::FrameTooLarge)
    ));
    assert!(matches!(
        Header::new(usize::MAX, CONTROL, MessageKind::Hello),
        Err(WireError::FrameTooLarge)
    ));
    for message in [
        Message::Input(vec![0; MAX_FRAME - HEADER_LEN + 1]),
        Message::Metadata(MetadataEvent::Title("x".repeat(MAX_FRAME))),
    ] {
        let mut written = Vec::new();
        assert!(matches!(
            Encoder::new(&mut written).send(ChannelId(1), &message),
            Err(WireError::FrameTooLarge)
        ));
        assert!(written.is_empty());
    }
    Ok(())
}

#[test]
fn partial_reads_and_writes_preserve_frames() -> Result<(), WireError> {
    let mut writer = ShortWriter(Vec::new());
    let samples = Message::samples();
    let mut encoder = Encoder::new(&mut writer);
    for message in &samples {
        encoder.send(channel(message), message)?;
    }
    let mut reader = ShortReader(writer.0.as_slice());
    let mut decoder = Decoder::new(&mut reader);
    for message in samples {
        assert_eq!(decoder.next()?, (channel(&message), message));
    }
    Ok(())
}

fn channel(message: &Message) -> ChannelId {
    match message.channel_kind() {
        ChannelKind::Control => CONTROL,
        ChannelKind::Session => ChannelId(1),
    }
}

struct ShortReader<'a>(&'a [u8]);

impl Read for ShortReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let length = output.len().min(1);
        self.0.read(&mut output[..length])
    }
}

struct ShortWriter(Vec<u8>);

impl Write for ShortWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.write(&bytes[..bytes.len().min(2)])
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
