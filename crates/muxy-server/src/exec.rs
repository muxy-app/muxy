use std::collections::{BTreeSet, HashMap};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

use muxy_protocol::{
    ErrorCode, ExecRequest, ExecResult, MAX_EXEC_OUTPUT, Message, ReplyBody, RequestId, ServerPath,
};

use crate::platform::ProcessGroup;
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
                        let root = project.directory.to_path_buf().map_err(|code| {
                            ServerError::new(code, "project folder is not valid here")
                        })?;
                        execute(&request, &root, &cancelled, || output.is_closed())
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

/// The user's login-shell PATH, which a server started from the app doesn't
/// inherit. Windows has no login shell; the registry holds `PATH` there.
pub(crate) fn login_path() -> Option<&'static str> {
    #[cfg(unix)]
    {
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
            let result =
                execute(&request, Path::new("/"), &AtomicBool::new(false), || false).ok()?;
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
    #[cfg(windows)]
    None
}

fn error(cause: impl std::fmt::Display) -> ServerError {
    ServerError::new(ErrorCode::BadRequest, cause.to_string())
}

/// Launch failures read like main's, since extensions match on the text.
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

/// Whether `program` is a program file in one of the `PATH` folders, which is
/// how main finds a command before launching it. On Windows a name without
/// an extension also matches each extension in `PATHEXT`.
fn on_path(program: &str, path: Option<&String>) -> bool {
    let path = path.map_or_else(|| std::env::var_os("PATH").unwrap_or_default(), Into::into);
    std::env::split_paths(&path)
        .filter(|folder| !folder.as_os_str().is_empty())
        .any(|folder| program_in(&folder, program).is_some())
}

/// The program file `name` names in `folder`.
pub(crate) fn program_in(folder: &Path, name: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        let file = folder.join(name);
        (file.is_file() && rustix::fs::access(&file, rustix::fs::Access::EXEC_OK).is_ok())
            .then_some(file)
    }
    #[cfg(windows)]
    {
        let file = folder.join(name);
        if Path::new(name).extension().is_some() {
            return (has_program_extension(&file) && crate::spawn::exists(&file)).then_some(file);
        }
        program_extensions()
            .into_iter()
            .map(|extension| folder.join(format!("{name}{extension}")))
            .find(|file| crate::spawn::exists(file))
    }
}

/// Whether Windows runs `path` by its extension, which `PATHEXT` lists.
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
        .filter(|extension| extension.starts_with('.') && extension.len() > 1)
        .map(str::to_owned)
        .collect()
}

