#[cfg(target_os = "linux")]
use std::fs;
use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, RawFd};
use std::time::{Duration, Instant};

use muxy_proto::session::{ExitStatus, LaunchSpecification, Resize};
use thiserror::Error;

use crate::shell::{self, ShellError};

#[derive(Debug, Error)]
pub enum PtyError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Shell(#[from] ShellError),
}

pub struct PtyChild {
    pub master: File,
    pub pid: i32,
    pub tty_device: u64,
}

pub fn spawn(launch: &LaunchSpecification, size: Resize) -> Result<PtyChild, PtyError> {
    let prepared = shell::prepare(launch)?;
    let mut master: RawFd = -1;
    let mut slave: RawFd = -1;
    let mut dimensions = libc::winsize {
        ws_row: size.rows,
        ws_col: size.columns,
        ws_xpixel: u16::try_from(size.width_px).unwrap_or(u16::MAX),
        ws_ypixel: u16::try_from(size.height_px).unwrap_or(u16::MAX),
    };
    if unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dimensions,
        )
    } != 0
    {
        return Err(io::Error::last_os_error().into());
    }

    for descriptor in [master, slave] {
        let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
        if flags < 0
            || unsafe { libc::fcntl(descriptor, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0
        {
            let error = io::Error::last_os_error();
            close_fd(master);
            close_fd(slave);
            return Err(error.into());
        }
    }

    let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
    if unsafe { libc::fstat(slave, &mut stat) } != 0 {
        close_fd(master);
        close_fd(slave);
        return Err(io::Error::last_os_error().into());
    }
    let tty_device = stat.st_rdev as u64;
    let mut arguments = prepared
        .arguments
        .iter()
        .map(|value| value.as_ptr())
        .collect::<Vec<_>>();
    arguments.push(std::ptr::null());
    let mut environment = prepared
        .environment
        .iter()
        .map(|value| value.as_ptr())
        .collect::<Vec<_>>();
    environment.push(std::ptr::null());

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        close_fd(master);
        close_fd(slave);
        return Err(io::Error::last_os_error().into());
    }
    if pid == 0 {
        unsafe {
            libc::close(master);
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
            let mut empty = std::mem::zeroed::<libc::sigset_t>();
            libc::sigemptyset(&mut empty);
            libc::sigprocmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
            if libc::setsid() < 0
                || libc::ioctl(slave, libc::TIOCSCTTY.into(), 0) < 0
                || libc::dup2(slave, libc::STDIN_FILENO) < 0
                || libc::dup2(slave, libc::STDOUT_FILENO) < 0
                || libc::dup2(slave, libc::STDERR_FILENO) < 0
                || libc::chdir(prepared.working_directory.as_ptr()) < 0
            {
                libc::_exit(126);
            }
            if slave > libc::STDERR_FILENO {
                libc::close(slave);
            }
            libc::execve(
                prepared.program.as_ptr(),
                arguments.as_ptr(),
                environment.as_ptr(),
            );
            libc::_exit(127);
        }
    }
    close_fd(slave);
    let flags = unsafe { libc::fcntl(master, libc::F_GETFL) };
    if flags >= 0 {
        unsafe { libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    }
    Ok(PtyChild {
        master: unsafe { File::from_raw_fd(master) },
        pid,
        tty_device,
    })
}

pub fn wait_ready(fd: RawFd, writable: bool, timeout: Duration) -> bool {
    if fd < 0 || fd as usize >= libc::FD_SETSIZE {
        std::thread::sleep(timeout.min(Duration::from_millis(5)));
        return false;
    }
    let mut set = unsafe { std::mem::zeroed::<libc::fd_set>() };
    unsafe { libc::FD_SET(fd, &mut set) };
    let mut window = libc::timeval {
        tv_sec: timeout.as_secs() as libc::time_t,
        tv_usec: timeout.subsec_micros() as libc::suseconds_t,
    };
    let (read_set, write_set) = if writable {
        (std::ptr::null_mut(), &mut set as *mut libc::fd_set)
    } else {
        (&mut set as *mut libc::fd_set, std::ptr::null_mut())
    };
    unsafe {
        libc::select(
            fd + 1,
            read_set,
            write_set,
            std::ptr::null_mut(),
            &mut window,
        ) > 0
    }
}

pub fn resize(fd: RawFd, size: Resize) -> io::Result<()> {
    let dimensions = libc::winsize {
        ws_row: size.rows,
        ws_col: size.columns,
        ws_xpixel: u16::try_from(size.width_px).unwrap_or(u16::MAX),
        ws_ypixel: u16::try_from(size.height_px).unwrap_or(u16::MAX),
    };
    if unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &dimensions) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn wait_for_child(pid: i32) -> ExitStatus {
    let mut status = 0;
    loop {
        let result = unsafe { libc::waitpid(pid, &mut status, 0) };
        if result == pid {
            if libc::WIFEXITED(status) {
                return ExitStatus {
                    code: Some(libc::WEXITSTATUS(status)),
                    signal: None,
                };
            }
            if libc::WIFSIGNALED(status) {
                return ExitStatus {
                    code: None,
                    signal: Some(libc::WTERMSIG(status)),
                };
            }
        } else if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return ExitStatus {
                code: Some(1),
                signal: None,
            };
        }
    }
}

