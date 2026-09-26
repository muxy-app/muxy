use std::error::Error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum WireError {
    Io(io::Error),
    Closed,
    FrameTooLarge,
    FlagsSet(u8),
    Decode(minicbor::decode::Error),
    Encode(String),
    UnsupportedVersion(u16),
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "wire I/O failed: {error}"),
            Self::Closed => formatter.write_str("Closed"),
            Self::FrameTooLarge => formatter.write_str("frame exceeds 16 MiB"),
            Self::FlagsSet(kind) => write!(formatter, "reserved flag bits set: {kind:#04x}"),
            Self::Decode(error) => write!(formatter, "invalid wire payload: {error}"),
            Self::Encode(error) => write!(formatter, "could not encode payload: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported wire version: {version}")
            }
        }
    }
}

impl Error for WireError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for WireError {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            Self::Closed
        } else {
            Self::Io(error)
        }
    }
}

impl From<minicbor::decode::Error> for WireError {
    fn from(error: minicbor::decode::Error) -> Self {
        Self::Decode(error)
    }
}

impl From<minicbor::encode::Error<std::convert::Infallible>> for WireError {
    fn from(error: minicbor::encode::Error<std::convert::Infallible>) -> Self {
        Self::Encode(error.to_string())
    }
}
