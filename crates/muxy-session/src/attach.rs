use std::env;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use muxy_proto::session::{
    AttachExisting, AttachRequest, EnvironmentEntry, LaunchSpecification, OwnerMetadata, Resize,
    SessionMessage,
};
use thiserror::Error;

use crate::security;
use crate::{WireError, read_message, write_message};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const IO_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum AttachError {
    #[error("missing or invalid attach environment: {0}")]
    Environment(&'static str),
    #[error("failed to start the session daemon")]
    DaemonSpawn(#[source] io::Error),
    #[error("session daemon did not become ready")]
    DaemonUnavailable,
    #[error("session attach failed: {0}")]
    Rejected(String),
    #[error(transparent)]
    Security(#[from] security::SecurityError),
    #[error(transparent)]
    Wire(#[from] WireError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

struct AttachConfiguration {
    socket: PathBuf,
    message: SessionMessage,
    daemon_idle_ms: Option<u64>,
}

pub fn run() -> Result<(), AttachError> {
    let configuration = AttachConfiguration::from_environment()?;
    security::validate_socket_path(&configuration.socket)?;
    let mut stream = connect_or_spawn(&configuration)?;
    write_message(&mut stream, &configuration.message)?;
    match read_message(&mut stream)? {
        SessionMessage::Attached(_) => {}
        SessionMessage::ProtocolError(error) => {
            return Err(AttachError::Rejected(format!(
                "{}: {}",
                error.code, error.message
            )));
        }
        _ => {
            return Err(AttachError::Rejected(
                "unexpected attach response".to_owned(),
            ));
        }
    }
    let _terminal_mode = TerminalMode::raw(libc::STDIN_FILENO)?;
    let writer = Arc::new(Mutex::new(stream.try_clone()?));
    spawn_input_pump(writer.clone());
    spawn_resize_pump(writer);
    let mut stdout = io::stdout().lock();
    loop {
        match read_message(&mut stream)? {
            SessionMessage::Replay(bytes) | SessionMessage::Output(bytes) => {
                stdout.write_all(&bytes)?;
                stdout.flush()?;
            }
            SessionMessage::Exited(status) => exit_with_status(status),
            SessionMessage::ProtocolError(error) => {
                return Err(AttachError::Rejected(format!(
                    "{}: {}",
                    error.code, error.message
                )));
            }
            _ => {
                return Err(AttachError::Rejected(
                    "unexpected session response".to_owned(),
                ));
            }
        }
    }
}

impl AttachConfiguration {
    fn from_environment() -> Result<Self, AttachError> {
        let socket = required_path("MUXY_SESSION_SOCKET")?;
        let session_id = required("MUXY_SESSION_ID")?;
        let size = terminal_size(libc::STDIN_FILENO).unwrap_or(Resize {
            columns: 80,
            rows: 24,
            width_px: 0,
            height_px: 0,
        });
        let message = match env::var("MUXY_SESSION_CREATE_POLICY").as_deref() {
            Ok("existing") => SessionMessage::AttachExisting(AttachExisting { session_id, size }),
            Ok("create-or-attach") => {
                let environment = env::vars()
                    .filter(|(key, _)| !key.starts_with("MUXY_SESSION_"))
                    .take(muxy_proto::session::MAX_ENVIRONMENT_ENTRIES)
                    .map(|(key, value)| EnvironmentEntry { key, value })
                    .collect();
                SessionMessage::AttachCreateOrAttach(AttachRequest {
                    session_id,
                    owner: OwnerMetadata {
                        project_id: required("MUXY_SESSION_PROJECT_ID")?,
                        worktree_id: optional("MUXY_SESSION_WORKTREE_ID"),
                        title: required("MUXY_SESSION_TITLE")?,
                    },
                    launch: LaunchSpecification {
                        shell: required("MUXY_SESSION_SHELL")?,
                        resources_directory: required("MUXY_SESSION_RESOURCES")?,
                        working_directory: required("MUXY_SESSION_DIRECTORY")?,
                        startup_command: optional("MUXY_SESSION_STARTUP_COMMAND"),
                        environment,
                    },
                    size,
                })
            }
            _ => return Err(AttachError::Environment("MUXY_SESSION_CREATE_POLICY")),
        };
        muxy_proto::session::SessionCodec::encode(&message)
            .map_err(|_| AttachError::Environment("bounded attach payload"))?;
        let daemon_idle_ms = optional("MUXY_SESSION_DAEMON_IDLE_MS")
            .map(|value| value.parse::<u64>())
            .transpose()
            .map_err(|_| AttachError::Environment("MUXY_SESSION_DAEMON_IDLE_MS"))?;
        Ok(Self {
            socket,
            message,
            daemon_idle_ms,
        })
    }
}

fn connect_or_spawn(configuration: &AttachConfiguration) -> Result<UnixStream, AttachError> {
    if let Ok(stream) = UnixStream::connect(&configuration.socket) {
        return Ok(stream);
    }
    let executable = env::current_exe()?;
    let mut command = Command::new(executable);
    command
        .arg("daemon")
        .arg("--socket")
        .arg(&configuration.socket)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(milliseconds) = configuration.daemon_idle_ms {
        command.arg("--idle-ms").arg(milliseconds.to_string());
    }
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    command.spawn().map_err(AttachError::DaemonSpawn)?;
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    loop {
        match UnixStream::connect(&configuration.socket) {
            Ok(stream) => return Ok(stream),
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return Err(AttachError::DaemonUnavailable),
        }
    }
}

fn spawn_input_pump(writer: Arc<Mutex<UnixStream>>) {
    std::thread::spawn(move || {
        let mut input = io::stdin().lock();
        let mut bytes = [0u8; IO_CHUNK_BYTES];
        loop {
            let length = match input.read(&mut bytes) {
                Ok(0) | Err(_) => return,
                Ok(length) => length,
            };
            if write_message(
                &mut lock(&writer),
                &SessionMessage::Input(bytes[..length].to_vec()),
            )
            .is_err()
            {
                return;
            }
        }
    });
}

fn spawn_resize_pump(writer: Arc<Mutex<UnixStream>>) {
    std::thread::spawn(move || {
        let mut previous = terminal_size(libc::STDIN_FILENO);
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let current = terminal_size(libc::STDIN_FILENO);
            if current.is_none() || current == previous {
                continue;
            }
            previous = current;
            if write_message(
                &mut lock(&writer),
                &SessionMessage::Resize(current.expect("checked as present")),
            )
            .is_err()
            {
                return;
            }
        }
    });
}

fn terminal_size(fd: i32) -> Option<Resize> {
    let mut size = unsafe { std::mem::zeroed::<libc::winsize>() };
    if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, &mut size) } != 0
        || size.ws_col == 0
        || size.ws_row == 0
    {
        return None;
    }
    Some(Resize {
        columns: size.ws_col,
        rows: size.ws_row,
        width_px: u32::from(size.ws_xpixel),
        height_px: u32::from(size.ws_ypixel),
    })
}

fn exit_with_status(status: muxy_proto::session::ExitStatus) -> ! {
    if let Some(signal) = status.signal {
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
            libc::raise(signal);
        }
        std::process::exit(128 + signal.clamp(1, 127));
    }
    std::process::exit(status.code.unwrap_or(1).clamp(0, 255));
}

fn required(name: &'static str) -> Result<String, AttachError> {
    env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or(AttachError::Environment(name))
}

fn required_path(name: &'static str) -> Result<PathBuf, AttachError> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(AttachError::Environment(name))
}

fn optional(name: &'static str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct TerminalMode {
    fd: i32,
    original: libc::termios,
}

impl TerminalMode {
    fn raw(fd: i32) -> io::Result<Option<Self>> {
        let mut original = unsafe { std::mem::zeroed::<libc::termios>() };
        if unsafe { libc::tcgetattr(fd, &mut original) } != 0 {
            if io::Error::last_os_error().raw_os_error() == Some(libc::ENOTTY) {
                return Ok(None);
            }
            return Err(io::Error::last_os_error());
        }
        let mut raw = original;
        unsafe { libc::cfmakeraw(&mut raw) };
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Some(Self { fd, original }))
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        unsafe { libc::tcsetattr(self.fd, libc::TCSANOW, &self.original) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_size_rejects_non_tty_descriptors() {
        let file = tempfile::tempfile().unwrap();
        assert_eq!(terminal_size(std::os::fd::AsRawFd::as_raw_fd(&file)), None);
    }

    #[test]
    fn signal_and_exit_statuses_map_to_bounded_process_outcomes() {
        let code = muxy_proto::session::ExitStatus {
            code: Some(42),
            signal: None,
        };
        assert_eq!(code.code.unwrap().clamp(0, 255), 42);
        let signal = muxy_proto::session::ExitStatus {
            code: None,
            signal: Some(15),
        };
        assert_eq!(128 + signal.signal.unwrap().clamp(1, 127), 143);
    }
}