/// A command that runs `script` with `cmd.exe /d /s /c`, which keeps the
/// script exactly as written between the outer quotes.
#[cfg(windows)]
pub(crate) fn cmd_script(script: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let system = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let mut command = Command::new(Path::new(&system).join(r"System32\cmd.exe"));
    command.raw_arg("/d /s /c").raw_arg(format!("\"{script}\""));
    command
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

/// The command and every process it starts, ended when dropped.
struct Process {
    child: Child,
    group: ProcessGroup,
}

impl Drop for Process {
    fn drop(&mut self) {
        self.group.kill();
        let _ = self.child.wait();
    }
}

/// Reads `pipe` until it closes, sending each chunk as `stream`, then `None`.
fn forward(
    stream: usize,
    mut pipe: impl Read + Send + 'static,
    chunks: mpsc::Sender<(usize, Option<Vec<u8>>)>,
) -> io::Result<()> {
    std::thread::Builder::new()
        .name("extension-command-output".into())
        .spawn(move || {
            let mut buffer = [0; 16 * 1024];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if chunks.send((stream, Some(buffer[..n].to_vec()))).is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = chunks.send((stream, None));
        })
        .map(drop)
}

fn keep(bytes: &mut Vec<u8>, chunk: &[u8], truncated: &mut bool) {
    let take = chunk.len().min(MAX_EXEC_OUTPUT.saturating_sub(bytes.len()));
    bytes.extend_from_slice(&chunk[..take]);
    *truncated |= take < chunk.len();
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
        let mut command = Command::new(program(&request.argv[0], request.env.get("PATH")));
        command.args(&request.argv[1..]);
        command
    };
    command
        .envs(&request.env)
        .current_dir(working_directory(root, request.cwd.as_ref())?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let (child, group) =
        ProcessGroup::spawn(&mut command).map_err(|cause| launch_failed(request, &cause))?;
    supervise(Process { child, group }, request, cancelled, disconnected).map_err(error)
}

/// The program file a command names. Windows looks a bare name up in
/// `PATH` with each `PATHEXT` extension, so `.cmd` shims run as they do in a
/// shell; Unix leaves that to the system.
fn program(name: &str, path: Option<&String>) -> PathBuf {
    #[cfg(windows)]
    if !name.contains(['/', '\\']) {
        let path = path.map_or_else(|| std::env::var_os("PATH").unwrap_or_default(), Into::into);
        if let Some(file) = std::env::split_paths(&path)
            .filter(|folder| folder.is_absolute())
            .find_map(|folder| program_in(&folder, name))
        {
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

/// Feeds stdin and collects output until the command exits, is cancelled, or
/// runs out of time. Pipes are read and written on their own threads, since
/// Windows pipes can't be non-blocking.
fn supervise(
    mut process: Process,
    request: &ExecRequest,
    cancelled: &AtomicBool,
    disconnected: impl Fn() -> bool,
) -> io::Result<ExecResult> {
    let mut input = process
        .child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("missing stdin"))?;
    let output = process
        .child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("missing stdout"))?;
    let errors = process
        .child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("missing stderr"))?;
    let (sender, chunks) = mpsc::channel();
    forward(0, output, sender.clone())?;
    forward(1, errors, sender)?;
    let stdin = request.stdin.clone();
    std::thread::Builder::new()
        .name("extension-command-input".into())
        .spawn(move || {
            let _ = input.write_all(&stdin);
        })?;
    let mut streams = [Vec::new(), Vec::new()];
    let mut open = [true, true];
    let mut truncated = false;
    let mut timed_out = false;
    let mut was_cancelled = false;
    let start = Instant::now();
    let mut stopping = None;
    let mut exited = None;
    let mut code = -1;
    loop {
        match chunks.recv_timeout(Duration::from_millis(5)) {
            Ok((stream, Some(chunk))) => keep(&mut streams[stream], &chunk, &mut truncated),
            Ok((stream, None)) => open[stream] = false,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        was_cancelled |= cancelled.load(Ordering::Acquire) || disconnected();
        timed_out |= start.elapsed() >= Duration::from_millis(u64::from(request.timeout_ms));
        if (was_cancelled || timed_out) && stopping.is_none() {
            process.group.terminate();
            stopping = Some(Instant::now());
        }
        if stopping.is_some_and(|time| time.elapsed() >= Duration::from_millis(150)) {
            process.group.kill();
        }
        if exited.is_none()
            && let Some(status) = process.child.try_wait()?
        {
            code = status.code().unwrap_or(-1);
            exited = Some(Instant::now());
            process.group.kill();
        }
        if exited.is_some_and(|time| {
            open == [false, false] || time.elapsed() >= Duration::from_millis(250)
        }) {
            break;
        }
    }
    for (stream, chunk) in chunks.try_iter() {
        if let Some(chunk) = chunk {
            keep(&mut streams[stream], &chunk, &mut truncated);
        }
    }
    let [stdout, stderr] = streams;
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

    #[cfg(unix)]
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

    #[cfg(unix)]
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

    #[cfg(unix)]
    #[test]
    fn output_is_bounded_while_both_pipes_are_drained() {
        let root = TestDirectory::new();
        let mut request = request("head -c 5000000 /dev/zero; head -c 5000000 /dev/zero >&2");
        request.timeout_ms = 30_000;
        let result = execute(&request, root.path(), &AtomicBool::new(false), || false).unwrap();
        assert_eq!(result.stdout.len(), MAX_EXEC_OUTPUT);
        assert_eq!(result.stderr.len(), MAX_EXEC_OUTPUT);
        assert!(result.truncated);
        assert_eq!(result.exit_code, 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_command_streams_stdin_and_reports_exit_and_working_directory() {
        let root = TestDirectory::new();
        let mut request = request("findstr \"^\" & cd & (echo problem)1>&2 & exit /b 7");
        request.stdin = b"input\r\n".to_vec();
        let result = execute(&request, root.path(), &AtomicBool::new(false), || false).unwrap();
        assert!(
            result.stdout.starts_with("input\r\n"),
            "{:?}",
            result.stdout
        );
        assert!(
            result
                .stdout
                .contains(root.path().file_name().unwrap().to_str().unwrap())
        );
        assert_eq!(result.stderr.trim_end(), "problem");
        assert_eq!(result.exit_code, 7);
        assert!(!result.timed_out);
        assert!(!result.cancelled);
    }

    #[cfg(windows)]
    #[test]
    fn windows_timeout_and_disconnect_stop_commands_even_when_descendants_keep_pipes_open() {
        let root = TestDirectory::new();
        let mut request = request("start /b ping -n 30 127.0.0.1 >nul & ping -n 30 127.0.0.1 >nul");
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

    #[cfg(windows)]
    #[test]
    fn windows_command_grandchild_ends_with_its_job() {
        let root = TestDirectory::new();
        let request =
            request("start /b cmd /d /c \"ping -n 3 127.0.0.1 >nul & echo late> grandchild.txt\"");
        let result = execute(&request, root.path(), &AtomicBool::new(false), || false).unwrap();
        assert_eq!(result.exit_code, 0);
        std::thread::sleep(Duration::from_secs(4));
        assert!(!root.path().join("grandchild.txt").exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_output_is_bounded_while_both_pipes_are_drained() {
        let root = TestDirectory::new();
        std::fs::write(root.path().join("big.txt"), vec![b'0'; 5_000_000]).unwrap();
        let mut request = request("type big.txt & type big.txt 1>&2");
        request.timeout_ms = 30_000;
        let result = execute(&request, root.path(), &AtomicBool::new(false), || false).unwrap();
        assert_eq!(result.stdout.len(), MAX_EXEC_OUTPUT);
        assert_eq!(result.stderr.len(), MAX_EXEC_OUTPUT);
        assert!(result.truncated);
        assert_eq!(result.exit_code, 0);
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
