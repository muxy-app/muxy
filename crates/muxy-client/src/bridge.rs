//! The bridge to a server on another computer. `muxy stdio` runs there,
//! joined to that computer's server, and its stdin and stdout carry the
//! protocol unchanged, so version negotiation stays end to end.
//!
//! The bridge writes a ready line before relaying, so the client can skip
//! whatever the remote shell prints first.

use std::fs::File;
use std::io::{self, Cursor, Read, Write};
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::{self, ByteStream, StreamCancellation};

use crate::client::DEFAULT_TIMEOUT;
use crate::{Client, ClientError, RemoteReason, local};

/// Starts the ready line; the bridge version and a newline follow.
const READY: &[u8] = b"MUXY-STDIO/";
const VERSION: &[u8] = b"1";
/// The most a remote shell may print before the ready line.
const MAX_NOISE: usize = 64 * 1024;
/// Longer than any version this build could need to read.
const MAX_VERSION: usize = 16;
/// Keeps error details to one readable line.
const MAX_DETAIL: usize = 300;
/// What `muxy stdio --no-start` reports when no server is running.
const NOT_RUNNING: &str = "the server isn't running";
/// How shells report that `muxy` isn't on PATH.
const NOT_FOUND: [&str; 3] = [
    "muxy: not found",
    "muxy: command not found",
    "command not found: muxy",
];
/// ssh's own words when the network or the host's sshd fails it.
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

/// Whether connecting may start a server that isn't running.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Start {
    /// Start the server if it isn't running, like most commands.
    IfNeeded,
    /// Only use a running server, like `muxy server status`.
    Never,
}

/// The command a remote login shell runs to start the bridge. It runs under
/// `sh` whatever the login shell is, and also looks in `~/.local/bin`, where
/// the installer puts `muxy`.
pub fn command(start: Start) -> &'static str {
    match start {
        Start::IfNeeded => r#"sh -c 'export PATH="$PATH:$HOME/.local/bin"; exec muxy stdio'"#,
        Start::Never => {
            r#"sh -c 'export PATH="$PATH:$HOME/.local/bin"; exec muxy stdio --no-start'"#
        }
    }
}

/// Runs the bridge: connects to the server behind `socket`, writes the ready
/// line, then relays stdin and stdout until the server closes. The copies are
/// unbuffered, since std's line-buffered stdout would hold binary data back.
pub fn serve(socket: &Path, executable: &Path, start: Start) -> Result<(), ClientError> {
    let server = match start {
        Start::IfNeeded => local::ensure_listening(socket, executable)?,
        Start::Never => transport::connect(socket).map_err(|error| {
            let error = ClientError::Io(error);
            if local::unavailable(&error) {
                io::Error::new(io::ErrorKind::NotFound, NOT_RUNNING).into()
            } else {
                error
            }
        })?,
    };
    let input = File::from(io::stdin().as_fd().try_clone_to_owned()?);
    let mut output = File::from(io::stdout().as_fd().try_clone_to_owned()?);
    output.write_all(&[READY, VERSION, b"\n"].concat())?;
    transport::relay(server, input, output)?;
    Ok(())
}

/// How a bridge's transport ended, to explain a connection that failed before
/// the bridge was ready.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BridgeExit {
    /// The exit status, such as 255 from ssh itself or 127 from a shell that
    /// can't find `muxy`.
    pub status: Option<i32>,
    /// The end of the transport's error output.
    pub stderr: String,
}

