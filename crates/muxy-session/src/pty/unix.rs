use crate::process_tree;
use crate::shell::{ShellInvocation, invocation};
use muxy_proto::session::{CreateSessionRequest, ProcessIdentity, WindowSize};
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct PseudoTerminal {
    master: File,
    pub child: ProcessIdentity,
    pub process_group_id: u32,
    pub process_session_id: u32,
    pub tty_device: u64,
}

impl PseudoTerminal {
    pub fn spawn(request: &CreateSessionRequest, socket_path: &Path) -> io::Result<Self> {
        request
            .validate()
            .map_err(|error| invalid(error.to_string()))?;
        validate_launch_paths(request)?;
        let launch = launch_invocation(request);
        let executable = CString::new(launch.executable.as_bytes())
            .map_err(|_| invalid("executable contains NUL"))?;
        let arguments: Vec<CString> = launch
            .arguments
            .iter()
            .map(|value| {
                CString::new(value.as_bytes()).map_err(|_| invalid("argument contains NUL"))
            })
            .collect::<io::Result<_>>()?;
        let environment = launch_environment(request, socket_path, launch)?;
        let environment: Vec<CString> = environment
            .into_iter()
            .map(|(key, value)| {
                CString::new(format!("{key}={value}"))
                    .map_err(|_| invalid("environment contains NUL"))
            })
            .collect::<io::Result<_>>()?;
        let working_directory = CString::new(request.working_directory.as_bytes())
            .map_err(|_| invalid("working directory contains NUL"))?;
        let mut argument_pointers: Vec<*const libc::c_char> =
            arguments.iter().map(|value| value.as_ptr()).collect();
        argument_pointers.push(std::ptr::null());
        let mut environment_pointers: Vec<*const libc::c_char> =
            environment.iter().map(|value| value.as_ptr()).collect();
        environment_pointers.push(std::ptr::null());
        let mut master_fd = -1;
        let mut size = libc::winsize {
            ws_row: request.initial_size.rows,
            ws_col: request.initial_size.columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let child = unsafe {
            libc::forkpty(
                &raw mut master_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &raw mut size,
            )
        };
        if child < 0 {
            return Err(io::Error::last_os_error());
        }
        if child == 0 {
            child_exec(
                &executable,
                &argument_pointers,
                &environment_pointers,
                &working_directory,
            );
        }
        let master = unsafe { File::from_raw_fd(master_fd) };
        let child_id = u32::try_from(child).map_err(|_| invalid("child PID is invalid"))?;
        if let Err(error) = set_nonblocking(master.as_raw_fd()) {
            terminate_spawned_child(child);
            return Err(error);
        }
        let record = match wait_for_pty_record(child_id, master.as_raw_fd()) {
            Ok(record) => record,
            Err(error) => {
                terminate_spawned_child(child);
                return Err(error);
            }
        };
        Ok(Self {
            master,
            child: record.identity,
            process_group_id: record.process_group_id,
            process_session_id: record.process_session_id,
            tty_device: record.tty_device,
        })
    }

    pub fn resize(&self, size: WindowSize) -> io::Result<()> {
        size.validate()
            .map_err(|error| invalid(error.to_string()))?;
        let window = libc::winsize {
            ws_row: size.rows,
            ws_col: size.columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &window) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let foreground = unsafe { libc::tcgetpgrp(self.master.as_raw_fd()) };
        if foreground > 0 {
            unsafe {
                libc::kill(-foreground, libc::SIGWINCH);
            }
        }
        Ok(())
    }

    pub fn write_all_nonblocking(&mut self, bytes: &[u8]) -> io::Result<()> {
        let mut remaining = bytes;
        let deadline = Instant::now() + Duration::from_secs(2);
        while !remaining.is_empty() {
            match self.master.write(remaining) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "PTY write returned zero",
                    ));
                }
                Ok(count) => remaining = &remaining[count..],
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    pub fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.master.read(bytes)
    }

    pub fn wait_status(&self) -> io::Result<Option<i32>> {
        let mut status = 0;
        let result = unsafe {
            libc::waitpid(
                libc::pid_t::try_from(self.child.process_id)
                    .map_err(|_| invalid("child PID is invalid"))?,
                &raw mut status,
                libc::WNOHANG,
            )
        };
        if result == 0 {
            return Ok(None);
        }
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ECHILD) {
                return Ok(Some(-1));
            }
            return Err(error);
        }
        let exit = if libc::WIFEXITED(status) {
            libc::WEXITSTATUS(status)
        } else if libc::WIFSIGNALED(status) {
            128 + libc::WTERMSIG(status)
        } else {
            -1
        };
        Ok(Some(exit))
    }
}

fn validate_launch_paths(request: &CreateSessionRequest) -> io::Result<()> {
    for (path, kind) in [
        (&request.working_directory, "working directory"),
        (&request.ghostty_resources, "Ghostty resources"),
        (&request.terminfo, "terminfo"),
    ] {
        let metadata = std::fs::metadata(path).map_err(|error| {
            io::Error::new(error.kind(), format!("{kind} is unavailable: {error}"))
        })?;
        if !metadata.is_dir() {
            return Err(invalid(format!("{kind} is not a directory")));
        }
    }
    let executable = std::fs::metadata(&request.shell_executable)?;
    if !executable.is_file() {
        return Err(invalid("shell executable is not a file"));
    }
    Ok(())
}

