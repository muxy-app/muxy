//! Encoded samples, kept for as long as their protocol version is supported.
//!
//! Each file is named by its message kind and a hash of its bytes, so changing
//! an encoding adds a file instead of replacing one. Every file must stay
//! readable, because it is what an older build of the same version sends.

use std::error::Error;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use muxy_protocol::wire::{Decoder, MessageKind, WireError, encode};
use muxy_protocol::{CONTROL, CURRENT, ChannelId, ChannelKind, Message};

/// Points at another build's fixtures to check that this build reads them.
const OTHER_FIXTURES: &str = "MUXY_PROTOCOL_FIXTURES";

fn directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(format!("v{}", CURRENT.0))
}

#[test]
fn every_sample_has_a_fixture() -> Result<(), Box<dyn Error>> {
    for message in Message::samples() {
        let path = directory().join(fixture_name(&message)?);
        assert!(
            path.exists(),
            "{} is missing. Run: cargo test -p muxy-protocol --test fixtures -- --ignored generate_fixtures",
            path.display()
        );
    }
    Ok(())
}

/// Without `MUXY_PROTOCOL_FIXTURES`, reads this build's fixtures strictly.
/// With it, reads a newer build's fixtures the way a running peer would:
/// messages this build doesn't know may be skipped or reported as unsupported.
#[test]
fn every_fixture_decodes() -> Result<(), Box<dyn Error>> {
    let other = std::env::var_os(OTHER_FIXTURES).map(PathBuf::from);
    let strict = other.is_none();
    let mut count = 0;
    for entry in fs::read_dir(other.unwrap_or_else(directory))? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "bin") {
            continue;
        }
        count += 1;
        let bytes = fs::read(&path)?;
        match Decoder::new(bytes.as_slice()).next() {
            Ok((_, message)) => {
                let partial = matches!(
                    message,
                    Message::UnsupportedRequest { .. } | Message::UnreadableReply { .. }
                );
                assert!(
                    !strict || !partial,
                    "{} is not fully readable",
                    path.display()
                );
                assert!(
                    partial || message.validate().is_ok(),
                    "{} is invalid",
                    path.display()
                );
            }
            Err(WireError::Closed) if !strict => {}
            Err(error) => return Err(format!("{}: {error}", path.display()).into()),
        }
    }
    assert!(count > 0, "no fixtures found");
    Ok(())
}

#[test]
#[ignore = "adds fixtures for new or changed samples; never rewrites existing ones"]
fn generate_fixtures() -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(directory())?;
    for message in Message::samples() {
        let path = directory().join(fixture_name(&message)?);
        if !path.exists() {
            fs::write(path, encoded(&message)?)?;
        }
    }
    Ok(())
}

fn encoded(message: &Message) -> Result<Vec<u8>, WireError> {
    let channel = match message.channel_kind() {
        ChannelKind::Control => CONTROL,
        ChannelKind::Session => ChannelId(1),
    };
    let mut bytes = Vec::new();
    encode(message, channel, &mut bytes)?;
    Ok(bytes)
}

fn fixture_name(message: &Message) -> Result<String, Box<dyn Error>> {
    let digest = ring::digest::digest(&ring::digest::SHA256, &encoded(message)?);
    let mut name = format!("{}-", kind_name(MessageKind::from(message)));
    for byte in &digest.as_ref()[..8] {
        write!(name, "{byte:02x}")?;
    }
    name.push_str(".bin");
    Ok(name)
}

fn kind_name(kind: MessageKind) -> &'static str {
    match kind {
        MessageKind::Hello => "hello",
        MessageKind::Request => "request",
        MessageKind::FrameAck => "frame_ack",
        MessageKind::HelloReply => "hello_reply",
        MessageKind::VersionUnsupported => "version_unsupported",
        MessageKind::Reply => "reply",
        MessageKind::SessionEnded => "session_ended",
        MessageKind::Fatal => "fatal",
        MessageKind::Input => "input",
        MessageKind::Frame => "frame",
        MessageKind::Metadata => "metadata",
        MessageKind::Mouse => "mouse",
        MessageKind::ServerRestarting => "server_restarting",
        MessageKind::CellSize => "cell_size",
        MessageKind::Changed => "changed",
        MessageKind::GitChanged => "git_changed",
        MessageKind::Progress => "progress",
        MessageKind::SessionMetadata => "session_metadata",
        MessageKind::FilesChanged => "files_changed",
    }
}
