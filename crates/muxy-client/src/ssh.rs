//! Servers on other computers, reached through the system's ssh, which runs
//! the bridge there.

use std::collections::VecDeque;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{ChildStderr, Command, Stdio};
use std::str::FromStr;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::ChildStream;

use crate::bridge::{self, BridgeExit, Start};
use crate::{Client, ClientError, RemoteReason};

/// Covers ssh's 10 s connect timeout, logging in, and starting the server.
const READY_TIMEOUT: Duration = Duration::from_secs(20);
/// How long a failed ssh gets to exit and finish its error output.
const EXIT_TIMEOUT: Duration = Duration::from_secs(1);
/// How much of ssh's error output is kept for messages.
const STDERR_TAIL: usize = 8 * 1024;
/// No prompts, since nobody may be there to answer. Keepalives end a dead
/// connection instead of hanging. A shared master would keep running with
/// our pipes, and the user's remote command and forwards belong to
/// interactive sessions, not to the bridge.
const OPTIONS: [&str; 7] = [
    "BatchMode=yes",
    "ConnectTimeout=10",
    "ServerAliveInterval=15",
    "ServerAliveCountMax=3",
    "ControlMaster=no",
    "RemoteCommand=none",
    "ClearAllForwardings=yes",
];

/// A computer to reach with ssh: an `ssh_config` alias, `user@host`, or
/// `ssh://user@host:port`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SshTarget {
    destination: String,
    program: Option<PathBuf>,
}

impl SshTarget {
    /// Refuses destinations that ssh could read as an option or that would
    /// split into several arguments.
    pub fn new(destination: &str) -> io::Result<Self> {
        let invalid = |message| Err(io::Error::new(io::ErrorKind::InvalidInput, message));
        if destination.is_empty() {
            return invalid("SSH destination must not be empty");
        }
        if destination.starts_with('-') {
            return invalid("SSH destination must not start with -");
        }
        if destination
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
        {
            return invalid("SSH destination must not contain spaces or control characters");
        }
        Ok(Self {
            destination: destination.into(),
            program: None,
        })
    }

    /// Runs `program` instead of ssh. It must accept ssh's arguments.
    #[must_use]
    pub fn with_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = Some(program.into());
        self
    }

    pub fn destination(&self) -> &str {
        &self.destination
    }

    fn arguments(&self, start: Start) -> Vec<&str> {
        let mut arguments = vec!["-T"];
        for option in OPTIONS {
            arguments.extend(["-o", option]);
        }
        arguments.extend(["--", &self.destination, bridge::command(start)]);
        arguments
    }
}

impl FromStr for SshTarget {
    type Err = io::Error;

    fn from_str(destination: &str) -> io::Result<Self> {
        Self::new(destination)
    }
}

impl fmt::Display for SshTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.destination)
    }
}

/// The program set on the target, else `MUXY_SSH`, else ssh from `PATH`.
fn program(explicit: Option<&Path>, environment: Option<OsString>) -> io::Result<PathBuf> {
    match (explicit, environment) {
        (Some(program), _) => Ok(program.into()),
        (None, Some(program)) if program.is_empty() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MUXY_SSH must not be empty",
        )),
        (None, Some(program)) => Ok(program.into()),
        (None, None) => Ok("ssh".into()),
    }
}

impl Client {
    /// Connects to the server on another computer through ssh, which runs
    /// `muxy stdio` there. ssh never prompts, so logging in needs ssh-agent,
    /// a key in `~/.ssh/config`, or a certificate, and the host's key must
    /// already be trusted.
    pub fn connect_ssh(target: &SshTarget, start: Start) -> Result<Self, ClientError> {
        let program = program(target.program.as_deref(), std::env::var_os("MUXY_SSH"))?;
        let mut command = Command::new(&program);
        command.args(target.arguments(start)).stderr(Stdio::piped());
        let mut stream = ChildStream::spawn(command).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                ClientError::Remote {
                    reason: RemoteReason::SshMissing,
                    destination: target.destination.clone(),
                    detail: program.display().to_string(),
                }
            } else {
                io::Error::new(
                    error.kind(),
                    format!("could not run {}: {error}", program.display()),
                )
                .into()
            }
        })?;
        let stderr = ErrorTail::drain(stream.take_stderr())?;
        let process = stream.process();
        Self::connect_bridge(Box::new(stream), &target.destination, READY_TIMEOUT, || {
            BridgeExit {
                status: process
                    .wait_timeout(EXIT_TIMEOUT)
                    .ok()
                    .flatten()
                    .and_then(|status| status.code()),
                stderr: stderr.wait(EXIT_TIMEOUT),
            }
        })
    }
}

/// The end of a child's error output. A thread drains it for the child's
/// whole life, so the child never blocks on a full pipe.
#[derive(Default)]
struct ErrorTail {
    state: Mutex<TailState>,
    ended: Condvar,
}

