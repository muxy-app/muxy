use std::collections::{BTreeSet, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use muxy_core::process::{Cap, Limits, Running};
use muxy_protocol::{
    ErrorCode, ExecRequest, ExecResult, MAX_EXEC_OUTPUT, Message, ReplyBody, RequestId, ServerPath,
};

use crate::{Registry, ServerError, connection::Connection};

#[derive(Default)]
pub(crate) struct Jobs {
    state: Arc<Mutex<JobState>>,
}

#[derive(Default)]
struct JobState {
    active: HashMap<u64, Arc<AtomicBool>>,
    cancelled: BTreeSet<u64>,
}

impl Drop for Jobs {
    fn drop(&mut self) {
        for cancelled in self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active
            .values()
        {
            cancelled.store(true, Ordering::Release);
        }
    }
}

impl Jobs {
    pub(crate) fn cancel(&self, job: u64) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(cancelled) = state.active.get(&job) {
            cancelled.store(true, Ordering::Release);
        } else {
            state.cancelled.insert(job);
            if state.cancelled.len() > 128 {
                state.cancelled.pop_first();
            }
        }
    }

    fn register(&self, job: u64) -> Result<Arc<AtomicBool>, ServerError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.active.len() >= 32 || state.active.contains_key(&job) {
            return Err(error("too many commands or duplicate job ID"));
        }
        let was_cancelled = state.cancelled.remove(&job);
        let cancelled = Arc::new(AtomicBool::new(was_cancelled));
        state.active.insert(job, Arc::clone(&cancelled));
        Ok(cancelled)
    }

    pub(crate) fn start(
        &self,
        mut request: ExecRequest,
        id: RequestId,
        registry: Arc<Registry>,
        output: Arc<Connection>,
    ) -> Result<(), ServerError> {
        let job = request.job;
        let cancelled = self.register(job)?;
        let state = Arc::clone(&self.state);
        let spawn = std::thread::Builder::new()
            .name("extension-command".into())
            .spawn(move || {
                if !request.env.contains_key("PATH")
                    && let Some(path) = login_path()
                {
                    request.env.insert("PATH".into(), path.into());
                }
                let result = registry
                    .catalog
                    .project(request.project)
                    .and_then(|project| {
                        let root = project.directory.to_path_buf().map_err(|code| {
                            ServerError::new(code, "project folder is not valid here")
                        })?;
                        execute(&request, &root, &cancelled, || output.is_closed())
                    });
                state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .active
                    .remove(&job);
                output.push_control(Message::Reply {
                    id,
                    body: match result {
                        Ok(result) => ReplyBody::Exec(result),
                        Err(error) => ReplyBody::Error(error.to_reply()),
                    },
                });
            });
        if let Err(cause) = spawn {
            self.state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .active
                .remove(&job);
            return Err(error(cause));
        }
        Ok(())
    }
}

pub(crate) fn login_path() -> Option<&'static str> {
    #[cfg(unix)]
    {
        static PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        PATH.get_or_init(|| read_login_path(&crate::process::user_shell()))
            .as_deref()
    }
    #[cfg(windows)]
    None
}

#[cfg(unix)]
fn read_login_path(shell: &Path) -> Option<String> {
    let request = ExecRequest {
        job: 1,
        project: muxy_protocol::ProjectId::new(),
        argv: vec![
            shell.to_str()?.to_owned(),
            "-lc".into(),
            "printf '\\0%s\\0' \"$PATH\"".into(),
        ],
        shell: None,
        cwd: None,
        env: std::collections::BTreeMap::new(),
        stdin: Vec::new(),
        timeout_ms: 3_000,
    };
    let result = execute(&request, Path::new("/"), &AtomicBool::new(false), || false).ok()?;
    if result.exit_code != 0 || result.timed_out {
        return None;
    }
    result
        .stdout
        .split('\0')
        .nth(1)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
}

fn error(cause: impl std::fmt::Display) -> ServerError {
    ServerError::new(ErrorCode::BadRequest, cause.to_string())
}

fn launch_failed(request: &ExecRequest, cause: &io::Error) -> ServerError {
    ServerError::spawn_failed(match request.argv.first() {
        Some(program)
            if !program.contains(['/', std::path::MAIN_SEPARATOR])
                && !on_path(program, request.env.get("PATH")) =>
        {
            format!("command not found: {program}")
        }
        _ => format!("spawn process: {}", os_message(cause)),
    })
}

fn on_path(program: &str, path: Option<&String>) -> bool {
    let path = path.map_or_else(|| std::env::var_os("PATH").unwrap_or_default(), Into::into);
    find_program(program, std::env::split_paths(&path)).is_some()
}

pub(crate) fn find_program(
    name: &str,
    folders: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        crate::process::find_executable(name, folders)
    }
    #[cfg(windows)]
    {
        folders
            .into_iter()
            .filter(|folder| !folder.as_os_str().is_empty())
            .find_map(|folder| program_in(&folder, name))
    }
}

