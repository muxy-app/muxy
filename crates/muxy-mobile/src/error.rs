use std::fmt;

use muxy_client::ClientError;
use muxy_protocol::ErrorCode;

/// Failures the app can act on.
#[derive(Debug, uniffi::Error)]
pub enum MobileError {
    /// The text is not a Muxy pairing link.
    InvalidLink,
    /// The saved pairing is damaged; pair again.
    InvalidCredential,
    /// No address of the server answered; check the network or VPN.
    Unreachable {
        // Not `message`, which every Kotlin exception already defines.
        reason: String,
    },
    /// The server no longer matches the pairing; pair again rather than trusting it.
    IdentityMismatch,
    /// The device was revoked or never paired, or mobile access is off.
    Unauthorized,
    /// The app and the server share no protocol version; update one of them.
    IncompatibleVersion,
    /// The phone and the computer run Muxy versions that disagree on this
    /// request: the server can't do it, or answered in a form the app can't read.
    Unsupported,
    Timeout,
    Disconnected,
    /// The server refused the request.
    Server {
        reason: String,
    },
}

impl fmt::Display for MobileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLink => formatter.write_str("This is not a Muxy pairing code."),
            Self::InvalidCredential => formatter.write_str("The saved pairing is damaged."),
            Self::Unreachable { reason } => {
                write!(formatter, "The server is unreachable: {reason}")
            }
            Self::IdentityMismatch => {
                formatter.write_str("The server's identity changed since this device paired.")
            }
            Self::Unauthorized => formatter.write_str("This device is not paired with the server."),
            Self::IncompatibleVersion => {
                formatter.write_str("The app and the server can't talk to each other.")
            }
            Self::Unsupported => {
                formatter.write_str("This needs a newer Muxy on the phone or the computer.")
            }
            Self::Timeout => formatter.write_str("The server did not answer in time."),
            Self::Disconnected => formatter.write_str("Disconnected from the server."),
            Self::Server { reason } => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for MobileError {}

impl From<ClientError> for MobileError {
    fn from(error: ClientError) -> Self {
        match error {
            ClientError::IdentityMismatch => Self::IdentityMismatch,
            ClientError::VersionUnsupported => Self::IncompatibleVersion,
            ClientError::Timeout => Self::Timeout,
            ClientError::Disconnected | ClientError::Wire(_) | ClientError::Protocol(_) => {
                Self::Disconnected
            }
            ClientError::Server(error) if error.code == ErrorCode::Unauthorized => {
                Self::Unauthorized
            }
            ClientError::Server(error) if error.code == ErrorCode::Unsupported => Self::Unsupported,
            ClientError::Server(error) => Self::Server {
                reason: error.message,
            },
            ClientError::Io(error) => Self::Unreachable {
                reason: error.to_string(),
            },
            ClientError::Invalid(_) | ClientError::UnexpectedReply(_) => Self::Server {
                reason: error.to_string(),
            },
        }
    }
}