#[derive(Default)]
struct TailState {
    bytes: VecDeque<u8>,
    ended: bool,
}

impl ErrorTail {
    fn drain(stderr: Option<ChildStderr>) -> io::Result<Arc<Self>> {
        let tail = Arc::new(Self::default());
        let filling = Arc::clone(&tail);
        match stderr {
            Some(stderr) => {
                thread::Builder::new()
                    .name("muxy-ssh-stderr".into())
                    .spawn(move || filling.fill(stderr))?;
            }
            None => filling.finish(),
        }
        Ok(tail)
    }

    fn fill(&self, mut stderr: impl Read) {
        let mut chunk = [0; 4096];
        loop {
            match stderr.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => self.push(&chunk[..read]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        self.finish();
    }

    fn push(&self, bytes: &[u8]) {
        let mut state = self.lock();
        state.bytes.extend(bytes);
        let excess = state.bytes.len().saturating_sub(STDERR_TAIL);
        state.bytes.drain(..excess);
    }

    fn finish(&self) {
        self.lock().ended = true;
        self.ended.notify_all();
    }

    /// Waits up to `timeout` for the output to end, then returns its tail.
    fn wait(&self, timeout: Duration) -> String {
        let (mut state, _) = self
            .ended
            .wait_timeout_while(self.lock(), timeout, |state| !state.ended)
            .unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(state.bytes.make_contiguous()).into_owned()
    }

    fn lock(&self) -> MutexGuard<'_, TailState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_ssh_names_the_program_and_the_host() -> io::Result<()> {
        let target = SshTarget::new("box")?.with_program("/nonexistent/ssh");
        let error = Client::connect_ssh(&target, Start::IfNeeded).err();
        assert!(matches!(
            &error,
            Some(ClientError::Remote { reason: RemoteReason::SshMissing, detail, .. })
                if detail == "/nonexistent/ssh"
        ));
        assert_eq!(
            error.map(|error| error.to_string()),
            Some("Muxy needs /nonexistent/ssh to reach box, but it isn't installed.".into())
        );
        Ok(())
    }

    #[test]
    fn destinations_that_ssh_could_misread_are_refused() {
        for accepted in [
            "box",
            "dev@box",
            "dev@10.0.0.4",
            "ssh://dev@box.example.com:2222",
            "dev@[::1]",
        ] {
            assert!(SshTarget::new(accepted).is_ok(), "{accepted}");
        }
        for refused in [
            "",
            "-oProxyCommand=evil",
            "-",
            "my box",
            "box\n",
            "box\t",
            "box\u{0}",
            "\u{7}box",
            "box\u{a0}",
        ] {
            let error = SshTarget::new(refused).err();
            assert_eq!(
                error.map(|error| error.kind()),
                Some(io::ErrorKind::InvalidInput),
                "{refused:?}"
            );
        }
        assert_eq!(
            "dev@box"
                .parse::<SshTarget>()
                .map(|target| target.to_string())
                .ok(),
            Some("dev@box".into())
        );
    }

    #[test]
    fn ssh_runs_without_prompts_and_with_the_bridge_as_the_last_argument() -> io::Result<()> {
        let target = SshTarget::new("dev@box")?;
        let options = [
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=3",
            "-o",
            "ControlMaster=no",
            "-o",
            "RemoteCommand=none",
            "-o",
            "ClearAllForwardings=yes",
            "--",
            "dev@box",
        ];
        for start in [Start::IfNeeded, Start::Never] {
            let mut expected = options.to_vec();
            expected.push(bridge::command(start));
            assert_eq!(target.arguments(start), expected);
        }
        Ok(())
    }

    #[test]
    fn the_program_is_the_targets_then_muxy_ssh_then_ssh() -> io::Result<()> {
        let fake = Path::new("/tmp/fake-ssh");
        assert_eq!(program(Some(fake), Some("/usr/bin/other".into()))?, fake);
        assert_eq!(
            program(None, Some("/usr/bin/other".into()))?,
            Path::new("/usr/bin/other")
        );
        assert_eq!(program(None, None)?, Path::new("ssh"));
        assert_eq!(
            program(None, Some(OsString::new()))
                .err()
                .map(|error| error.kind()),
            Some(io::ErrorKind::InvalidInput)
        );
        assert_eq!(
            SshTarget::new("box")?.with_program(fake).program.as_deref(),
            Some(fake)
        );
        Ok(())
    }

    #[test]
    fn only_the_end_of_the_error_output_is_kept() {
        let tail = ErrorTail::default();
        tail.push(&[b'a'; STDERR_TAIL]);
        tail.push(b"last line\n");
        tail.finish();
        assert_eq!(
            tail.wait(Duration::ZERO),
            format!("{}last line\n", "a".repeat(STDERR_TAIL - 10))
        );
    }
}
