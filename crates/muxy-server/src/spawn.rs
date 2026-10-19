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
const TERMINAL_ENV: [(&str, &str); 3] = [
    ("TERM", "xterm-256color"),
    ("COLORTERM", "truecolor"),
    ("TERM_PROGRAM", "muxy"),
];

/// A session's shell and, on Windows, the job holding every process it
/// starts, which ends them all when dropped.
pub(crate) struct Shell {
    pub(crate) pty: Pty,
    #[cfg(windows)]
    pub(crate) job: crate::platform::windows::Job,
}

pub(crate) fn spawn_shell(
    settings: &ServerSettings,
    integration: Option<&crate::ShellIntegration>,
    directory: &Path,
    size: PtySize,
    identity: (muxy_protocol::ServerIdentity, muxy_protocol::SessionId),
) -> Result<Shell, ServerError> {
    if !directory.is_dir() {
        return Err(ServerError::new(
            ErrorCode::BadPath,
            format!("{} is not a directory", directory.display()),
        ));
    }
    let program = resolve_shell(settings);
    let mut request = SpawnRequest {
        args: shell_arguments(&program),
        program,
        cwd: directory.to_path_buf(),
        env: environment(identity),
        size,
    };
    request.env.retain(|(name, _)| !same_name(name, "SHELL"));
    request.env.push((
        OsString::from("SHELL"),
        request.program.clone().into_os_string(),
    ));
    if let Some(integration) = integration {
        integration.configure(&mut request, settings.shell_integration);
    }
    let pty = Pty::spawn(request).map_err(ServerError::spawn_failed)?;
    #[cfg(windows)]
    {
        let mut pty = pty;
        let job = pty
            .process_handle()
            .ok_or_else(|| std::io::Error::other("the shell has no process handle"))
            .and_then(|process| {
                let job = crate::platform::windows::Job::killed_on_drop()?;
                job.assign(process)?;
                Ok(job)
            });
        match job {
            Ok(job) => Ok(Shell { pty, job }),
            Err(error) => {
                let _ = pty.kill();
                let _ = pty.wait();
                Err(ServerError::spawn_failed(error))
            }
        }
    }
    #[cfg(unix)]
    Ok(Shell { pty })
}

/// A login shell on Unix. Windows shells have no common flag; `PowerShell`
/// only skips its banner.
fn shell_arguments(program: &Path) -> Vec<OsString> {
    #[cfg(unix)]
    {
        let _ = program;
        vec!["-l".into()]
    }
    #[cfg(windows)]
    {
        if crate::shell::is_powershell(program) {
            vec!["-NoLogo".into()]
        } else {
            Vec::new()
        }
    }
}

fn resolve_shell(settings: &ServerSettings) -> PathBuf {
    #[cfg(unix)]
    {
        select_shell(
            settings.default_shell.clone(),
            env::var_os("SHELL"),
            cfg!(target_os = "linux"),
        )
    }
    #[cfg(windows)]
    {
        select_windows_shell(
            settings.default_shell.clone(),
            env::var_os("SHELL"),
            env::var_os("PATH"),
            env::var_os("SystemRoot"),
        )
    }
}

#[cfg(unix)]
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

/// The configured shell, then `SHELL` when it names an existing `.exe`, then
/// `pwsh.exe` on `PATH`, then `powershell.exe` from the system.
#[cfg(windows)]
fn select_windows_shell(
    configured: Option<PathBuf>,
    inherited: Option<OsString>,
    path: Option<OsString>,
    system_root: Option<OsString>,
) -> PathBuf {
    let inherited = inherited.map(PathBuf::from).filter(|shell| {
        shell.is_absolute()
            && shell
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            && exists(shell)
    });
    configured
        .or(inherited)
        .or_else(|| {
            env::split_paths(&path.unwrap_or_default())
                .filter(|folder| folder.is_absolute())
                .map(|folder| folder.join("pwsh.exe"))
                .find(|shell| exists(shell))
        })
        .unwrap_or_else(|| {
            PathBuf::from(system_root.unwrap_or_else(|| r"C:\Windows".into()))
                .join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
        })
}

/// Whether `path` exists, counting the Store's app execution aliases, which
/// can't be opened as files.
#[cfg(windows)]
pub(crate) fn exists(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| !metadata.is_dir())
}

fn environment(
    identity: (muxy_protocol::ServerIdentity, muxy_protocol::SessionId),
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<_> = env::vars_os().filter(|(name, _)| inherited(name)).collect();
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

/// Whether a session inherits the server's variable `name`. On Windows the
/// pseudoconsole reads `PATH` from the registry instead, so a change to it
/// reaches new sessions without restarting the server.
fn inherited(name: &OsString) -> bool {
    !(same_name(name, "MUXY_SERVER_ID")
        || same_name(name, "MUXY_SESSION_ID")
        || (cfg!(windows) && same_name(name, "PATH")))
}

/// Windows variable names ignore case; Unix ones do not.
pub(crate) fn same_name(name: &OsString, expected: &str) -> bool {
    if cfg!(windows) {
        name.eq_ignore_ascii_case(expected)
    } else {
        name == expected
    }
}

#[cfg(all(test, windows))]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    #[test]
    fn windows_shell_prefers_settings_then_an_exe_shell_then_pwsh_on_path() {
        let root =
            env::temp_dir().join(format!("muxy-shell-{}", muxy_protocol::OperationId::new()));
        let folder = root.join("bin");
        std::fs::create_dir_all(&folder).unwrap();
        let system = OsString::from(r"C:\Windows");
        let fallback = PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");
        assert_eq!(
            select_windows_shell(
                None,
                None,
                Some(folder.clone().into()),
                Some(system.clone())
            ),
            fallback
        );
        std::fs::write(folder.join("pwsh.exe"), "").unwrap();
        assert_eq!(
            select_windows_shell(
                None,
                None,
                Some(folder.clone().into()),
                Some(system.clone())
            ),
            folder.join("pwsh.exe")
        );
        let script = root.join("shell.cmd");
        std::fs::write(&script, "").unwrap();
        assert_eq!(
            select_windows_shell(None, Some(script.into()), None, Some(system.clone())),
            fallback,
            "SHELL is ignored unless it names an existing .exe"
        );
        let shell = root.join("nu.exe");
        std::fs::write(&shell, "").unwrap();
        assert_eq!(
            select_windows_shell(None, Some(shell.clone().into()), None, Some(system.clone())),
            shell
        );
        let configured = PathBuf::from(r"C:\Tools\shell.exe");
        assert_eq!(
            select_windows_shell(
                Some(configured.clone()),
                Some(shell.into()),
                None,
                Some(system)
            ),
            configured
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn store_pwsh_alias_counts_as_an_existing_shell() {
        let Some(local) = env::var_os("LOCALAPPDATA") else {
            return;
        };
        let alias = PathBuf::from(local).join(r"Microsoft\WindowsApps\pwsh.exe");
        if std::fs::symlink_metadata(&alias).is_err() {
            return;
        }
        assert!(exists(&alias));
        assert!(std::fs::metadata(&alias).is_err() || alias.is_file());
    }

    #[test]
    fn sessions_read_path_from_the_registry_and_match_names_without_case() {
        assert!(!inherited(&"Path".into()));
        assert!(!inherited(&"muxy_session_id".into()));
        assert!(inherited(&"TEMP".into()));
        assert!(same_name(&"shell".into(), "SHELL"));
    }
}
