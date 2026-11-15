use std::error::Error;
use std::fmt;
use std::io;

use muxy_protocol::wire::WireError;
use muxy_protocol::{ErrorCode, ErrorReply, ReplyBody};

#[derive(Debug)]
pub enum ClientError {
    Io(io::Error),
    Wire(WireError),
    VersionUnsupported,
    Protocol(String),
    Invalid(ErrorCode),
    Server(ErrorReply),
    UnexpectedReply(Box<ReplyBody>),
    Timeout,
    Disconnected,
    IdentityMismatch,
    Remote {
        reason: RemoteReason,
        destination: String,
        detail: String,
    },
    ServerElsewhere(ServerElsewhere),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerElsewhere {
    Session,
    Elevated,
    NotElevated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteReason {
    HostKeyUnknown,
    HostKeyChanged,
    AuthenticationFailed,
    Unreachable,
    NotInstalled,
    NotRunning,
    SshMissing,
    UnexpectedOutput,
    BridgeFailed,
    Timeout,
    Incompatible,
    Unpaired,
    IdentityChanged,
}

impl RemoteReason {
    pub fn recoverable(self) -> bool {
        matches!(self, Self::Unreachable | Self::Timeout | Self::BridgeFailed)
    }
}

impl ClientError {
    pub fn remote_reason(&self) -> Option<RemoteReason> {
        match self {
            Self::Remote { reason, .. } => Some(*reason),
            Self::IdentityMismatch => Some(RemoteReason::IdentityChanged),
            Self::Server(error) if error.code == ErrorCode::Unauthorized => {
                Some(RemoteReason::Unpaired)
            }
            _ => None,
        }
    }

    pub fn recoverable(&self) -> bool {
        match self {
            Self::Remote { reason, .. } => reason.recoverable(),
            Self::ServerElsewhere(_) => false,
            _ => true,
        }
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "connection failed: {error}"),
            Self::Wire(error) => write!(formatter, "wire error: {error}"),
            Self::VersionUnsupported => formatter.write_str("This app and the running server can't talk to each other. Restarting the server will end active terminal sessions."),
            Self::Protocol(message) => write!(formatter, "protocol violation: {message}"),
            Self::Invalid(code) => write!(formatter, "invalid request: {code:?}"),
            Self::Server(error) => write!(formatter, "{:?}: {}", error.code, error.message),
            Self::UnexpectedReply(body) => write!(formatter, "unexpected reply: {body:?}"),
            Self::Timeout => formatter.write_str("request timed out"),
            Self::Disconnected => formatter.write_str("disconnected from server"),
            Self::IdentityMismatch => formatter.write_str(
                "The server's identity changed since this device paired. Pair again to trust it.",
            ),
            Self::Remote {
                reason,
                destination,
                detail,
            } => remote(formatter, *reason, destination, detail),
            Self::ServerElsewhere(place) => formatter.write_str(match place {
                ServerElsewhere::Session => {
                    "Muxy's server runs in another Windows session, such as an SSH login. Run muxy server stop there, then try again."
                }
                ServerElsewhere::Elevated => {
                    "Muxy's server runs as administrator. Run muxy server stop from an administrator terminal, then try again."
                }
                ServerElsewhere::NotElevated => {
                    "Muxy's server doesn't run as administrator, but this program does. Run muxy server stop from a normal terminal, or run this program without administrator rights."
                }
            }),
        }
    }
}

fn remote(
    formatter: &mut fmt::Formatter<'_>,
    reason: RemoteReason,
    destination: &str,
    detail: &str,
) -> fmt::Result {
    match reason {
        RemoteReason::HostKeyUnknown => write!(
            formatter,
            "{destination} isn't a trusted SSH host yet. Run ssh {destination} once to check and trust its key."
        ),
        RemoteReason::HostKeyChanged => write!(
            formatter,
            "The SSH host key for {destination} changed. If you expect that, run ssh {destination} and follow its steps to update known_hosts."
        ),
        RemoteReason::AuthenticationFailed => write!(
            formatter,
            "SSH refused the login to {destination}. Muxy connects without prompts, so use ssh-agent, a key in ~/.ssh/config, or a certificate."
        ),
        RemoteReason::Unreachable => write!(formatter, "Can't reach {destination}: {detail}"),
        RemoteReason::NotInstalled => write!(
            formatter,
            "Muxy isn't installed on {destination} (looked on PATH and in ~/.local/bin)."
        ),
        RemoteReason::NotRunning => {
            write!(formatter, "Muxy's server isn't running on {destination}.")
        }
        RemoteReason::SshMissing => write!(
            formatter,
            "Muxy needs {detail} to reach {destination}, but it isn't installed."
        ),
        RemoteReason::UnexpectedOutput if detail.is_empty() => write!(
            formatter,
            "The shell on {destination} printed output before Muxy could start. Keep its startup files quiet when it isn't interactive."
        ),
        RemoteReason::UnexpectedOutput => write!(
            formatter,
            "The shell on {destination} printed \"{detail}\" before Muxy could start. Keep its startup files quiet when it isn't interactive."
        ),
        RemoteReason::BridgeFailed => write!(formatter, "Muxy on {destination} failed: {detail}"),
        RemoteReason::Timeout if detail.is_empty() => {
            write!(formatter, "Timed out connecting to {destination}.")
        }
        RemoteReason::Timeout => {
            write!(formatter, "Timed out connecting to {destination}: {detail}")
        }
        RemoteReason::Incompatible => write!(
            formatter,
            "Muxy on {destination} can't accept connections from this version of Muxy ({detail}). Install the same version on both computers."
        ),
        RemoteReason::Unpaired => write!(
            formatter,
            "{destination} no longer accepts this device. Pair again with a new link."
        ),
        RemoteReason::IdentityChanged => write!(
            formatter,
            "{destination}'s identity changed since pairing. Pair again with a new link to trust it."
        ),
    }
}

impl Error for ClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Wire(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ClientError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<WireError> for ClientError {
    fn from(error: WireError) -> Self {
        match error {
            WireError::Closed => Self::Disconnected,
            WireError::Io(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::BrokenPipe
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::ConnectionAborted
                        | io::ErrorKind::NotConnected
                ) =>
            {
                Self::Disconnected
            }
            other => Self::Wire(other),
        }
    }
}
