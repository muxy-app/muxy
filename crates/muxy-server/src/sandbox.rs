use std::os::unix::ffi::{OsStrExt, OsStringExt};

use muxy_protocol::{ErrorCode, SandboxInfo, SandboxSpec};

use crate::ServerError;

pub(crate) const NONO_VERSION: &str = "0.79.0";

pub(crate) fn info(mut spec: SandboxSpec) -> Result<SandboxInfo, ServerError> {
    spec.validate()
        .map_err(|code| ServerError::new(code, "Invalid sandbox policy"))?;
    if !cfg!(target_os = "macos") {
        return Err(ServerError::new(
            ErrorCode::Unsupported,
            "Sandboxed terminals currently require macOS",
        ));
    }
    if spec.policy.network == muxy_protocol::SandboxNetwork::Unrestricted {
        return Err(ServerError::new(
            ErrorCode::Unsupported,
            "Unrestricted networking is unavailable with nono 0.79.0: it permits access to host control sockets. Choose blocked networking or approved domains.",
        ));
    }
    for path in std::iter::once(&mut spec.workspace).chain(&mut spec.policy.read_paths) {
        let resolved = std::path::PathBuf::from(std::ffi::OsString::from_vec(path.0.clone()))
            .canonicalize()
            .map_err(|error| {
                ServerError::new(ErrorCode::BadPath, format!("Sandbox path: {error}"))
            })?;
        path.0 = resolved.as_os_str().as_bytes().to_vec();
    }
    Ok(SandboxInfo {
        spec,
        backend_version: NONO_VERSION.into(),
    })
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::spawn;

#[cfg(not(target_os = "macos"))]
pub(crate) fn spawn(
    _request: muxy_terminal::pty::SpawnRequest,
    _settings: &crate::ServerSettings,
    _sandbox: &SandboxInfo,
) -> Result<muxy_terminal::pty::Pty, ServerError> {
    Err(ServerError::new(
        ErrorCode::Unsupported,
        "Sandboxed terminals currently require macOS",
    ))
}
