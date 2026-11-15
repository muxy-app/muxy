pub mod bundle;
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "posix_spawn with a disclaimed responsible process"
)]
pub mod host;
#[cfg(windows)]
#[allow(unsafe_code, reason = "Win32 process and token queries")]
mod windows;

use crate::{Client, ClientError};
use muxy_core::file_lock::{self, FileLock};
use muxy_protocol::transport::ByteStream;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn server_executable() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("MUXY_SERVER_BIN") {
        if path.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MUXY_SERVER_BIN must not be empty",
            ));
        }
        return Ok(path.into());
    }
    Ok(muxy_core::executable::sibling(
        &muxy_core::executable::current_path()?,
        "muxy-server",
    ))
}

pub fn connect(socket: &Path) -> Result<Box<dyn ByteStream>, ClientError> {
    let stream = muxy_protocol::transport::connect_local(socket)?;
    #[cfg(unix)]
    if !muxy_protocol::transport::peer_is_same_user(&stream)? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "the server at {} runs as another user, so it was not used",
                socket.display()
            ),
        )
        .into());
    }
    #[cfg(windows)]
    windows::check_server(&stream)?;
    Ok(Box::new(stream))
}

pub fn ensure_running(socket: &Path, executable: &Path) -> Result<Client, ClientError> {
    start_and_connect(socket, executable, spawn, Client::connect_with_timeout)
}

pub fn ensure_running_hosted(socket: &Path, executable: &Path) -> Result<Client, ClientError> {
    start_and_connect(
        socket,
        executable,
        spawn_hosted,
        Client::connect_with_timeout,
    )
}

pub fn ensure_listening(
    socket: &Path,
    executable: &Path,
) -> Result<Box<dyn ByteStream>, ClientError> {
    start_and_connect(socket, executable, spawn, |socket, _| connect(socket))
}

fn start_and_connect<T>(
    socket: &Path,
    executable: &Path,
    launch: fn(&Path, &[OsString]) -> io::Result<()>,
    connect: impl Fn(&Path, Duration) -> Result<T, ClientError>,
) -> Result<T, ClientError> {
    let _startup = wait_for_startup_lock(socket)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    match connect(socket, deadline.saturating_duration_since(Instant::now())) {
        Ok(connected) => return Ok(connected),
        Err(error) if unavailable(&error) => {}
        Err(error) => return Err(error),
    }
    launch(executable, &server_arguments(socket)).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("could not launch server {}: {error}", executable.display()),
        )
    })?;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ClientError::Timeout);
        }
        match connect(socket, remaining) {
            Ok(connected) => return Ok(connected),
            Err(error) if unavailable(&error) => {}
            Err(error) => return Err(error),
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(25)),
        );
    }
}

fn server_arguments(socket: &Path) -> [OsString; 6] {
    let socket = std::path::absolute(socket).unwrap_or_else(|_| socket.to_owned());
    [
        "--socket".into(),
        socket.clone().into(),
        "--settings".into(),
        socket.with_file_name("server.toml").into(),
        "--log".into(),
        socket.with_file_name("server.log").into(),
    ]
}

fn spawn(executable: &Path, arguments: &[OsString]) -> io::Result<()> {
    let mut child = spawn_detached(executable, arguments)?;
    thread::Builder::new()
        .name("muxy-server-wait".into())
        .spawn(move || {
            let _ = child.wait();
        })?;
    Ok(())
}

#[cfg(unix)]
pub type Detached = std::process::Child;
#[cfg(windows)]
pub use windows::Detached;

pub fn spawn_detached(program: &Path, arguments: &[OsString]) -> io::Result<Detached> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Command::new(program)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
    }
    #[cfg(windows)]
    {
        windows::spawn_detached(program, arguments)
    }
}

fn spawn_hosted(executable: &Path, arguments: &[OsString]) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    if let Some(app) = host::app_for(executable)? {
        std::fs::metadata(executable)?;
        return host::spawn(&app, arguments);
    }
    spawn(executable, arguments)
}

pub fn unavailable(error: &ClientError) -> bool {
    matches!(error, ClientError::Io(error) if matches!(error.kind(), io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound))
}

const STARTUP_BUSY: &str = "A server startup or update is already in progress";

fn wait_for_startup_lock(socket: &Path) -> io::Result<FileLock> {
    file_lock::acquire(&startup_lock(socket)?, Duration::from_secs(3))
        .map_err(file_lock::busy(STARTUP_BUSY))
}

pub fn lock_startup(socket: &Path) -> io::Result<FileLock> {
    try_lock(&startup_lock(socket)?).map_err(file_lock::busy(STARTUP_BUSY))
}

fn startup_lock(socket: &Path) -> io::Result<PathBuf> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(socket.with_extension("update-lock"))
}

pub use muxy_core::file_lock::try_lock;

pub fn read_build_info(executable: &Path) -> io::Result<muxy_protocol::BuildInfo> {
    let bytes = muxy_core::executable::build_metadata(executable)?;
    let info: muxy_protocol::BuildInfo = serde_json::from_slice(&bytes)?;
    if info.version.len() > 128 || info.compatibility == 0 {
        return Err(io::Error::other("Invalid server build metadata"));
    }
    Ok(info)
}