#[cfg(windows)]
fn program_in(folder: &Path, name: &str) -> Option<PathBuf> {
    let file = folder.join(name);
    if Path::new(name).extension().is_some() {
        return (has_program_extension(&file) && crate::spawn::exists(&file)).then_some(file);
    }
    program_extensions()
        .into_iter()
        .map(|extension| folder.join(format!("{name}{extension}")))
        .find(|file| crate::spawn::exists(file))
}

#[cfg(windows)]
pub(crate) fn has_program_extension(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        program_extensions().iter().any(|known| {
            known
                .get(1..)
                .is_some_and(|known| extension.eq_ignore_ascii_case(known))
        })
    })
}

#[cfg(windows)]
fn program_extensions() -> Vec<String> {
    std::env::var("PATHEXT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into())
        .split(';')
        .filter(|extension| {
            [".com", ".exe", ".bat", ".cmd"]
                .iter()
                .any(|startable| extension.eq_ignore_ascii_case(startable))
        })
        .map(str::to_owned)
        .collect()
}

#[cfg(windows)]
pub(crate) fn cmd_script(script: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let system = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let mut command = Command::new(Path::new(&system).join(r"System32\cmd.exe"));
    command.raw_arg("/d /s /c").raw_arg(format!("\"{script}\""));
    command
}

fn os_message(error: &io::Error) -> String {
    let text = error.to_string();
    let suffix = error
        .raw_os_error()
        .map(|code| format!(" (os error {code})"))
        .unwrap_or_default();
    text.strip_suffix(&suffix).unwrap_or(&text).to_owned()
}

fn working_directory(root: &Path, cwd: Option<&ServerPath>) -> Result<PathBuf, ServerError> {
    let Some(path) = cwd else {
        return Ok(root.to_owned());
    };
    let path = path
        .to_path_buf()
        .map_err(|code| ServerError::new(code, "working directory is not valid here"))?;
    if let Ok(relative) = path.strip_prefix("~")
        && let Ok(home) = muxy_core::paths::home_dir()
    {
        return Ok(home.join(relative));
    }
    Ok(root.join(path))
}

fn utf8(bytes: &[u8], truncated: &mut bool) -> String {
    let mut text = String::from_utf8_lossy(bytes).into_owned();
    if text.len() > MAX_EXEC_OUTPUT {
        let mut end = MAX_EXEC_OUTPUT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        *truncated = true;
    }
    text
}

fn execute(
    request: &ExecRequest,
    root: &Path,
    cancelled: &AtomicBool,
    disconnected: impl Fn() -> bool,
) -> Result<ExecResult, ServerError> {
    request.validate().map_err(|_| error("invalid command"))?;
    if cancelled.load(Ordering::Acquire) || disconnected() {
        return Ok(ExecResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: -1,
            timed_out: false,
            truncated: false,
            cancelled: true,
        });
    }
    let mut command = if let Some(shell) = &request.shell {
        script_command(shell)
    } else {
        let path = request
            .env
            .iter()
            .rfind(|(name, _)| crate::spawn::same_name(std::ffi::OsStr::new(name), "PATH"))
            .map(|(_, value)| value);
        let mut command = Command::new(program(&request.argv[0], path));
        command.args(&request.argv[1..]);
        command
    };
    command
        .envs(&request.env)
        .current_dir(working_directory(root, request.cwd.as_ref())?);
    let stopped = || cancelled.load(Ordering::Acquire) || disconnected();
    let limits = Limits {
        timeout: Duration::from_millis(u64::from(request.timeout_ms)),
        stdout: Cap::TruncateAt(MAX_EXEC_OUTPUT),
        stderr: Cap::TruncateAt(MAX_EXEC_OUTPUT),
        stdin: Some(&request.stdin),
        cancelled: Some(&stopped),
        term_grace: Duration::from_millis(150),
        kill_leftovers: true,
    };
    let output = Running::start(&mut command, limits)
        .map_err(|cause| launch_failed(request, &cause))?
        .wait()
        .map_err(error)?;
    let mut truncated = output.stdout.truncated || output.stderr.truncated;
    Ok(ExecResult {
        stdout: utf8(&output.stdout.bytes, &mut truncated),
        stderr: utf8(&output.stderr.bytes, &mut truncated),
        exit_code: output.status.code().unwrap_or(-1),
        timed_out: output.timed_out,
        truncated,
        cancelled: output.cancelled,
    })
}

fn program(name: &str, path: Option<&String>) -> PathBuf {
    #[cfg(windows)]
    if !name.contains(['/', '\\']) {
        let path = path.map_or_else(|| std::env::var_os("PATH").unwrap_or_default(), Into::into);
        if let Some(file) = find_program(
            name,
            std::env::split_paths(&path).filter(|folder| folder.is_absolute()),
        ) {
            return file;
        }
    }
    #[cfg(unix)]
    let _ = path;
    PathBuf::from(name)
}

#[cfg(unix)]
fn script_command(script: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", script]);
    command
}

