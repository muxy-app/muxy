//! Apple platforms wait with one kqueue for the child's exit and its pipes.

use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::process::Child;
use std::time::{Duration, Instant};

use rustix::buffer::Buffer;
use rustix::event::kqueue::{self, Event, EventFilter, EventFlags, ProcessEvents};
use rustix::io::Errno;
use rustix::process::{Pid, WaitId, WaitIdOptions};

use super::{Ready, Watch};

/// Learns when a child exits, without reaping it.
#[derive(Debug)]
pub struct ExitWatch {
    queue: OwnedFd,
    exited: bool,
}

impl ExitWatch {
    /// `child` must not have been reaped yet.
    pub fn new(child: &Child) -> io::Result<Self> {
        let pid = Pid::from_child(child);
        let queue = kqueue::kqueue()?;
        let exit = EventFilter::Proc {
            pid,
            flags: ProcessEvents::EXIT,
        };
        let exited = match register(&queue, &[event(exit, EventFlags::ADD)]) {
            // The event only covers an exit after this point.
            Ok(()) => exited_unreaped(pid)?,
            Err(Errno::SRCH) => true,
            Err(error) => return Err(error.into()),
        };
        Ok(Self { queue, exited })
    }

    /// Waits up to `timeout`, or for ever with `None`. Returns whether the
    /// child has exited.
    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<bool> {
        let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
        while !self.exited {
            let left = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
            let mut events = Vec::with_capacity(1);
            match next(&self.queue, &mut events, left) {
                Ok(()) => self.exited = events.iter().any(is_exit),
                Err(Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
            if events.is_empty() {
                break;
            }
        }
        Ok(self.exited)
    }
}

pub(super) struct Events {
    exit: ExitWatch,
    stdout: Option<RawFd>,
    stderr: Option<RawFd>,
    stdin: Option<RawFd>,
}

impl Events {
    /// Registers the pipes once. Closing one removes it from the queue.
    pub(super) fn new(child: &Child, watch: &Watch<'_>) -> io::Result<Self> {
        let exit = ExitWatch::new(child)?;
        let raw = |fd: Option<BorrowedFd<'_>>| fd.map(|fd| fd.as_raw_fd());
        let events = Self {
            exit,
            stdout: raw(watch.stdout),
            stderr: raw(watch.stderr),
            stdin: raw(watch.stdin),
        };
        let changes: Vec<_> = [
            events.stdout.map(EventFilter::Read),
            events.stderr.map(EventFilter::Read),
            events.stdin.map(EventFilter::Write),
        ]
        .into_iter()
        .flatten()
        .map(|filter| event(filter, EventFlags::ADD))
        .collect();
        register(&events.exit.queue, &changes)?;
        Ok(events)
    }

    pub(super) fn wait(
        &mut self,
        watch: &Watch<'_>,
        timeout: Option<Duration>,
    ) -> io::Result<Ready> {
        let open = |fd: Option<BorrowedFd<'_>>, registered: Option<RawFd>| {
            fd.map(|fd| fd.as_raw_fd())
                .filter(|fd| Some(*fd) == registered)
        };
        let stdout = open(watch.stdout, self.stdout);
        let stderr = open(watch.stderr, self.stderr);
        let stdin = open(watch.stdin, self.stdin);
        let timeout = if watch.exit && self.exit.exited {
            Some(Duration::ZERO)
        } else {
            timeout
        };
        let mut ready = Ready::default();
        let mut events = Vec::with_capacity(4);
        match next(&self.exit.queue, &mut events, timeout) {
            Ok(()) | Err(Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
        for event in &events {
            match event.filter() {
                EventFilter::Read(fd) if Some(fd) == stdout => ready.stdout = true,
                EventFilter::Read(fd) if Some(fd) == stderr => ready.stderr = true,
                EventFilter::Write(fd) if Some(fd) == stdin => ready.stdin = true,
                _ if is_exit(event) => self.exit.exited = true,
                _ => {}
            }
        }
        ready.exited = watch.exit && self.exit.exited;
        Ok(ready)
    }
}

fn event(filter: EventFilter, flags: EventFlags) -> Event {
    Event::new(filter, flags, std::ptr::null_mut())
}

fn is_exit(event: &Event) -> bool {
    matches!(event.filter(), EventFilter::Proc { .. })
}

/// Whether the child has exited, leaving it to be reaped.
fn exited_unreaped(pid: Pid) -> io::Result<bool> {
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
    Ok(rustix::process::waitid(WaitId::Pid(pid), options)?.is_some())
}

/// Adds `changes`. With no room for events, a failed change is the error.
fn register(queue: &OwnedFd, changes: &[Event]) -> rustix::io::Result<()> {
    let mut none: [Event; 0] = [];
    kevent(queue, changes, &mut none, Some(Duration::ZERO)).map(drop)
}

/// Waits up to `timeout` for as many events as `events` has room for.
fn next(
    queue: &OwnedFd,
    events: &mut Vec<Event>,
    timeout: Option<Duration>,
) -> rustix::io::Result<()> {
    kevent(queue, &[], rustix::buffer::spare_capacity(events), timeout).map(drop)
}

#[allow(
    unsafe_code,
    reason = "kevent is unsafe only because events may name descriptors"
)]
fn kevent<B: Buffer<Event>>(
    queue: &OwnedFd,
    changes: &[Event],
    events: B,
    timeout: Option<Duration>,
) -> rustix::io::Result<B::Output> {
    // SAFETY: events name only the child and the runner's own pipes. Closing
    // a pipe removes its events from every kqueue, so a reused descriptor
    // number is never mistaken for it.
    unsafe { kqueue::kevent(queue, changes, events, timeout) }
}
