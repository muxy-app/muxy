//! Windows runs each command in its own job object, which stands in for a
//! process group. Anonymous pipes can't be waited on together with a
//! process, so threads read and write them and report to one channel.

use std::io::{self, Read, Write};
use std::os::windows::io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::ptr;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CREATE_SUSPENDED, INFINITE, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME,
    WaitForSingleObject,
};

use super::{CANCEL_CHECK, CHUNK, Cap, Captured, DRAIN, Limits, Output};

/// Learns when a child exits, without reaping it.
#[derive(Debug)]
pub struct ExitWatch(OwnedHandle);

impl ExitWatch {
    /// `child` must not have been reaped yet.
    pub fn new(child: &Child) -> io::Result<Self> {
        child.as_handle().try_clone_to_owned().map(Self)
    }

    /// Waits up to `timeout`, or for ever with `None`. Returns whether the
    /// child has exited.
    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<bool> {
        let milliseconds = timeout.map_or(INFINITE, |timeout| {
            u32::try_from(timeout.as_nanos().div_ceil(1_000_000))
                .unwrap_or(INFINITE - 1)
                .min(INFINITE - 1)
        });
        // SAFETY: the process handle is open.
        match unsafe { WaitForSingleObject(self.0.as_raw_handle(), milliseconds) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

/// A command started by [`Running::start`]. Dropping it before
/// [`Running::wait`] kills its job.
#[derive(Debug)]
pub struct Running<'a> {
    child: Child,
    group: Group,
    limits: Limits<'a>,
    started: Instant,
}

impl<'a> Running<'a> {
    /// Starts `command` in a new job, without a console window, so every
    /// process it starts joins the job too.
    pub fn start(command: &mut Command, limits: Limits<'a>) -> io::Result<Self> {
        let job = Job::new()?;
        let mut child = command
            .stdin(if limits.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW)
            .spawn()?;
        let joined = job.assign(&child).and_then(|()| resume_threads(child.id()));
        if let Err(error) = joined {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok(Self {
            group: Group(Arc::new(Mutex::new(Some(job)))),
            child,
            limits,
            started: Instant::now(),
        })
    }

    pub fn group(&self) -> Group {
        self.group.clone()
    }

    /// Feeds stdin and collects output until the command exits and its
    /// output closes, or a limit stops it.
    pub fn wait(mut self) -> io::Result<Output> {
        let limits = &self.limits;
        let (sender, events) = mpsc::channel();
        let stdout = self.child.stdout.take();
        let mut stdout = Stream::new(stdout, limits.stdout, Source::Stdout, &sender)?;
        let stderr = self.child.stderr.take();
        let mut stderr = Stream::new(stderr, limits.stderr, Source::Stderr, &sender)?;
        if let (Some(stdin), Some(bytes)) = (self.child.stdin.take(), limits.stdin) {
            feed(stdin, bytes.to_vec())?;
        }
        let mut exit = ExitWatch::new(&self.child)?;
        thread::Builder::new()
            .name("child-exit".into())
            .spawn(move || {
                let _ = exit.wait(None);
                let _ = sender.send(Event::Exited);
            })?;
        let deadline = self.started.checked_add(limits.timeout);
        let mut stopped = false;
        let mut timed_out = false;
        let mut cancelled = false;
        let mut exited = None;
        loop {
            let now = Instant::now();
            let running = exited.is_none() || !limits.kill_leftovers;
            if running && !stopped {
                cancelled = limits.cancelled.is_some_and(|cancelled| cancelled());
                timed_out = !cancelled && deadline.is_some_and(|deadline| now >= deadline);
                if cancelled || timed_out {
                    self.group.kill();
                    stopped = true;
                }
            }
            let drain = exited
                .filter(|_| limits.kill_leftovers || stopped)
                .map(|exited: Instant| exited + DRAIN);
            if exited.is_some()
                && (stdout.closed && stderr.closed || drain.is_some_and(|end| now >= end))
            {
                break;
            }
            let limited = running && !stopped;
            let wake = [
                deadline.filter(|_| limited),
                drain,
                (limited && limits.cancelled.is_some()).then(|| now + CANCEL_CHECK),
            ]
            .into_iter()
            .flatten()
            .min();
            let event = match wake {
                Some(wake) => match events.recv_timeout(wake.saturating_duration_since(now)) {
                    Ok(event) => Some(event),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                None => match events.recv() {
                    Ok(event) => Some(event),
                    Err(_) => break,
                },
            };
            let overflowed = match event {
                Some(Event::Output(Source::Stdout, chunk)) => stdout.take(chunk),
                Some(Event::Output(Source::Stderr, chunk)) => stderr.take(chunk),
                Some(Event::Exited) if exited.is_none() => {
                    exited = Some(Instant::now());
                    if limits.kill_leftovers {
                        self.group.kill();
                    }
                    false
                }
                Some(Event::Exited) | None => false,
            };
            if overflowed && !stopped {
                self.group.kill();
                stopped = true;
            }
        }
        self.group.release();
        Ok(Output {
            status: self.child.wait()?,
            stdout: stdout.captured,
            stderr: stderr.captured,
            timed_out,
            cancelled,
        })
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.group.kill();
        self.group.release();
        let _ = self.child.wait();
    }
}

/// A started command's job object, until the command is reaped.
#[derive(Clone, Debug)]
pub struct Group(Arc<Mutex<Option<Job>>>);

impl Group {
    /// Kills every process in the job.
    pub fn kill(&self) {
        if let Some(job) = &*self.lock() {
            job.terminate();
        }
    }

    fn release(&self) {
        *self.lock() = None;
    }

    fn lock(&self) -> MutexGuard<'_, Option<Job>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl PartialEq for Group {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Debug)]
struct Job(OwnedHandle);

impl Job {
    fn new() -> io::Result<Self> {
        // SAFETY: null attributes and name create an unnamed, uninheritable job.
        let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the job handle is new and owned here.
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(job) }))
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        // SAFETY: both handles are open.
        if unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), child.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn terminate(&self) {
        // SAFETY: the job handle is open.
        unsafe { TerminateJobObject(self.0.as_raw_handle(), 1) };
    }
}

/// Resumes the first thread of a process created suspended.
fn resume_threads(pid: u32) -> io::Result<()> {
    // SAFETY: CreateToolhelp32Snapshot has no memory arguments.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the snapshot handle is new and owned here.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).map_err(io::Error::other)?,
        ..THREADENTRY32::default()
    };
    let mut resumed = false;
    // SAFETY: `entry` is a live structure whose size field is set.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: OpenThread has no memory arguments.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if thread.is_null() {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: the thread handle is new and owned here.
            let thread = unsafe { OwnedHandle::from_raw_handle(thread) };
            // SAFETY: the thread handle is open with suspend and resume access.
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            resumed = true;
        }
        // SAFETY: as for Thread32First.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    }
    if resumed {
        Ok(())
    } else {
        Err(io::Error::other("the new process has no thread to resume"))
    }
}

