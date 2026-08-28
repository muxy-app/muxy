use crate::subprocess::{
    CancellationSignal, Deadline, EnvironmentMode, StdinMode, SubprocessError, SubprocessRequest,
    run,
};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

pub const FALLBACK_EXECUTABLE_PATHS: [&str; 6] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

const SHELL_OUTPUT_LIMIT: usize = 262_144;
const SHELL_TIMEOUT: Duration = Duration::from_secs(3);
const PATH_START: &str = "__MUXY_PATH_START__";
const PATH_END: &str = "__MUXY_PATH_END__";
const COPILOT_HOME_START: &str = "__MUXY_COPILOT_HOME_START__";
const COPILOT_HOME_END: &str = "__MUXY_COPILOT_HOME_END__";
const LOGIN_SHELL_COMMAND: &str = "printf '__MUXY_PATH_START__'; /usr/bin/printenv PATH; printf '__MUXY_PATH_END__'; printf '__MUXY_COPILOT_HOME_START__'; /usr/bin/printenv COPILOT_HOME; printf '__MUXY_COPILOT_HOME_END__'";

const GIT_REDIRECTION_VARIABLES: [&str; 19] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_QUARANTINE_PATH",
    "GIT_SHALLOW_FILE",
    "GIT_REPLACE_REF_BASE",
    "GIT_GRAFT_FILE",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_CONFIG_COUNT",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_EXEC_PATH",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionEnvironment {
    variables: Arc<BTreeMap<OsString, OsString>>,
}

impl ExecutionEnvironment {
    pub fn fallback<I>(inherited: I) -> Self
    where
        I: IntoIterator<Item = (OsString, OsString)>,
    {
        let mut variables: BTreeMap<OsString, OsString> = inherited.into_iter().collect();
        let current_path = variables
            .get(OsStr::new("PATH"))
            .cloned()
            .unwrap_or_default();
        let mut paths: Vec<PathBuf> = std::env::split_paths(&current_path)
            .filter(|path| !path.as_os_str().is_empty())
            .collect();
        for fallback in FALLBACK_EXECUTABLE_PATHS {
            let fallback = PathBuf::from(fallback);
            if !paths.contains(&fallback) {
                paths.push(fallback);
            }
        }
        let path = std::env::join_paths(&paths).unwrap_or(current_path);
        variables.insert(OsString::from("PATH"), path);
        Self {
            variables: Arc::new(variables),
        }
    }

    pub fn from_current_process() -> Self {
        Self::fallback(std::env::vars_os())
    }

    #[cfg(test)]
    pub(crate) fn exact<I>(variables: I) -> Self
    where
        I: IntoIterator<Item = (OsString, OsString)>,
    {
        Self {
            variables: Arc::new(variables.into_iter().collect()),
        }
    }