fn launch_invocation(request: &CreateSessionRequest) -> ShellInvocation {
    if let Some(command) = request.startup_command.as_deref() {
        if request.keep_shell_open {
            let shell = quote_shell_word(&request.shell_executable);
            let arguments = request
                .argv
                .iter()
                .map(|value| quote_shell_word(value))
                .collect::<Vec<_>>()
                .join(" ");
            let tail = if arguments.is_empty() {
                format!("exec {shell}")
            } else {
                format!("exec {shell} {arguments}")
            };
            let command = format!("{command}; {tail}");
            let mut value = invocation(
                "",
                &request.shell_executable,
                &request.ghostty_resources,
                &request.environment,
            );
            value.executable = "/bin/sh".into();
            value.arguments = vec!["/bin/sh".into(), "-c".into(), command];
            value
        } else {
            invocation(
                command,
                &request.shell_executable,
                &request.ghostty_resources,
                &request.environment,
            )
        }
    } else {
        let mut value = invocation(
            "",
            &request.shell_executable,
            &request.ghostty_resources,
            &request.environment,
        );
        value.arguments.extend(request.argv.iter().cloned());
        value
    }
}

fn launch_environment(
    request: &CreateSessionRequest,
    socket_path: &Path,
    invocation: ShellInvocation,
) -> io::Result<Vec<(String, String)>> {
    let mut entries: Vec<(String, String)> = invocation
        .environment
        .into_iter()
        .filter(|entry| {
            !matches!(
                entry.key.as_str(),
                "MUXY_PANE_ID"
                    | "MUXY_PROJECT_ID"
                    | "MUXY_WORKTREE_ID"
                    | "MUXY_SOCKET_PATH"
                    | "MUXY_HOOK_BIN"
                    | "MUXY_HOOK_SCRIPT"
            ) && !entry.key.starts_with("MUXY_SESSION_")
        })
        .map(|entry| (entry.key, entry.value))
        .collect();
    set_entry(&mut entries, "MUXY_PANE_ID", &request.owner.original_tab_id);
    set_entry(&mut entries, "MUXY_PROJECT_ID", &request.owner.project_id);
    set_entry(&mut entries, "MUXY_WORKTREE_ID", &request.owner.worktree_id);
    set_entry(
        &mut entries,
        "MUXY_SESSION_ID",
        &request.session_id.uppercase(),
    );
    set_entry(
        &mut entries,
        "MUXY_SESSION_SOCKET",
        socket_path
            .to_str()
            .ok_or_else(|| invalid("session socket path is not UTF-8"))?,
    );
    set_entry(
        &mut entries,
        "GHOSTTY_RESOURCES_DIR",
        &request.ghostty_resources,
    );
    set_entry(&mut entries, "TERMINFO", &request.terminfo);
    set_entry(&mut entries, "TERM", &request.terminal_type);
    set_entry(&mut entries, "COLORTERM", &request.color_terminal);
    Ok(entries)
}

fn set_entry(entries: &mut Vec<(String, String)>, key: &str, value: &str) {
    entries.retain(|(held, _)| held != key);
    entries.push((key.to_owned(), value.to_owned()));
}

fn quote_shell_word(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn wait_for_pty_record(
    process_id: u32,
    master_fd: libc::c_int,
) -> io::Result<process_tree::ProcessRecord> {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match process_tree::record(process_id) {
            Ok(record)
                if record.parent_process_id == std::process::id()
                    && record.process_group_id == process_id
                    && record.process_session_id == process_id
                    && record.tty_device != 0
                    && unsafe { libc::tcgetpgrp(master_fd) }
                        == libc::pid_t::try_from(process_id)
                            .map_err(|_| invalid("child PID is invalid"))? =>
            {
                return Ok(record);
            }
            Ok(_) | Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(_) => return Err(io::Error::other("PTY child identity did not stabilize")),
            Err(error) => return Err(error),
        }
    }
}

fn terminate_spawned_child(child: libc::pid_t) {
    unsafe {
        libc::kill(child, libc::SIGKILL);
        let mut status = 0;
        libc::waitpid(child, &raw mut status, 0);
    }
}

fn set_nonblocking(fd: libc::c_int) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn child_exec(
    executable: &CString,
    arguments: &[*const libc::c_char],
    environment: &[*const libc::c_char],
    working_directory: &CString,
) -> ! {
    unsafe {
        for signal in [
            libc::SIGCHLD,
            libc::SIGHUP,
            libc::SIGINT,
            libc::SIGQUIT,
            libc::SIGPIPE,
            libc::SIGTERM,
            libc::SIGTSTP,
            libc::SIGTTIN,
            libc::SIGTTOU,
            libc::SIGWINCH,
        ] {
            libc::signal(signal, libc::SIG_DFL);
        }
        if libc::chdir(working_directory.as_ptr()) != 0 {
            libc::_exit(126);
        }
        let maximum = libc::sysconf(libc::_SC_OPEN_MAX).clamp(3, 65_536);
        for fd in 3..maximum {
            libc::close(fd as libc::c_int);
        }
        libc::execve(
            executable.as_ptr(),
            arguments.as_ptr(),
            environment.as_ptr(),
        );
        libc::_exit(127);
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
