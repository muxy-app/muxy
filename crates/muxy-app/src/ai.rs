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
    interactive: &'static [&'static str],
    initial_prompt: InitialPrompt,
    model_flag: Option<&'static str>,
    prompt_tail: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InitialPrompt {
    Argument,
    Flag(&'static str),
    Typed,
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
        interactive: &[],
        initial_prompt: InitialPrompt::Argument,
        model_flag: Some("--model"),
        prompt_tail: 0,
    },
    Provider {
        id: "codex",
        name: "Codex",
        executable: "codex",
        arguments: &[
            "exec",
            "--ephemeral",
            "--skip-git-repo-check",
            "--sandbox",
            "read-only",
            "--color",
            "never",
        ],
        environment: &[],
        interactive: &[],
        initial_prompt: InitialPrompt::Argument,
        model_flag: Some("--model"),
        prompt_tail: 0,
    },
    Provider {
        id: "opencode",
        name: "OpenCode",
        executable: "opencode",
        arguments: &["run"],
        environment: &[("OPENCODE_PERMISSION", r#"{"*":"deny"}"#)],
        interactive: &[],
        initial_prompt: InitialPrompt::Flag("--prompt"),
        model_flag: Some("--model"),
        prompt_tail: 0,
    },
    Provider {
        id: "copilot",
        name: "GitHub Copilot",
        executable: "copilot",
        arguments: &["--silent", "--no-ask-user", "--available-tools=", "-p"],
        environment: &[],
        interactive: &[],
        initial_prompt: InitialPrompt::Flag("-i"),
        model_flag: Some("--model"),
        prompt_tail: 1,
    },
    Provider {
        id: "cursor",
        name: "Cursor",
        executable: "cursor-agent",
        arguments: &["--print", "--output-format", "text"],
        environment: &[],
        interactive: &[],
        initial_prompt: InitialPrompt::Argument,
        model_flag: Some("--model"),
        prompt_tail: 0,
    },
    Provider {
        id: "droid",
        name: "Droid",
        executable: "droid",
        arguments: &["exec", "--output-format", "text"],
        environment: &[],
        interactive: &[],
        initial_prompt: InitialPrompt::Argument,
        model_flag: Some("--model"),
        prompt_tail: 0,
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
        interactive: &[],
        initial_prompt: InitialPrompt::Typed,
        model_flag: Some("--model"),
        prompt_tail: 1,
    },
    Provider {
        id: "kiro",
        name: "Kiro CLI",
        executable: "kiro-cli",
        arguments: &["chat", "--no-interactive", "--trust-tools="],
        environment: &[],
        interactive: &["chat"],
        initial_prompt: InitialPrompt::Typed,
        model_flag: Some("--model"),
        prompt_tail: 0,
    },
    Provider {
        id: "pi",
        name: "Pi",
        executable: "pi",
        arguments: &["--print", "--no-session", "--no-tools"],
        environment: &[],
        interactive: &[],
        initial_prompt: InitialPrompt::Typed,
        model_flag: Some("--model"),
        prompt_tail: 0,
    },
    Provider {
        id: "xal",
        name: "Xal",
        executable: "xal",
        arguments: &["run", "--format", "text"],
        environment: &[],
        interactive: &[],
        initial_prompt: InitialPrompt::Typed,
        model_flag: None,
        prompt_tail: 0,
    },
    Provider {
        id: "antigravity",
        name: "Antigravity",
        executable: "agy",
        arguments: &["--print", "--output-format", "text", "--mode=plan"],
        environment: &[],
        interactive: &[],
        initial_prompt: InitialPrompt::Typed,
        model_flag: Some("--model"),
        prompt_tail: 0,
    },
    Provider {
        id: "omp",
        name: "Oh My Pi",
        executable: "omp",
        arguments: &["--print", "--no-session", "--no-tools"],
        environment: &[],
        interactive: &[],
        initial_prompt: InitialPrompt::Typed,
        model_flag: Some("--model"),
        prompt_tail: 0,
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
        if let Ok(home) = muxy_core::paths::home_dir() {
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
        #[cfg(unix)]
        directories.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
        #[cfg(windows)]
        directories.extend(std::env::var_os("APPDATA").map(|data| PathBuf::from(data).join("npm")));
        if let Some(login) = LOGIN_PATH.get() {
            directories.extend(login.iter().cloned());
        }
        directories.retain(|path| path.is_absolute());
        directories
    }

    #[cfg(all(windows, not(test)))]
    pub(crate) fn discover() -> Vec<Self> {
        Self::installed()
    }

    #[cfg(all(unix, not(test)))]
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
        executable_in(self.executable, search)
    }

    pub(crate) fn interactive(self) -> (&'static [&'static str], InitialPrompt) {
        (self.interactive, self.initial_prompt)
    }

    pub(crate) fn generate(
        self,
        prompt: &str,
        directory: &Path,
        cancellation: &Cancellation,
    ) -> Result<String, String> {
        self.generate_with_model(prompt, None, directory, cancellation)
    }

    pub(crate) fn generate_with_model(
        self,
        prompt: &str,
        model: Option<&str>,
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
        let arguments = self.arguments_for(prompt, model)?;
        #[cfg(windows)]
        if let Some(reason) = command_line_problem(&executable, prompt, self.name) {
            return Err(reason);
        }
        let mut command = Command::new(executable);
        command.args(arguments);
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
            let detail = String::from_utf8_lossy(&stderr);
            let detail = detail.trim();
            return Err(if detail.is_empty() {
                tr!("%@ returned an empty response", self.name).to_string()
            } else {
                tr!("%@ returned an empty response: %@", self.name, detail).to_string()
            });
        }
        Ok(output)
    }

    pub(crate) fn arguments_for(
        self,
        prompt: &str,
        model: Option<&str>,
    ) -> Result<Vec<String>, String> {
        let mut arguments: Vec<String> = self
            .arguments
            .iter()
            .map(|word| (*word).to_owned())
            .collect();
        let model = model.unwrap_or("").trim();
        if !model.is_empty() {
            if model.starts_with('-')
                || model.len() > 256
                || model
                    .chars()
                    .any(|character| character.is_control() || character.is_whitespace())
            {
                return Err(tr!(
                    "Invalid model \"%@\". Model names cannot start with \"-\" or contain spaces.",
                    model
                )
                .to_string());
            }
            let flag = self.model_flag.ok_or_else(|| {
                tr!(
                    "%@ has no confirmed model flag. Clear the model to use its default.",
                    self.name
                )
                .to_string()
            })?;
            let at = arguments.len() - self.prompt_tail;
            arguments.insert(at, flag.to_owned());
            arguments.insert(at + 1, model.to_owned());
        }
        arguments.push(if prompt.starts_with('-') {
            format!(" {prompt}")
        } else {
            prompt.to_owned()
        });
        Ok(arguments)
    }
}

