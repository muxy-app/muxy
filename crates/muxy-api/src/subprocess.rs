use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(unix)]
const TERMINATION_GRACE: Duration = Duration::from_millis(500);
#[cfg(not(unix))]
const READER_FALLBACK_GRACE: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub struct Deadline {
    expires_at: Instant,
}

impl Deadline {
    pub fn new(timeout: Duration) -> Self {
        Self {
            expires_at: Instant::now() + timeout,
        }
    }

    pub fn remaining(&self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }

    pub fn is_expired(&self) -> bool {
        self.remaining().is_zero()
    }
}

#[derive(Clone, Debug, Default)]
pub struct CancellationSignal {
    cancelled: Arc<AtomicBool>,
}

impl CancellationSignal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StdinMode {
    Closed,
    Bytes(Vec<u8>),
    Inherit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentMode {
    Replace(Vec<(OsString, OsString)>),
    Inherit {
        set: Vec<(OsString, OsString)>,
        remove: Vec<OsString>,
    },
}

#[derive(Clone, Debug)]
pub struct SubprocessRequest {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    pub stdin: StdinMode,
    pub environment: EnvironmentMode,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
    pub cancellation: Option<CancellationSignal>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapturedOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Debug)]
pub struct SubprocessOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputStream {
    Stdin,
    Stdout,
    Stderr,
}

