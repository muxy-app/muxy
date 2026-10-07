use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use muxy_protocol::ErrorCode;
use muxy_terminal::pty::{Pty, PtySize, SpawnRequest};

use crate::error::ServerError;
use crate::settings::ServerSettings;

#[cfg(target_os = "macos")]
const FALLBACK_SHELL: &str = "/bin/zsh";
#[cfg(target_os = "linux")]
const FALLBACK_SHELL: &str = "/bin/sh";
const LOGIN_FLAG: &str = "-l";
const TERMINAL_ENV: [(&str, &str); 3] = [
    ("TERM", "xterm-256color"),
    ("COLORTERM", "truecolor"),
    ("TERM_PROGRAM", "muxy"),
];

pub(crate) fn spawn_shell(
    settings: &ServerSettings,
    integration: Option<&crate::ShellIntegration>,
    directory: &Path,
    size: PtySize,
    identity: (muxy_protocol::ServerIdentity, muxy_protocol::SessionId),
    sandbox: Option<&muxy_protocol::SandboxInfo>,
) -> Result<Pty, ServerError> {
    if !directory.is_dir() {
        return Err(ServerError::new(
            ErrorCode::BadPath,
            format!("{} is not a directory", directory.display()),
        ));
    }
    let mut request = SpawnRequest {
        clear_env: false,
        program: resolve_shell(settings),
        args: vec![OsString::from(LOGIN_FLAG)],
        cwd: directory.to_path_buf(),
        env: environment(identity),
        size,
    };
    request.env.retain(|(name, _)| name != "SHELL");
    request.env.push((
        OsString::from("SHELL"),
        request.program.clone().into_os_string(),
    ));
    if let Some(sandbox) = sandbox {
        return crate::sandbox::spawn(request, settings, sandbox);
    }
    if let Some(integration) = integration {
        integration.configure(&mut request, settings.shell_integration);
    }
    Pty::spawn(request).map_err(ServerError::spawn_failed)
}

fn resolve_shell(settings: &ServerSettings) -> PathBuf {
    select_shell(
        settings.default_shell.clone(),
        env::var_os("SHELL"),
        cfg!(target_os = "linux"),
    )
}

fn select_shell(configured: Option<PathBuf>, inherited: Option<OsString>, linux: bool) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    configured
        .or_else(|| {
            inherited
                .filter(|shell| !shell.is_empty())
                .map(PathBuf::from)
                .filter(|shell| {
                    !linux
                        || (shell.is_absolute()
                            && shell.metadata().is_ok_and(|metadata| {
                                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                            }))
                })
        })
        .unwrap_or_else(|| PathBuf::from(if linux { "/bin/sh" } else { FALLBACK_SHELL }))
}

fn environment(
    identity: (muxy_protocol::ServerIdentity, muxy_protocol::SessionId),
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<_> = env::vars_os().collect();
    env.retain(|(name, _)| name != "MUXY_SERVER_ID" && name != "MUXY_SESSION_ID");
    env.push(("MUXY_SERVER_ID".into(), identity.0.to_string().into()));
    env.push((
        "MUXY_SESSION_ID".into(),
        identity.1.get().to_string().into(),
    ));
    env.extend(
        TERMINAL_ENV
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    );
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_falls_back_only_for_invalid_inherited_shells() {
        for inherited in [
            None,
            Some("".into()),
            Some("/does-not-exist".into()),
            Some("/tmp".into()),
            Some("sh".into()),
        ] {
            assert_eq!(
                select_shell(None, inherited, true),
                PathBuf::from("/bin/sh")
            );
        }
        assert_eq!(
            select_shell(None, Some("/bin/bash".into()), true),
            PathBuf::from("/bin/bash")
        );
        assert_eq!(
            select_shell(Some("/does-not-exist".into()), Some("/bin/sh".into()), true),
            PathBuf::from("/does-not-exist")
        );
        assert_eq!(
            select_shell(None, Some("/does-not-exist".into()), false),
            PathBuf::from("/does-not-exist")
        );
    }
}
