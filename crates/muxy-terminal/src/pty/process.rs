use std::ffi::OsString;
use std::fmt;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::io::RawFd;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::io::RawHandle;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

#[cfg(unix)]
use portable_pty::MasterPty;
use portable_pty::{ChildKiller, CommandBuilder, PtyPair, native_pty_system};

#[cfg(windows)]
use crate::pty::conpty::{self, Console};
use crate::pty::error::{PtyError, PtyStep};
use crate::pty::reader::{PtyEvent, ReaderHandle, spawn_reader};

#[cfg(unix)]
type Master = Box<dyn MasterPty + Send>;
#[cfg(windows)]
type Master = Console;

/// On Unix, `std`'s child keeps the number of the signal that ended it.
#[cfg(unix)]
type ChildProcess = std::process::Child;
#[cfg(windows)]
type ChildProcess = Box<dyn portable_pty::Child + Send + Sync>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
}

impl From<crate::Size> for PtySize {
    fn from(size: crate::Size) -> Self {
        Self {
            cols: size.cols,
            rows: size.rows,
        }
    }
}

fn window_size(size: PtySize, cell: crate::CellSize) -> portable_pty::PtySize {
    portable_pty::PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: size.cols.saturating_mul(cell.width),
        pixel_height: size.rows.saturating_mul(cell.height),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpawnRequest {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: Vec<(OsString, OsString)>,
    pub size: PtySize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExitStatus {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

#[cfg(unix)]
impl From<std::process::ExitStatus> for ExitStatus {
    fn from(status: std::process::ExitStatus) -> Self {
        Self {
            code: status.code(),
            signal: status.signal(),
        }
    }
}

#[cfg(windows)]
impl From<portable_pty::ExitStatus> for ExitStatus {
    fn from(status: portable_pty::ExitStatus) -> Self {
        Self {
            code: Some(status.exit_code().cast_signed()),
            signal: None,
        }
    }
}

pub struct Pty {
    master: Master,
    writer: Option<Box<dyn Write + Send>>,
    child: ChildProcess,
    child_pid: u32,
    #[cfg(unix)]
    master_fd: RawFd,
    size: PtySize,
    cell: crate::CellSize,
}

impl Pty {
    pub fn spawn(request: SpawnRequest) -> Result<Self, PtyError> {
        let size = request.size;
        let cell = crate::CellSize::default();
        let PtyPair { master, slave } = native_pty_system()
            .openpty(window_size(size, cell))
            .map_err(|error| PtyError::wrap(PtyStep::Open, error))?;
        #[cfg(unix)]
        let master_fd = master
            .as_raw_fd()
            .ok_or_else(|| PtyError::wrap(PtyStep::Open, "master side has no file descriptor"))?;

        let mut command = CommandBuilder::new(request.program);
        command.args(request.args);
        command.cwd(request.cwd);
        for (key, value) in request.env {
            command.env(key, value);
        }
        let child = slave
            .spawn_command(command)
            .map_err(|error| PtyError::wrap(PtyStep::Spawn, error))?;
        drop(slave);
        let (child, child_pid) = into_process(child)?;
        #[cfg(windows)]
        let master = Console::new(master);

        let writer = master
            .take_writer()
            .map_err(|error| PtyError::wrap(PtyStep::TakeWriter, error))?;

        Ok(Self {
            master,
            writer: Some(writer),
            child,
            child_pid,
            #[cfg(unix)]
            master_fd,
            size,
            cell,
        })
    }

    pub fn start_reader<T>(&self, sink: Sender<T>) -> Result<ReaderHandle, PtyError>
    where
        T: From<PtyEvent> + Send + 'static,
    {
        let reader = self
            .master
            .try_clone_reader()
            .map_err(|error| PtyError::wrap(PtyStep::CloneReader, error))?;
        #[cfg(windows)]
        self.master
            .close_on_exit(&*self.child)
            .map_err(|error| PtyError::new(PtyStep::WatchExit, error))?;
        Ok(spawn_reader(reader, sink))
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), PtyError> {
        self.writer
            .as_mut()
            .ok_or_else(|| PtyError::wrap(PtyStep::Write, "PTY writer has been taken"))?
            .write_all(bytes)
            .map_err(|error| PtyError::new(PtyStep::Write, error))
    }

    pub fn take_writer(&mut self) -> Result<Box<dyn Write + Send>, PtyError> {
        self.writer
            .take()
            .ok_or_else(|| PtyError::wrap(PtyStep::TakeWriter, "PTY writer has been taken"))
    }

    pub fn resize(&mut self, size: PtySize) -> Result<(), PtyError> {
        self.set_window_size(size, self.cell)
    }

    pub fn set_cell_size(&mut self, cell: crate::CellSize) -> Result<(), PtyError> {
        self.set_window_size(self.size, cell)
    }

    fn set_window_size(&mut self, size: PtySize, cell: crate::CellSize) -> Result<(), PtyError> {
        self.master
            .resize(window_size(size, cell))
            .map_err(|error| PtyError::wrap(PtyStep::Resize, error))?;
        self.size = size;
        self.cell = cell;
        Ok(())
    }

    #[cfg(unix)]
    pub fn try_wait(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten().map(ExitStatus::from)
    }

    #[cfg(windows)]
    pub fn try_wait(&mut self) -> Option<ExitStatus> {
        if !conpty::has_exited(&*self.child) {
            return None;
        }
        self.child.wait().ok().map(ExitStatus::from)
    }

    pub fn wait(&mut self) -> Result<ExitStatus, PtyError> {
        self.child
            .wait()
            .map(ExitStatus::from)
            .map_err(|error| PtyError::new(PtyStep::Wait, error))
    }

    pub fn kill(&mut self) -> Result<(), PtyError> {
        #[cfg(unix)]
        let child = &mut self.child;
        #[cfg(windows)]
        let child = &mut *self.child;
        ChildKiller::kill(child).map_err(|error| PtyError::new(PtyStep::Kill, error))
    }

    pub fn child_pid(&self) -> u32 {
        self.child_pid
    }

    #[cfg(unix)]
    pub fn foreground_pid(&self) -> Option<u32> {
        self.master
            .process_group_leader()
            .and_then(|pid| u32::try_from(pid).ok())
    }

    /// Windows has no foreground process group to ask for.
    #[cfg(windows)]
    #[allow(clippy::unused_self, reason = "Matches the Unix signature")]
    pub fn foreground_pid(&self) -> Option<u32> {
        None
    }

    #[cfg(unix)]
    pub fn master_fd(&self) -> RawFd {
        self.master_fd
    }

    /// The shell's process handle, open for as long as this `Pty` lives.
    #[cfg(windows)]
    pub fn process_handle(&self) -> Option<RawHandle> {
        self.child.as_raw_handle()
    }
}

#[cfg(unix)]
fn into_process(
    child: Box<dyn portable_pty::Child + Send + Sync>,
) -> Result<(ChildProcess, u32), PtyError> {
    let child: Box<dyn portable_pty::Child> = child;
    let child = child
        .downcast::<std::process::Child>()
        .map_err(|_| PtyError::wrap(PtyStep::Spawn, "child is not a process handle"))?;
    let pid = child.id();
    Ok((*child, pid))
}

#[cfg(windows)]
fn into_process(
    child: Box<dyn portable_pty::Child + Send + Sync>,
) -> Result<(ChildProcess, u32), PtyError> {
    let pid = child
        .process_id()
        .ok_or_else(|| PtyError::wrap(PtyStep::Spawn, "child has no process id"))?;
    Ok((child, pid))
}

impl fmt::Debug for Pty {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("Pty");
        debug.field("child_pid", &self.child_pid);
        #[cfg(unix)]
        debug.field("master_fd", &self.master_fd);
        debug.finish_non_exhaustive()
    }
}
