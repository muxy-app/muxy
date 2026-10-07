//! The server a command talks to: this computer's, or the one on another
//! computer, reached over SSH with `--host`.

use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use muxy_client::{Client, ClientError, SshTarget, Start};
use muxy_protocol::ServerIdentity;

/// How long to wait after each failed attempt in a row to reach another
/// computer; the last wait repeats. Backing off spares its sshd and keeps
/// tools like fail2ban from blocking this computer.
const SSH_RETRIES: [Duration; 5] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
];
const LOCAL_RETRY: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
pub(crate) enum Target {
    /// The server in the local profile, started from `executable` if needed.
    Local {
        profile: PathBuf,
        executable: PathBuf,
    },
    /// The server on another computer. The TUI still keeps its layouts in the
    /// local profile.
    Ssh { profile: PathBuf, host: SshTarget },
}

impl Target {
    /// The server on `host` if one is given, else this computer's.
    pub(crate) fn new(host: Option<SshTarget>) -> io::Result<Self> {
        let profile = muxy_core::dirs::muxy_dir()?;
        Ok(match host {
            Some(host) => Self::Ssh { profile, host },
            None => Self::Local {
                profile,
                executable: muxy_client::local::server_executable()?,
            },
        })
    }

    pub(crate) fn connect(&self, start: Start) -> Result<Client, ClientError> {
        match self {
            Self::Local {
                profile,
                executable,
            } => {
                let socket = profile.join("server.sock");
                match start {
                    Start::IfNeeded => muxy_client::local::ensure_running(&socket, executable),
                    Start::Never => Client::connect(&socket),
                }
            }
            Self::Ssh { host, .. } => Client::connect_ssh(host, start),
        }
    }

    /// Names the server in messages.
    pub(crate) fn describe(&self) -> &str {
        match self {
            Self::Local { .. } => "local server",
            Self::Ssh { host, .. } => host.destination(),
        }
    }

    /// Where the TUI keeps its layout for `server`. The local server's stays
    /// in the profile; each remote server gets a private folder of its own.
    pub(crate) fn layout_directory(&self, server: ServerIdentity) -> PathBuf {
        match self {
            Self::Local { profile, .. } => profile.clone(),
            Self::Ssh { profile, .. } => profile.join("servers").join(server.to_string()),
        }
    }

    /// How long to wait before connecting again after `failures` failed
    /// attempts in a row.
    pub(crate) fn retry_delay(&self, failures: usize) -> Duration {
        match self {
            Self::Local { .. } => LOCAL_RETRY,
            Self::Ssh { .. } => SSH_RETRIES[failures.saturating_sub(1).min(SSH_RETRIES.len() - 1)],
        }
    }

    /// Adds the other computer's name to the errors that don't say which
    /// server stopped answering.
    pub(crate) fn explain(&self, error: impl Into<Box<dyn Error>>) -> Box<dyn Error> {
        let error = error.into();
        match (self, error.downcast_ref::<ClientError>()) {
            (Self::Ssh { host, .. }, Some(ClientError::Timeout | ClientError::Disconnected)) => {
                format!("{host}: {error}").into()
            }
            _ => error,
        }
    }
}