impl std::fmt::Display for OutputStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stdin => formatter.write_str("stdin"),
            Self::Stdout => formatter.write_str("stdout"),
            Self::Stderr => formatter.write_str("stderr"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubprocessError {
    #[error("could not start subprocess: {0}")]
    Spawn(io::Error),
    #[error("could not capture subprocess {0}")]
    MissingPipe(OutputStream),
    #[error("could not read subprocess {stream}: {source}")]
    Read {
        stream: OutputStream,
        source: io::Error,
        output: CapturedOutput,
    },
    #[error("could not write subprocess stdin: {source}")]
    Write {
        source: io::Error,
        output: CapturedOutput,
    },
    #[error("could not wait for subprocess: {source}")]
    Wait {
        source: io::Error,
        output: CapturedOutput,
    },
    #[error("could not terminate subprocess: {source}")]
    Terminate {
        source: io::Error,
        output: CapturedOutput,
    },
    #[error("subprocess {stream} reader stopped unexpectedly")]
    ReaderStopped {
        stream: OutputStream,
        output: CapturedOutput,
    },
    #[error("subprocess timed out")]
    TimedOut { output: CapturedOutput },
    #[error("subprocess was cancelled")]
    Cancelled { output: CapturedOutput },
}

pub fn bounded_error_text(bytes: &[u8]) -> String {
    let mut printable = VecDeque::with_capacity(1_000);
    for character in String::from_utf8_lossy(bytes).chars() {
        if character.is_control() {
            continue;
        }
        if printable.len() == 1_000 {
            printable.pop_front();
        }
        printable.push_back(character);
    }
    printable.into_iter().collect()
}

enum StopReason {
    TimedOut,
    Cancelled,
    Wait(io::Error),
    Write(io::Error),
}

struct StreamCapture {
    bytes: Vec<u8>,
    truncated: bool,
    error: Option<io::Error>,
}

enum ProcessEvent {
    Stream(OutputStream, StreamCapture),
    Writer(Result<(), io::Error>),
}

pub fn run(
    request: SubprocessRequest,
    deadline: Option<&Deadline>,
) -> Result<SubprocessOutput, SubprocessError> {
    if deadline.is_some_and(Deadline::is_expired) {
        return Err(SubprocessError::TimedOut {
            output: CapturedOutput::default(),
        });
    }
    if request
        .cancellation
        .as_ref()
        .is_some_and(CancellationSignal::is_cancelled)
    {
        return Err(SubprocessError::Cancelled {
            output: CapturedOutput::default(),
        });
    }

    let mut command = Command::new(&request.executable);
    command
        .args(&request.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match &request.stdin {
        StdinMode::Closed => {
            command.stdin(Stdio::null());
        }
        StdinMode::Bytes(_) => {
            command.stdin(Stdio::piped());
        }
        StdinMode::Inherit => {
            command.stdin(Stdio::inherit());
        }
    }
    match &request.environment {
        EnvironmentMode::Replace(environment) => {
            command
                .env_clear()
                .envs(environment.iter().map(|(key, value)| (key, value)));
        }
        EnvironmentMode::Inherit { set, remove } => {
            command.envs(set.iter().map(|(key, value)| (key, value)));
            for key in remove {
                command.env_remove(key);
            }
        }
    }
    if let Some(current_dir) = &request.current_dir {
        command.current_dir(current_dir);
    }
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn().map_err(SubprocessError::Spawn)?;
    let process_id = child.id();
    let Some(stdout) = child.stdout.take() else {
        stop_after_missing_pipe(&mut child, process_id);
        return Err(SubprocessError::MissingPipe(OutputStream::Stdout));
    };
    let Some(stderr) = child.stderr.take() else {
        stop_after_missing_pipe(&mut child, process_id);
        return Err(SubprocessError::MissingPipe(OutputStream::Stderr));
    };
    let (sender, receiver) = mpsc::channel();
    let stdout_reader = spawn_reader(
        sender.clone(),
        OutputStream::Stdout,
        stdout,
        request.stdout_limit,
    );
    let stderr_reader = spawn_reader(
        sender.clone(),
        OutputStream::Stderr,
        stderr,
        request.stderr_limit,
    );
    let writer_expected = matches!(request.stdin, StdinMode::Bytes(_));
    let stdin_writer = match request.stdin {
        StdinMode::Bytes(bytes) => {
            let Some(stdin) = child.stdin.take() else {
                stop_after_missing_pipe(&mut child, process_id);
                return Err(SubprocessError::MissingPipe(OutputStream::Stdin));
            };
            Some(spawn_writer(sender.clone(), stdin, bytes))
        }
        StdinMode::Closed | StdinMode::Inherit => None,
    };
    drop(sender);

    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    let mut writer = (!writer_expected).then_some(Ok(()));
    let mut stop_reason = None;
    let mut reader_failed = false;
    loop {
        receive_events(&receiver, &mut stdout, &mut stderr, &mut writer);
        if stdout
            .as_ref()
            .is_some_and(|capture| capture.error.is_some())
            || stderr
                .as_ref()
                .is_some_and(|capture| capture.error.is_some())
        {
            reader_failed = true;
            break;
        }
        if matches!(writer, Some(Err(_)))
            && let Some(Err(error)) = writer.take()
        {
            stop_reason = Some(StopReason::Write(error));
            break;
        }
        if writer.is_none() && !writer_expected {
            writer = Some(Ok(()));
        }
        if status.is_none() {
            match poll_child(&mut child) {
                Ok(result) => status = result,
                Err(error) => {
                    stop_reason = Some(StopReason::Wait(error));
                    break;
                }
            }
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() && writer.is_some() {
            break;
        }
        if deadline.is_some_and(Deadline::is_expired) {
            stop_reason = Some(StopReason::TimedOut);
            break;
        }
        if request
            .cancellation
            .as_ref()
            .is_some_and(CancellationSignal::is_cancelled)
        {
            stop_reason = Some(StopReason::Cancelled);
            break;
        }
        let delay = deadline.map_or(POLL_INTERVAL, |deadline| {
            POLL_INTERVAL.min(deadline.remaining())
        });
        std::thread::sleep(delay.max(Duration::from_millis(1)));
    }

    let terminate_error = if stop_reason.is_some() || reader_failed {
        terminate(&mut child, process_id, &mut status).err()
    } else {
        None
    };

    collect_threads(
        receiver,
        stdout_reader,
        stderr_reader,
        stdin_writer,
        &mut stdout,
        &mut stderr,
        &mut writer,
    );
    let stopped_reader = if stdout.is_none() {
        Some(OutputStream::Stdout)
    } else if stderr.is_none() {
        Some(OutputStream::Stderr)
    } else {
        None
    };
    let (output, read_error) = captured_output(stdout, stderr);

    if let Some(source) = terminate_error {
        return Err(SubprocessError::Terminate { source, output });
    }
    if let Some((stream, source)) = read_error {
        return Err(SubprocessError::Read {
            stream,
            source,
            output,
        });
    }
    if let Some(stream) = stopped_reader {
        return Err(SubprocessError::ReaderStopped { stream, output });
    }
    if let Some(reason) = stop_reason {
        return Err(match reason {
            StopReason::TimedOut => SubprocessError::TimedOut { output },
            StopReason::Cancelled => SubprocessError::Cancelled { output },
            StopReason::Wait(source) => SubprocessError::Wait { source, output },
            StopReason::Write(source) => SubprocessError::Write { source, output },
        });
    }
    if let Some(Err(source)) = writer {
        return Err(SubprocessError::Write { source, output });
    }
    let status = status.ok_or_else(|| SubprocessError::Wait {
        source: io::Error::other("missing status"),
        output: output.clone(),
    })?;
    Ok(SubprocessOutput {
        status,
        stdout: output.stdout,
        stderr: output.stderr,
        stdout_truncated: output.stdout_truncated,
        stderr_truncated: output.stderr_truncated,
    })
}

fn spawn_reader<R: Read + Send + 'static>(
    sender: mpsc::Sender<ProcessEvent>,
    stream: OutputStream,
    reader: R,
    limit: usize,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let _ = sender.send(ProcessEvent::Stream(stream, read_stream(reader, limit)));
    })
}