#[derive(Clone, Copy)]
enum Source {
    Stdout,
    Stderr,
}

enum Event {
    /// A chunk of output, or `None` once the stream ends.
    Output(Source, Option<Vec<u8>>),
    Exited,
}

/// An output stream that a thread reads, and what was kept of it.
struct Stream {
    cap: Cap,
    closed: bool,
    captured: Captured,
}

impl Stream {
    fn new(
        pipe: Option<impl Read + Send + 'static>,
        cap: Cap,
        source: Source,
        events: &Sender<Event>,
    ) -> io::Result<Self> {
        let Some(mut pipe) = pipe else {
            return Ok(Self {
                cap,
                closed: true,
                captured: Captured::default(),
            });
        };
        let events = events.clone();
        thread::Builder::new()
            .name("child-output".into())
            .spawn(move || {
                let mut buffer = vec![0; CHUNK];
                loop {
                    let chunk = match pipe.read(&mut buffer) {
                        Ok(0) => None,
                        Ok(read) => Some(buffer[..read].to_vec()),
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => None,
                    };
                    let ended = chunk.is_none();
                    if events.send(Event::Output(source, chunk)).is_err() || ended {
                        return;
                    }
                }
            })?;
        Ok(Self {
            cap,
            closed: false,
            captured: Captured::default(),
        })
    }

    /// Keeps what the cap allows of `chunk`, or closes on `None`. Returns
    /// whether the stream passed a cap that stops the command.
    fn take(&mut self, chunk: Option<Vec<u8>>) -> bool {
        let Some(chunk) = chunk else {
            self.closed = true;
            return false;
        };
        let kept = &mut self.captured.bytes;
        let take = chunk.len().min(self.cap.bytes().saturating_sub(kept.len()));
        kept.extend_from_slice(&chunk[..take]);
        self.captured.truncated |= take < chunk.len();
        self.captured.truncated && matches!(self.cap, Cap::StopAt(_))
    }
}

/// Writes `bytes` to stdin on a thread, then closes it. A command that stops
/// reading closes the pipe, which ends the thread.
fn feed(mut stdin: impl Write + Send + 'static, bytes: Vec<u8>) -> io::Result<()> {
    thread::Builder::new()
        .name("child-input".into())
        .spawn(move || {
            let _ = stdin.write_all(&bytes);
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::super::run;
    use super::*;

    fn cmd(script: &str) -> Command {
        let mut command = Command::new("cmd.exe");
        command.args(["/d", "/c", script]);
        command
    }

    #[test]
    fn collects_output_and_the_exit_status() {
        let output = run(
            &mut cmd("echo out& echo err 1>&2& exit 3"),
            Limits::new(Duration::from_secs(10), 1024),
        )
        .unwrap();
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(output.stdout.bytes, b"out\r\n");
        assert_eq!(output.stderr.bytes, b"err \r\n");
        assert!(!output.timed_out && !output.cancelled);
    }

    #[test]
    fn stdin_is_written_while_output_is_read() {
        let output = run(
            &mut cmd("findstr x"),
            Limits {
                stdin: Some(b"a\r\nx1\r\nb\r\nx2\r\n"),
                ..Limits::new(Duration::from_secs(10), 1024)
            },
        )
        .unwrap();
        assert_eq!(output.stdout.bytes, b"x1\r\nx2\r\n");
    }

    #[test]
    fn timeout_kills_the_whole_job() {
        let started = Instant::now();
        let output = run(
            &mut cmd("ping -n 30 127.0.0.1 >nul"),
            Limits::new(Duration::from_millis(300), 1024),
        )
        .unwrap();
        assert!(output.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn output_past_a_stopping_cap_stops_the_command() {
        let output = run(
            &mut cmd("for /l %i in (1,1,100000) do @echo line %i"),
            Limits::new(Duration::from_secs(30), 100),
        )
        .unwrap();
        assert_eq!(output.stdout.bytes.len(), 100);
        assert!(output.stdout.truncated);
        assert!(!output.status.success());
    }

    #[test]
    fn leftovers_holding_the_output_are_killed_when_asked() {
        let started = Instant::now();
        let output = run(
            &mut cmd("start /b ping -n 30 127.0.0.1 & echo done"),
            Limits {
                kill_leftovers: true,
                ..Limits::new(Duration::from_secs(20), 1024)
            },
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout.bytes).contains("done"));
        assert!(!output.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
