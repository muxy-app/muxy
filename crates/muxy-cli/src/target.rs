use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use muxy_client::{Backoff, Client, ClientError, SshTarget, Start};
use muxy_protocol::{ServerIdentity, SessionId};

const LOCAL_RETRY: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
pub(crate) enum Target {
    Local {
        profile: PathBuf,
        executable: PathBuf,
    },
    Ssh {
        profile: PathBuf,
        host: Box<SshTarget>,
    },
}

impl Target {
    pub(crate) fn new(host: Option<SshTarget>) -> io::Result<Self> {
        let profile = muxy_core::dirs::muxy_dir()?;
        Ok(match host {
            Some(host) => Self::Ssh {
                profile,
                host: Box::new(host),
            },
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

    pub(crate) fn describe(&self) -> &str {
        match self {
            Self::Local { .. } => "local server",
            Self::Ssh { host, .. } => host.destination(),
        }
    }

    pub(crate) fn layout_directory(&self, server: ServerIdentity) -> PathBuf {
        match self {
            Self::Local { profile, .. } => profile.clone(),
            Self::Ssh { profile, .. } => profile.join("servers").join(server.to_string()),
        }
    }

    pub(crate) fn retry_delay(&self, failures: usize) -> Duration {
        match self {
            Self::Local { .. } => LOCAL_RETRY,
            Self::Ssh { .. } => Backoff::Interactive
                .delay(u32::try_from(failures.saturating_sub(1)).unwrap_or(u32::MAX)),
        }
    }

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

pub(crate) fn hosting_session(server: ServerIdentity) -> Option<SessionId> {
    let inherited: ServerIdentity = std::env::var("MUXY_SERVER_ID").ok()?.parse().ok()?;
    (inherited == server).then_some(())?;
    SessionId::new(std::env::var("MUXY_SESSION_ID").ok()?.parse().ok()?)
}
