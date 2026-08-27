use std::ffi::OsString;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
#[cfg(unix)]
const TERMINATION_GRACE: Duration = Duration::from_millis(100);
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

#[derive(Clone, Debug)]
pub struct SubprocessRequest {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    pub environment: Vec<(OsString, OsString)>,
}

#[derive(Debug)]
pub struct SubprocessOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

impl std::fmt::Display for OutputStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
    },
    #[error("could not wait for subprocess: {0}")]
    Wait(io::Error),
    #[error("could not terminate subprocess: {0}")]
    Terminate(io::Error),
    #[error("subprocess {0} reader stopped unexpectedly")]
    ReaderStopped(OutputStream),
    #[error("subprocess timed out")]
    TimedOut { stdout: Vec<u8>, stderr: Vec<u8> },
}

pub fn run(
    request: SubprocessRequest,
    deadline: Option<&Deadline>,
) -> Result<SubprocessOutput, SubprocessError> {
    let mut command = Command::new(request.executable);
    command
        .args(request.args)
        .envs(request.environment)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(current_dir) = request.current_dir {
        command.current_dir(current_dir);
    }
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn().map_err(SubprocessError::Spawn)?;
    let process_id = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or(SubprocessError::MissingPipe(OutputStream::Stdout))?;
    let stderr = child
        .stderr
        .take()
        .ok_or(SubprocessError::MissingPipe(OutputStream::Stderr))?;
    let (sender, receiver) = mpsc::channel();
    let stdout_reader = spawn_reader(sender.clone(), OutputStream::Stdout, stdout);
    let stderr_reader = spawn_reader(sender, OutputStream::Stderr, stderr);

    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    let mut timed_out = false;
    loop {
        receive_readers(&receiver, &mut stdout, &mut stderr);
        if status.is_none() {
            status = poll_child(&mut child)?;
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            break;
        }
        if deadline.is_some_and(Deadline::is_expired) {
            timed_out = true;
            terminate(&mut child, process_id, &mut status)?;
            break;
        }
        let delay = deadline.map_or(POLL_INTERVAL, |deadline| {
            POLL_INTERVAL.min(deadline.remaining())
        });
        std::thread::sleep(delay.max(Duration::from_millis(1)));
    }

    if timed_out {
        #[cfg(unix)]
        {
            receive_until_closed(&receiver, &mut stdout, &mut stderr)?;
            join_reader(stdout_reader, OutputStream::Stdout)?;
            join_reader(stderr_reader, OutputStream::Stderr)?;
        }
        #[cfg(not(unix))]
        {
            let reader_deadline = Instant::now() + READER_FALLBACK_GRACE;
            while (stdout.is_none() || stderr.is_none()) && Instant::now() < reader_deadline {
                let remaining = reader_deadline.saturating_duration_since(Instant::now());
                let Ok((stream, result)) = receiver.recv_timeout(remaining) else {
                    break;
                };
                match stream {
                    OutputStream::Stdout => stdout = Some(result),
                    OutputStream::Stderr => stderr = Some(result),
                }
            }
            if stdout.is_some() {
                join_reader(stdout_reader, OutputStream::Stdout)?;
            } else {
                drop(stdout_reader);
                stdout = Some(Ok(Vec::new()));
            }
            if stderr.is_some() {
                join_reader(stderr_reader, OutputStream::Stderr)?;
            } else {
                drop(stderr_reader);
                stderr = Some(Ok(Vec::new()));
            }
        }
    } else {
        join_reader(stdout_reader, OutputStream::Stdout)?;
        join_reader(stderr_reader, OutputStream::Stderr)?;
        receive_until_closed(&receiver, &mut stdout, &mut stderr)?;
    }

    let stdout = reader_result(stdout, OutputStream::Stdout)?;
    let stderr = reader_result(stderr, OutputStream::Stderr)?;
    if timed_out {
        return Err(SubprocessError::TimedOut { stdout, stderr });
    }
    Ok(SubprocessOutput {
        status: status.ok_or_else(|| SubprocessError::Wait(io::Error::other("missing status")))?,
        stdout,
        stderr,
    })
}

type ReaderResult = Result<Vec<u8>, SubprocessError>;

fn spawn_reader<R: Read + Send + 'static>(
    sender: mpsc::Sender<(OutputStream, ReaderResult)>,
    stream: OutputStream,
    reader: R,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let _ = sender.send((stream, read_stream(stream, reader)));
    })
}