fn spawn_writer<W: Write + Send + 'static>(
    sender: mpsc::Sender<ProcessEvent>,
    mut writer: W,
    bytes: Vec<u8>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let result = writer.write_all(&bytes).or_else(|error| {
            if error.kind() == io::ErrorKind::BrokenPipe {
                Ok(())
            } else {
                Err(error)
            }
        });
        drop(writer);
        let _ = sender.send(ProcessEvent::Writer(result));
    })
}

fn read_stream<R: Read>(mut reader: R, limit: usize) -> StreamCapture {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0_u8; 8_192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                return StreamCapture {
                    bytes,
                    truncated,
                    error: None,
                };
            }
            Ok(count) => {
                let remaining = limit.saturating_sub(bytes.len());
                let retained = remaining.min(count);
                bytes.extend_from_slice(&buffer[..retained]);
                truncated |= retained < count;
            }
            Err(error) => {
                return StreamCapture {
                    bytes,
                    truncated,
                    error: Some(error),
                };
            }
        }
    }
}

fn receive_events(
    receiver: &mpsc::Receiver<ProcessEvent>,
    stdout: &mut Option<StreamCapture>,
    stderr: &mut Option<StreamCapture>,
    writer: &mut Option<Result<(), io::Error>>,
) {
    while let Ok(event) = receiver.try_recv() {
        apply_event(event, stdout, stderr, writer);
    }
}

fn apply_event(
    event: ProcessEvent,
    stdout: &mut Option<StreamCapture>,
    stderr: &mut Option<StreamCapture>,
    writer: &mut Option<Result<(), io::Error>>,
) {
    match event {
        ProcessEvent::Stream(OutputStream::Stdout, capture) => *stdout = Some(capture),
        ProcessEvent::Stream(OutputStream::Stderr, capture) => *stderr = Some(capture),
        ProcessEvent::Stream(OutputStream::Stdin, _) => {}
        ProcessEvent::Writer(result) => *writer = Some(result),
    }
}

