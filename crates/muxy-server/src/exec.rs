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

use crate::{Registry, ServerError, connection::Outbox};

#[derive(Default)]
pub(crate) struct Jobs {
    active: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    cancelled: Mutex<BTreeSet<u64>>,
}

impl Drop for Jobs {
    fn drop(&mut self) {
        for cancelled in self
            .active
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
        {
            cancelled.store(true, Ordering::Release);
        }
    }
}

impl Jobs {
    pub(crate) fn cancel(&self, job: u64) {
        let active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(cancelled) = active.get(&job) {
            cancelled.store(true, Ordering::Release);
        } else {
            // Cancellation may arrive before the background sender transmits Exec.
            let mut cancelled = self
                .cancelled
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            cancelled.insert(job);
            if cancelled.len() > 128 {
                cancelled.pop_first();
            }
        }
    }

    fn register(&self, job: u64) -> Result<Arc<AtomicBool>, ServerError> {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if active.len() >= 32 || active.contains_key(&job) {
            return Err(error("too many commands or duplicate job ID"));
        }
        let was_cancelled = self
            .cancelled
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&job);
        let cancelled = Arc::new(AtomicBool::new(was_cancelled));
        active.insert(job, Arc::clone(&cancelled));
        Ok(cancelled)
    }

    pub(crate) fn start(
        &self,
        mut request: ExecRequest,
        id: RequestId,
        registry: Arc<Registry>,
        output: Arc<Outbox>,
    ) -> Result<(), ServerError> {
        let job = request.job;
        let cancelled = self.register(job)?;
        let active = Arc::clone(&self.active);
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
                        execute(&request, project.directory.as_path(), &cancelled, || {
                            output.is_closed()
                        })
                    });
                active
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
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
            self.active
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&job);
            return Err(error(cause));
        }
        Ok(())
    }
}

/// The user's login-shell PATH, which a server started from the app doesn't inherit.
pub(crate) fn login_path() -> Option<&'static str> {
    static PATH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    PATH.get_or_init(|| read_login_path(&crate::process::user_shell()))
        .as_deref()
}

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

/// Launch failures read like main's, since extensions match on the text.
fn launch_failed(request: &ExecRequest, cause: &io::Error) -> ServerError {
    ServerError::spawn_failed(match request.argv.first() {
        Some(program) if !program.contains('/') && !on_path(program, request.env.get("PATH")) => {
            format!("command not found: {program}")
        }
        _ => format!("spawn process: {}", os_message(cause)),
    })
}

/// Whether `program` is an executable file in one of the `PATH` folders,
/// which is how main finds a command before launching it.
fn on_path(program: &str, path: Option<&String>) -> bool {
    let path = path.map_or_else(|| std::env::var_os("PATH").unwrap_or_default(), Into::into);
    crate::process::find_executable(program, std::env::split_paths(&path)).is_some()
}

/// The system's description of an error, without Rust's "(os error N)".
fn os_message(error: &io::Error) -> String {
    let text = error.to_string();
    let suffix = error
        .raw_os_error()
        .map(|code| format!(" (os error {code})"))
        .unwrap_or_default();
    text.strip_suffix(&suffix).unwrap_or(&text).to_owned()
}

/// Where a command runs: `~` is the home folder, and relative paths start in
/// the project folder.
fn working_directory(root: &Path, cwd: Option<&ServerPath>) -> PathBuf {
    let Some(path) = cwd.map(ServerPath::as_path) else {
        return root.to_owned();
    };
    if let Ok(relative) = path.strip_prefix("~")
        && let Some(home) = std::env::home_dir()
    {
        return home.join(relative);
    }
    root.join(path)
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
        let mut command = Command::new("/bin/sh");
        command.args(["-c", shell]);
        command
    } else {
        let mut command = Command::new(&request.argv[0]);
        command.args(&request.argv[1..]);
        command
    };
    command
        .envs(&request.env)
        .current_dir(working_directory(root, request.cwd.as_ref()));
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
        assert!(jobs.cancelled.lock().unwrap().len() <= 128);
    }
}