pub fn terminate_session(session_id: i32) -> bool {
    if signal_until_empty(
        session_id,
        &[libc::SIGHUP, libc::SIGTERM],
        Duration::from_secs(2),
    ) {
        return true;
    }
    for _ in 0..3 {
        if signal_until_empty(session_id, &[libc::SIGKILL], Duration::from_secs(2)) {
            return true;
        }
    }
    session_members(session_id).is_empty()
}

fn signal_until_empty(session_id: i32, signals: &[i32], timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let members = session_members(session_id);
        if members.is_empty() {
            return true;
        }
        for process_id in members {
            for signal in signals {
                unsafe { libc::kill(process_id, *signal) };
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub fn session_members(session_id: i32) -> Vec<i32> {
    process_ids()
        .into_iter()
        .filter(|process_id| *process_id != unsafe { libc::getpid() })
        .filter(|process_id| unsafe { libc::getsid(*process_id) } == session_id)
        .collect()
}

#[cfg(target_os = "macos")]
fn process_ids() -> Vec<i32> {
    const PROC_ALL_PIDS: u32 = 1;
    const MAX_PROCESS_BYTES: usize = 4 * 1024 * 1024;

    let required = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    let Ok(required) = usize::try_from(required) else {
        return Vec::new();
    };
    if required == 0 || required > MAX_PROCESS_BYTES {
        return Vec::new();
    }
    let mut process_ids = vec![0i32; required.div_ceil(std::mem::size_of::<i32>())];
    let Ok(buffer_size) = i32::try_from(process_ids.len() * std::mem::size_of::<i32>()) else {
        return Vec::new();
    };
    let received = unsafe {
        libc::proc_listpids(
            PROC_ALL_PIDS,
            0,
            process_ids.as_mut_ptr().cast(),
            buffer_size,
        )
    };
    let Ok(received) = usize::try_from(received) else {
        return Vec::new();
    };
    process_ids.truncate((received / std::mem::size_of::<i32>()).min(process_ids.len()));
    process_ids.retain(|process_id| *process_id > 0);
    process_ids
}

#[cfg(target_os = "linux")]
fn process_ids() -> Vec<i32> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_string_lossy().parse::<i32>().ok())
        .filter(|process_id| *process_id > 0)
        .collect()
}

fn close_fd(fd: RawFd) {
    if fd >= 0 {
        unsafe { libc::close(fd) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_is_visible_but_never_returned_as_a_target_member() {
        let current = unsafe { libc::getpid() };
        let session = unsafe { libc::getsid(current) };
        assert!(session > 0);
        assert!(!session_members(session).contains(&current));
    }
}
