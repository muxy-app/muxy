//! Servers on other computers, reached through ssh or a custom command that
//! runs the bridge there.

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
/// `ssh://user@host:port`, or a custom command prefix.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SshTarget {
    destination: String,
    program: Option<PathBuf>,
    custom_arguments: Option<Vec<String>>,
    identity: Option<PathBuf>,
    askpass: Option<Askpass>,
    proxy: Option<ProxyCommand>,
    host_key_prompt: bool,
}

/// A program that connects ssh to the computer, such as through a proxy. ssh
/// runs it with the host and port after `arguments` and talks to it over its
/// standard input and output.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProxyCommand {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    /// More environment for the program, such as which proxy to use.
    pub environment: Vec<(OsString, OsString)>,
}

impl ProxyCommand {
    /// ssh expands `%` tokens, then runs the line with the user's shell.
    fn option(&self) -> String {
        let quote = |value: &str| format!("'{}'", value.replace('\'', r"'\''").replace('%', "%%"));
        let mut line = quote(&self.program.to_string_lossy());
        for argument in &self.arguments {
            line.push(' ');
            line.push_str(&quote(argument));
        }
        format!("ProxyCommand={line} %h %p")
    }
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
            custom_arguments: None,
            identity: None,
            askpass: None,
            proxy: None,
            host_key_prompt: false,
        })
    }

    pub fn custom_command(source: &str) -> io::Result<Self> {
        let mut arguments = muxy_core::executable::command_arguments(source)?.into_iter();
        let program = arguments
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Enter a command"))?;
        Ok(Self {
            destination: source.trim().into(),
            program: Some(program.into()),
            custom_arguments: Some(arguments.collect()),
            identity: None,
            askpass: None,
            proxy: None,
            host_key_prompt: false,
        })
    }

    pub fn is_custom_command(&self) -> bool {
        self.custom_arguments.is_some()
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
    pub fn with_proxy(mut self, proxy: ProxyCommand) -> Self {
        self.proxy = Some(proxy);
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

    /// Whether ssh's config already reaches this destination its own way,
    /// with `ProxyJump` or `ProxyCommand`.
    pub fn has_own_route(&self) -> io::Result<bool> {
        Ok(route_from(&self.config()?))
    }

    pub fn default_identity(&self) -> io::Result<Option<PathBuf>> {
        let config = self.config()?;
        Ok(std::env::home_dir().and_then(|home| identity_from(&config, &home)))
    }

    fn config(&self) -> io::Result<String> {
        if self.is_custom_command() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Custom commands manage their own SSH settings and authentication",
            ));
        }
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
        let proxy = self
            .proxy
            .as_ref()
            .filter(|_| !self.has_own_route().unwrap_or(false));
        let mut command = Command::new(program);
        command
            .args(self.arguments_for(remote, proxy))
            .envs(self.environment(proxy));
        Ok(command)
    }

    /// Prompts only reach an askpass program, since nobody may be there to
    /// answer them.
    fn arguments_for(&self, remote: &str, proxy: Option<&ProxyCommand>) -> Vec<OsString> {
        if let Some(arguments) = &self.custom_arguments {
            return arguments
                .iter()
                .map(OsString::from)
                .chain([remote.into()])
                .collect();
        }
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
        if let Some(proxy) = proxy {
            arguments.extend([
                "-o".into(),
                "ControlPath=none".into(),
                "-o".into(),
                proxy.option().into(),
            ]);
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
    fn environment(&self, proxy: Option<&ProxyCommand>) -> Vec<(OsString, OsString)> {
        if self.is_custom_command() {
            return Vec::new();
        }
        let proxy = proxy
            .into_iter()
            .flat_map(|proxy| proxy.environment.iter().cloned());
        let Some(askpass) = &self.askpass else {
            return proxy.collect();
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
        .chain(proxy)
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
fn route_from(config: &str) -> bool {
    config.lines().any(|line| {
        line.split_once(' ').is_some_and(|(key, value)| {
            matches!(key, "proxyjump" | "proxycommand") && value.trim() != "none"
        })
    })
}

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
        let mut command = target.command(bridge::command(start))?;
        let program = PathBuf::from(command.get_program());
        command.stderr(Stdio::piped());
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
    fn custom_commands_do_not_inherit_ssh_arguments_or_credentials() -> io::Result<()> {
        let target = SshTarget::custom_command("vm ssh 'dev box' --")?
            .with_identity("/private/key")
            .with_host_key_prompt()
            .with_askpass(Askpass {
                program: "/private/askpass".into(),
                environment: vec![("TOKEN".into(), "secret".into())],
            });
        let remote = bridge::command(Start::Never);
        let command = target.command(remote)?;
        assert_eq!(command.get_program(), "vm");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["ssh", "dev box", "--", remote]
        );
        assert_eq!(command.get_envs().count(), 0);
        assert_eq!(
            target.login().err().map(|error| error.kind()),
            Some(io::ErrorKind::Unsupported)
        );
        assert_eq!(
            target.default_identity().err().map(|error| error.kind()),
            Some(io::ErrorKind::Unsupported)
        );
        Ok(())
    }

    fn proxy() -> ProxyCommand {
        ProxyCommand {
            program: "/usr/bin/printf".into(),
            arguments: vec!["%s|".into(), "it's".into()],
            environment: vec![("MUXY_PROXY".into(), "secret".into())],
        }
    }

    #[test]
    fn a_proxy_reaches_ssh_as_an_option_and_its_environment() -> io::Result<()> {
        let target = SshTarget::new("box")?
            .with_program("/usr/bin/true")
            .with_proxy(proxy());
        let command = target.command("true")?;
        let arguments: Vec<_> = command.get_args().collect();
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-o", "ControlPath=none"])
        );
        assert!(arguments.windows(2).any(|pair| pair
            == [
                "-o",
                r"ProxyCommand='/usr/bin/printf' '%%s|' 'it'\''s' %h %p"
            ]));
        assert_eq!(
            command.get_envs().collect::<Vec<_>>(),
            [("MUXY_PROXY".as_ref(), Some("secret".as_ref()))]
        );
        let custom = SshTarget::custom_command("vm ssh box")?.with_proxy(proxy());
        let command = custom.command("true")?;
        assert!(command.get_args().all(|argument| argument != "-o"));
        assert_eq!(command.get_envs().count(), 0);
        Ok(())
    }

    #[test]
    fn jump_hosts_and_proxy_commands_in_ssh_config_are_their_own_route() {
        assert!(route_from("user me\nproxyjump bastion.example\nport 22\n"));
        assert!(route_from("proxycommand nc -X 5 %h %p\n"));
        assert!(!route_from("user me\nproxycommand none\nport 22\n"));
        assert!(!route_from("user me\nhostname box\n"));
    }

    #[test]
    fn a_missing_custom_program_is_not_a_missing_remote_installation() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let source = format!(
            "'{}' ssh box",
            directory.path().join("missing-vm").display()
        );
        let target = SshTarget::custom_command(&source)?;
        assert!(
            matches!(Client::connect_ssh(&target, Start::IfNeeded), Err(ClientError::Remote {
            reason: RemoteReason::SshMissing,
            destination,
            detail,
        }) if destination == source && detail.ends_with("missing-vm"))
        );
        Ok(())
    }

    #[test]
    fn host_key_prompts_keep_verification_and_disable_shared_connections() -> io::Result<()> {
        let target = SshTarget::new("ssh://dev@box:2222")?.with_host_key_prompt();
        let args = target.arguments_for(bridge::command(Start::IfNeeded), None);
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
}