fn collect_threads(
    receiver: mpsc::Receiver<ProcessEvent>,
    stdout_reader: std::thread::JoinHandle<()>,
    stderr_reader: std::thread::JoinHandle<()>,
    stdin_writer: Option<std::thread::JoinHandle<()>>,
    stdout: &mut Option<StreamCapture>,
    stderr: &mut Option<StreamCapture>,
    writer: &mut Option<Result<(), io::Error>>,
) {
    #[cfg(unix)]
    {
        let _ = stdout_reader.join();
        let _ = stderr_reader.join();
        if let Some(stdin_writer) = stdin_writer {
            let _ = stdin_writer.join();
        }
        while let Ok(event) = receiver.try_recv() {
            apply_event(event, stdout, stderr, writer);
        }
    }
    #[cfg(not(unix))]
    {
        let end = Instant::now() + READER_FALLBACK_GRACE;
        while (stdout.is_none() || stderr.is_none()) && Instant::now() < end {
            let remaining = end.saturating_duration_since(Instant::now());
            let Ok(event) = receiver.recv_timeout(remaining) else {
                break;
            };
            apply_event(event, stdout, stderr, writer);
        }
        if stdout.is_some() {
            let _ = stdout_reader.join();
        }
        if stderr.is_some() {
            let _ = stderr_reader.join();
        }
        if let Some(stdin_writer) = stdin_writer {
            if writer.is_some() {
                let _ = stdin_writer.join();
            }
        }
    }
}

fn captured_output(
    stdout: Option<StreamCapture>,
    stderr: Option<StreamCapture>,
) -> (CapturedOutput, Option<(OutputStream, io::Error)>) {
    let stdout = stdout.unwrap_or_else(empty_capture);
    let stderr = stderr.unwrap_or_else(empty_capture);
    let read_error = stdout
        .error
        .map(|error| (OutputStream::Stdout, error))
        .or_else(|| stderr.error.map(|error| (OutputStream::Stderr, error)));
    (
        CapturedOutput {
            stdout: stdout.bytes,
            stderr: stderr.bytes,
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
        },
        read_error,
    )
}

fn empty_capture() -> StreamCapture {
    StreamCapture {
        bytes: Vec::new(),
        truncated: false,
        error: None,
    }
}

fn poll_child(child: &mut Child) -> Result<Option<ExitStatus>, io::Error> {
    child.try_wait()
}

fn stop_after_missing_pipe(child: &mut Child, process_id: u32) {
    let mut status = None;
    let _ = terminate(child, process_id, &mut status);
}