pub(crate) fn prompt_provider(
    installed: &[Provider],
    configured: &str,
) -> Result<Provider, String> {
    let configured = configured.trim();
    if configured.is_empty() {
        return installed
            .first()
            .copied()
            .ok_or_else(|| tr!("No agent CLI is installed").to_string());
    }
    let provider =
        provider(configured).ok_or_else(|| tr!("Unknown CLI \"%@\".", configured).to_string())?;
    if installed.iter().any(|item| item.id == provider.id) {
        Ok(provider)
    } else {
        Err(tr!("%@ CLI is not installed", provider.name).to_string())
    }
}

#[cfg(windows)]
fn command_line_problem(executable: &Path, prompt: &str, name: &str) -> Option<String> {
    let batch = executable.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
    });
    let limit = if batch { 7_500 } else { 32_000 };
    if batch && prompt.contains(['\r', '\n']) {
        return Some(
            tr!(
                "%@ is installed with a command launcher, which Windows can't hand a prompt of several lines to.",
                name
            )
            .to_string(),
        );
    }
    (prompt.encode_utf16().count() > limit)
        .then(|| tr!("The prompt is too long for Windows to hand to %@.", name).to_string())
}

fn executable_in(name: &str, search: &[PathBuf]) -> Option<PathBuf> {
    muxy_core::executable::find_program(search, name)
}

pub(crate) fn find_program(name: &str) -> Option<PathBuf> {
    executable_in(name, &Provider::search_path())
}

pub(crate) fn find_in_login_shell(name: &str) -> Option<PathBuf> {
    executable_in(name, LOGIN_PATH.get()?)
}

pub(crate) fn run_tool(
    program: &str,
    arguments: &[&str],
    directory: Option<&Path>,
) -> Result<String, String> {
    let executable =
        find_program(program).ok_or_else(|| tr!("%@ is not installed", program).to_string())?;
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1");
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    if let Ok(path) = std::env::join_paths(Provider::search_path()) {
        command.env("PATH", path);
    }
    let (stdout, _) = capture(command, TIMEOUT, OUTPUT_LIMIT, &Cancellation::default())
        .map_err(|failure| failure.describe(program))?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_owned())
}

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

