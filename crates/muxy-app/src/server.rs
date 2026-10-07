use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::{Client, ClientError};
use muxy_ui::tr;

pub(crate) fn ensure_server_running(socket: &Path) -> Result<Client, ClientError> {
    muxy_client::local::ensure_running(socket, &server_executable()?)
}

pub(crate) fn reconnect_after_update(
    socket: &Path,
    previous_instance: u64,
) -> Result<Client, ClientError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let result = ensure_server_running(socket);
        match result {
            Ok(client) if client.server_info().instance != previous_instance => return Ok(client),
            Ok(_) if Instant::now() >= deadline => return Err(ClientError::Timeout),
            Ok(_) => {}
            Err(error) if Instant::now() < deadline && retryable_restart(&error) => {}
            Err(error) => return Err(error),
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn retryable_restart(error: &ClientError) -> bool {
    matches!(error, ClientError::Disconnected | ClientError::Timeout)
        || matches!(error, ClientError::Io(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused | io::ErrorKind::TimedOut))
}

pub(crate) use muxy_client::local::server_executable;

pub(crate) fn stop_server(client: &Client, socket: &Path) -> Result<(), ClientError> {
    use std::os::unix::fs::MetadataExt;
    let identity =
        std::fs::symlink_metadata(socket).map(|metadata| (metadata.dev(), metadata.ino()))?;
    client.stop_server()?;
    wait_stopped(socket, identity)
}

fn wait_stopped(socket: &Path, identity: (u64, u64)) -> Result<(), ClientError> {
    use std::os::unix::fs::MetadataExt;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match std::fs::symlink_metadata(socket) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
            Ok(metadata) if (metadata.dev(), metadata.ino()) != identity => return Ok(()),
            Ok(_) => {}
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                tr!("Server has not finished stopping. Use Connect after shutdown completes.")
                    .to_string(),
            )
            .into());
        }
        thread::sleep(Duration::from_millis(25));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UpdateMode {
    Preserve,
    WhenIdle,
    EndSessions,
}

pub(crate) fn prepare_update(
    client: &Client,
    socket: &Path,
    update: &crate::updater::PreparedUpdate,
    mode: UpdateMode,
    expected: &muxy_protocol::ServerInfo,
) -> Result<Option<std::fs::File>, ClientError> {
    let installation = match lock_for_update(socket) {
        Ok(lock) => lock,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    update
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    // A fresh connection protects against a server replaced since the prompt.
    let probe = Client::connect(socket)?;
    if probe.server_info() != expected || client.server_info() != expected {
        return Err(io::Error::other(
            tr!("The server changed. Review the update again.").to_string(),
        )
        .into());
    }
    match mode {
        UpdateMode::Preserve => {
            if !update.compatible_with(expected) {
                return Err(io::Error::other(
                    tr!("This update requires a server restart").to_string(),
                )
                .into());
            }
        }
        UpdateMode::WhenIdle => {
            if !stop_if_idle(&probe, socket)? {
                return Ok(None);
            }
        }
        UpdateMode::EndSessions => stop_server(&probe, socket)?,
    }
    Ok(Some(installation))
}

fn stop_if_idle(client: &Client, socket: &Path) -> Result<bool, ClientError> {
    use std::os::unix::fs::MetadataExt;
    let identity = std::fs::symlink_metadata(socket).map(|m| (m.dev(), m.ino()))?;
    if !client.stop_server_if_idle()? {
        return Ok(false);
    }
    wait_stopped(socket, identity)?;
    Ok(true)
}

#[derive(Debug)]
pub(crate) struct ServerUpdate {
    pub(crate) server: muxy_protocol::ServerInfo,
    pub(crate) sessions: usize,
    pub(crate) replaced: bool,
}

pub(crate) fn check_update(
    client: &Client,
    socket: &Path,
    replace: bool,
) -> Result<ServerUpdate, ClientError> {
    let server = client.server_info().clone();
    let sessions = client.list_sessions()?.len();
    let mut status = ServerUpdate {
        server,
        sessions,
        replaced: false,
    };
    if replace && sessions == 0 && newer_build(&status.server.build.version) {
        let _installation = match lock_for_update(socket) {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(status),
            Err(error) => return Err(error.into()),
        };
        let executable = server_executable()?;
        let installed = read_build_info(&executable)?;
        if installed != muxy_protocol::BuildInfo::current() {
            return Err(io::Error::other(
                tr!("The installed app changed. Reopen Muxy to update the server.").to_string(),
            )
            .into());
        }
        let probe = Client::connect(socket)?;
        if probe.server_info() == &status.server && stop_if_idle(&probe, socket)? {
            status.replaced = true;
        }
    }
    Ok(status)
}

pub(crate) fn newer_build(running: &str) -> bool {
    muxy_core::release::is_newer(env!("CARGO_PKG_VERSION"), running)
}

pub(crate) use muxy_client::local::lock_startup as lock_for_update;
pub(crate) use muxy_client::local::read_build_info;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_startup_respects_the_update_lock_and_releases_it_after_failure() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let socket = directory.path().join("server.sock");
        let lock = lock_for_update(&socket)?;
        assert!(
            ensure_server_running(&socket)
                .is_err_and(|error| error.to_string().contains("already in progress"))
        );
        assert!(
            lock_for_update(&socket).is_err_and(|error| error.kind() == io::ErrorKind::WouldBlock)
        );
        drop(lock);
        assert!(lock_for_update(&socket).is_ok());
        Ok(())
    }
}
