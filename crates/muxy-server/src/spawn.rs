use std::collections::BTreeMap;
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
    extra: &BTreeMap<String, String>,
) -> Result<Pty, ServerError> {
    if !directory.is_dir() {
        return Err(ServerError::new(
            ErrorCode::BadPath,
            format!("{} is not a directory", directory.display()),
        ));
    }
    let mut request = SpawnRequest {
        program: resolve_shell(settings),
        args: vec![OsString::from(LOGIN_FLAG)],
        cwd: directory.to_path_buf(),
        env: environment(identity, extra),
        size,
    };
    request.env.retain(|(name, _)| name != "SHELL");
    request.env.push((
        OsString::from("SHELL"),
        request.program.clone().into_os_string(),
    ));
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

/// `extra`, such as a proxy, replaces inherited variables but never Muxy's own.
fn environment(
    identity: (muxy_protocol::ServerIdentity, muxy_protocol::SessionId),
    extra: &BTreeMap<String, String>,
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<_> = env::vars_os()
        .filter(|(name, _)| name.to_str().is_none_or(|name| !extra.contains_key(name)))
        .collect();
    env.extend(
        extra
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    );
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

    fn last<'a>(env: &'a [(OsString, OsString)], name: &str) -> Option<&'a OsString> {
        env.iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    #[test]
    fn requested_variables_replace_inherited_ones_but_not_muxys_own() {
        let session = muxy_protocol::SessionId::new(7).expect("non-zero");
        let extra = BTreeMap::from([
            ("PATH".to_owned(), "/proxy/bin".to_owned()),
            ("http_proxy".to_owned(), "http://proxy:8080".to_owned()),
            ("MUXY_SESSION_ID".to_owned(), "1".to_owned()),
            ("TERM".to_owned(), "dumb".to_owned()),
        ]);
        let env = environment(
            (muxy_protocol::ServerIdentity::from_u128(1), session),
            &extra,
        );
        assert_eq!(env.iter().filter(|(key, _)| key == "PATH").count(), 1);
        assert_eq!(last(&env, "PATH"), Some(&OsString::from("/proxy/bin")));
        assert_eq!(
            last(&env, "http_proxy"),
            Some(&OsString::from("http://proxy:8080"))
        );
        assert_eq!(last(&env, "MUXY_SESSION_ID"), Some(&OsString::from("7")));
        assert_eq!(last(&env, "TERM"), Some(&OsString::from("xterm-256color")));
    }
}
