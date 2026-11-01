//! Running a child process with a time limit and bounded output, and waiting
//! for one to exit. The exit and the pipes wake the same wait, so nothing
//! sleeps on a timer.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal};

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
use linux as platform;
#[cfg(target_vendor = "apple")]
mod macos;
#[cfg(target_vendor = "apple")]
use macos as platform;

pub use platform::ExitWatch;

/// How long output is still read after the command exits, while helpers it
/// started keep its pipes open.
const DRAIN: Duration = Duration::from_millis(250);
/// Nothing wakes the wait when a command is cancelled, so it looks this often.
const CANCEL_CHECK: Duration = Duration::from_millis(25);
const CHUNK: usize = 16 * 1024;

/// How much of an output stream is kept.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cap {
    /// Keeps this many bytes and stops the command once it writes more.
    StopAt(usize),
    /// Keeps this many bytes and discards the rest while the command runs on.
    TruncateAt(usize),
}

impl Cap {
    fn bytes(self) -> usize {
        match self {
            Self::StopAt(bytes) | Self::TruncateAt(bytes) => bytes,
        }
    }
}

/// What a command may do before it is stopped.
pub struct Limits<'a> {
    pub timeout: Duration,
    pub stdout: Cap,
    pub stderr: Cap,
    /// Written to a piped stdin, which then closes. `None` reads `/dev/null`.
    pub stdin: Option<&'a [u8]>,
    /// Stops the command once it returns true.
    pub cancelled: Option<&'a dyn Fn() -> bool>,
    /// How long a command being stopped gets between TERM and KILL. Zero
    /// sends KILL at once.
    pub term_grace: Duration,
    /// Kills whatever the command left running in its process group once it
    /// exits, instead of waiting for those to close its output.
    pub kill_leftovers: bool,
}

impl std::fmt::Debug for Limits<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Limits")
            .field("timeout", &self.timeout)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .field("stdin_bytes", &self.stdin.map(<[u8]>::len))
            .field("cancellable", &self.cancelled.is_some())
            .field("term_grace", &self.term_grace)
            .field("kill_leftovers", &self.kill_leftovers)
            .finish()
    }
}

impl Limits<'_> {
    /// Stops the command after `timeout`, or once stdout or stderr passes
    /// `cap` bytes.
    pub fn new(timeout: Duration, cap: usize) -> Self {
        Self {
            timeout,
            stdout: Cap::StopAt(cap),
            stderr: Cap::StopAt(cap),
            stdin: None,
            cancelled: None,
            term_grace: Duration::ZERO,
            kill_leftovers: false,
        }
    }
}

/// The part of an output stream that was kept.
#[derive(Debug, Default)]
pub struct Captured {
    pub bytes: Vec<u8>,
    /// The stream wrote more than its cap.
    pub truncated: bool,
}

#[derive(Debug)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: Captured,
    pub stderr: Captured,
    pub timed_out: bool,
    pub cancelled: bool,
}

/// Runs `command` in its own process group with piped output, until it exits
/// or a limit stops it.
pub fn run(command: &mut Command, limits: Limits<'_>) -> io::Result<Output> {
    Running::start(command, limits)?.wait()
}

/// Waits up to `timeout` for `child` to exit, and reaps it if it did.
pub fn wait_timeout(child: &mut Child, timeout: Duration) -> io::Result<Option<ExitStatus>> {
    if let Some(status) = child.try_wait()? {
        return Ok(Some(status));
    }
    ExitWatch::new(child)?.wait(Some(timeout))?;
    child.try_wait()
}

/// A command started by [`Running::start`]. Dropping it before
/// [`Running::wait`] kills its process group.
#[derive(Debug)]
pub struct Running<'a> {
    child: Child,
    group: Group,
    limits: Limits<'a>,
    started: Instant,
}