fn capture(
    mut command: Command,
    timeout: Duration,
    limit: usize,
    cancellation: &Cancellation,
) -> Result<(Vec<u8>, Vec<u8>), Failure> {
    if cancellation.is_cancelled() {
        return Err(Failure::Cancelled);
    }
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

#[cfg(test)]
mod tests {
    use super::{PROVIDERS, prompt_provider, provider};

    #[test]
    fn one_shot_arguments_keep_each_providers_flags_and_then_the_prompt() {
        for item in PROVIDERS {
            let prompt = "fix the login bug";
            let arguments = item.arguments_for(prompt, None).expect("arguments");
            let mut expected: Vec<String> = item
                .arguments
                .iter()
                .map(|word| (*word).to_owned())
                .collect();
            expected.push(prompt.to_owned());
            assert_eq!(arguments, expected);
        }
    }

    #[test]
    fn a_model_is_inserted_before_a_prompt_flag() {
        let copilot = provider("copilot").expect("copilot");
        let arguments = copilot
            .arguments_for("hello", Some("gpt-5.4"))
            .expect("arguments");
        assert_eq!(words(&arguments, 4), ["--model", "gpt-5.4", "-p", "hello"]);
        let grok = provider("grok").expect("grok");
        let arguments = grok
            .arguments_for("hello", Some("grok-3"))
            .expect("arguments");
        assert_eq!(words(&arguments, 4), ["--model", "grok-3", "-p", "hello"]);
        let codex = provider("codex").expect("codex");
        let arguments = codex
            .arguments_for("hello", Some("gpt-5.4"))
            .expect("arguments");
        let exec = arguments
            .iter()
            .position(|word| word == "exec")
            .expect("exec");
        let model = arguments
            .iter()
            .position(|word| word == "--model")
            .expect("model");
        assert!(arguments.iter().any(|word| word == "--skip-git-repo-check"));
        assert!(exec < model);
        assert_eq!(words(&arguments, 3), ["--model", "gpt-5.4", "hello"]);
    }

    fn words(arguments: &[String], count: usize) -> Vec<&str> {
        arguments[arguments.len() - count..]
            .iter()
            .map(String::as_str)
            .collect()
    }

    #[test]
    fn a_prompt_that_starts_with_a_dash_is_passed_as_text() {
        let claude = provider("claude").expect("claude");
        let arguments = claude.arguments_for("-n", None).expect("arguments");
        assert_eq!(arguments.last().map(String::as_str), Some(" -n"));
    }

    #[test]
    fn invalid_model_names_are_rejected_before_launching_a_cli() {
        let codex = provider("codex").expect("codex");
        assert!(codex.arguments_for("hello", Some("gpt-6-luna")).is_ok());
        assert!(codex.arguments_for("hello", Some("openai/gpt-5.4")).is_ok());
        assert!(
            codex
                .arguments_for("hello", Some("--dangerously-bypass-approvals-and-sandbox"))
                .is_err()
        );
        assert!(codex.arguments_for("hello", Some("-s")).is_err());
        assert!(codex.arguments_for("hello", Some("gpt 6")).is_err());
        assert!(codex.arguments_for("hello", Some("gpt-6\n--flag")).is_err());
        assert!(codex.arguments_for("hello", Some("gpt-6\0")).is_err());
        assert!(
            codex
                .arguments_for("hello", Some(&"a".repeat(257)))
                .is_err()
        );
    }

    #[test]
    fn xal_keeps_its_default_model_until_one_is_requested() {
        let xal = provider("xal").expect("xal");
        assert!(xal.arguments_for("hello", Some("   ")).is_ok());
        let error = xal.arguments_for("hello", Some("big")).expect_err("model");
        assert!(error.contains("no confirmed model flag"));
    }

    #[test]
    fn automatic_uses_the_first_installed_cli_and_an_explicit_missing_cli_does_not_fall_through() {
        let installed = [
            provider("omp").expect("omp"),
            provider("codex").expect("codex"),
        ];
        assert_eq!(prompt_provider(&installed, "").expect("auto").id, "omp");
        let installed = [
            provider("codex").expect("codex"),
            provider("omp").expect("omp"),
        ];
        assert_eq!(prompt_provider(&installed, "  ").expect("auto").id, "codex");
        assert_eq!(
            prompt_provider(&installed, "omp").expect("chosen").id,
            "omp"
        );
        assert!(prompt_provider(&installed, "claude").is_err());
        assert!(prompt_provider(&installed, "nope").is_err());
        assert!(prompt_provider(&[], "").is_err());
    }
}

#[cfg(test)]
#[cfg(windows)]
mod windows_tests {
    use super::*;

    #[test]
    fn prompts_windows_cannot_pass_are_refused_with_a_reason() {
        let shim = Path::new(r"C:\npm\codex.CMD");
        let program = Path::new(r"C:\bin\codex.exe");
        assert!(command_line_problem(shim, "one line", "Codex").is_none());
        assert!(command_line_problem(shim, "two\nlines", "Codex").is_some());
        assert!(command_line_problem(program, "two\nlines", "Codex").is_none());
        assert!(command_line_problem(program, &"x".repeat(40_000), "Codex").is_some());
    }
}
