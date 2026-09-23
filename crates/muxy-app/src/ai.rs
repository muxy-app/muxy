//! Headless AI providers. Callers own their prompts and any resulting actions.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, mpsc};
use std::time::{Duration, Instant};

use muxy_core::worker::WorkerPool;

const OUTPUT_LIMIT: usize = 256 * 1024;
const PROMPT_LIMIT: usize = 256 * 1024;
const TIMEOUT: Duration = Duration::from_secs(300);
/// How long to keep reading after a provider exits while its helpers hold the output open.
const EXIT_GRACE: Duration = Duration::from_millis(250);
static WORKERS: OnceLock<Result<WorkerPool, String>> = OnceLock::new();
static LOGIN_PATH: OnceLock<Vec<PathBuf>> = OnceLock::new();
static RUNNING_GROUPS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Provider {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    executable: &'static str,
    arguments: &'static [&'static str],
    environment: &'static [(&'static str, &'static str)],
}

pub(crate) const PROVIDERS: &[Provider] = &[
    Provider {
        id: "claude",
        name: "Claude Code",
        executable: "claude",
        arguments: &[
            "--print",
            "--output-format",
            "text",
            "--permission-mode",
            "dontAsk",
            "--no-session-persistence",
            "--tools=",
        ],
        environment: &[],
    },
    Provider {
        id: "codex",
        name: "Codex",
        executable: "codex",
        arguments: &[
            "exec",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--color",
            "never",
        ],
        environment: &[],
    },
    Provider {
        id: "opencode",
        name: "OpenCode",
        executable: "opencode",
        arguments: &["run"],
        environment: &[("OPENCODE_PERMISSION", r#"{"*":"deny"}"#)],
    },
    Provider {
        id: "copilot",
        name: "GitHub Copilot",
        executable: "copilot",
        arguments: &["--silent", "--no-ask-user", "--available-tools=", "-p"],
        environment: &[],
    },
    Provider {
        id: "cursor",
        name: "Cursor",
        executable: "cursor-agent",
        arguments: &["--print", "--output-format", "text"],
        environment: &[],
    },
    Provider {
        id: "droid",
        name: "Droid",
        executable: "droid",
        arguments: &["exec", "--output-format", "text"],
        environment: &[],
    },
    Provider {
        id: "grok",
        name: "Grok",
        executable: "grok",
        arguments: &[
            "--no-auto-update",
            "--sandbox",
            "workspace",
            "--permission-mode",
            "dontAsk",
            "--no-subagents",
            "--disable-web-search",
            "--output-format",
            "plain",
            "-p",
        ],
        environment: &[],
    },
    Provider {
        id: "kiro",
        name: "Kiro CLI",
        executable: "kiro-cli",
        arguments: &["chat", "--no-interactive", "--trust-tools="],
        environment: &[],
    },
    Provider {
        id: "pi",
        name: "Pi",
        executable: "pi",
        arguments: &["--print", "--no-session", "--no-tools"],
        environment: &[],
    },
    Provider {
        id: "xal",
        name: "Xal",
        executable: "xal",
        arguments: &["run", "--format", "text"],
        environment: &[],
    },
    Provider {
        id: "antigravity",
        name: "Antigravity",
        executable: "agy",
        arguments: &["--print", "--output-format", "text", "--mode=plan"],
        environment: &[],
    },
];

pub(crate) fn selected(installed: &[Provider], configured: Option<&str>) -> Option<Provider> {
    match configured.filter(|id| !id.is_empty()) {
        Some(id) => installed.iter().copied().find(|provider| provider.id == id),
        None => installed.first().copied(),
    }
}

pub(crate) fn provider(id: &str) -> Option<Provider> {
    PROVIDERS.iter().copied().find(|provider| provider.id == id)
}

/// Stops a provider run from another thread.
#[derive(Clone, Debug, Default)]
pub(crate) struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl Provider {
    fn search_path() -> Vec<PathBuf> {
        let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            for relative in [
                ".local/bin",
                ".npm-global/bin",
                ".opencode/bin",
                ".factory/bin",
                ".cargo/bin",
                ".bun/bin",
            ] {
                directories.push(home.join(relative));
            }
        }
        directories.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
        if let Some(login) = LOGIN_PATH.get() {
            directories.extend(login.iter().cloned());
        }
        directories.retain(|path| path.is_absolute());
        directories
    }

    #[cfg(not(test))]
    pub(crate) fn discover() -> Vec<Self> {
        LOGIN_PATH.get_or_init(|| {
            let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
            let mut command = Command::new(shell);
            command.args([
                "-l",
                "-i",
                "-c",
                "printf '__MUXY_PATH_START__%s__MUXY_PATH_END__' \"$PATH\"",
            ]);
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let Ok((stdout, _)) = capture(
                command,
                Duration::from_secs(5),
                OUTPUT_LIMIT,
                &Cancellation::default(),
            ) else {
                return Vec::new();
            };
            let output = String::from_utf8_lossy(&stdout);
            let Some((_, tail)) = output.rsplit_once("__MUXY_PATH_START__") else {
                return Vec::new();
            };
            let Some((path, _)) = tail.split_once("__MUXY_PATH_END__") else {
                return Vec::new();
            };
            std::env::split_paths(path).collect()
        });
        Self::installed()
    }

    pub(crate) fn installed() -> Vec<Self> {
        let search = Self::search_path();
        PROVIDERS
            .iter()
            .copied()
            .filter(|provider| provider.executable_path_in(&search).is_some())
            .collect()
    }

    pub(crate) fn executable_path(self) -> Option<PathBuf> {
        self.executable_path_in(&Self::search_path())
    }

    fn executable_path_in(self, search: &[PathBuf]) -> Option<PathBuf> {
        search
            .iter()
            .map(|dir| dir.join(self.executable))
            .find(|path| {
                path.metadata().is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
            })
    }

    /// Runs the provider inside the project, where it reads the project's own instructions.
    pub(crate) fn generate(
        self,
        prompt: &str,
        directory: &Path,
        cancellation: &Cancellation,
    ) -> Result<String, String> {
        if prompt.len() > PROMPT_LIMIT {
            return Err("AI prompt exceeds the 256 KB limit".into());
        }
        if !directory.is_dir() {
            return Err("The project directory isn't available on this Mac".into());
        }
        let executable = self
            .executable_path()
            .ok_or_else(|| format!("{} CLI is not installed", self.name))?;
        let mut command = Command::new(executable);
        command
            .args(self.arguments)
            .arg(if prompt.starts_with('-') {
                format!(" {prompt}")
            } else {
                prompt.to_owned()
            });
        command
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("MUXY_PANE_ID", "")
            .env("PWD", directory);
        if let Ok(path) = std::env::join_paths(Self::search_path()) {
            command.env("PATH", path);
        }
        for (key, value) in self.environment {
            command.env(key, value);
        }
        let (stdout, stderr) = capture(command, TIMEOUT, OUTPUT_LIMIT, cancellation)
            .map_err(|failure| failure.describe(self.name))?;
        let output = String::from_utf8(stdout)
            .map_err(|_| format!("{} returned invalid UTF-8", self.name))?;
        if output.trim().is_empty() {
            return Err(format!(
                "{} returned an empty response: {}",
                self.name,
                String::from_utf8_lossy(&stderr).trim()
            ));
        }
        Ok(output)
    }
}

/// Kills every provider that is still running, such as when the app quits.
#[cfg(not(test))]
pub(crate) fn terminate_all() {
    let groups = RUNNING_GROUPS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    for group in groups {
        kill_group(group);
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Failure {
    Cancelled,
    TimedOut(Duration),
    TooLarge,
    Exited(String),
    Io(String),
}

impl Failure {
    fn describe(self, provider: &str) -> String {
        match self {
            Self::Cancelled => "Cancelled".into(),
            Self::TimedOut(_) => format!("{provider} did not respond within five minutes"),
            Self::TooLarge => format!("{provider}'s response exceeded the 256 KB output limit"),
            Self::Exited(detail) => format!("{provider} failed: {detail}"),
            Self::Io(error) => error,
        }
    }
}

enum Output {
    Bytes(usize, Vec<u8>),
    Done,
    Failed(String),
}

/// A provider's process group, killed as a whole so helpers cannot outlive it.
struct Group(u32);

impl Group {
    fn register(id: u32) -> Self {
        RUNNING_GROUPS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(id);
        Self(id)
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        kill_group(self.0);
        RUNNING_GROUPS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|group| *group != self.0);
    }
}

fn kill_group(id: u32) {
    let _ = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{id}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn capture(
    mut command: Command,
    timeout: Duration,
    limit: usize,
    cancellation: &Cancellation,
) -> Result<(Vec<u8>, Vec<u8>), Failure> {
    let mut child = command
        .process_group(0)
        .spawn()
        .map_err(|error| Failure::Io(error.to_string()))?;
    let group = Group::register(child.id());
    let (sender, receiver) = mpsc::sync_channel(16);
    let pipes: [Option<Box<dyn Read + Send>>; 2] = [
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    ];
    for (index, pipe) in pipes.into_iter().enumerate() {
        let Some(mut pipe) = pipe else {
            return Err(Failure::Io("AI output pipe unavailable".into()));
        };
        let sender = sender.clone();
        std::thread::spawn(move || {
            let mut buffer = [0; 8192];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(size) => {
                        if sender
                            .send(Output::Bytes(index, buffer[..size].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Output::Failed(error.to_string()));
                        return;
                    }
                }
            }
            let _ = sender.send(Output::Done);
        });
    }
    drop(sender);
    let mut output = [Vec::new(), Vec::new()];
    let mut done = 0;
    let mut exited = None;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if cancellation.is_cancelled() {
            break Err(Failure::Cancelled);
        }
        if Instant::now() >= deadline {
            break Err(Failure::TimedOut(timeout));
        }
        match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(Output::Bytes(index, bytes)) => {
                if output[index].len() + bytes.len() > limit {
                    break Err(Failure::TooLarge);
                }
                output[index].extend(bytes);
            }
            Ok(Output::Done) => done += 1,
            Ok(Output::Failed(error)) => break Err(Failure::Io(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => done = 2,
        }
        if exited.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => exited = Some((status, Instant::now())),
                Ok(None) => (),
                Err(error) => break Err(Failure::Io(error.to_string())),
            }
        }
        match exited {
            Some((status, at)) if done >= 2 || at.elapsed() >= EXIT_GRACE => break Ok(status),
            _ => (),
        }
    };
    drop(group);
    let _ = child.wait();
    let status = status?;
    if !status.success() {
        let message = String::from_utf8_lossy(&output[1]);
        return Err(Failure::Exited(if message.trim().is_empty() {
            status.to_string()
        } else {
            message.trim().chars().take(1000).collect()
        }));
    }
    let [stdout, stderr] = output;
    Ok((stdout, stderr))
}

