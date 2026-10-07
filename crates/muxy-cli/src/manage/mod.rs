pub(crate) mod args;
pub(crate) mod help;
mod projects;
mod records;
mod sessions;

use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use muxy_client::{Client, SshTarget, Start};
use muxy_protocol::{FilesAction, FilesRequest, GitAction, GitRequest, ServerPath};
use serde::Serialize;
use serde_json::{Value, json};

use crate::target::Target;
use args::{Action, Invocation, Server};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(crate) fn run(invocation: Invocation, host: Option<SshTarget>) -> Result {
    if let Action::Help(text) = invocation.action {
        writeln!(io::stdout(), "{text}")?;
        return Ok(());
    }
    let _lease =
        muxy_client::local::bundle::acquire_runtime(&muxy_core::executable::current_path()?)?;
    let target = Target::new(host)?;
    let start = if matches!(
        invocation.action,
        Action::Server(Server::Status | Server::Stop { .. })
    ) {
        Start::Never
    } else {
        Start::IfNeeded
    };
    let client = target
        .connect(start)
        .map_err(|error| target.explain(error))?;
    let paths = match target {
        Target::Local { .. } => Paths::Local,
        Target::Ssh { .. } => Paths::Remote,
    };
    let result = match client.identify(muxy_protocol::ClientKind::Cli) {
        Ok(_) => execute(invocation, &client, paths),
        Err(error) => Err(error.into()),
    };
    client.disconnect();
    result.map_err(|error| target.explain(error))
}

/// How directory arguments name folders. This computer's may be relative or
/// start with `~`. Another computer's must be absolute, and its server checks
/// that they exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Paths {
    Local,
    Remote,
}

impl Paths {
    fn directory(self, path: &Path) -> Result<PathBuf> {
        match self {
            Self::Local => absolute(path),
            Self::Remote if path.is_absolute() => Ok(std::path::absolute(path)?),
            Self::Remote => Err(format!(
                "{}: with --host, directories must be absolute paths on that computer",
                path.display()
            )
            .into()),
        }
    }
}

fn execute(invocation: Invocation, client: &Client, paths: Paths) -> Result {
    let output = Output {
        json: invocation.json,
    };
    match invocation.action {
        Action::Help(_) => Ok(()),
        Action::Server(command) => match command {
            Server::Start | Server::Status => Output::json(client.server_info()),
            Server::Stop { force } => {
                if force {
                    client.stop_server()?;
                } else if !client.stop_server_if_idle()? {
                    return Err("server has running terminals; use --force to end them".into());
                }
                output.ok()
            }
        },
        Action::Project(command) => projects::run(command, client, &output, paths),
        Action::Session(command) => sessions::run(command, client, &output, paths),
        Action::Worktree(command) => projects::worktree(command, client, &output, paths),
        Action::SettingsGet => settings(client),
        Action::SettingsSet { key, value } => {
            set_setting(client, &key, &value)?;
            output.ok()
        }
        Action::Activity(events) if events.is_empty() => {
            Output::json(&records::activity(&client.activity()?))
        }
        Action::Activity(events) => {
            client.acknowledge_activity(events)?;
            output.ok()
        }
        Action::Git { project, action } => {
            let action: GitAction = serde_json::from_str(&action)?;
            if matches!(action, GitAction::Watch) {
                return Err("watch requires a persistent client".into());
            }
            let project = projects::resolve(client, &project, paths)?.id;
            Output::json(&client.git(GitRequest { project, action })?)
        }
        Action::Files { project, action } => {
            let action: FilesAction = serde_json::from_str(&action)?;
            if matches!(action, FilesAction::Watch | FilesAction::Unwatch) {
                return Err("watch requires a persistent client".into());
            }
            let project = projects::resolve(client, &project, paths)?.id;
            Output::json(&client.files(FilesRequest { project, action })?)
        }
        Action::Exec {
            project,
            argv,
            timeout_ms,
        } => {
            let project = projects::resolve(client, &project, paths)?.id;
            let result = client.exec(muxy_protocol::ExecRequest {
                job: 1,
                project,
                argv,
                shell: None,
                cwd: None,
                env: std::collections::BTreeMap::new(),
                stdin: Vec::new(),
                timeout_ms,
            })?;
            if output.json {
                Output::json(&result)?;
            } else {
                io::stdout().write_all(result.stdout.as_bytes())?;
                io::stderr().write_all(result.stderr.as_bytes())?;
            }
            if result.exit_code != 0 || result.timed_out || result.cancelled || result.truncated {
                return Err(format!(
                    "exec failed: exit={}, timed_out={}, cancelled={}, truncated={}",
                    result.exit_code, result.timed_out, result.cancelled, result.truncated
                )
                .into());
            }
            Ok(())
        }
    }
}

