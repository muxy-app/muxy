pub mod bundle;

use crate::{Client, ClientError};
use muxy_protocol::transport::ByteStream;
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
    start_and_connect(socket, executable, Client::connect_with_timeout)
}

/// Like [`ensure_running`], but returns the socket without a handshake, so a
/// bridge can leave version negotiation to the client at its other end.
pub fn ensure_listening(
    socket: &Path,
    executable: &Path,
) -> Result<Box<dyn ByteStream>, ClientError> {
    start_and_connect(socket, executable, |socket, _| {
        Ok(muxy_protocol::transport::connect(socket)?)
    })
}

/// Connects, first starting the server if nothing listens on `socket`.
fn start_and_connect<T>(
    socket: &Path,
    executable: &Path,
    connect: impl Fn(&Path, Duration) -> Result<T, ClientError>,
) -> Result<T, ClientError> {
    let _startup = wait_for_startup_lock(socket)?;
    let deadline = Instant::now() + Duration::from_secs(3);
    match connect(socket, deadline.saturating_duration_since(Instant::now())) {
        Ok(connected) => return Ok(connected),
        Err(error) if unavailable(&error) => {}
        Err(error) => return Err(error),
    }
    let mut child = Command::new(executable)
        .arg("--socket")
        .arg(socket)
        .arg("--settings")
        .arg(socket.with_file_name("server.toml"))
        .arg("--log")
        .arg(socket.with_file_name("server.log"))
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not launch server {}: {error}", executable.display()),
            )
        })?;
    thread::Builder::new()
        .name("muxy-server-wait".into())
        .spawn(move || {
            let _ = child.wait();
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
