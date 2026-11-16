mod readiness;

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::{self, ByteStream, StreamCancellation};

use crate::client::DEFAULT_TIMEOUT;
use crate::{Client, ClientError, RemoteReason, local};
use readiness::{NotReady, READY, VERSION, ready};

const MAX_DETAIL: usize = 300;
const NOT_RUNNING: &str = "the server isn't running";
const NOT_FOUND: [&str; 3] = [
    "muxy: not found",
    "muxy: command not found",
    "command not found: muxy",
];
const UNREACHABLE: [&str; 8] = [
    "Could not resolve hostname",
    "connect to host",
    "Connection closed by",
    "Connection reset by",
    "Connection timed out",
    "kex_exchange_identification",
    "No route to host",
    "Network is unreachable",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Start {
    IfNeeded,
    Never,
}

pub fn command(start: Start) -> &'static str {
    match start {
        Start::IfNeeded => r#"sh -c 'export PATH="$PATH:$HOME/.local/bin"; exec muxy stdio'"#,
        Start::Never => {
            r#"sh -c 'export PATH="$PATH:$HOME/.local/bin"; exec muxy stdio --no-start'"#
        }
    }
}

pub fn serve(socket: &Path, executable: &Path, start: Start) -> Result<(), ClientError> {
    let server = match start {
        Start::IfNeeded => local::ensure_listening(socket, executable)?,
        Start::Never => local::connect(socket).map_err(|error| {
            if local::unavailable(&error) {
                io::Error::new(io::ErrorKind::NotFound, NOT_RUNNING).into()
            } else {
                error
            }
        })?,
    };
    let input = File::from(owned(&io::stdin())?);
    let mut output = File::from(owned(&io::stdout())?);
    output.write_all(&[READY, VERSION, b"\n"].concat())?;
    transport::relay(server, input, output)?;
    Ok(())
}

#[cfg(unix)]
fn owned(stream: &impl std::os::fd::AsFd) -> io::Result<std::os::fd::OwnedFd> {
    stream.as_fd().try_clone_to_owned()
}

#[cfg(windows)]
fn owned(
    stream: &impl std::os::windows::io::AsHandle,
) -> io::Result<std::os::windows::io::OwnedHandle> {
    stream.as_handle().try_clone_to_owned()
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BridgeExit {
    pub status: Option<i32>,
    pub stderr: String,
}

impl Client {
    pub fn connect_bridge(
        stream: Box<dyn ByteStream>,
        destination: &str,
        timeout: Duration,
        exited: impl FnOnce() -> BridgeExit,
    ) -> Result<Self, ClientError> {
        let cancellation: Arc<dyn StreamCancellation> = stream.cancellation()?.into();
        let (reader, writer) = stream.split()?;
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("muxy-bridge-ready".into())
            .spawn(move || {
                let _ = sender.send(ready(reader));
            })?;
        let (reason, detail) = match receiver.recv_timeout(timeout) {
            Ok(Ok(reader)) => {
                let stream = Bridged {
                    reader: Box::new(reader),
                    writer,
                    cancellation,
                };
                return Self::from_stream_with_timeout(Box::new(stream), DEFAULT_TIMEOUT).map_err(
                    |error| match error {
                        ClientError::VersionUnsupported => ClientError::Remote {
                            reason: RemoteReason::Incompatible,
                            destination: destination.into(),
                            detail: "its running server is a different version".into(),
                        },
                        error => error,
                    },
                );
            }
            Ok(Err(NotReady::Ended(printed))) => {
                let exit = exited();
                cancellation.cancel();
                explain(&exit, &printed)
            }
            Ok(Err(NotReady::Noisy(printed))) => {
                cancellation.cancel();
                (RemoteReason::UnexpectedOutput, last_line(&printed))
            }
            Ok(Err(NotReady::Version(version))) => {
                cancellation.cancel();
                (
                    RemoteReason::Incompatible,
                    format!("it speaks bridge version {}", last_line(&version)),
                )
            }
            Err(_) => {
                cancellation.cancel();
                (RemoteReason::Timeout, last_line(exited().stderr.as_bytes()))
            }
        };
        Err(ClientError::Remote {
            reason,
            destination: destination.into(),
            detail,
        })
    }
}

struct Bridged {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    cancellation: Arc<dyn StreamCancellation>,
}

impl ByteStream for Bridged {
    fn cancellation(&self) -> io::Result<Box<dyn StreamCancellation>> {
        Ok(Box::new(SharedCancellation(Arc::clone(&self.cancellation))))
    }

    #[allow(clippy::type_complexity)]
    fn split(self: Box<Self>) -> io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
        Ok((self.reader, self.writer))
    }
}

