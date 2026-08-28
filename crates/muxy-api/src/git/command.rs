use super::{GitError, GitOptions};
use crate::execution_environment::ExecutionEnvironment;
use crate::subprocess::{
    CancellationSignal, Deadline, EnvironmentMode, StdinMode, SubprocessError, SubprocessOutput,
    SubprocessRequest,
};
use std::ffi::OsString;
use std::path::Path;

pub(crate) struct GitCommand {
    pub args: Vec<OsString>,
    pub stdin: StdinMode,
    pub environment: EnvironmentMode,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
    pub cancellation: Option<CancellationSignal>,
}

pub(crate) struct RepositoryCommandRequest {
    pub args: Vec<OsString>,
    pub read_only: bool,
    pub network: bool,
    pub stdin: StdinMode,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
    pub cancellation: Option<CancellationSignal>,
}

pub(crate) fn repository_command(
    environment: &ExecutionEnvironment,
    request: RepositoryCommandRequest,
) -> GitCommand {
    let mut variables = environment.git_variables();
    variables.retain(|(key, _)| key != "GIT_OPTIONAL_LOCKS");
    if request.read_only {
        variables.push((OsString::from("GIT_OPTIONAL_LOCKS"), OsString::from("0")));
    }
    GitCommand {
        args: repository_arguments(environment, request.args, request.network),
        stdin: request.stdin,
        environment: EnvironmentMode::Replace(variables),
        stdout_limit: request.stdout_limit,
        stderr_limit: request.stderr_limit,
        cancellation: request.cancellation,
    }
}

fn repository_arguments(
    environment: &ExecutionEnvironment,
    args: Vec<OsString>,
    network: bool,
) -> Vec<OsString> {
    let gh = environment.resolve_executable("gh".as_ref());
    repository_arguments_with_gh(args, network, gh.as_deref())
}

fn repository_arguments_with_gh(
    args: Vec<OsString>,
    network: bool,
    gh: Option<&Path>,
) -> Vec<OsString> {
    if !network {
        return args;
    }
    let Some(gh) = gh else {
        return args;
    };
    let Some(helper) = credential_helper(gh) else {
        return args;
    };
    let mut configured = vec![
        OsString::from("-c"),
        OsString::from("credential.helper="),
        OsString::from("-c"),
        helper,
    ];
    configured.extend(args);
    configured
}

#[cfg(unix)]
fn credential_helper(path: &Path) -> Option<OsString> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let mut value = Vec::from(&b"credential.https://github.com.helper=!'"[..]);
    for byte in path.as_os_str().as_bytes() {
        if *byte == b'\'' {
            value.extend_from_slice(b"'\\''");
        } else {
            value.push(*byte);
        }
    }
    value.extend_from_slice(b"' auth git-credential");
    Some(OsString::from_vec(value))
}

#[cfg(not(unix))]
fn credential_helper(_path: &Path) -> Option<OsString> {
    None
}

pub(crate) fn run_git(
    options: &GitOptions,
    path: &Path,
    args: &[&str],
) -> Result<String, GitError> {
    let deadline = Deadline::new(std::time::Duration::from_secs(30));
    run_git_with_deadline(options, path, args, &deadline)
}

