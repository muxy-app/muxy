//! Linux and Android wait with poll(2) on the child's exit and its pipes.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::process::Child;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;
use rustix::process::{Pid, WaitId, WaitIdOptions};

use super::{Ready, Watch};

/// Learns when a child exits, without reaping it.
///
/// It is a pidfd, or where there is none (Android, or a sandbox that refuses
/// it), a pipe that a thread closes once the child exits. Either becomes
/// readable then.
#[derive(Debug)]
pub struct ExitWatch(OwnedFd);

impl ExitWatch {
    /// `child` must not have been reaped yet.
    pub fn new(child: &Child) -> io::Result<Self> {
        let pid = Pid::from_child(child);
        #[cfg(target_os = "linux")]
        match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
            Ok(pidfd) => return Ok(Self(pidfd)),
            Err(Errno::NOSYS | Errno::PERM) => {}
            Err(error) => return Err(error.into()),
        }
        Self::waiter(pid)
    }

    fn waiter(pid: Pid) -> io::Result<Self> {
        let (exited, closer) = io::pipe()?;
        std::thread::Builder::new()
            .name("child-exit".into())
            .spawn(move || {
                let options = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT;
                while matches!(
                    rustix::process::waitid(WaitId::Pid(pid), options),
                    Err(Errno::INTR)
                ) {}
                drop(closer);
            })?;
        Ok(Self(exited.into()))
    }

    /// Waits up to `timeout`, or for ever with `None`. Returns whether the
    /// child has exited.
    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<bool> {
        let deadline = timeout.and_then(|timeout| Instant::now().checked_add(timeout));
        loop {
            let left = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
            let mut descriptors = [PollFd::new(&self.0, PollFlags::IN)];
            match poll(&mut descriptors, left) {
                Ok(()) => return Ok(!descriptors[0].revents().is_empty()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }
}

pub(super) struct Events(ExitWatch);

impl Events {
    pub(super) fn new(child: &Child, _: &Watch<'_>) -> io::Result<Self> {
        ExitWatch::new(child).map(Self)
    }

    pub(super) fn wait(
        &mut self,
        watch: &Watch<'_>,
        timeout: Option<Duration>,
    ) -> io::Result<Ready> {
        let wanted: [(Option<BorrowedFd<'_>>, PollFlags); 4] = [
            (watch.exit.then(|| self.0.0.as_fd()), PollFlags::IN),
            (watch.stdout, PollFlags::IN),
            (watch.stderr, PollFlags::IN),
            (watch.stdin, PollFlags::OUT),
        ];
        let mut descriptors = Vec::with_capacity(wanted.len());
        let mut slots = Vec::with_capacity(wanted.len());
        for (slot, (fd, flags)) in wanted.into_iter().enumerate() {
            if let Some(fd) = fd {
                descriptors.push(PollFd::from_borrowed_fd(fd, flags));
                slots.push(slot);
            }
        }
        let mut hit = [false; 4];
        match poll(&mut descriptors, timeout) {
            Ok(()) => {
                for (descriptor, slot) in descriptors.iter().zip(slots) {
                    // Hang-ups and errors count: the next read or write reports them.
                    hit[slot] = !descriptor.revents().is_empty();
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
        let [exited, stdout, stderr, stdin] = hit;
        Ok(Ready {
            exited,
            stdout,
            stderr,
            stdin,
        })
    }
}

fn poll(descriptors: &mut [PollFd<'_>], timeout: Option<Duration>) -> io::Result<()> {
    let timeout = timeout
        .map(Timespec::try_from)
        .transpose()
        .map_err(io::Error::other)?;
    rustix::event::poll(descriptors, timeout.as_ref())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Tests fail immediately on fixture errors"
    )]
    use std::process::Command;

    use super::*;

    #[test]
    fn the_waiter_thread_sees_the_exit_without_reaping() {
        let mut child = Command::new("sleep").arg("0.2").spawn().unwrap();
        let mut exit = ExitWatch::waiter(Pid::from_child(&child)).unwrap();
        assert!(!exit.wait(Some(Duration::ZERO)).unwrap());
        assert!(exit.wait(None).unwrap());
        assert!(
            child
                .try_wait()
                .unwrap()
                .is_some_and(|status| status.success())
        );
    }
}