impl<'a> Running<'a> {
    pub fn start(command: &mut Command, limits: Limits<'a>) -> io::Result<Self> {
        let child = command
            .stdin(if limits.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()?;
        Ok(Self {
            group: Group::of(&child),
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
        let mut stdout = Pipe::new(self.child.stdout.take(), limits.stdout)?;
        let mut stderr = Pipe::new(self.child.stderr.take(), limits.stderr)?;
        let mut stdin = Input::new(self.child.stdin.take(), limits.stdin.unwrap_or_default())?;
        let deadline = self.started.checked_add(limits.timeout);
        let mut events =
            platform::Events::new(&self.child, &watch(None, &stdout, &stderr, &stdin))?;
        let mut stop = Stop::default();
        let mut exited = None;
        loop {
            let now = Instant::now();
            // Leftovers killed at the exit leave nothing to stop.
            let running = exited.is_none() || !limits.kill_leftovers;
            if running && !stop.begun() {
                if limits.cancelled.is_some_and(|cancelled| cancelled()) {
                    stop.cancelled = true;
                } else if deadline.is_some_and(|deadline| now >= deadline) {
                    stop.timed_out = true;
                }
                if stop.cancelled || stop.timed_out {
                    stop.begin(&self.group, limits.term_grace, now);
                }
            }
            stop.escalate(&self.group, limits.term_grace, now);
            let drain = exited
                .filter(|_| limits.kill_leftovers || stop.begun())
                .map(|exited: Instant| exited + DRAIN);
            if exited.is_some()
                && (stdout.closed() && stderr.closed() || drain.is_some_and(|end| now >= end))
            {
                stop.kill_now(&self.group);
                break;
            }
            let limited = running && !stop.begun();
            let wake = [
                deadline.filter(|_| limited),
                stop.kill_at(limits.term_grace),
                drain,
                (limited && limits.cancelled.is_some()).then(|| now + CANCEL_CHECK),
            ]
            .into_iter()
            .flatten()
            .min();
            let ready = events.wait(
                &watch(exited, &stdout, &stderr, &stdin),
                wake.map(|wake| wake.saturating_duration_since(now)),
            )?;
            let mut overflowed = false;
            if ready.stdout {
                overflowed |= stdout.read()?;
            }
            if ready.stderr {
                overflowed |= stderr.read()?;
            }
            if ready.stdin {
                stdin.write();
            }
            if overflowed {
                stop.begin(&self.group, limits.term_grace, Instant::now());
            }
            if ready.exited && exited.is_none() {
                exited = Some(Instant::now());
                stdin.close();
                if limits.kill_leftovers {
                    self.group.signal(Signal::KILL);
                }
            }
        }
        self.group.release();
        Ok(Output {
            status: self.child.wait()?,
            stdout: stdout.captured,
            stderr: stderr.captured,
            timed_out: stop.timed_out,
            cancelled: stop.cancelled,
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

/// A started command's process group. Signals reach it only until its leader
/// is reaped, after which its ID may belong to another process.
#[derive(Clone, Debug)]
pub struct Group(Arc<Mutex<Option<Pid>>>);

impl Group {
    fn of(child: &Child) -> Self {
        Self(Arc::new(Mutex::new(Some(Pid::from_child(child)))))
    }

    /// Kills every process in the group.
    pub fn kill(&self) {
        self.signal(Signal::KILL);
    }

    fn signal(&self, signal: Signal) {
        if let Some(pid) = *self.lock() {
            let _ = rustix::process::kill_process_group(pid, signal);
        }
    }

    /// Called just before the leader is reaped.
    fn release(&self) {
        *self.lock() = None;
    }

    fn lock(&self) -> MutexGuard<'_, Option<Pid>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl PartialEq for Group {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Why and since when a command is being stopped.
#[derive(Default)]
struct Stop {
    since: Option<Instant>,
    killed: bool,
    timed_out: bool,
    cancelled: bool,
}

impl Stop {
    fn begun(&self) -> bool {
        self.since.is_some()
    }

    fn begin(&mut self, group: &Group, grace: Duration, now: Instant) {
        if self.since.is_some() {
            return;
        }
        self.since = Some(now);
        if grace.is_zero() {
            group.signal(Signal::KILL);
            self.killed = true;
        } else {
            group.signal(Signal::TERM);
        }
    }

    fn kill_at(&self, grace: Duration) -> Option<Instant> {
        self.since
            .filter(|_| !self.killed)
            .and_then(|since| since.checked_add(grace))
    }

    fn escalate(&mut self, group: &Group, grace: Duration, now: Instant) {
        if self.kill_at(grace).is_some_and(|at| now >= at) {
            self.kill_now(group);
        }
    }

    /// A stopped command's group still gets KILL when the run ends before
    /// its grace period does.
    fn kill_now(&mut self, group: &Group) {
        if self.begun() && !self.killed {
            group.signal(Signal::KILL);
            self.killed = true;
        }
    }
}

/// The descriptors still worth waiting on, and whether the exit still is.
struct Watch<'a> {
    exit: bool,
    stdout: Option<BorrowedFd<'a>>,
    stderr: Option<BorrowedFd<'a>>,
    stdin: Option<BorrowedFd<'a>>,
}

fn watch<'a>(
    exited: Option<Instant>,
    stdout: &'a Pipe,
    stderr: &'a Pipe,
    stdin: &'a Input,
) -> Watch<'a> {
    Watch {
        exit: exited.is_none(),
        stdout: stdout.file.as_ref().map(AsFd::as_fd),
        stderr: stderr.file.as_ref().map(AsFd::as_fd),
        stdin: stdin.file.as_ref().map(AsFd::as_fd),
    }
}

#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one flag for each thing a wait reports"
)]
struct Ready {
    exited: bool,
    stdout: bool,
    stderr: bool,
    stdin: bool,
}

/// An output stream, closed once it ends.
struct Pipe {
    file: Option<File>,
    cap: Cap,
    captured: Captured,
}

impl Pipe {
    fn new(pipe: Option<impl Into<OwnedFd>>, cap: Cap) -> io::Result<Self> {
        let file = pipe.map(|pipe| File::from(pipe.into()));
        if let Some(file) = &file {
            rustix::io::ioctl_fionbio(file, true)?;
        }
        Ok(Self {
            file,
            cap,
            captured: Captured::default(),
        })
    }

    fn closed(&self) -> bool {
        self.file.is_none()
    }

    /// Reads what is ready. Returns whether the stream passed a cap that
    /// stops the command.
    fn read(&mut self) -> io::Result<bool> {
        let Some(file) = &mut self.file else {
            return Ok(false);
        };
        let mut buffer = [0; CHUNK];
        match file.read(&mut buffer) {
            Ok(0) => self.file = None,
            Ok(read) => {
                let kept = &mut self.captured.bytes;
                let take = read.min(self.cap.bytes().saturating_sub(kept.len()));
                kept.extend_from_slice(&buffer[..take]);
                self.captured.truncated |= take < read;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
        Ok(self.captured.truncated && matches!(self.cap, Cap::StopAt(_)))
    }
}

/// The bytes still to write to stdin. It closes once they are written, or
/// once the command stops reading.
struct Input<'a> {
    file: Option<File>,
    bytes: &'a [u8],
}

impl<'a> Input<'a> {
    fn new(pipe: Option<impl Into<OwnedFd>>, bytes: &'a [u8]) -> io::Result<Self> {
        let file = pipe
            .filter(|_| !bytes.is_empty())
            .map(|pipe| File::from(pipe.into()));
        if let Some(file) = &file {
            rustix::io::ioctl_fionbio(file, true)?;
        }
        Ok(Self { file, bytes })
    }

    fn write(&mut self) {
        let Some(file) = &mut self.file else {
            return;
        };
        match file.write(self.bytes) {
            Ok(written) => self.bytes = &self.bytes[written..],
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => self.bytes = &[],
        }
        if self.bytes.is_empty() {
            self.close();
        }
    }

    fn close(&mut self) {
        self.file = None;
    }
}

#[cfg(test)]
mod tests;