#[cfg(windows)]
fn script_command(script: &str) -> Command {
    cmd_script(script)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::panic,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("muxy-exec-{}", muxy_protocol::OperationId::new()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn request(shell: &str) -> ExecRequest {
        ExecRequest {
            job: 1,
            project: muxy_protocol::ProjectId::new(),
            argv: Vec::new(),
            shell: Some(shell.into()),
            cwd: None,
            env: std::collections::BTreeMap::new(),
            stdin: Vec::new(),
            timeout_ms: 3000,
        }
    }

    #[cfg(unix)]
    #[test]
    fn login_path_comes_from_the_fallback_shell() {
        let path = read_login_path(Path::new("/bin/sh")).unwrap();
        assert!(std::env::split_paths(&path).any(|folder| folder == Path::new("/usr/bin")));
    }

    #[test]
    fn cancel_before_registration_never_launches_the_command() {
        let root = TestDirectory::new();
        let jobs = Jobs::default();
        jobs.cancel(1);
        let cancelled = jobs.register(1).unwrap();
        let result = execute(
            &request("touch should-not-exist"),
            root.path(),
            &cancelled,
            || false,
        )
        .unwrap();
        assert!(result.cancelled);
        assert!(!root.path().join("should-not-exist").exists());
        for job in 2..1000 {
            jobs.cancel(job);
        }
        assert!(jobs.state.lock().unwrap().cancelled.len() <= 128);
    }

    #[test]
    fn duplicate_registration_keeps_the_original_cancellation_handle() {
        let jobs = Jobs::default();
        let first = jobs.register(1).unwrap();
        let second = jobs.register(2).unwrap();
        let duplicate = jobs.register(1).unwrap_err();
        assert_eq!(duplicate.code(), ErrorCode::BadRequest);
        assert_eq!(duplicate.message(), "too many commands or duplicate job ID");
        jobs.cancel(1);
        assert!(first.load(Ordering::Acquire));
        assert!(!second.load(Ordering::Acquire));
    }

    #[test]
    fn registration_capacity_preserves_the_running_jobs() {
        let jobs = Jobs::default();
        let running: Vec<_> = (1..=32).map(|job| jobs.register(job).unwrap()).collect();
        jobs.cancel(33);
        let refused = jobs.register(33).unwrap_err();
        assert_eq!(refused.code(), ErrorCode::BadRequest);
        assert_eq!(refused.message(), "too many commands or duplicate job ID");
        assert!(running.iter().all(|flag| !flag.load(Ordering::Acquire)));
        for job in 1..=32 {
            jobs.cancel(job);
        }
        assert!(running.iter().all(|flag| flag.load(Ordering::Acquire)));
    }

    #[test]
    fn remembered_cancellations_evict_the_smallest_job_id() {
        let jobs = Jobs::default();
        jobs.cancel(200);
        for job in 1..=128 {
            jobs.cancel(job);
        }
        assert!(!jobs.register(1).unwrap().load(Ordering::Acquire));
        assert!(jobs.register(2).unwrap().load(Ordering::Acquire));
        assert!(jobs.register(200).unwrap().load(Ordering::Acquire));
        assert!(!jobs.register(129).unwrap().load(Ordering::Acquire));
    }

    #[test]
    fn dropping_jobs_cancels_all_registered_commands() {
        let jobs = Jobs::default();
        let first = jobs.register(1).unwrap();
        let second = jobs.register(2).unwrap();
        jobs.cancel(1);
        assert!(!second.load(Ordering::Acquire));
        drop(jobs);
        assert!(first.load(Ordering::Acquire));
        assert!(second.load(Ordering::Acquire));
    }

    #[test]
    fn cancellation_racing_registration_reaches_every_command() {
        let jobs = Jobs::default();
        let start = std::sync::Barrier::new(2);
        let running = std::thread::scope(|scope| {
            let registering = scope.spawn(|| {
                start.wait();
                (1..=32)
                    .map(|job| {
                        let flag = jobs.register(job).unwrap();
                        std::thread::yield_now();
                        flag
                    })
                    .collect::<Vec<_>>()
            });
            scope.spawn(|| {
                start.wait();
                for job in 1..=32 {
                    jobs.cancel(job);
                    std::thread::yield_now();
                }
            });
            registering.join().unwrap()
        });
        assert!(running.iter().all(|flag| flag.load(Ordering::Acquire)));
    }

    #[cfg(windows)]
    #[test]
    fn windows_programs_are_found_with_pathext_and_missing_ones_read_like_main() {
        let root = TestDirectory::new();
        std::fs::write(root.path().join("tool.cmd"), "@echo shim %1").unwrap();
        let path = root.path().to_str().unwrap().to_owned();
        assert!(on_path("tool", Some(&path)));
        assert!(on_path("TOOL.CMD", Some(&path)));
        assert!(!on_path("tool.txt", Some(&path)));
        let mut request = request("");
        request.shell = None;
        request.argv = vec!["tool".into(), "argument".into()];
        request.env.insert("PATH".into(), path);
        let result = execute(&request, root.path(), &AtomicBool::new(false), || false).unwrap();
        assert_eq!(result.stdout.trim_end(), "shim argument");
        request.argv = vec!["missing-muxy-tool".into()];
        let missing = execute(&request, root.path(), &AtomicBool::new(false), || false);
        assert_eq!(
            missing.unwrap_err().message(),
            "command not found: missing-muxy-tool"
        );
    }
}
