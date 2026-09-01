use std::collections::BTreeMap;
use std::ffi::{CString, OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use muxy_proto::session::LaunchSpecification;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("shell launch field contains a NUL byte")]
    Nul,
    #[error("unsupported shell executable: {0}")]
    Unsupported(String),
}

pub struct PreparedShell {
    pub program: CString,
    pub arguments: Vec<CString>,
    pub environment: Vec<CString>,
    pub working_directory: CString,
}

pub fn prepare(launch: &LaunchSpecification) -> Result<PreparedShell, ShellError> {
    let shell = Path::new(&launch.shell);
    let name = shell
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ShellError::Unsupported(launch.shell.clone()))?;
    if !matches!(name, "zsh" | "bash" | "fish" | "elvish" | "nu" | "sh") {
        return Err(ShellError::Unsupported(launch.shell.clone()));
    }

    let resources = PathBuf::from(&launch.resources_directory);
    let integration = resources.join("ghostty/shell-integration");
    let mut environment = BTreeMap::new();
    for entry in &launch.environment {
        environment.insert(OsString::from(&entry.key), OsString::from(&entry.value));
    }
    environment.insert("TERM".into(), "xterm-ghostty".into());
    environment.insert("COLORTERM".into(), "truecolor".into());
    environment.insert("TERM_PROGRAM".into(), "ghostty".into());
    environment.insert(
        "TERM_PROGRAM_VERSION".into(),
        env!("CARGO_PKG_VERSION").into(),
    );
    environment.insert(
        "TERMINFO".into(),
        resources.join("terminfo").into_os_string(),
    );
    environment.insert("GHOSTTY_SHELL_FEATURES".into(), "cursor,title".into());

    let mut shell_arguments = shell_arguments(name, &integration, &mut environment);
    let (program, arguments) = if let Some(command) = &launch.startup_command {
        environment.insert("MUXY_STARTUP_COMMAND".into(), command.into());
        let startup = muxy_core::terminal_launch::startup_shell_command(&launch.shell, true);
        (
            CString::new("/bin/sh").map_err(|_| ShellError::Nul)?,
            ["/bin/sh", "-c", &format!("exec {startup}")]
                .into_iter()
                .map(|value| CString::new(value).map_err(|_| ShellError::Nul))
                .collect::<Result<Vec<_>, _>>()?,
        )
    } else {
        let program = CString::new(launch.shell.as_bytes()).map_err(|_| ShellError::Nul)?;
        let mut arguments = Vec::with_capacity(shell_arguments.len() + 1);
        arguments.push(login_argv0(name)?);
        arguments.append(&mut shell_arguments);
        (program, arguments)
    };

    let environment = environment
        .into_iter()
        .map(|(key, value)| {
            let mut bytes = key.into_vec();
            bytes.push(b'=');
            bytes.extend(value.into_vec());
            CString::new(bytes).map_err(|_| ShellError::Nul)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PreparedShell {
        program,
        arguments,
        environment,
        working_directory: CString::new(launch.working_directory.as_bytes())
            .map_err(|_| ShellError::Nul)?,
    })
}

fn shell_arguments(
    name: &str,
    integration: &Path,
    environment: &mut BTreeMap<OsString, OsString>,
) -> Vec<CString> {
    match name {
        "zsh" => {
            if let Some(value) = environment.get(OsStr::new("ZDOTDIR")).cloned() {
                environment.insert("GHOSTTY_ZSH_ZDOTDIR".into(), value);
            }
            environment.insert("ZDOTDIR".into(), integration.join("zsh").into_os_string());
            cstrings(["-l", "-i"])
        }
        "bash" | "sh" => {
            if let Some(value) = environment.get(OsStr::new("ENV")).cloned() {
                environment.insert("GHOSTTY_BASH_ENV".into(), value);
            }
            environment.insert(
                "ENV".into(),
                integration.join("bash/ghostty.bash").into_os_string(),
            );
            environment.insert("GHOSTTY_BASH_INJECT".into(), "-l -i".into());
            cstrings(["--posix", "-l", "-i"])
        }
        "fish" => {
            prepend_xdg(integration, environment);
            cstrings(["-l", "-i"])
        }
        "elvish" => {
            prepend_xdg(integration, environment);
            vec![
                CString::new("-i").unwrap(),
                CString::new("-rc").unwrap(),
                CString::new(
                    integration
                        .join("bash/elvish/lib/ghostty-integration.elv")
                        .into_os_string()
                        .into_vec(),
                )
                .unwrap(),
            ]
        }
        "nu" => {
            prepend_xdg(integration, environment);
            cstrings(["--login", "--interactive"])
        }
        _ => Vec::new(),
    }
}

fn prepend_xdg(integration: &Path, environment: &mut BTreeMap<OsString, OsString>) {
    let mut value = integration.as_os_str().as_bytes().to_vec();
    if let Some(existing) = environment.get(OsStr::new("XDG_DATA_DIRS"))
        && !existing.is_empty()
    {
        value.push(b':');
        value.extend(existing.as_encoded_bytes());
    }
    environment.insert("XDG_DATA_DIRS".into(), OsString::from_vec(value));
    environment.insert(
        "GHOSTTY_SHELL_INTEGRATION_XDG_DIR".into(),
        integration.as_os_str().to_owned(),
    );
}

fn login_argv0(name: &str) -> Result<CString, ShellError> {
    CString::new(format!("-{name}")).map_err(|_| ShellError::Nul)
}

fn cstrings<const N: usize>(values: [&str; N]) -> Vec<CString> {
    values
        .into_iter()
        .map(|value| CString::new(value).unwrap())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_proto::session::EnvironmentEntry;

    fn launch(shell: &str) -> LaunchSpecification {
        LaunchSpecification {
            shell: shell.to_owned(),
            resources_directory: "/bundle/Resources".to_owned(),
            working_directory: "/tmp".to_owned(),
            startup_command: None,
            environment: vec![EnvironmentEntry {
                key: "PATH".to_owned(),
                value: "/bin".to_owned(),
            }],
        }
    }

    #[test]
    fn five_shells_receive_their_integration_bootstrap() {
        for shell in ["zsh", "bash", "fish", "elvish", "nu"] {
            let prepared = prepare(&launch(&format!("/bin/{shell}"))).unwrap();
            let environment = prepared
                .environment
                .iter()
                .map(|value| value.to_string_lossy())
                .collect::<Vec<_>>();
            assert!(
                environment
                    .iter()
                    .any(|value| value.starts_with("TERM=xterm-ghostty"))
            );
            assert!(
                environment
                    .iter()
                    .any(|value| { value.contains("ghostty/shell-integration") })
            );
        }
    }

    #[test]
    fn startup_command_is_present_only_in_the_create_plan() {
        let mut specification = launch("/bin/bash");
        specification.startup_command = Some("printf ready".to_owned());
        let prepared = prepare(&specification).unwrap();
        assert_eq!(prepared.program.to_string_lossy(), "/bin/sh");
        assert!(
            prepared
                .environment
                .iter()
                .any(|value| { value.to_bytes() == b"MUXY_STARTUP_COMMAND=printf ready" })
        );
        assert!(prepared.arguments[2].to_string_lossy().contains("--posix"));
    }
}