    pub fn get<'a>(&'a self, key: &OsStr) -> Option<&'a OsStr> {
        self.variables.get(key).map(OsString::as_os_str)
    }

    pub fn variables(&self) -> Vec<(OsString, OsString)> {
        self.variables
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    pub fn git_variables(&self) -> Vec<(OsString, OsString)> {
        self.filtered_variables(is_git_redirection_variable)
    }

    pub fn github_variables(&self) -> Vec<(OsString, OsString)> {
        self.filtered_variables(|key| key == OsStr::new("GH_REPO"))
    }

    pub fn provider_variables(&self) -> Vec<(OsString, OsString)> {
        self.filtered_variables(|key| key == OsStr::new("MUXY_PANE_ID"))
    }

    pub fn resolve_executable(&self, name: &OsStr) -> Option<PathBuf> {
        if name.is_empty() {
            return None;
        }
        let requested = Path::new(name);
        if requested.is_absolute() || requested.components().count() > 1 {
            return executable_path(requested);
        }
        let path = self.get(OsStr::new("PATH"))?;
        std::env::split_paths(path)
            .map(|directory| directory.join(requested))
            .find_map(|candidate| executable_path(&candidate))
    }

    fn filtered_variables(&self, remove: impl Fn(&OsStr) -> bool) -> Vec<(OsString, OsString)> {
        self.variables
            .iter()
            .filter(|(key, _)| !remove(key.as_os_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    fn hydrated(&self, values: HydratedEnvironment) -> Self {
        let mut variables = self.variables.as_ref().clone();
        variables.insert(OsString::from("PATH"), values.path);
        match values.copilot_home {
            Some(value) => {
                variables.insert(OsString::from("COPILOT_HOME"), value);
            }
            None => {
                variables.remove(OsStr::new("COPILOT_HOME"));
            }
        }
        Self {
            variables: Arc::new(variables),
        }
    }
}

fn is_git_redirection_variable(key: &OsStr) -> bool {
    GIT_REDIRECTION_VARIABLES
        .iter()
        .any(|candidate| key == OsStr::new(candidate))
        || key.to_str().is_some_and(|key| {
            key.starts_with("GIT_CONFIG_KEY_") || key.starts_with("GIT_CONFIG_VALUE_")
        })
}

fn executable_path(path: &Path) -> Option<PathBuf> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    std::fs::canonicalize(path).ok()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HydratedEnvironment {
    path: OsString,
    copilot_home: Option<OsString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HydrationFailure {
    MissingShell,
    Spawn,
    Process,
    NonzeroExit,
    TimedOut,
    Cancelled,
    TruncatedOutput,
    MalformedOutput,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HydrationOutcome {
    Upgraded { revision: u64 },
    KeptFallback(HydrationFailure),
}

#[derive(Clone)]
pub struct ExecutionEnvironmentSource {
    shared: Arc<EnvironmentSourceShared>,
}

struct EnvironmentSourceShared {
    state: Mutex<EnvironmentSourceState>,
    cancellation: CancellationSignal,
}

struct EnvironmentSourceState {
    environment: ExecutionEnvironment,
    revision: u64,
    started: bool,
    closed: bool,
    completions: u64,
    outcome: Option<HydrationOutcome>,
    thread: Option<JoinHandle<()>>,
}

impl ExecutionEnvironmentSource {
    pub fn new(fallback: ExecutionEnvironment) -> Self {
        Self {
            shared: Arc::new(EnvironmentSourceShared {
                state: Mutex::new(EnvironmentSourceState {
                    environment: fallback,
                    revision: 0,
                    started: false,
                    closed: false,
                    completions: 0,
                    outcome: None,
                    thread: None,
                }),
                cancellation: CancellationSignal::new(),
            }),
        }
    }

    pub fn from_current_process() -> Self {
        Self::new(ExecutionEnvironment::from_current_process())
    }

    pub fn snapshot(&self) -> ExecutionEnvironment {
        lock(&self.shared.state).environment.clone()
    }

    pub fn revision(&self) -> u64 {
        lock(&self.shared.state).revision
    }

    pub fn start_hydration(&self) -> Option<async_channel::Receiver<HydrationOutcome>> {
        #[cfg(unix)]
        {
            let environment = self.snapshot();
            let shell = select_login_shell(
                environment.get(OsStr::new("SHELL")),
                account_login_shell().as_deref(),
            );
            self.start(shell, login_shell_arguments(), SHELL_TIMEOUT)
        }
        #[cfg(not(unix))]
        {
            self.complete_without_process(HydrationFailure::Unsupported)
        }
    }

    pub fn cancel_hydration(&self) {
        self.shared.cancellation.cancel();
    }

    pub fn close(&self) {
        self.cancel_hydration();
        let thread = {
            let mut state = lock(&self.shared.state);
            state.closed = true;
            state.thread.take()
        };
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }

    #[cfg(test)]
    fn start_hydration_command(
        &self,
        executable: PathBuf,
        args: Vec<OsString>,
        timeout: Duration,
    ) -> Option<async_channel::Receiver<HydrationOutcome>> {
        self.start(executable, args, timeout)
    }

    #[cfg(test)]
    fn completion_count(&self) -> u64 {
        lock(&self.shared.state).completions
    }

    fn start(
        &self,
        executable: PathBuf,
        args: Vec<OsString>,
        timeout: Duration,
    ) -> Option<async_channel::Receiver<HydrationOutcome>> {
        let (sender, receiver) = async_channel::bounded(1);
        let mut state = lock(&self.shared.state);
        if state.started || state.closed {
            return None;
        }
        state.started = true;
        let fallback = state.environment.clone();
        let source = self.clone();
        let cancellation = self.shared.cancellation.clone();
        state.thread = Some(std::thread::spawn(move || {
            let result = hydrate_once(&fallback, &executable, args, timeout, &cancellation);
            let outcome = source.complete(result);
            let _ = sender.send_blocking(outcome);
        }));
        drop(state);
        Some(receiver)
    }

    fn complete(&self, result: Result<ExecutionEnvironment, HydrationFailure>) -> HydrationOutcome {
        let mut state = lock(&self.shared.state);
        if let Some(outcome) = &state.outcome {
            return outcome.clone();
        }
        state.completions = state.completions.saturating_add(1);
        let outcome = match result {
            Ok(environment) => {
                state.environment = environment;
                state.revision = state.revision.saturating_add(1);
                HydrationOutcome::Upgraded {
                    revision: state.revision,
                }
            }
            Err(error) => HydrationOutcome::KeptFallback(error),
        };
        state.outcome = Some(outcome.clone());
        outcome
    }

    #[cfg(not(unix))]
    fn complete_without_process(
        &self,
        failure: HydrationFailure,
    ) -> Option<async_channel::Receiver<HydrationOutcome>> {
        let (sender, receiver) = async_channel::bounded(1);
        let mut state = lock(&self.shared.state);
        if state.started || state.closed {
            return None;
        }
        state.started = true;
        state.completions = state.completions.saturating_add(1);
        state.outcome = Some(HydrationOutcome::KeptFallback(failure));
        drop(state);
        let _ = sender.send_blocking(HydrationOutcome::KeptFallback(failure));
        Some(receiver)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn login_shell_arguments() -> Vec<OsString> {
    vec![
        OsString::from("-l"),
        OsString::from("-i"),
        OsString::from("-c"),
        OsString::from(LOGIN_SHELL_COMMAND),
    ]
}

fn hydrate_once(
    fallback: &ExecutionEnvironment,
    executable: &Path,
    args: Vec<OsString>,
    timeout: Duration,
    cancellation: &CancellationSignal,
) -> Result<ExecutionEnvironment, HydrationFailure> {
    let deadline = Deadline::new(timeout);
    let output = run(
        SubprocessRequest {
            executable: executable.to_path_buf(),
            args,
            current_dir: None,
            stdin: StdinMode::Closed,
            environment: EnvironmentMode::Replace(fallback.variables()),
            stdout_limit: SHELL_OUTPUT_LIMIT,
            stderr_limit: SHELL_OUTPUT_LIMIT,
            cancellation: Some(cancellation.clone()),
        },
        Some(&deadline),
    )
    .map_err(|error| match error {
        SubprocessError::Spawn(_) => HydrationFailure::Spawn,
        SubprocessError::TimedOut { .. } => HydrationFailure::TimedOut,
        SubprocessError::Cancelled { .. } => HydrationFailure::Cancelled,
        _ => HydrationFailure::Process,
    })?;
    if cancellation.is_cancelled() {
        return Err(HydrationFailure::Cancelled);
    }
    if output.stdout_truncated || output.stderr_truncated {
        return Err(HydrationFailure::TruncatedOutput);
    }
    if !output.status.success() {
        return Err(HydrationFailure::NonzeroExit);
    }
    let values = parse_hydrated_environment(
        &output.stdout,
        output.stdout_truncated,
        output.stderr_truncated,
    )
    .ok_or(HydrationFailure::MalformedOutput)?;
    Ok(fallback.hydrated(values))
}

fn parse_hydrated_environment(
    output: &[u8],
    stdout_truncated: bool,
    stderr_truncated: bool,
) -> Option<HydratedEnvironment> {
    if stdout_truncated || stderr_truncated {
        return None;
    }
    let output = std::str::from_utf8(output).ok()?;
    let path = last_complete_value(output, PATH_START, PATH_END)?.trim();
    if path.is_empty() {
        return None;
    }
    let copilot_home = last_complete_value(output, COPILOT_HOME_START, COPILOT_HOME_END)?.trim();
    Some(HydratedEnvironment {
        path: OsString::from(path),
        copilot_home: (!copilot_home.is_empty()).then(|| OsString::from(copilot_home)),
    })
}

fn last_complete_value<'a>(output: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let mut search_from = 0;
    let mut found = None;
    while let Some(relative_start) = output[search_from..].find(start) {
        let value_start = search_from + relative_start + start.len();
        if let Some(relative_end) = output[value_start..].find(end) {
            let value_end = value_start + relative_end;
            found = Some(&output[value_start..value_end]);
            search_from = value_end + end.len();
        } else {
            search_from = value_start;
        }
    }
    found
}

fn select_login_shell(environment: Option<&OsStr>, account: Option<&OsStr>) -> PathBuf {
    environment
        .filter(|value| !value.is_empty())
        .or_else(|| account.filter(|value| !value.is_empty()))
        .map(PathBuf::from)
        .unwrap_or_else(platform_login_shell)
}

fn platform_login_shell() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/bin/zsh")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        PathBuf::from("/bin/sh")
    }
    #[cfg(not(unix))]
    {
        PathBuf::new()
    }
}

#[cfg(unix)]
fn account_login_shell() -> Option<OsString> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStringExt;
    use std::ptr;

    let requested = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let buffer_size = if requested > 0 {
        usize::try_from(requested).ok()?
    } else {
        16_384
    };
    let mut buffer = vec![0_u8; buffer_size];
    let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut result = ptr::null_mut();
    let status = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            record.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if status != 0 || result.is_null() {
        return None;
    }
    let record = unsafe { record.assume_init() };
    if record.pw_shell.is_null() {
        return None;
    }
    let shell = unsafe { CStr::from_ptr(record.pw_shell) };
    (!shell.to_bytes().is_empty()).then(|| OsString::from_vec(shell.to_bytes().to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn environment(values: &[(&str, &str)]) -> ExecutionEnvironment {
        ExecutionEnvironment::fallback(
            values
                .iter()
                .map(|(key, value)| (OsString::from(key), OsString::from(value))),
        )
    }

    fn variables(values: Vec<(OsString, OsString)>) -> BTreeMap<OsString, OsString> {
        values.into_iter().collect()
    }

    fn shell_args(script: impl Into<OsString>) -> Vec<OsString> {
        vec![OsString::from("-c"), script.into()]
    }

    #[test]
    fn execution_environment_fallback_keeps_inherited_path_first_and_appends_defaults_once() {
        let inherited = "/custom/bin:/usr/bin:/custom/bin";
        let environment = environment(&[("PATH", inherited), ("COPILOT_HOME", "/copilot")]);
        let path = environment
            .get(OsStr::new("PATH"))
            .unwrap()
            .to_string_lossy();
        let entries: Vec<&str> = path.split(':').collect();

        assert_eq!(&entries[..3], ["/custom/bin", "/usr/bin", "/custom/bin"]);
        for fallback in FALLBACK_EXECUTABLE_PATHS {
            assert_eq!(
                entries.iter().filter(|entry| **entry == fallback).count(),
                1
            );
        }
        assert_eq!(
            environment.get(OsStr::new("COPILOT_HOME")),
            Some(OsStr::new("/copilot"))
        );
    }

    #[test]
    fn execution_environment_fallback_supplies_defaults_when_path_is_absent() {
        let environment = environment(&[("HOME", "/home")]);
        assert_eq!(
            environment
                .get(OsStr::new("PATH"))
                .unwrap()
                .to_string_lossy(),
            FALLBACK_EXECUTABLE_PATHS.join(":")
        );
    }

    #[test]
    fn execution_environment_selects_shell_from_environment_account_then_platform() {
        assert_eq!(
            select_login_shell(
                Some(OsStr::new("/custom/shell")),
                Some(OsStr::new("/account"))
            ),
            PathBuf::from("/custom/shell")
        );
        assert_eq!(
            select_login_shell(Some(OsStr::new("")), Some(OsStr::new("/account"))),
            PathBuf::from("/account")
        );
        assert_eq!(
            select_login_shell(None, Some(OsStr::new(""))),
            platform_login_shell()
        );
    }

    #[test]
    fn execution_environment_uses_the_locked_login_shell_invocation() {
        let arguments = login_shell_arguments();
        assert_eq!(&arguments[..3], ["-l", "-i", "-c"]);
        let command = arguments[3].to_string_lossy();
        for value in [
            "/usr/bin/printenv PATH",
            "/usr/bin/printenv COPILOT_HOME",
            PATH_START,
            PATH_END,
            COPILOT_HOME_START,
            COPILOT_HOME_END,
        ] {
            assert!(command.contains(value));
        }
    }

    #[test]
    fn execution_environment_parses_noisy_final_markers_and_optional_copilot_home() {
        let output = b"noise__MUXY_PATH_START__/spoof__MUXY_PATH_END__\n\
            __MUXY_COPILOT_HOME_START__/spoof-home__MUXY_COPILOT_HOME_END__\n\
            __MUXY_PATH_START__/hydrated/bin:/usr/bin__MUXY_PATH_END__\n\
            __MUXY_COPILOT_HOME_START__/hydrated/copilot__MUXY_COPILOT_HOME_END__tail";
        assert_eq!(
            parse_hydrated_environment(output, false, false),
            Some(HydratedEnvironment {
                path: OsString::from("/hydrated/bin:/usr/bin"),
                copilot_home: Some(OsString::from("/hydrated/copilot")),
            })
        );

        let without_copilot = b"__MUXY_PATH_START__/bin__MUXY_PATH_END__\
            __MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__";
        assert_eq!(
            parse_hydrated_environment(without_copilot, false, false),
            Some(HydratedEnvironment {
                path: OsString::from("/bin"),
                copilot_home: None,
            })
        );
    }

    #[test]
    fn execution_environment_rejects_malformed_truncated_and_non_utf8_shell_output() {
        assert_eq!(parse_hydrated_environment(b"/bin", false, false), None);
        assert_eq!(
            parse_hydrated_environment(
                b"__MUXY_PATH_START____MUXY_PATH_END__\
                  __MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__",
                false,
                false,
            ),
            None
        );
        assert_eq!(
            parse_hydrated_environment(
                b"__MUXY_PATH_START__/bin__MUXY_PATH_END__\
                  __MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__",
                true,
                false,
            ),
            None
        );
        assert_eq!(
            parse_hydrated_environment(
                b"__MUXY_PATH_START__/bin__MUXY_PATH_END__\
                  __MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__",
                false,
                true,
            ),
            None
        );
        assert_eq!(
            parse_hydrated_environment(&[0xff, 0xfe], false, false),
            None
        );
    }

    #[test]
    fn execution_environment_builds_exact_consumer_sanitizers() {
        let environment = environment(&[
            ("PATH", "/bin"),
            ("COPILOT_HOME", "/copilot"),
            ("GIT_DIR", "/redirect"),
            ("GIT_WORK_TREE", "/worktree"),
            ("GIT_CONFIG", "/config"),
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "core.worktree"),
            ("GIT_CONFIG_VALUE_0", "/escape"),
            ("GIT_EXEC_PATH", "/fake"),
            ("GIT_SSH_COMMAND", "ssh fixture"),
            ("GIT_ASKPASS", "/askpass"),
            ("GH_REPO", "wrong/target"),
            ("GH_TOKEN", "token"),
            ("MUXY_PANE_ID", "PANE"),
            ("UNRELATED", "kept"),
        ]);

        let git = variables(environment.git_variables());
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_CONFIG",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_KEY_0",
            "GIT_CONFIG_VALUE_0",
            "GIT_EXEC_PATH",
        ] {
            assert!(!git.contains_key(OsStr::new(key)));
        }
        for key in ["GIT_SSH_COMMAND", "GIT_ASKPASS", "GH_REPO", "MUXY_PANE_ID"] {
            assert!(git.contains_key(OsStr::new(key)));
        }

        let github = variables(environment.github_variables());
        assert!(!github.contains_key(OsStr::new("GH_REPO")));
        assert_eq!(
            github.get(OsStr::new("GH_TOKEN")),
            Some(&OsString::from("token"))
        );

        let provider = variables(environment.provider_variables());
        assert!(!provider.contains_key(OsStr::new("MUXY_PANE_ID")));
        assert_eq!(
            provider.get(OsStr::new("COPILOT_HOME")),
            Some(&OsString::from("/copilot"))
        );
        assert_eq!(
            provider.get(OsStr::new("UNRELATED")),
            Some(&OsString::from("kept"))
        );
    }

    #[test]
    fn execution_environment_removes_every_git_redirection_key_and_prefix() {
        let mut inherited: Vec<_> = GIT_REDIRECTION_VARIABLES
            .iter()
            .map(|key| (OsString::from(key), OsString::from("redirected")))
            .collect();
        inherited.extend([
            (
                OsString::from("GIT_CONFIG_KEY_0"),
                OsString::from("core.worktree"),
            ),
            (
                OsString::from("GIT_CONFIG_VALUE_0"),
                OsString::from("/escape"),
            ),
            (
                OsString::from("GIT_CONFIG_KEY_EXTRA"),
                OsString::from("value"),
            ),
            (
                OsString::from("GIT_CONFIG_VALUE_EXTRA"),
                OsString::from("value"),
            ),
            (
                OsString::from("GIT_SSH_COMMAND"),
                OsString::from("ssh fixture"),
            ),
            (OsString::from("GIT_ASKPASS"), OsString::from("/askpass")),
        ]);
        let sanitized = variables(ExecutionEnvironment::fallback(inherited).git_variables());

        for key in GIT_REDIRECTION_VARIABLES {
            assert!(!sanitized.contains_key(OsStr::new(key)));
        }
        for key in [
            "GIT_CONFIG_KEY_0",
            "GIT_CONFIG_VALUE_0",
            "GIT_CONFIG_KEY_EXTRA",
            "GIT_CONFIG_VALUE_EXTRA",
        ] {
            assert!(!sanitized.contains_key(OsStr::new(key)));
        }
        assert!(sanitized.contains_key(OsStr::new("GIT_SSH_COMMAND")));
        assert!(sanitized.contains_key(OsStr::new("GIT_ASKPASS")));
    }

    #[cfg(unix)]
    #[test]
    fn execution_environment_resolves_only_executable_files_from_snapshot_path() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("fixture-tool");
        let plain = directory.path().join("plain-tool");
        std::fs::write(&executable, b"fixture").unwrap();
        std::fs::write(&plain, b"fixture").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o600)).unwrap();
        let environment = environment(&[("PATH", directory.path().to_str().unwrap())]);

        assert_eq!(
            environment.resolve_executable(OsStr::new("fixture-tool")),
            Some(std::fs::canonicalize(executable).unwrap())
        );
        assert_eq!(
            environment.resolve_executable(OsStr::new("plain-tool")),
            None
        );
        assert_eq!(
            environment.resolve_executable(OsStr::new("missing-tool")),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn execution_environment_hydrates_with_eof_and_replaces_path_and_copilot_home() {
        let fallback = environment(&[("PATH", "/fallback/bin"), ("COPILOT_HOME", "/fallback")]);
        let script = "if read value; then exit 9; fi; \
            printf '__MUXY_PATH_START__/hydrated/bin:/usr/bin__MUXY_PATH_END__'; \
            printf '__MUXY_COPILOT_HOME_START__/hydrated/home__MUXY_COPILOT_HOME_END__'";
        let hydrated = hydrate_once(
            &fallback,
            Path::new("/bin/sh"),
            shell_args(script),
            Duration::from_secs(2),
            &CancellationSignal::new(),
        )
        .unwrap();

        assert_eq!(
            hydrated.get(OsStr::new("PATH")),
            Some(OsStr::new("/hydrated/bin:/usr/bin"))
        );
        assert_eq!(
            hydrated.get(OsStr::new("COPILOT_HOME")),
            Some(OsStr::new("/hydrated/home"))
        );

        let unset = hydrate_once(
            &fallback,
            Path::new("/bin/sh"),
            shell_args(
                "printf '__MUXY_PATH_START__/bin__MUXY_PATH_END__'; \
                 printf '__MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__'",
            ),
            Duration::from_secs(2),
            &CancellationSignal::new(),
        )
        .unwrap();
        assert_eq!(unset.get(OsStr::new("COPILOT_HOME")), None);
    }

    #[cfg(unix)]
    #[test]
    fn execution_environment_hydration_keeps_fallback_on_process_and_output_failures() {
        let fallback = environment(&[("PATH", "/fallback/bin")]);
        let cases = [
            (shell_args("exit 7"), HydrationFailure::NonzeroExit),
            (
                shell_args("printf malformed"),
                HydrationFailure::MalformedOutput,
            ),
            (
                shell_args("printf '\\377\\376'"),
                HydrationFailure::MalformedOutput,
            ),
            (
                shell_args(
                    "yes x | head -c 300000; \
                     printf '__MUXY_PATH_START__/bin__MUXY_PATH_END__'; \
                     printf '__MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__'",
                ),
                HydrationFailure::TruncatedOutput,
            ),
            (
                shell_args(
                    "yes e | head -c 300000 >&2; \
                     printf '__MUXY_PATH_START__/bin__MUXY_PATH_END__'; \
                     printf '__MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__'",
                ),
                HydrationFailure::TruncatedOutput,
            ),
        ];
        for (args, expected) in cases {
            assert_eq!(
                hydrate_once(
                    &fallback,
                    Path::new("/bin/sh"),
                    args,
                    Duration::from_secs(3),
                    &CancellationSignal::new(),
                )
                .unwrap_err(),
                expected
            );
        }

        assert_eq!(
            hydrate_once(
                &fallback,
                Path::new("/bin/sh"),
                shell_args("sleep 30"),
                Duration::from_millis(100),
                &CancellationSignal::new(),
            )
            .unwrap_err(),
            HydrationFailure::TimedOut
        );
        assert_eq!(
            hydrate_once(
                &fallback,
                Path::new("/bin/sh"),
                shell_args(
                    "sleep 30 & \
                     printf '__MUXY_PATH_START__/bin__MUXY_PATH_END__'; \
                     printf '__MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__'",
                ),
                Duration::from_millis(100),
                &CancellationSignal::new(),
            )
            .unwrap_err(),
            HydrationFailure::TimedOut
        );
    }

    #[cfg(unix)]
    #[test]
    fn execution_environment_source_caches_one_success_and_revision() {
        let fallback = environment(&[("PATH", "/fallback/bin")]);
        let source = ExecutionEnvironmentSource::new(fallback);
        let receiver = source
            .start_hydration_command(
                PathBuf::from("/bin/sh"),
                shell_args(
                    "printf '__MUXY_PATH_START__/hydrated/bin__MUXY_PATH_END__'; \
                     printf '__MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__'",
                ),
                Duration::from_secs(2),
            )
            .unwrap();
        assert_eq!(
            receiver.recv_blocking().unwrap(),
            HydrationOutcome::Upgraded { revision: 1 }
        );
        assert_eq!(source.revision(), 1);
        assert_eq!(
            source.snapshot().get(OsStr::new("PATH")),
            Some(OsStr::new("/hydrated/bin"))
        );
        assert!(
            source
                .start_hydration_command(
                    PathBuf::from("/bin/sh"),
                    shell_args("exit 0"),
                    Duration::from_secs(1),
                )
                .is_none()
        );
        assert_eq!(source.completion_count(), 1);
        source.close();
    }

    #[cfg(unix)]
    #[test]
    fn execution_environment_source_failure_preserves_fallback_without_revision() {
        let fallback = environment(&[("PATH", "/fallback/bin")]);
        let source = ExecutionEnvironmentSource::new(fallback.clone());
        let receiver = source
            .start_hydration_command(
                PathBuf::from("/bin/sh"),
                shell_args("exit 4"),
                Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(
            receiver.recv_blocking().unwrap(),
            HydrationOutcome::KeptFallback(HydrationFailure::NonzeroExit)
        );
        assert_eq!(source.snapshot(), fallback);
        assert_eq!(source.revision(), 0);
        assert_eq!(source.completion_count(), 1);
        source.close();
    }

    #[test]
    fn execution_environment_completion_guard_applies_only_the_first_outcome() {
        let fallback = environment(&[("PATH", "/fallback/bin")]);
        let source = ExecutionEnvironmentSource::new(fallback.clone());
        assert_eq!(
            source.complete(Err(HydrationFailure::MalformedOutput)),
            HydrationOutcome::KeptFallback(HydrationFailure::MalformedOutput)
        );
        assert_eq!(
            source.complete(Ok(environment(&[("PATH", "/late/bin")]))),
            HydrationOutcome::KeptFallback(HydrationFailure::MalformedOutput)
        );
        assert_eq!(source.snapshot(), fallback);
        assert_eq!(source.revision(), 0);
        assert_eq!(source.completion_count(), 1);
        source.close();
    }

    #[cfg(unix)]
    #[test]
    fn execution_environment_close_cancels_reaps_and_completes_once() {
        let directory = tempfile::tempdir().unwrap();
        let pid_file = directory.path().join("pids");
        let script = format!(
            "trap '' TERM; sleep 30 & printf '%s %s' $$ $! > '{}'; wait",
            pid_file.display()
        );
        let source = ExecutionEnvironmentSource::new(environment(&[("PATH", "/bin")]));
        let receiver = source
            .start_hydration_command(
                PathBuf::from("/bin/sh"),
                shell_args(script),
                Duration::from_secs(30),
            )
            .unwrap();
        let end = Instant::now() + Duration::from_secs(2);
        while !pid_file.exists() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(pid_file.exists());

        source.close();
        assert_eq!(
            receiver.recv_blocking().unwrap(),
            HydrationOutcome::KeptFallback(HydrationFailure::Cancelled)
        );
        assert_eq!(source.completion_count(), 1);
        let processes: Vec<libc::pid_t> = std::fs::read_to_string(pid_file)
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect();
        for process in processes {
            assert_eq!(unsafe { libc::kill(process, 0) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::ESRCH)
            );
        }
    }

    #[cfg(not(unix))]
    #[test]
    fn execution_environment_non_unix_hydration_keeps_inherited_snapshot() {
        let fallback = environment(&[("PATH", "fixture")]);
        let source = ExecutionEnvironmentSource::new(fallback.clone());
        let receiver = source.start_hydration().unwrap();
        assert_eq!(
            receiver.recv_blocking().unwrap(),
            HydrationOutcome::KeptFallback(HydrationFailure::Unsupported)
        );
        assert_eq!(source.snapshot(), fallback);
        assert_eq!(source.revision(), 0);
    }
}