#[cfg(unix)]
fn terminate(
    child: &mut Child,
    process_id: u32,
    status: &mut Option<ExitStatus>,
) -> Result<(), io::Error> {
    let mut first_error = signal_group(process_id, libc::SIGTERM).err();
    if first_error.is_none() {
        let grace_end = Instant::now() + TERMINATION_GRACE;
        while Instant::now() < grace_end {
            match group_exists(process_id) {
                Ok(false) => break,
                Ok(true) => {}
                Err(error) => {
                    first_error = Some(error);
                    break;
                }
            }
            if status.is_none() {
                match poll_child(child) {
                    Ok(result) => *status = result,
                    Err(error) => {
                        first_error = Some(error);
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    if let Err(error) = signal_group(process_id, libc::SIGKILL)
        && first_error.is_none()
    {
        first_error = Some(error);
    }
    if status.is_none() {
        if let Err(error) = child.kill()
            && error.kind() != io::ErrorKind::InvalidInput
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        match child.wait() {
            Ok(exit_status) => *status = Some(exit_status),
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {}
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(unix)]
fn signal_group(process_id: u32, signal: libc::c_int) -> Result<(), io::Error> {
    let result = unsafe { libc::kill(-(process_id as libc::pid_t), signal) };
    if result == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(unix)]
fn group_exists(process_id: u32) -> Result<bool, io::Error> {
    let result = unsafe { libc::kill(-(process_id as libc::pid_t), 0) };
    if result == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(error),
    }
}

#[cfg(not(unix))]
fn terminate(
    child: &mut Child,
    _process_id: u32,
    status: &mut Option<ExitStatus>,
) -> Result<(), io::Error> {
    if status.is_none() {
        child.kill()?;
        *status = Some(child.wait()?);
    }
    Ok(())
}

#[cfg(test)]
mod test_support {
    use super::*;

    pub fn read<R: Read>(stream: OutputStream, reader: R) -> Result<Vec<u8>, SubprocessError> {
        read_bounded(stream, reader, usize::MAX)
    }

    pub fn read_bounded<R: Read>(
        stream: OutputStream,
        reader: R,
        limit: usize,
    ) -> Result<Vec<u8>, SubprocessError> {
        let capture = read_stream(reader, limit);
        let output = match stream {
            OutputStream::Stdin => CapturedOutput::default(),
            OutputStream::Stdout => CapturedOutput {
                stdout: capture.bytes,
                stdout_truncated: capture.truncated,
                ..CapturedOutput::default()
            },
            OutputStream::Stderr => CapturedOutput {
                stderr: capture.bytes,
                stderr_truncated: capture.truncated,
                ..CapturedOutput::default()
            },
        };
        match capture.error {
            Some(source) => Err(SubprocessError::Read {
                stream,
                source,
                output,
            }),
            None => Ok(match stream {
                OutputStream::Stdin => Vec::new(),
                OutputStream::Stdout => output.stdout,
                OutputStream::Stderr => output.stderr,
            }),
        }
    }

    pub fn wait(child: &mut Child) -> Result<Option<ExitStatus>, SubprocessError> {
        poll_child(child).map_err(|source| SubprocessError::Wait {
            source,
            output: CapturedOutput::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::io::{self, Read};
    use std::path::PathBuf;
    #[cfg(unix)]
    use std::process::Command;
    use std::time::Duration;

    #[cfg(unix)]
    fn shell(script: &str) -> SubprocessRequest {
        SubprocessRequest {
            executable: PathBuf::from("/bin/sh"),
            args: vec![OsString::from("-c"), OsString::from(script)],
            current_dir: None,
            stdin: StdinMode::Inherit,
            environment: EnvironmentMode::Inherit {
                set: Vec::new(),
                remove: Vec::new(),
            },
            stdout_limit: usize::MAX,
            stderr_limit: usize::MAX,
            cancellation: None,
        }
    }

    #[test]
    fn subprocess_deadline_reports_remaining_and_expiration() {
        let deadline = Deadline::new(Duration::from_millis(20));
        assert!(!deadline.is_expired());
        assert!(deadline.remaining() <= Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(30));
        assert!(deadline.is_expired());
        assert_eq!(deadline.remaining(), Duration::ZERO);
    }

    #[test]
    fn subprocess_error_text_strips_controls_and_keeps_the_last_thousand_characters() {
        let mut bytes = vec![b'a'; 1_005];
        bytes.extend_from_slice(b"\0\n\tlast");

        let text = bounded_error_text(&bytes);
        assert_eq!(text.chars().count(), 1_000);
        assert!(text.ends_with("last"));
        assert!(!text.chars().any(char::is_control));
    }

    #[test]
    #[cfg(unix)]
    fn subprocess_drains_stdout_and_stderr_beyond_pipe_capacity() {
        let deadline = Deadline::new(Duration::from_secs(5));
        let output = run(
            shell("yes o | head -c 262144; yes e | head -c 262144 >&2"),
            Some(&deadline),
        )
        .unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 262_144);
        assert_eq!(output.stderr.len(), 262_144);
    }

    #[test]
    #[cfg(unix)]
    fn subprocess_returns_nonzero_status_with_captured_stderr() {
        let output = run(shell("printf failure >&2; exit 7"), None).unwrap();

        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stderr, b"failure");
    }

    #[test]
    fn subprocess_reports_spawn_and_read_errors() {
        let missing = SubprocessRequest {
            executable: PathBuf::from("/missing/muxy-subprocess"),
            args: Vec::new(),
            current_dir: None,
            stdin: StdinMode::Inherit,
            environment: EnvironmentMode::Inherit {
                set: Vec::new(),
                remove: Vec::new(),
            },
            stdout_limit: usize::MAX,
            stderr_limit: usize::MAX,
            cancellation: None,
        };
        assert!(matches!(run(missing, None), Err(SubprocessError::Spawn(_))));

        struct FailedReader;

        impl Read for FailedReader {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("read failed"))
            }
        }

        assert!(matches!(
            test_support::read(OutputStream::Stdout, FailedReader),
            Err(SubprocessError::Read {
                stream: OutputStream::Stdout,
                ..
            })
        ));

        struct PartialReader(bool);

        impl Read for PartialReader {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Err(io::Error::other("partial read failed"));
                }
                self.0 = true;
                buffer[..6].copy_from_slice(b"abcdef");
                Ok(6)
            }
        }

        let error =
            test_support::read_bounded(OutputStream::Stdout, PartialReader(false), 3).unwrap_err();
        let SubprocessError::Read { output, .. } = error else {
            panic!("expected read error");
        };
        assert_eq!(output.stdout, b"abc");
        assert!(output.stdout_truncated);
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_reports_wait_errors() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let mut status = 0;
        let waited = unsafe { libc::waitpid(child.id() as libc::pid_t, &mut status, 0) };
        assert_eq!(waited, child.id() as libc::pid_t);
        assert!(matches!(
            test_support::wait(&mut child),
            Err(SubprocessError::Wait { .. })
        ));
    }

    #[cfg(not(unix))]
    #[test]
    fn subprocess_non_unix_direct_child_timeout_is_reported_deterministically() {
        let deadline = Deadline::new(Duration::from_millis(100));
        #[cfg(windows)]
        let request = SubprocessRequest {
            executable: PathBuf::from("cmd.exe"),
            args: ["/C", "ping -n 30 127.0.0.1 >NUL"]
                .into_iter()
                .map(OsString::from)
                .collect(),
            current_dir: None,
            stdin: StdinMode::Inherit,
            environment: EnvironmentMode::Inherit {
                set: Vec::new(),
                remove: Vec::new(),
            },
            stdout_limit: usize::MAX,
            stderr_limit: usize::MAX,
            cancellation: None,
        };
        #[cfg(not(windows))]
        let request = SubprocessRequest {
            executable: PathBuf::from("sleep"),
            args: vec![OsString::from("30")],
            current_dir: None,
            stdin: StdinMode::Inherit,
            environment: EnvironmentMode::Inherit {
                set: Vec::new(),
                remove: Vec::new(),
            },
            stdout_limit: usize::MAX,
            stderr_limit: usize::MAX,
            cancellation: None,
        };

        assert!(matches!(
            run(request, Some(&deadline)),
            Err(SubprocessError::TimedOut { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_timeout_terminates_the_group_and_reaps_descendants() {
        let deadline = Deadline::new(Duration::from_millis(150));
        let error = run(
            shell("trap '' TERM; sleep 30 & echo $$ $!; wait"),
            Some(&deadline),
        )
        .unwrap_err();
        let SubprocessError::TimedOut { output } = error else {
            panic!("expected timeout");
        };
        let processes: Vec<libc::pid_t> = String::from_utf8(output.stdout)
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect();

        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(unsafe { libc::kill(processes[0], 0) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
        assert_eq!(unsafe { libc::kill(processes[1], 0) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
        assert_eq!(unsafe { libc::kill(-processes[0], 0) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_stops_a_descendant_holding_pipes_after_the_shell_exits() {
        let deadline = Deadline::new(Duration::from_millis(150));
        let error = run(shell("sleep 30 & echo $!"), Some(&deadline)).unwrap_err();
        let SubprocessError::TimedOut { output } = error else {
            panic!("expected timeout");
        };
        let descendant: libc::pid_t = String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(unsafe { libc::kill(descendant, 0) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
    }

    #[cfg(unix)]
    fn contracted_shell(script: &str) -> SubprocessRequest {
        SubprocessRequest {
            executable: PathBuf::from("/bin/sh"),
            args: vec![OsString::from("-c"), OsString::from(script)],
            current_dir: None,
            stdin: StdinMode::Closed,
            environment: EnvironmentMode::Replace(Vec::new()),
            stdout_limit: 1_024,
            stderr_limit: 1_024,
            cancellation: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_contracts_closed_supplied_and_inherited_stdin() {
        let closed = run(
            contracted_shell("if read value; then exit 7; else printf eof; fi"),
            None,
        )
        .unwrap();
        assert_eq!(closed.stdout, b"eof");

        let mut supplied = contracted_shell("cat");
        supplied.stdin = StdinMode::Bytes(b"supplied bytes".to_vec());
        let supplied = run(supplied, None).unwrap();
        assert_eq!(supplied.stdout, b"supplied bytes");

        let mut inherited = contracted_shell("exit 0");
        inherited.stdin = StdinMode::Inherit;
        assert!(run(inherited, None).unwrap().status.success());
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_contracts_replace_and_inherit_environment_modes() {
        let mut replaced = contracted_shell("printf '%s|%s' \"${HOME-unset}\" \"$MUXY_PHASE1\"");
        replaced.environment = EnvironmentMode::Replace(vec![(
            OsString::from("MUXY_PHASE1"),
            OsString::from("replacement"),
        )]);
        assert_eq!(run(replaced, None).unwrap().stdout, b"unset|replacement");

        let mut inherited = contracted_shell("printf '%s|%s' \"${HOME-unset}\" \"$MUXY_PHASE1\"");
        inherited.environment = EnvironmentMode::Inherit {
            set: vec![(OsString::from("MUXY_PHASE1"), OsString::from("override"))],
            remove: vec![OsString::from("HOME")],
        };
        assert_eq!(run(inherited, None).unwrap().stdout, b"unset|override");
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_contracts_bounded_capture_while_draining_both_streams() {
        let mut request =
            contracted_shell("yes stdout | head -c 262144; yes stderr | head -c 262144 >&2");
        request.stdout_limit = 137;
        request.stderr_limit = 211;
        let output = run(request, Some(&Deadline::new(Duration::from_secs(5)))).unwrap();

        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 137);
        assert_eq!(output.stderr.len(), 211);
        assert!(output.stdout_truncated);
        assert!(output.stderr_truncated);
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_contracts_early_stdin_close_and_non_utf8_output() {
        let mut early_close = contracted_shell("exit 0");
        early_close.stdin = StdinMode::Bytes(vec![b'x'; 1_048_576]);
        assert!(run(early_close, None).unwrap().status.success());

        let output = run(contracted_shell("printf '\\377\\376'"), None).unwrap();
        assert_eq!(output.stdout, [0xff, 0xfe]);
        assert!(!output.stdout_truncated);
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_contracts_cancellation_before_spawn_and_while_running() {
        let cancelled = CancellationSignal::new();
        cancelled.cancel();
        let mut before_spawn = contracted_shell("exit 0");
        before_spawn.executable = PathBuf::from("/missing/muxy-cancelled-before-spawn");
        before_spawn.cancellation = Some(cancelled);
        assert!(matches!(
            run(before_spawn, None),
            Err(SubprocessError::Cancelled { output }) if output == CapturedOutput::default()
        ));

        let cancelled = CancellationSignal::new();
        let trigger = cancelled.clone();
        let mut running = contracted_shell("printf started; trap '' TERM; sleep 30");
        running.cancellation = Some(cancelled);
        let task = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            trigger.cancel();
        });
        let error = run(running, Some(&Deadline::new(Duration::from_secs(5)))).unwrap_err();
        task.join().unwrap();
        let SubprocessError::Cancelled { output } = error else {
            panic!("expected cancellation");
        };
        assert_eq!(output.stdout, b"started");
    }

    #[cfg(unix)]
    #[test]
    fn subprocess_contracts_deadline_precedence_and_timeout_partial_bounds() {
        let cancelled = CancellationSignal::new();
        cancelled.cancel();
        let mut request = contracted_shell("exit 0");
        request.cancellation = Some(cancelled);
        let error = run(request, Some(&Deadline::new(Duration::ZERO))).unwrap_err();
        assert!(matches!(error, SubprocessError::TimedOut { .. }));

        let mut request =
            contracted_shell("yes stdout | head -c 4096; yes stderr | head -c 4096 >&2; sleep 30");
        request.stdout_limit = 31;
        request.stderr_limit = 47;
        let error = run(request, Some(&Deadline::new(Duration::from_millis(100)))).unwrap_err();
        let SubprocessError::TimedOut { output } = error else {
            panic!("expected timeout");
        };
        assert_eq!(output.stdout.len(), 31);
        assert_eq!(output.stderr.len(), 47);
        assert!(output.stdout_truncated);
        assert!(output.stderr_truncated);
    }
}