impl Client {
    /// Connects through `stream` to the bridge on another computer, skipping
    /// what its shell prints first. `destination` names that computer in
    /// errors, and `timeout` limits the wait for the bridge, after which the
    /// handshake gets the usual time. If the stream ends before the bridge is
    /// ready, `exited` explains how its transport ended.
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
            Ok(Ok((reader, rest))) => {
                let stream = Bridged {
                    reader: Box::new(Cursor::new(rest).chain(reader)),
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

/// The bridge's stream once it is ready, with any bytes read past the ready
/// line put back in front.
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

#[derive(Debug, Eq, PartialEq)]
enum NotReady {
    /// The stream ended first, after the shell printed this.
    Ended(Vec<u8>),
    /// The shell printed more than `MAX_NOISE` bytes first.
    Noisy(Vec<u8>),
    /// A bridge answered with a version this build can't speak.
    Version(Vec<u8>),
}

/// Reads up to the ready line, then returns the reader and any bytes read past it.
fn ready<R: Read>(mut reader: R) -> Result<(R, Vec<u8>), NotReady> {
    let mut line = ReadyLine::default();
    let mut chunk = [0; 4096];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => return Err(line.ended()),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(line.ended()),
        };
        if let Some(outcome) = line.push(&chunk[..read]) {
            return outcome.map(|rest| (reader, rest));
        }
    }
}

/// Finds the ready line in what the bridge's transport prints first.
#[derive(Default)]
struct ReadyLine {
    seen: Vec<u8>,
    /// Where the ready line starts, once found.
    start: Option<usize>,
    /// The ready line can't start before this.
    searched: usize,
}

impl ReadyLine {
    /// Adds the next bytes. Returns the bytes after the ready line once it is
    /// complete, or why it can't come.
    fn push(&mut self, bytes: &[u8]) -> Option<Result<Vec<u8>, NotReady>> {
        self.seen.extend_from_slice(bytes);
        let Some(start) = self.start.or_else(|| self.find()) else {
            return (self.seen.len() >= MAX_NOISE + READY.len())
                .then(|| Err(NotReady::Noisy(std::mem::take(&mut self.seen))));
        };
        if start > MAX_NOISE {
            return Some(Err(NotReady::Noisy(self.seen[..start].to_vec())));
        }
        let line = &self.seen[start + READY.len()..];
        match line.iter().position(|&byte| byte == b'\n') {
            Some(end) if &line[..end] == VERSION => {
                Some(Ok(self.seen.split_off(start + READY.len() + end + 1)))
            }
            Some(end) => Some(Err(NotReady::Version(line[..end].to_vec()))),
            None if line.len() > MAX_VERSION => {
                Some(Err(NotReady::Version(line[..MAX_VERSION].to_vec())))
            }
            None => None,
        }
    }

    /// Searches only the bytes not searched before, and keeps a match.
    fn find(&mut self) -> Option<usize> {
        self.start = self.seen[self.searched..]
            .windows(READY.len())
            .position(|window| window == READY)
            .map(|offset| self.searched + offset);
        self.searched = self.seen.len().saturating_sub(READY.len() - 1);
        self.start
    }

    fn ended(self) -> NotReady {
        NotReady::Ended(self.seen)
    }
}

/// Explains a transport that ended before the bridge was ready, from its
/// exit status, its error output, and what it printed.
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

/// The last line with text, cleaned up for a one-line message.
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

    fn scan(chunks: &[&[u8]]) -> Result<Vec<u8>, NotReady> {
        let mut line = ReadyLine::default();
        for chunk in chunks {
            if let Some(outcome) = line.push(chunk) {
                return outcome;
            }
        }
        Err(line.ended())
    }

    fn exit(status: Option<i32>, stderr: &str) -> BridgeExit {
        BridgeExit {
            status,
            stderr: stderr.into(),
        }
    }

    #[test]
    fn the_ready_line_is_found_after_any_noise_and_keeps_later_bytes() {
        assert_eq!(scan(&[b"MUXY-STDIO/1\n"]), Ok(vec![]));
        assert_eq!(
            scan(&[b"Welcome!\nlast login\n", b"MUXY-STDIO/1\nhello"]),
            Ok(b"hello".to_vec())
        );
        assert_eq!(scan(&[b"no newline", b"MUXY-STDIO/1\n"]), Ok(vec![]));
        assert_eq!(
            scan(&[b"noise MUXY-", b"ST", b"DIO/", b"1", b"\nrest"]),
            Ok(b"rest".to_vec())
        );
    }

    #[test]
    fn up_to_64_kib_of_noise_is_skipped_and_more_is_refused() {
        let allowed = vec![b'x'; MAX_NOISE];
        assert_eq!(scan(&[&allowed, b"MUXY-STDIO/1\n"]), Ok(vec![]));
        let mut chunks: Vec<&[u8]> = allowed.chunks(1000).collect();
        chunks.push(b"MUXY-STDIO/1\n");
        assert_eq!(scan(&chunks), Ok(vec![]));

        let too_much = vec![b'x'; MAX_NOISE + 1];
        assert!(matches!(
            scan(&[&too_much, b"MUXY-STDIO/1\n"]),
            Err(NotReady::Noisy(noise)) if noise.len() == MAX_NOISE + 1
        ));
        let endless = vec![b'y'; MAX_NOISE + READY.len()];
        assert!(matches!(scan(&[&endless]), Err(NotReady::Noisy(_))));
    }

