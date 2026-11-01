//! Headless AI providers. Callers own their prompts and any resulting actions.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use muxy_core::process::{Group, Limits, Running};
use muxy_core::worker::WorkerPool;
use muxy_ui::tr;

const OUTPUT_LIMIT: usize = 256 * 1024;
const PROMPT_LIMIT: usize = 256 * 1024;
const TIMEOUT: Duration = Duration::from_secs(300);
static WORKERS: OnceLock<Result<WorkerPool, String>> = OnceLock::new();
static LOGIN_PATH: OnceLock<Vec<PathBuf>> = OnceLock::new();
static RUNNING_GROUPS: Mutex<Vec<Group>> = Mutex::new(Vec::new());

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
    Provider {
        id: "omp",
        name: "Oh My Pi",
        executable: "omp",
        arguments: &["--print", "--no-session", "--no-tools"],
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
            return Err(tr!("AI prompt exceeds the 256 KB limit").to_string());
        }
        if !directory.is_dir() {
            return Err(tr!("The project directory isn't available on this Mac").to_string());
        }
        let executable = self
            .executable_path()
            .ok_or_else(|| tr!("%@ CLI is not installed", self.name).to_string())?;
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
            .map_err(|_| tr!("%@ returned invalid UTF-8", self.name).to_string())?;
        if output.trim().is_empty() {
            return Err(tr!(
                "%@ returned an empty response: %@",
                self.name,
                String::from_utf8_lossy(&stderr).trim()
            )
            .to_string());
        }
        Ok(output)
    }
}

/// Kills every provider that is still running, such as when the app quits.
#[cfg(not(test))]
pub(crate) fn terminate_all() {
    for group in RUNNING_GROUPS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
    {
        group.kill();
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
            Self::Cancelled => tr!("Cancelled").to_string(),
            Self::TimedOut(_) => {
                tr!("%@ did not respond within five minutes", provider).to_string()
            }
            Self::TooLarge => {
                tr!("%@'s response exceeded the 256 KB output limit", provider).to_string()
            }
            Self::Exited(detail) => tr!("%@ failed: %@", provider, detail).to_string(),
            Self::Io(error) => error,
        }
    }
}

/// A running provider's process group, listed so quitting the app can kill it.
struct Registered(Group);

impl Registered {
    fn new(group: Group) -> Self {
        RUNNING_GROUPS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(group.clone());
        Self(group)
    }
}

impl Drop for Registered {
    fn drop(&mut self) {
        RUNNING_GROUPS
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|group| *group != self.0);
    }
}

/// Runs a provider and kills any helpers it leaves behind.
fn capture(
    mut command: Command,
    timeout: Duration,
    limit: usize,
    cancellation: &Cancellation,
) -> Result<(Vec<u8>, Vec<u8>), Failure> {
    let cancelled = || cancellation.is_cancelled();
    let limits = Limits {
        cancelled: Some(&cancelled),
        kill_leftovers: true,
        ..Limits::new(timeout, limit)
    };
    let running =
        Running::start(&mut command, limits).map_err(|error| Failure::Io(error.to_string()))?;
    let _registered = Registered::new(running.group());
    let output = running
        .wait()
        .map_err(|error| Failure::Io(error.to_string()))?;
    if output.cancelled {
        return Err(Failure::Cancelled);
    }
    if output.timed_out {
        return Err(Failure::TimedOut(timeout));
    }
    if output.stdout.truncated || output.stderr.truncated {
        return Err(Failure::TooLarge);
    }
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr.bytes);
        return Err(Failure::Exited(if message.trim().is_empty() {
            output.status.to_string()
        } else {
            message.trim().chars().take(1000).collect()
        }));
    }
    Ok((output.stdout.bytes, output.stderr.bytes))
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
            .unwrap_or_else(|_| Err(tr!("AI worker stopped").to_string()))
    }
}
