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
/// Keepalives end a dead connection instead of hanging. A shared master
/// would keep running with our pipes, and the user's remote command and
/// forwards belong to interactive sessions, not to the bridge.
const OPTIONS: [&str; 6] = [
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
    identity: Option<PathBuf>,
    askpass: Option<Askpass>,
    host_key_prompt: bool,
}

/// A program that answers ssh's prompts, such as for a saved password. ssh
/// runs it with the prompt as its only argument and reads the answer.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Askpass {
    pub program: PathBuf,
    /// More environment for the program, such as which answer to give.
    pub environment: Vec<(OsString, OsString)>,
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
            identity: None,
            askpass: None,
            host_key_prompt: false,
        })
    }

    /// Logs in with this private key file only.
    #[must_use]
    pub fn with_identity(mut self, identity: impl Into<PathBuf>) -> Self {
        self.identity = Some(identity.into());
        self
    }

    /// Lets ssh ask `askpass` once for a password or a key's passphrase.
    /// Without it, ssh never prompts.
    #[must_use]
    pub fn with_askpass(mut self, askpass: Askpass) -> Self {
        self.askpass = Some(askpass);
        self
    }

    #[must_use]
    pub fn with_host_key_prompt(mut self) -> Self {
        self.host_key_prompt = true;
        self
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

    pub fn askpass(&self) -> Option<&Askpass> {
        self.askpass.as_ref()
    }

    /// The login ssh prompts for, `user@host`, as its config resolves this
    /// destination (`ssh -G`, which reads config and contacts nothing). A
    /// password prompt that names another login, such as a jump host's, is
    /// not for this destination.
    pub fn login(&self) -> io::Result<String> {
        login_from(&self.config()?).ok_or_else(|| {
            io::Error::other(format!("no SSH user or host for {}", self.destination))
        })
    }

    pub fn default_identity(&self) -> io::Result<Option<PathBuf>> {
        let config = self.config()?;
        Ok(std::env::home_dir().and_then(|home| identity_from(&config, &home)))
    }

    fn config(&self) -> io::Result<String> {
        let program = program(self.program.as_deref(), std::env::var_os("MUXY_SSH"))?;
        let output = Command::new(program)
            .args(["-G", "--", &self.destination])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "could not read the SSH settings for {}",
                self.destination
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Runs `remote` there through the same ssh and login as the bridge,
    /// such as an installer. Its output is the caller's to read.
    pub fn command(&self, remote: &str) -> io::Result<Command> {
        let program = program(self.program.as_deref(), std::env::var_os("MUXY_SSH"))?;
        let mut command = Command::new(program);
        command
            .args(self.arguments_for(remote))
            .envs(self.environment());
        Ok(command)
    }

    fn arguments(&self, start: Start) -> Vec<OsString> {
        self.arguments_for(bridge::command(start))
    }

    /// Prompts only reach an askpass program, since nobody may be there to
    /// answer them.
    fn arguments_for(&self, remote: &str) -> Vec<OsString> {
        let mut arguments: Vec<OsString> = vec!["-T".into()];
        let prompts: &[&str] = if self.askpass.is_some() {
            &["BatchMode=no", "NumberOfPasswordPrompts=1"]
        } else {
            &["BatchMode=yes"]
        };
        for option in prompts.iter().chain(&OPTIONS) {
            arguments.extend(["-o".into(), (*option).into()]);
        }
        if self.host_key_prompt {
            for option in [
                "StrictHostKeyChecking=ask",
                "FingerprintHash=sha256",
                "ControlPath=none",
            ] {
                arguments.extend(["-o".into(), option.into()]);
            }
        }
        if let Some(identity) = &self.identity {
            arguments.extend([
                "-i".into(),
                identity.clone().into_os_string(),
                "-o".into(),
                "IdentitiesOnly=yes".into(),
            ]);
        }
        arguments.extend(["--".into(), self.destination.clone().into(), remote.into()]);
        arguments
    }

    /// `SSH_ASKPASS_REQUIRE=force` makes ssh use the program even without
    /// a display or a terminal.
    fn environment(&self) -> Vec<(OsString, OsString)> {
        let Some(askpass) = &self.askpass else {
            return Vec::new();
        };
        [
            (
                "SSH_ASKPASS".into(),
                askpass.program.clone().into_os_string(),
            ),
            ("SSH_ASKPASS_REQUIRE".into(), "force".into()),
        ]
        .into_iter()
        .chain(askpass.environment.iter().cloned())
        .collect()
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

/// `user@host` from `ssh -G` output. ssh names a host key alias in its
/// prompts instead of the host name, when there is one.
fn login_from(config: &str) -> Option<String> {
    let value = |key: &str| {
        config.lines().find_map(|line| {
            let (name, value) = line.split_once(' ')?;
            (name == key).then(|| value.trim().to_owned())
        })
    };
    let user = value("user")?;
    let host = value("hostkeyalias").or_else(|| value("hostname"))?;
    Some(format!("{user}@{host}"))
}

fn identity_from(config: &str, home: &Path) -> Option<PathBuf> {
    config.lines().find_map(|line| {
        let value = line.strip_prefix("identityfile ")?.trim();
        let path = if let Some(relative) = value
            .strip_prefix("~/")
            .or_else(|| value.strip_prefix("%d/"))
        {
            home.join(relative)
        } else if value.contains('%') || value.starts_with('~') || value == "none" {
            return None;
        } else {
            PathBuf::from(value)
        };
        path.is_file().then_some(path)
    })
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
    /// `muxy stdio` there. ssh prompts only through the target's askpass
    /// program, so logging in otherwise needs ssh-agent, a key, or a
    /// certificate. The host's key must already be trusted.
    pub fn connect_ssh(target: &SshTarget, start: Start) -> Result<Self, ClientError> {
        let program = program(target.program.as_deref(), std::env::var_os("MUXY_SSH"))?;
        let mut command = Command::new(&program);
        command
            .args(target.arguments(start))
            .envs(target.environment())
            .stderr(Stdio::piped());
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
    fn host_key_prompts_keep_verification_and_disable_shared_connections() -> io::Result<()> {
        let target = SshTarget::new("ssh://dev@box:2222")?.with_host_key_prompt();
        let args = target.arguments(Start::IfNeeded);
        for option in [
            "StrictHostKeyChecking=ask",
            "FingerprintHash=sha256",
            "ControlPath=none",
        ] {
            assert!(args.windows(2).any(|args| args == ["-o", option]));
        }
        assert!(
            !args.iter().any(|arg| arg == "StrictHostKeyChecking=no"
                || arg == "StrictHostKeyChecking=accept-new")
        );
        Ok(())
    }

    #[test]
    fn identity_detection_uses_existing_configured_keys_in_order() -> io::Result<()> {
        let home = tempfile::tempdir()?;
        let ssh = home.path().join(".ssh");
        std::fs::create_dir(&ssh)?;
        std::fs::write(ssh.join("id_ed25519"), "fixture")?;
        std::fs::write(ssh.join("custom key"), "fixture")?;
        let config = "identityfile ~/.ssh/missing\nidentityfile %d/.ssh/custom key\nidentityfile ~/.ssh/id_ed25519\n";
        assert_eq!(
            identity_from(config, home.path()),
            Some(ssh.join("custom key"))
        );
        assert_eq!(identity_from("identityfile none\n", home.path()), None);
        assert_eq!(identity_from("identityfile ~/.ssh\n", home.path()), None);
        assert_eq!(identity_from("identityfile %h/key\n", home.path()), None);
        assert_eq!(
            identity_from(
                &format!("identityfile {}\n", ssh.join("id_ed25519").display()),
                home.path()
            ),
            Some(ssh.join("id_ed25519"))
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
    fn the_login_is_the_resolved_user_and_host_or_host_key_alias() {
        let config = "user dev\nhostname box.example.com\nport 22\n";
        assert_eq!(login_from(config).as_deref(), Some("dev@box.example.com"));
        let aliased = "user dev\nhostname 10.0.0.5\nhostkeyalias box\n";
        assert_eq!(login_from(aliased).as_deref(), Some("dev@box"));
        assert_eq!(login_from("hostname box\n"), None);
    }

    #[test]
    fn a_key_file_and_an_askpass_program_allow_one_prompt() -> io::Result<()> {
        let askpass = Askpass {
            program: "/Applications/Muxy.app/Contents/MacOS/muxy-app".into(),
            environment: vec![("MUXY_ASKPASS_ACCOUNT".into(), "box".into())],
        };
        let target = SshTarget::new("dev@box")?
            .with_identity("/Users/dev/.ssh/box key")
            .with_askpass(askpass);
        let arguments = target.arguments(Start::IfNeeded);
        let expected = [
            "-T",
            "-o",
            "BatchMode=no",
            "-o",
            "NumberOfPasswordPrompts=1",
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
            "-i",
            "/Users/dev/.ssh/box key",
            "-o",
            "IdentitiesOnly=yes",
            "--",
            "dev@box",
            bridge::command(Start::IfNeeded),
        ];
        assert_eq!(arguments, expected);
        let environment: Vec<_> = target
            .environment()
            .into_iter()
            .map(|(key, value)| format!("{}={}", key.display(), value.display()))
            .collect();
        assert_eq!(
            environment,
            [
                "SSH_ASKPASS=/Applications/Muxy.app/Contents/MacOS/muxy-app",
                "SSH_ASKPASS_REQUIRE=force",
                "MUXY_ASKPASS_ACCOUNT=box",
            ]
        );
        Ok(())
    }
}