fn read_stream<R: Read>(stream: OutputStream, mut reader: R) -> ReaderResult {
    let mut output = Vec::new();
    reader
        .read_to_end(&mut output)
        .map_err(|source| SubprocessError::Read { stream, source })?;
    Ok(output)
}

fn receive_readers(
    receiver: &mpsc::Receiver<(OutputStream, ReaderResult)>,
    stdout: &mut Option<ReaderResult>,
    stderr: &mut Option<ReaderResult>,
) {
    while let Ok((stream, result)) = receiver.try_recv() {
        match stream {
            OutputStream::Stdout => *stdout = Some(result),
            OutputStream::Stderr => *stderr = Some(result),
        }
    }
}

fn receive_until_closed(
    receiver: &mpsc::Receiver<(OutputStream, ReaderResult)>,
    stdout: &mut Option<ReaderResult>,
    stderr: &mut Option<ReaderResult>,
) -> Result<(), SubprocessError> {
    while stdout.is_none() || stderr.is_none() {
        let (stream, result) = receiver.recv().map_err(|_| {
            SubprocessError::ReaderStopped(if stdout.is_none() {
                OutputStream::Stdout
            } else {
                OutputStream::Stderr
            })
        })?;
        match stream {
            OutputStream::Stdout => *stdout = Some(result),
            OutputStream::Stderr => *stderr = Some(result),
        }
    }
    Ok(())
}

fn reader_result(
    result: Option<ReaderResult>,
    stream: OutputStream,
) -> Result<Vec<u8>, SubprocessError> {
    result.ok_or(SubprocessError::ReaderStopped(stream))?
}

fn join_reader(
    reader: std::thread::JoinHandle<()>,
    stream: OutputStream,
) -> Result<(), SubprocessError> {
    reader
        .join()
        .map_err(|_| SubprocessError::ReaderStopped(stream))
}

fn poll_child(child: &mut Child) -> Result<Option<ExitStatus>, SubprocessError> {
    child.try_wait().map_err(SubprocessError::Wait)
}

#[cfg(unix)]
fn terminate(
    child: &mut Child,
    process_id: u32,
    status: &mut Option<ExitStatus>,
) -> Result<(), SubprocessError> {
    signal_group(process_id, libc::SIGTERM)?;
    let grace_end = Instant::now() + TERMINATION_GRACE;
    while Instant::now() < grace_end {
        if status.is_none() {
            *status = poll_child(child)?;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    signal_group(process_id, libc::SIGKILL)?;
    if status.is_none() {
        *status = Some(child.wait().map_err(SubprocessError::Wait)?);
    }
    Ok(())
}

#[cfg(unix)]
fn signal_group(process_id: u32, signal: libc::c_int) -> Result<(), SubprocessError> {
    let result = unsafe { libc::kill(-(process_id as libc::pid_t), signal) };
    if result == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(SubprocessError::Terminate(error))
    }
}

#[cfg(not(unix))]
fn terminate(
    child: &mut Child,
    _process_id: u32,
    status: &mut Option<ExitStatus>,
) -> Result<(), SubprocessError> {
    if status.is_none() {
        child.kill().map_err(SubprocessError::Terminate)?;
        *status = Some(child.wait().map_err(SubprocessError::Wait)?);
    }
    Ok(())
}

#[cfg(test)]
mod test_support {
    use super::*;

    pub fn read<R: Read>(stream: OutputStream, reader: R) -> ReaderResult {
        read_stream(stream, reader)
    }

    pub fn wait(child: &mut Child) -> Result<Option<ExitStatus>, SubprocessError> {
        poll_child(child)
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
            environment: Vec::new(),
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
            environment: Vec::new(),
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
            Err(SubprocessError::Wait(_))
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
            environment: Vec::new(),
        };
        #[cfg(not(windows))]
        let request = SubprocessRequest {
            executable: PathBuf::from("sleep"),
            args: vec![OsString::from("30")],
            current_dir: None,
            environment: Vec::new(),
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
        let SubprocessError::TimedOut { stdout, .. } = error else {
            panic!("expected timeout");
        };
        let processes: Vec<libc::pid_t> = String::from_utf8(stdout)
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
        let SubprocessError::TimedOut { stdout, .. } = error else {
            panic!("expected timeout");
        };
        let descendant: libc::pid_t = String::from_utf8(stdout).unwrap().trim().parse().unwrap();

        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(unsafe { libc::kill(descendant, 0) }, -1);
        assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
    }
}
