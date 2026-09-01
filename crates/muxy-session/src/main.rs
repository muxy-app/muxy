mod attach;
mod daemon;
mod pty;
mod security;
mod shell;

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;

use muxy_proto::session::{HEADER_BYTES, SessionCodec, SessionCodecError, SessionMessage};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("muxy-session-v2: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os();
    let _executable = arguments.next();
    match arguments.next().as_deref().and_then(|value| value.to_str()) {
        Some("attach") if arguments.next().is_none() => attach::run().map_err(Into::into),
        Some("daemon") => daemon::run(arguments.collect()).map_err(Into::into),
        _ => Err("expected private attach or daemon mode".into()),
    }
}

pub(crate) fn write_message(
    stream: &mut UnixStream,
    message: &SessionMessage,
) -> Result<(), WireError> {
    let frame = SessionCodec::encode(message)?;
    stream.write_all(&frame)?;
    Ok(())
}

pub(crate) fn read_message(stream: &mut UnixStream) -> Result<SessionMessage, WireError> {
    let mut header = [0u8; HEADER_BYTES];
    stream.read_exact(&mut header)?;
    let payload_len = muxy_proto::session::validated_payload_length(&header)?;
    let mut frame = Vec::with_capacity(HEADER_BYTES + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(HEADER_BYTES + payload_len, 0);
    stream.read_exact(&mut frame[HEADER_BYTES..])?;
    Ok(SessionCodec::decode(&frame)?)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum WireError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Codec(#[from] SessionCodecError),
}