pub(crate) fn run_git_with_deadline(
    options: &GitOptions,
    path: &Path,
    args: &[&str],
    deadline: &Deadline,
) -> Result<String, GitError> {
    let output = run_output(
        options,
        path,
        GitCommand {
            args: args.iter().map(OsString::from).collect(),
            stdin: StdinMode::Inherit,
            environment: EnvironmentMode::Inherit {
                set: options
                    .environment
                    .iter()
                    .map(|(key, value)| (OsString::from(key), OsString::from(value)))
                    .collect(),
                remove: Vec::new(),
            },
            stdout_limit: usize::MAX,
            stderr_limit: usize::MAX,
            cancellation: None,
        },
        deadline,
    )?;
    if !output.status.success() {
        return Err(GitError::Status {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| GitError::NonUtf8)
}

pub(crate) fn run_output(
    options: &GitOptions,
    path: &Path,
    request: GitCommand,
    deadline: &Deadline,
) -> Result<SubprocessOutput, GitError> {
    let mut args = vec![OsString::from("-C"), path.as_os_str().to_owned()];
    args.extend(request.args);
    crate::subprocess::run(
        SubprocessRequest {
            executable: options.executable.clone(),
            args,
            current_dir: None,
            stdin: request.stdin,
            environment: request.environment,
            stdout_limit: request.stdout_limit,
            stderr_limit: request.stderr_limit,
            cancellation: request.cancellation,
        },
        Some(deadline),
    )
    .map_err(|error| match error {
        SubprocessError::Spawn(source) => GitError::Execute {
            executable: options.executable.clone(),
            source,
        },
        other => GitError::Process(other),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn environment(path: &Path) -> ExecutionEnvironment {
        ExecutionEnvironment::fallback([
            (OsString::from("PATH"), path.as_os_str().to_owned()),
            (OsString::from("HOME"), OsString::from("/nonexistent")),
        ])
    }

    #[test]
    fn repository_network_arguments_do_not_add_a_helper_when_gh_is_missing() {
        let args = repository_arguments_with_gh(vec![OsString::from("fetch")], true, None);

        assert_eq!(args, [OsString::from("fetch")]);
    }

    #[test]
    fn repository_commands_set_optional_locks_only_for_reads() {
        let temp = tempfile::tempdir().unwrap();
        let mut variables = environment(temp.path()).variables();
        variables.push((
            OsString::from("GIT_OPTIONAL_LOCKS"),
            OsString::from("ambient"),
        ));
        let environment = ExecutionEnvironment::fallback(variables);
        let build = |read_only| {
            repository_command(
                &environment,
                RepositoryCommandRequest {
                    args: vec![OsString::from("status")],
                    read_only,
                    network: false,
                    stdin: StdinMode::Closed,
                    stdout_limit: 1_024,
                    stderr_limit: 1_024,
                    cancellation: None,
                },
            )
        };

        let EnvironmentMode::Replace(read) = build(true).environment else {
            panic!("replace environment");
        };
        let EnvironmentMode::Replace(write) = build(false).environment else {
            panic!("replace environment");
        };
        assert_eq!(
            read.iter()
                .find(|(key, _)| key == "GIT_OPTIONAL_LOCKS")
                .map(|(_, value)| value.as_os_str()),
            Some(OsStr::new("0"))
        );
        assert!(!write.iter().any(|(key, _)| key == "GIT_OPTIONAL_LOCKS"));
    }

    #[cfg(unix)]
    #[test]
    fn repository_network_arguments_quote_gh_once_and_pass_git_credential_operation() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let tools = temp.path().join("gh path' ;touch pwned;echo '$");
        std::fs::create_dir(&tools).unwrap();
        let gh = tools.join("gh");
        std::fs::write(
            &gh,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$MUXY_TEST_LOG\"\nif test \"$3\" = get; then printf 'username=muxy\\npassword=secret\\n'; fi\n",
        )
        .unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o700)).unwrap();
        let real_git = ExecutionEnvironment::from_current_process()
            .resolve_executable(OsStr::new("git"))
            .unwrap();
        std::fs::create_dir(temp.path().join("repo")).unwrap();
        let log = temp.path().join("helper.log");
        let mut variables = environment(&tools).variables();
        variables.push((OsString::from("MUXY_TEST_LOG"), log.as_os_str().to_owned()));
        let environment = ExecutionEnvironment::fallback(variables);
        let options = GitOptions {
            executable: real_git,
            environment: Default::default(),
        };
        let command = repository_command(
            &environment,
            RepositoryCommandRequest {
                args: vec![OsString::from("credential"), OsString::from("fill")],
                read_only: true,
                network: true,
                stdin: StdinMode::Bytes(b"protocol=https\nhost=github.com\n\n".to_vec()),
                stdout_limit: 1_024,
                stderr_limit: 1_024,
                cancellation: None,
            },
        );
        assert_eq!(
            command
                .args
                .iter()
                .filter(|arg| *arg == OsStr::new("credential.helper="))
                .count(),
            1
        );
        assert_eq!(
            command
                .args
                .iter()
                .filter(|arg| {
                    arg.to_string_lossy()
                        .starts_with("credential.https://github.com.helper=")
                })
                .count(),
            1
        );

        let output = run_output(
            &options,
            &temp.path().join("repo"),
            command,
            &Deadline::new(std::time::Duration::from_secs(5)),
        )
        .unwrap();

        assert!(output.status.success());
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "auth\ngit-credential\nget\n"
        );
        for (action, input, expected) in [
            (
                "approve",
                b"protocol=https\nhost=github.com\nusername=muxy\npassword=secret\n\n".as_slice(),
                "auth\ngit-credential\nstore\n",
            ),
            (
                "reject",
                b"protocol=https\nhost=github.com\nusername=muxy\n\n".as_slice(),
                "auth\ngit-credential\nerase\n",
            ),
        ] {
            let command = repository_command(
                &environment,
                RepositoryCommandRequest {
                    args: vec![OsString::from("credential"), OsString::from(action)],
                    read_only: false,
                    network: true,
                    stdin: StdinMode::Bytes(input.to_vec()),
                    stdout_limit: 1_024,
                    stderr_limit: 1_024,
                    cancellation: None,
                },
            );
            let output = run_output(
                &options,
                &temp.path().join("repo"),
                command,
                &Deadline::new(std::time::Duration::from_secs(5)),
            )
            .unwrap();
            assert!(output.status.success());
            assert_eq!(std::fs::read_to_string(&log).unwrap(), expected);
        }
        assert!(!temp.path().join("repo/pwned").exists());
    }
}
