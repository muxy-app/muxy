pub mod bundle;
#[cfg(target_os = "macos")]
#[allow(
    unsafe_code,
    reason = "posix_spawn with a disclaimed responsible process"
)]
pub mod host;

use crate::{Client, ClientError};
use muxy_protocol::transport::ByteStream;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
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
    Ok(muxy_core::executable::current_path()?.with_file_name("muxy-server"))
}

pub fn ensure_running(socket: &Path, executable: &Path) -> Result<Client, ClientError> {
    start_and_connect(socket, executable, spawn, Client::connect_with_timeout)
}

/// Like [`ensure_running`], but a packaged app starts the server through a
/// `host`, so terminals keep the app's macOS privacy permissions after the
/// app quits. Only for the app, whose `main` runs the host.
pub fn ensure_running_hosted(socket: &Path, executable: &Path) -> Result<Client, ClientError> {
    start_and_connect(
        socket,
        executable,
        spawn_hosted,
        Client::connect_with_timeout,
    )
}

/// Like [`ensure_running`], but returns the socket without a handshake, so a
/// bridge can leave version negotiation to the client at its other end.
pub fn ensure_listening(
    socket: &Path,
    executable: &Path,
) -> Result<Box<dyn ByteStream>, ClientError> {
    start_and_connect(socket, executable, spawn, |socket, _| {
        Ok(muxy_protocol::transport::connect(socket)?)
    })
}

/// Connects, first starting the server if nothing listens on `socket`.
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
    [
        "--socket".into(),
        socket.into(),
        "--settings".into(),
        socket.with_file_name("server.toml").into(),
        "--log".into(),
        socket.with_file_name("server.log").into(),
    ]
}

fn spawn(executable: &Path, arguments: &[OsString]) -> io::Result<()> {
    let mut child = Command::new(executable)
        .args(arguments)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    thread::Builder::new()
        .name("muxy-server-wait".into())
        .spawn(move || {
            let _ = child.wait();
        })?;
    Ok(())
}

fn spawn_hosted(executable: &Path, arguments: &[OsString]) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    if let Some(app) = host::app_for(executable)? {
        // The host can't report a server that fails to start, so a missing
        // one fails here, as it would when started directly.
        std::fs::metadata(executable)?;
        return host::spawn(&app, arguments);
    }
    spawn(executable, arguments)
}

pub fn unavailable(error: &ClientError) -> bool {
    matches!(error, ClientError::Io(error) if matches!(error.kind(), io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound))
}

fn wait_for_startup_lock(socket: &Path) -> io::Result<File> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match lock_startup(socket) {
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(25));
            }
            result => return result,
        }
    }
}

pub fn lock_startup(socket: &Path) -> io::Result<File> {
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }
    try_lock(&socket.with_extension("update-lock"))
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
