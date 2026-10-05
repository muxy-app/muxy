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

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_client::RemoteReason;

    fn ssh() -> io::Result<Target> {
        Ok(Target::Ssh {
            profile: "/profile".into(),
            host: SshTarget::new("dev@box")?,
        })
    }

    fn local() -> Target {
        Target::Local {
            profile: "/profile".into(),
            executable: "/bin/muxy-server".into(),
        }
    }

    #[test]
    fn ssh_backs_off_and_local_retries_steadily() -> io::Result<()> {
        let delays: Vec<_> = (1..=7)
            .map(|failures| ssh().map(|target| target.retry_delay(failures)))
            .collect::<io::Result<_>>()?;
        assert_eq!(
            delays,
            [500, 1_000, 2_000, 5_000, 10_000, 10_000, 10_000].map(Duration::from_millis)
        );
        assert_eq!(local().retry_delay(9), Duration::from_millis(500));
        Ok(())
    }

    #[test]
    fn remote_layouts_live_in_a_folder_per_server() -> io::Result<()> {
        let server = ServerIdentity::from_u128(0x2a);
        assert_eq!(
            ssh()?.layout_directory(server),
            PathBuf::from(format!("/profile/servers/{server}"))
        );
        assert_eq!(local().layout_directory(server), PathBuf::from("/profile"));
        assert_eq!(ssh()?.describe(), "dev@box");
        assert_eq!(local().describe(), "local server");
        Ok(())
    }

    #[test]
    fn only_remote_timeouts_and_disconnects_name_the_host() -> io::Result<()> {
        let explain = |target: &Target, error: ClientError| target.explain(error).to_string();
        assert_eq!(
            explain(&ssh()?, ClientError::Timeout),
            "dev@box: request timed out"
        );
        assert_eq!(
            explain(&ssh()?, ClientError::Disconnected),
            "dev@box: disconnected from server"
        );
        assert_eq!(explain(&local(), ClientError::Timeout), "request timed out");
        let refused = ClientError::Remote {
            reason: RemoteReason::NotRunning,
            destination: "dev@box".into(),
            detail: String::new(),
        };
        assert_eq!(
            explain(&ssh()?, refused),
            "Muxy's server isn't running on dev@box."
        );
        Ok(())
    }

    #[test]
    fn only_network_and_bridge_failures_are_tried_again() {
        let remote = |reason| ClientError::Remote {
            reason,
            destination: "dev@box".into(),
            detail: String::new(),
        };
        for reason in [
            RemoteReason::Unreachable,
            RemoteReason::Timeout,
            RemoteReason::BridgeFailed,
        ] {
            assert!(remote(reason).recoverable(), "{reason:?}");
        }
        for reason in [
            RemoteReason::HostKeyUnknown,
            RemoteReason::HostKeyChanged,
            RemoteReason::AuthenticationFailed,
            RemoteReason::NotInstalled,
            RemoteReason::NotRunning,
            RemoteReason::SshMissing,
            RemoteReason::UnexpectedOutput,
            RemoteReason::Incompatible,
        ] {
            assert!(!remote(reason).recoverable(), "{reason:?}");
        }
        assert!(ClientError::Timeout.recoverable());
        assert!(ClientError::Disconnected.recoverable());
        assert!(ClientError::Io(io::ErrorKind::ConnectionRefused.into()).recoverable());
    }
}