struct SharedCancellation(Arc<dyn StreamCancellation>);

impl StreamCancellation for SharedCancellation {
    fn cancel(&self) {
        self.0.cancel();
    }
}

fn explain(exit: &BridgeExit, printed: &[u8]) -> (RemoteReason, String) {
    let stderr = exit.stderr.as_str();
    let said = |texts: &[&str]| texts.iter().any(|text| stderr.contains(text));
    let last = last_line(stderr.as_bytes());
    let ssh = matches!(exit.status, Some(255) | None);
    let reason = if ssh && said(&["REMOTE HOST IDENTIFICATION HAS CHANGED"]) {
        RemoteReason::HostKeyChanged
    } else if ssh && said(&["Host key verification failed", "host key is known for"]) {
        RemoteReason::HostKeyUnknown
    } else if ssh && said(&["Permission denied (", "Too many authentication failures"]) {
        RemoteReason::AuthenticationFailed
    } else if exit.status == Some(127) || said(&NOT_FOUND) {
        RemoteReason::NotInstalled
    } else if said(&["muxy: unknown command"]) {
        RemoteReason::Incompatible
    } else if said(&[NOT_RUNNING]) {
        RemoteReason::NotRunning
    } else if !last.is_empty() && (exit.status == Some(255) || (ssh && said(&UNREACHABLE))) {
        RemoteReason::Unreachable
    } else if !last.is_empty() && exit.status != Some(0) {
        RemoteReason::BridgeFailed
    } else if !printed.trim_ascii().is_empty() {
        RemoteReason::UnexpectedOutput
    } else {
        RemoteReason::BridgeFailed
    };
    let detail = match reason {
        RemoteReason::Incompatible => "it predates remote connections".into(),
        RemoteReason::UnexpectedOutput => last_line(printed),
        RemoteReason::BridgeFailed if last.is_empty() => match exit.status {
            Some(status) => {
                format!("the connection closed before Muxy started (exit status {status})")
            }
            None => "the connection closed before Muxy started".into(),
        },
        RemoteReason::BridgeFailed => last.strip_prefix("muxy: ").unwrap_or(&last).into(),
        _ => last,
    };
    (reason, detail)
}

fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_DETAIL)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exit(status: Option<i32>, stderr: &str) -> BridgeExit {
        BridgeExit {
            status,
            stderr: stderr.into(),
        }
    }

    #[test]
    fn ssh_failures_are_told_apart_by_its_own_words() {
        let ssh = |stderr: &str| explain(&exit(Some(255), stderr), b"");
        let changed = "@@@@\n@ WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED! @\n\
                       Host key verification failed.\n";
        assert_eq!(ssh(changed).0, RemoteReason::HostKeyChanged);
        for unknown in [
            "Host key verification failed.\r\n",
            "No ED25519 host key is known for box and you have requested strict checking.\n",
        ] {
            assert_eq!(ssh(unknown).0, RemoteReason::HostKeyUnknown);
        }
        assert_eq!(
            ssh("dev@box: Permission denied (publickey).\r\n"),
            (
                RemoteReason::AuthenticationFailed,
                "dev@box: Permission denied (publickey).".into()
            )
        );
        for unreachable in [
            "ssh: Could not resolve hostname box: Name or service not known\n",
            "ssh: connect to host box port 22: Connection refused\n",
            "/home/me/.ssh/config: line 3: Bad configuration option: foo\n",
        ] {
            assert_eq!(
                ssh(unreachable),
                (RemoteReason::Unreachable, unreachable.trim().into())
            );
        }
        let reset = exit(
            None,
            "kex_exchange_identification: Connection reset by peer\n",
        );
        assert_eq!(explain(&reset, b"").0, RemoteReason::Unreachable);
        assert_eq!(
            explain(&exit(None, "oops\n"), b"").0,
            RemoteReason::BridgeFailed
        );
    }
}