fn set_setting(client: &Client, key: &str, value: &str) -> Result {
    let mut settings = client.read_server_settings()?;
    match key {
        "default-shell" => {
            settings.default_shell = if value == "default" {
                None
            } else {
                let path = Path::new(value);
                if !path.is_absolute() {
                    return Err("default-shell must be an absolute path or 'default'".into());
                }
                Some(server_path(path))
            }
        }
        "history-budget-bytes" => settings.history_budget_bytes = value.parse()?,
        "shell-integration" => settings.shell_integration = value.parse()?,
        key @ ("sandbox-executable"
        | "sandbox-network"
        | "sandbox-domains"
        | "sandbox-tools"
        | "sandbox-environment") => {
            let sandbox = settings.sandbox.get_or_insert_with(Default::default);
            let values = |separator| {
                value
                    .split(separator)
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            match key {
                "sandbox-executable" => sandbox.executable = Some(server_path(Path::new(value))),
                "sandbox-network" => {
                    sandbox.policy.network =
                        match value {
                            "blocked" => muxy_protocol::SandboxNetwork::Blocked,
                            "domains" => muxy_protocol::SandboxNetwork::Domains,
                            "unrestricted" => return Err(
                                "Unrestricted networking is unavailable with this sandbox backend"
                                    .into(),
                            ),
                            _ => return Err("Use blocked or domains".into()),
                        }
                }
                "sandbox-domains" => sandbox.policy.domains = values(','),
                "sandbox-tools" => {
                    sandbox.policy.read_paths = values(';')
                        .iter()
                        .map(|p| server_path(Path::new(p)))
                        .collect();
                }
                "sandbox-environment" => sandbox.policy.environment = values(','),
                _ => unreachable!(),
            }
        }
        _ => return Err("unknown server setting; run muxy settings --help".into()),
    }
    settings
        .validate()
        .map_err(|code| format!("invalid setting: {code:?}"))?;
    client.write_server_settings(settings)?;
    Ok(())
}

fn settings(client: &Client) -> Result {
    let settings = client.read_server_settings()?;
    Output::json(&json!({
        "default_shell": settings.default_shell.as_ref().map(path_text),
        "history_budget_bytes": settings.history_budget_bytes,
        "shell_integration": settings.shell_integration,
        "sandbox": settings.sandbox,
    }))
}

struct Output {
    json: bool,
}
impl Output {
    fn json(value: &impl Serialize) -> Result {
        let mut stdout = io::stdout().lock();
        serde_json::to_writer(&mut stdout, value)?;
        writeln!(stdout)?;
        Ok(())
    }
    fn ok(&self) -> Result {
        if self.json {
            Self::json(&json!({"ok":true}))
        } else {
            writeln!(io::stdout(), "ok")?;
            Ok(())
        }
    }
    fn record(&self, value: &Value, columns: &[&str]) -> Result {
        if self.json {
            Self::json(value)
        } else {
            Self::row(value, columns)
        }
    }
    fn list(&self, values: &[Value], columns: &[&str]) -> Result {
        if self.json {
            Self::json(&values)
        } else {
            for value in values {
                Self::row(value, columns)?;
            }
            Ok(())
        }
    }
    fn row(value: &Value, columns: &[&str]) -> Result {
        let fields: Vec<_> = columns
            .iter()
            .map(|key| match &value[key] {
                Value::String(text) => escape(text),
                Value::Null => String::new(),
                other => other.to_string(),
            })
            .collect();
        writeln!(io::stdout(), "{}", fields.join("\t"))?;
        Ok(())
    }
}

fn escape(text: &str) -> String {
    text.escape_debug().to_string()
}
fn path_text(path: &ServerPath) -> String {
    String::from_utf8_lossy(&path.0).into_owned()
}
fn local_path(path: &ServerPath) -> PathBuf {
    std::ffi::OsString::from_vec(path.0.clone()).into()
}
fn server_path(path: &Path) -> ServerPath {
    ServerPath(path.as_os_str().as_bytes().to_vec())
}
fn absolute(path: &Path) -> Result<PathBuf> {
    let path = if path == Path::new("~") {
        PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?)
    } else if let Ok(rest) = path.strip_prefix("~/") {
        PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?).join(rest)
    } else {
        path.to_owned()
    };
    Ok(std::path::absolute(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_directories_must_be_absolute_and_are_not_resolved_here() -> Result {
        assert_eq!(
            Paths::Remote.directory(Path::new("/srv//app/./src"))?,
            PathBuf::from("/srv/app/src")
        );
        for relative in ["app", "./app", "~", "~/app"] {
            assert!(
                Paths::Remote.directory(Path::new(relative)).is_err(),
                "{relative}"
            );
        }
        assert_eq!(
            Paths::Local.directory(Path::new("app"))?,
            std::env::current_dir()?.join("app")
        );
        Ok(())
    }
}
