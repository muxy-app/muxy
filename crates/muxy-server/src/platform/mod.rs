//! Operating-system differences in how the server starts and stops processes.

#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) mod windows;

use std::io;
use std::process::{Child, Command};

/// A one-off command and every process it starts: a process group on Unix,
/// a job object on Windows.
#[derive(Debug)]
pub(crate) struct ProcessGroup {
    #[cfg(unix)]
    leader: u32,
    #[cfg(windows)]
    job: windows::Job,
}

impl ProcessGroup {
    /// Starts `command` as the first process of a new group. On Windows it
    /// never opens a console window.
    pub(crate) fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let child = command.process_group(0).spawn()?;
            let leader = child.id();
            Ok((child, Self { leader }))
        }
        #[cfg(windows)]
        {
            let job = windows::Job::new()?;
            let child = windows::spawn_in(&job, command)?;
            Ok((child, Self { job }))
        }
    }

    /// Asks every process to stop: `SIGTERM` on Unix. Windows has no such
    /// request for programs without a window, so there it ends them.
    pub(crate) fn terminate(&self) {
        #[cfg(unix)]
        self.signal(rustix::process::Signal::TERM);
        #[cfg(windows)]
        self.job.terminate();
    }

    /// Ends every process now.
    pub(crate) fn kill(&self) {
        #[cfg(unix)]
        self.signal(rustix::process::Signal::KILL);
        #[cfg(windows)]
        self.job.terminate();
    }

    #[cfg(unix)]
    fn signal(&self, signal: rustix::process::Signal) {
        if let Some(pid) = rustix::process::Pid::from_raw(self.leader.cast_signed()) {
            let _ = rustix::process::kill_process_group(pid, signal);
        }
    }
}
