use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::io::{self, Read, Write};
use std::os::fd::AsFd;
use std::os::unix::{ffi::OsStrExt, process::CommandExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use muxy_protocol::{
    ErrorCode, ExecRequest, ExecResult, MAX_EXEC_OUTPUT, Message, ReplyBody, RequestId,
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
                        execute(
                            &request,
                            Path::new(OsStr::from_bytes(&project.directory.0)),
                            &cancelled,
                            || output.is_closed(),
                        )
                        .map_err(error)
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
    PATH.get_or_init(|| {
        let request = ExecRequest {
            job: 1,
            project: muxy_protocol::ProjectId::new(),
            argv: vec![
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()),
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
    })
    .as_deref()
}

fn error(cause: impl std::fmt::Display) -> ServerError {
    ServerError::new(ErrorCode::BadRequest, cause.to_string())
}

fn nonblocking(fd: &impl AsFd) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(fd)?;
    rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
    Ok(())
}

struct Process(Child);
impl Process {
    fn signal(&mut self, signal: rustix::process::Signal) {
        if let Some(pid) = rustix::process::Pid::from_raw(self.0.id().cast_signed()) {
            let _ = rustix::process::kill_process_group(pid, signal);
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.signal(rustix::process::Signal::KILL);
        let _ = self.0.wait();
    }
}

fn drain(reader: &mut impl Read, bytes: &mut Vec<u8>, truncated: &mut bool) -> io::Result<bool> {
    let mut buffer = [0; 16 * 1024];
    for _ in 0..16 {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                let take = n.min(MAX_EXEC_OUTPUT.saturating_sub(bytes.len()));
                bytes.extend_from_slice(&buffer[..take]);
                *truncated |= take < n;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) => return Err(error),
        }
    }
    Ok(false)
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

#[allow(
    clippy::too_many_lines,
    reason = "One loop owns child status, pipe draining, and cancellation"
)]
fn execute(
    request: &ExecRequest,
    root: &Path,
    cancelled: &AtomicBool,
    disconnected: impl Fn() -> bool,
) -> io::Result<ExecResult> {
    request
        .validate()
        .map_err(|_| io::Error::other("invalid command"))?;
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
    let cwd = request.cwd.as_ref().map_or_else(
        || root.to_owned(),
        |path| root.join(OsStr::from_bytes(&path.0)),
    );
    command
        .envs(&request.env)
        .current_dir(cwd)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = Process(command.spawn()?);
    let input = child
        .0
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("missing stdin"))?;
    let mut output = child
        .0
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("missing stdout"))?;
    let mut errors = child
        .0
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("missing stderr"))?;
    nonblocking(&input)?;
    nonblocking(&output)?;
    nonblocking(&errors)?;
    let mut input = Some(input);
    let mut written = 0;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut truncated = false;
    let mut timed_out = false;
    let mut was_cancelled = false;
    let start = Instant::now();
    let mut stopping = None;
    let mut exited = None;
    let mut code = -1;
    loop {
        let output_done = drain(&mut output, &mut stdout, &mut truncated)?;
        let errors_done = drain(&mut errors, &mut stderr, &mut truncated)?;
        if written < request.stdin.len()
            && let Some(pipe) = &mut input
        {
            match pipe.write(&request.stdin[written..]) {
                Ok(n) => written += n,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => (),
                Err(_) => input = None,
            }
        }
        if written == request.stdin.len() {
            input = None;
        }
        was_cancelled |= cancelled.load(Ordering::Acquire) || disconnected();
        timed_out |= start.elapsed() >= Duration::from_millis(u64::from(request.timeout_ms));
        if (was_cancelled || timed_out) && stopping.is_none() {
            child.signal(rustix::process::Signal::TERM);
            stopping = Some(Instant::now());
        }
        if stopping.is_some_and(|time| time.elapsed() >= Duration::from_millis(150)) {
            child.signal(rustix::process::Signal::KILL);
        }
        if exited.is_none()
            && let Some(status) = child.0.try_wait()?
        {
            code = status.code().unwrap_or(-1);
            exited = Some(Instant::now());
            child.signal(rustix::process::Signal::KILL);
        }
        if exited.is_some_and(|time| {
            (output_done && errors_done) || time.elapsed() >= Duration::from_millis(250)
        }) {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(ExecResult {
        stdout: utf8(&stdout, &mut truncated),
        stderr: utf8(&stderr, &mut truncated),
        exit_code: code,
        timed_out,
        truncated,
        cancelled: was_cancelled,
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

    struct TestDirectory(std::path::PathBuf);
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

    #[test]
    fn command_streams_stdin_and_reports_exit_and_working_directory() {
        let root = TestDirectory::new();
        let mut request = request("cat; pwd; printf problem >&2; exit 7");
        request.stdin = b"input\n".to_vec();
        let result = execute(&request, root.path(), &AtomicBool::new(false), || false).unwrap();
        assert!(result.stdout.starts_with("input\n"));
        assert!(
            result
                .stdout
                .contains(root.path().file_name().unwrap().to_str().unwrap())
        );
        assert_eq!(result.stderr, "problem");
        assert_eq!(result.exit_code, 7);
        assert!(!result.timed_out);
        assert!(!result.cancelled);
    }

    #[test]
    fn timeout_and_disconnect_stop_commands_even_when_descendants_keep_pipes_open() {
        let root = TestDirectory::new();
        let mut request = request("trap '' TERM; sleep 30 & wait");
        request.timeout_ms = 50;
        let started = Instant::now();
        let result = execute(&request, root.path(), &AtomicBool::new(false), || false).unwrap();
        assert!(result.timed_out);
        assert!(started.elapsed() < Duration::from_secs(3));
        request.timeout_ms = 3000;
        let started = Instant::now();
        let result = execute(&request, root.path(), &AtomicBool::new(false), || {
            started.elapsed() > Duration::from_millis(30)
        })
        .unwrap();
        assert!(result.cancelled);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn output_is_bounded_while_both_pipes_are_drained() {
        let root = TestDirectory::new();
        let request = request("head -c 5000000 /dev/zero; head -c 5000000 /dev/zero >&2");
        let result = execute(&request, root.path(), &AtomicBool::new(false), || false).unwrap();
        assert_eq!(result.stdout.len(), MAX_EXEC_OUTPUT);
        assert_eq!(result.stderr.len(), MAX_EXEC_OUTPUT);
        assert!(result.truncated);
        assert_eq!(result.exit_code, 0);
    }
}