pub(crate) fn run<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> impl Future<Output = Result<T, String>> {
    let (reply, result) = async_channel::bounded(1);
    let failed = reply.clone();
    let queued = WORKERS
        .get_or_init(|| WorkerPool::new("ai-provider", 2, 16).map_err(|error| error.to_string()))
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|workers| {
            workers
                .try_spawn(move || {
                    let _ = reply.try_send(operation());
                })
                .map_err(|error| error.to_string())
        });
    if let Err(error) = queued {
        let _ = failed.try_send(Err(error));
    }
    async move {
        result
            .recv()
            .await
            .unwrap_or_else(|_| Err("AI worker stopped".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn alive(pid: &str) -> bool {
        Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[test]
    fn configured_provider_is_preserved_and_auto_selects_installed() {
        let installed = [PROVIDERS[1], PROVIDERS[0]];
        assert_eq!(
            selected(&installed, Some("codex")).map(|p| p.id),
            Some("codex")
        );
        assert_eq!(selected(&installed, None).map(|p| p.id), Some("codex"));
        assert_eq!(selected(&installed, Some("unknown")), None);
    }

    #[test]
    fn providers_use_supported_headless_arguments() {
        let catalog: Vec<_> = PROVIDERS
            .iter()
            .map(|provider| (provider.id, provider.executable, provider.arguments))
            .collect();
        assert_eq!(
            catalog,
            [
                (
                    "claude",
                    "claude",
                    &[
                        "--print",
                        "--output-format",
                        "text",
                        "--permission-mode",
                        "dontAsk",
                        "--no-session-persistence",
                        "--tools=",
                    ][..]
                ),
                (
                    "codex",
                    "codex",
                    &[
                        "exec",
                        "--ephemeral",
                        "--sandbox",
                        "read-only",
                        "--color",
                        "never"
                    ][..]
                ),
                ("opencode", "opencode", &["run"][..]),
                (
                    "copilot",
                    "copilot",
                    &["--silent", "--no-ask-user", "--available-tools=", "-p"][..]
                ),
                (
                    "cursor",
                    "cursor-agent",
                    &["--print", "--output-format", "text"][..]
                ),
                ("droid", "droid", &["exec", "--output-format", "text"][..]),
                (
                    "grok",
                    "grok",
                    &[
                        "--no-auto-update",
                        "--sandbox",
                        "workspace",
                        "--permission-mode",
                        "dontAsk",
                        "--no-subagents",
                        "--disable-web-search",
                        "--output-format",
                        "plain",
                        "-p",
                    ][..]
                ),
                (
                    "kiro",
                    "kiro-cli",
                    &["chat", "--no-interactive", "--trust-tools="][..]
                ),
                ("pi", "pi", &["--print", "--no-session", "--no-tools"][..]),
                ("xal", "xal", &["run", "--format", "text"][..]),
                (
                    "antigravity",
                    "agy",
                    &["--print", "--output-format", "text", "--mode=plan"][..]
                ),
            ]
        );
    }

    #[test]
    fn timeout_kills_helpers_that_keep_running() {
        let directory = tempfile::tempdir().unwrap();
        let pid = directory.path().join("helper");
        let result = capture(
            shell(&format!("sleep 30 & echo $! > '{}'; wait", pid.display())),
            Duration::from_millis(300),
            OUTPUT_LIMIT,
            &Cancellation::default(),
        );
        assert_eq!(result, Err(Failure::TimedOut(Duration::from_millis(300))));
        assert!(!alive(&std::fs::read_to_string(pid).unwrap()));
    }

    #[test]
    fn finished_provider_is_not_held_open_by_its_helpers() {
        let started = Instant::now();
        let (stdout, _) = capture(
            shell("sleep 30 & echo answer"),
            Duration::from_secs(20),
            OUTPUT_LIMIT,
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(stdout, b"answer\n");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn cancellation_stops_a_running_provider() {
        let cancellation = Cancellation::default();
        let cancel = cancellation.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel.cancel();
        });
        let started = Instant::now();
        let result = capture(
            shell("sleep 30"),
            Duration::from_secs(20),
            OUTPUT_LIMIT,
            &cancellation,
        );
        assert_eq!(result, Err(Failure::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn provider_failures_describe_the_provider() {
        let result = capture(
            shell("echo 'Not inside a trusted directory' >&2; exit 1"),
            Duration::from_secs(20),
            OUTPUT_LIMIT,
            &Cancellation::default(),
        );
        assert_eq!(
            result.unwrap_err().describe("Codex"),
            "Codex failed: Not inside a trusted directory"
        );
    }
}