    #[test]
    fn an_early_end_or_another_version_is_reported() {
        assert_eq!(scan(&[]), Err(NotReady::Ended(vec![])));
        assert_eq!(
            scan(&[b"Welcome\n", b"MUXY-STDIO/"]),
            Err(NotReady::Ended(b"Welcome\nMUXY-STDIO/".to_vec()))
        );
        assert_eq!(
            scan(&[b"MUXY-STDIO/2\n"]),
            Err(NotReady::Version(b"2".to_vec()))
        );
        assert_eq!(
            scan(&[b"MUXY-STDIO/12345678901234567890"]),
            Err(NotReady::Version(b"1234567890123456".to_vec()))
        );
    }

    #[test]
    fn reading_returns_the_reader_with_the_bytes_after_the_ready_line() -> io::Result<()> {
        let (mut reader, rest) =
            ready(&b"motd\nMUXY-STDIO/1\nab"[..]).map_err(|_| io::Error::other("no ready line"))?;
        assert_eq!(rest, b"ab");
        let mut remaining = Vec::new();
        reader.read_to_end(&mut remaining)?;
        assert!(remaining.is_empty());
        assert_eq!(
            ready(&b"motd\n"[..]).err(),
            Some(NotReady::Ended(b"motd\n".to_vec()))
        );
        Ok(())
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

    #[test]
    fn remote_failures_are_told_apart_by_status_and_words() {
        let failed = |status, stderr: &str| explain(&exit(Some(status), stderr), b"Welcome\n");
        assert_eq!(
            failed(127, "sh: 1: exec: muxy: not found\n").0,
            RemoteReason::NotInstalled
        );
        let zsh = exit(None, "zsh:1: command not found: muxy\n");
        assert_eq!(explain(&zsh, b"").0, RemoteReason::NotInstalled);
        assert_eq!(
            failed(1, "muxy: unknown command; run muxy --help\n"),
            (
                RemoteReason::Incompatible,
                "it predates remote connections".into()
            )
        );
        assert_eq!(
            failed(1, "muxy: connection failed: the server isn't running\n").0,
            RemoteReason::NotRunning
        );
        assert_eq!(
            failed(
                1,
                "muxy: connection failed: could not launch server /x: denied\n"
            ),
            (
                RemoteReason::BridgeFailed,
                "connection failed: could not launch server /x: denied".into()
            )
        );
        assert_eq!(
            failed(
                126,
                "sh: 1: exec: /home/me/.local/bin/muxy: Exec format error\n"
            )
            .0,
            RemoteReason::BridgeFailed
        );
    }

    #[test]
    fn output_without_a_bridge_is_reported_and_silence_says_so() {
        assert_eq!(
            explain(
                &exit(Some(0), ""),
                b"Welcome to box\nThis account is restricted\n"
            ),
            (
                RemoteReason::UnexpectedOutput,
                "This account is restricted".into()
            )
        );
        let warned = exit(Some(0), "bash: warning: setlocale\n");
        assert_eq!(
            explain(&warned, b"restricted\n").0,
            RemoteReason::UnexpectedOutput
        );
        assert_eq!(
            explain(&exit(Some(1), ""), b""),
            (
                RemoteReason::BridgeFailed,
                "the connection closed before Muxy started (exit status 1)".into()
            )
        );
        assert_eq!(
            explain(&exit(None, ""), b" \n").1,
            "the connection closed before Muxy started"
        );
    }

    #[test]
    fn details_are_one_clean_line() {
        assert_eq!(last_line(b"first\n\x1b[31mred\x1b[0m\r\n\n"), "[31mred[0m");
        assert_eq!(last_line(b"\xff\xfe broken\n"), "\u{fffd}\u{fffd} broken");
        assert_eq!(last_line(&[b'a'; 1000]).len(), MAX_DETAIL);
        assert_eq!(last_line(b" \n\t\n"), "");
    }

    #[test]
    fn the_remote_command_runs_the_bridge_under_sh_with_the_installer_path() {
        assert_eq!(
            command(Start::IfNeeded),
            r#"sh -c 'export PATH="$PATH:$HOME/.local/bin"; exec muxy stdio'"#
        );
        assert_eq!(
            command(Start::Never),
            r#"sh -c 'export PATH="$PATH:$HOME/.local/bin"; exec muxy stdio --no-start'"#
        );
    }
}
