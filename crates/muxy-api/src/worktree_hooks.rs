use crate::subprocess::{Deadline, SubprocessError, SubprocessRequest, run};
use crate::worktree_config::{
    CommandSource, HookKind, ProjectHookApproval, ResolvedCommand, WorktreeConfigError,
    commands_for_execution,
};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookContext {
    pub project_path: PathBuf,
    pub worktree_id: String,
    pub worktree_path: PathBuf,
    pub worktree_name: String,
    pub worktree_branch: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookOptions {
    pub global_config_path: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SetupPolicy {
    SkipAll,
    NativeApproved(ProjectHookApproval),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookCommandOutput {
    pub command: String,
    pub name: Option<String>,
    pub source: CommandSource,
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub struct HookProcessFailure {
    pub command: String,
    pub name: Option<String>,
    pub source: CommandSource,
    pub error: SubprocessError,
}

impl std::fmt::Display for HookProcessFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.command)
    }
}

impl std::fmt::Display for HookCommandOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.command)
    }
}

impl SetupPolicy {
    pub fn native(enabled: bool, approval: ProjectHookApproval) -> Self {
        if enabled {
            Self::NativeApproved(approval)
        } else {
            Self::SkipAll
        }
    }

    pub fn cli() -> Self {
        Self::SkipAll
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorktreeHookError {
    #[error(transparent)]
    Config(#[from] WorktreeConfigError),
    #[error("Worktree hook process failed: {failed}")]
    ProcessFailed {
        completed: Vec<HookCommandOutput>,
        failed: Box<HookProcessFailure>,
    },
    #[error("Worktree hook failed: {failed}")]
    CommandFailed {
        completed: Vec<HookCommandOutput>,
        failed: Box<HookCommandOutput>,
    },
}

pub fn run_setup(
    context: &HookContext,
    policy: SetupPolicy,
    options: &HookOptions,
    deadline: &Deadline,
) -> Result<Vec<HookCommandOutput>, WorktreeHookError> {
    let approval = match policy {
        SetupPolicy::SkipAll => return Ok(Vec::new()),
        SetupPolicy::NativeApproved(approval) => approval,
    };
    let commands = commands_for_execution(
        HookKind::Setup,
        &context.project_path,
        &options.global_config_path,
        Some(&approval),
    )?;
    run_commands(context, options, deadline, commands)
}

pub fn run_teardown(
    context: &HookContext,
    approval: Option<&ProjectHookApproval>,
    options: &HookOptions,
    deadline: &Deadline,
) -> Result<Vec<HookCommandOutput>, WorktreeHookError> {
    let commands = commands_for_execution(
        HookKind::Teardown,
        &context.project_path,
        &options.global_config_path,
        approval,
    )?;
    run_commands(context, options, deadline, commands)
}

pub fn select_shell(configured: Option<&OsStr>) -> Result<PathBuf, WorktreeHookError> {
    Ok(configured
        .map(PathBuf::from)
        .filter(|path| is_executable(path))
        .unwrap_or_else(platform_fallback_shell))
}

pub fn platform_fallback_shell() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/bin/zsh")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        PathBuf::from("/bin/sh")
    }
    #[cfg(windows)]
    {
        PathBuf::from("cmd.exe")
    }
    #[cfg(not(any(unix, windows)))]
    {
        PathBuf::from("sh")
    }
}

fn run_commands(
    context: &HookContext,
    options: &HookOptions,
    deadline: &Deadline,
    commands: Vec<ResolvedCommand>,
) -> Result<Vec<HookCommandOutput>, WorktreeHookError> {
    let environment = hook_environment(context, options);
    let configured_shell = environment_value(&environment, OsStr::new("SHELL"));
    let shell = select_shell(configured_shell.as_deref())?;
    let mut completed = Vec::new();
    for command in commands {
        if deadline.is_expired() {
            return Err(process_failure(
                completed,
                command,
                SubprocessError::TimedOut {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                },
            ));
        }
        let output = match run(
            SubprocessRequest {
                executable: shell.clone(),
                args: shell_arguments(&command.command.command),
                current_dir: Some(context.worktree_path.clone()),
                environment: environment.clone(),
            },
            Some(deadline),
        ) {
            Ok(output) => output,
            Err(error) => return Err(process_failure(completed, command, error)),
        };
        let succeeded = output.status.success();
        let output = HookCommandOutput {
            command: command.command.command,
            name: command.command.name,
            source: command.source,
            status: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        };
        if !succeeded {
            return Err(WorktreeHookError::CommandFailed {
                completed,
                failed: Box::new(output),
            });
        }
        completed.push(output);
    }
    Ok(completed)
}

fn process_failure(
    completed: Vec<HookCommandOutput>,
    command: ResolvedCommand,
    error: SubprocessError,
) -> WorktreeHookError {
    WorktreeHookError::ProcessFailed {
        completed,
        failed: Box::new(HookProcessFailure {
            command: command.command.command,
            name: command.command.name,
            source: command.source,
            error,
        }),
    }
}

fn hook_environment(context: &HookContext, options: &HookOptions) -> Vec<(OsString, OsString)> {
    let mut environment = options.environment.clone();
    let required = [
        (
            OsString::from("MUXY_PROJECT_PATH"),
            context.project_path.as_os_str().to_owned(),
        ),
        (
            OsString::from("MUXY_WORKTREE_ID"),
            OsString::from(&context.worktree_id),
        ),
        (
            OsString::from("MUXY_WORKTREE_PATH"),
            context.worktree_path.as_os_str().to_owned(),
        ),
        (
            OsString::from("MUXY_WORKTREE_NAME"),
            OsString::from(&context.worktree_name),
        ),
        (
            OsString::from("MUXY_WORKTREE_BRANCH"),
            OsString::from(context.worktree_branch.as_deref().unwrap_or_default()),
        ),
    ];
    for (key, value) in required {
        environment.retain(|(existing, _)| existing != &key);
        environment.push((key, value));
    }
    environment
}

fn environment_value(environment: &[(OsString, OsString)], key: &OsStr) -> Option<OsString> {
    environment
        .iter()
        .rev()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var_os(key))
}

#[cfg(unix)]
fn shell_arguments(command: &str) -> Vec<OsString> {
    vec![OsString::from("-c"), OsString::from(command)]
}

#[cfg(windows)]
fn shell_arguments(command: &str) -> Vec<OsString> {
    vec![OsString::from("/C"), OsString::from(command)]
}

#[cfg(not(any(unix, windows)))]
fn shell_arguments(command: &str) -> Vec<OsString> {
    vec![OsString::from("-c"), OsString::from(command)]
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worktree_config::{HookKind, ProjectHookApproval, resolved_commands};
    use std::ffi::{OsStr, OsString};
    use std::time::Duration;

    fn write(path: &std::path::Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn context(path: &std::path::Path) -> HookContext {
        HookContext {
            project_path: path.join("project"),
            worktree_id: "WORKTREE-ID".into(),
            worktree_path: path.join("worktree"),
            worktree_name: "Feature".into(),
            worktree_branch: Some("feature/one".into()),
        }
    }

    fn options(global_config_path: std::path::PathBuf) -> HookOptions {
        HookOptions {
            global_config_path,
            environment: vec![(OsString::from("SHELL"), OsString::from("/bin/sh"))],
        }
    }

    #[cfg(unix)]
    #[test]
    fn worktree_hooks_run_setup_global_to_project_and_teardown_project_to_global_with_environment()
    {
        let temp = tempfile::tempdir().unwrap();
        let context = context(temp.path());
        std::fs::create_dir_all(&context.worktree_path).unwrap();
        let global = temp.path().join("global.json");
        let log = temp.path().join("order.log");
        write(
            &global,
            &format!(
                r#"{{"setup":["test -n \"$PATH\" && printf 'global:%s:%s\\n' \"$MUXY_PROJECT_PATH\" \"$MUXY_WORKTREE_ID\" >> {}"],"teardown":["printf 'global-down\\n' >> {}"]}}"#,
                log.display(),
                log.display()
            ),
        );
        write(
            &context.project_path.join(".muxy/worktree.json"),
            &format!(
                r#"{{"setup":["printf 'project:%s:%s:%s\\n' \"$MUXY_WORKTREE_PATH\" \"$MUXY_WORKTREE_NAME\" \"$MUXY_WORKTREE_BRANCH\" >> {}"],"teardown":["printf 'project-down\\n' >> {}"]}}"#,
                log.display(),
                log.display()
            ),
        );
        let displayed =
            resolved_commands(HookKind::Setup, &context.project_path, &global, true).unwrap();
        let setup_approval = ProjectHookApproval::from_resolved(&displayed);
        let deadline = crate::subprocess::Deadline::new(Duration::from_secs(5));

        run_setup(
            &context,
            SetupPolicy::NativeApproved(setup_approval),
            &options(global.clone()),
            &deadline,
        )
        .unwrap();
        let displayed =
            resolved_commands(HookKind::Teardown, &context.project_path, &global, true).unwrap();
        let teardown_approval = ProjectHookApproval::from_resolved(&displayed);
        run_teardown(
            &context,
            Some(&teardown_approval),
            &options(global),
            &deadline,
        )
        .unwrap();

        let contents = std::fs::read_to_string(log).unwrap();
        assert!(contents.starts_with(&format!(
            "global:{}:WORKTREE-ID\nproject:{}:Feature:feature/one\n",
            context.project_path.display(),
            context.worktree_path.display()
        )));
        assert!(contents.ends_with("project-down\nglobal-down\n"));
    }

    #[cfg(unix)]
    #[test]
    fn worktree_hooks_skip_all_reads_nothing_and_failures_stop_later_commands() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(temp.path());
        std::fs::create_dir_all(&context.worktree_path).unwrap();
        let global = temp.path().join("global.json");
        let sentinel = temp.path().join("sentinel");
        write(&global, "{invalid");
        write(
            &context.project_path.join(".muxy/worktree.json"),
            "{invalid",
        );
        let deadline = crate::subprocess::Deadline::new(Duration::from_secs(5));

        run_setup(
            &context,
            SetupPolicy::SkipAll,
            &options(global.clone()),
            &deadline,
        )
        .unwrap();
        assert!(!sentinel.exists());

        write(
            &global,
            &format!(r#"{{"setup":["exit 7","touch {}"]}}"#, sentinel.display()),
        );
        assert!(
            run_setup(
                &context,
                SetupPolicy::NativeApproved(ProjectHookApproval::default()),
                &options(global),
                &deadline,
            )
            .is_err()
        );
        assert!(!sentinel.exists());
    }

    #[cfg(unix)]
    #[test]
    fn worktree_hooks_share_one_deadline_across_commands() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(temp.path());
        std::fs::create_dir_all(&context.worktree_path).unwrap();
        let global = temp.path().join("global.json");
        let sentinel = temp.path().join("sentinel");
        write(
            &global,
            &format!(
                r#"{{"setup":["sleep 0.3","sleep 0.3; touch {}"]}}"#,
                sentinel.display()
            ),
        );
        let deadline = crate::subprocess::Deadline::new(Duration::from_millis(400));

        assert!(
            run_setup(
                &context,
                SetupPolicy::NativeApproved(ProjectHookApproval::default()),
                &options(global),
                &deadline,
            )
            .is_err()
        );
        assert!(!sentinel.exists());
    }

    #[cfg(unix)]
    #[test]
    fn worktree_hooks_do_not_launch_after_the_shared_deadline_expires() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(temp.path());
        std::fs::create_dir_all(&context.worktree_path).unwrap();
        let global = temp.path().join("global.json");
        let sentinel = temp.path().join("launched");
        write(
            &global,
            &format!(r#"{{"setup":["touch {}"]}}"#, sentinel.display()),
        );
        let deadline = crate::subprocess::Deadline::new(Duration::ZERO);

        assert!(
            run_setup(
                &context,
                SetupPolicy::NativeApproved(ProjectHookApproval::default()),
                &options(global),
                &deadline,
            )
            .is_err()
        );
        assert!(!sentinel.exists());
    }

    #[cfg(unix)]
    #[test]
    fn worktree_hooks_preserve_completed_and_failing_timeout_diagnostics() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(temp.path());
        std::fs::create_dir_all(&context.worktree_path).unwrap();
        let global = temp.path().join("global.json");
        write(
            &global,
            r#"{"setup":[{"command":"printf first","name":"First"},{"command":"printf before-timeout; sleep 1","name":"Second"}]}"#,
        );

        let error = run_setup(
            &context,
            SetupPolicy::NativeApproved(ProjectHookApproval::default()),
            &options(global),
            &crate::subprocess::Deadline::new(Duration::from_millis(200)),
        )
        .unwrap_err();

        let WorktreeHookError::ProcessFailed { completed, failed } = error else {
            panic!("expected process failure");
        };
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].stdout, b"first");
        assert_eq!(failed.command, "printf before-timeout; sleep 1");
        assert_eq!(failed.name.as_deref(), Some("Second"));
        assert_eq!(failed.source, crate::worktree_config::CommandSource::Global);
        assert!(matches!(
            failed.error,
            crate::subprocess::SubprocessError::TimedOut { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn worktree_hooks_return_source_labeled_captured_diagnostics() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(temp.path());
        std::fs::create_dir_all(&context.worktree_path).unwrap();
        let global = temp.path().join("global.json");
        write(&global, r#"{"setup":["printf stdout; printf stderr >&2"]}"#);

        let output = run_setup(
            &context,
            SetupPolicy::NativeApproved(ProjectHookApproval::default()),
            &options(global),
            &crate::subprocess::Deadline::new(Duration::from_secs(5)),
        )
        .unwrap();

        assert_eq!(output.len(), 1);
        assert_eq!(
            output[0].source,
            crate::worktree_config::CommandSource::Global
        );
        assert_eq!(output[0].stdout, b"stdout");
        assert_eq!(output[0].stderr, b"stderr");
    }

    #[cfg(unix)]
    #[test]
    fn worktree_hooks_select_valid_shell_and_platform_fallbacks() {
        let temp = tempfile::tempdir().unwrap();
        let non_executable = temp.path().join("shell");
        std::fs::write(&non_executable, b"").unwrap();

        assert_eq!(
            select_shell(Some(OsStr::new("/bin/sh"))).unwrap(),
            std::path::PathBuf::from("/bin/sh")
        );
        assert_eq!(select_shell(None).unwrap(), platform_fallback_shell());
        assert_eq!(
            select_shell(Some(non_executable.as_os_str())).unwrap(),
            platform_fallback_shell()
        );
    }

    #[test]
    fn worktree_hook_setup_policy_maps_native_and_cli_requests() {
        let approval = ProjectHookApproval::default();
        assert_eq!(SetupPolicy::cli(), SetupPolicy::SkipAll);
        assert_eq!(
            SetupPolicy::native(false, approval.clone()),
            SetupPolicy::SkipAll
        );
        assert_eq!(
            SetupPolicy::native(true, approval.clone()),
            SetupPolicy::NativeApproved(approval)
        );
    }

    #[test]
    fn worktree_hook_environment_deduplicates_authoritative_overrides() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(temp.path());
        let options = HookOptions {
            global_config_path: temp.path().join("global.json"),
            environment: vec![
                (OsString::from("MUXY_WORKTREE_ID"), OsString::from("first")),
                (OsString::from("MUXY_WORKTREE_ID"), OsString::from("last")),
            ],
        };

        let environment = hook_environment(&context, &options);
        let values = environment
            .iter()
            .filter(|(key, _)| key == "MUXY_WORKTREE_ID")
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        assert_eq!(values, vec![&OsString::from("WORKTREE-ID")]);
    }

    #[cfg(unix)]
    #[test]
    fn worktree_hooks_absent_teardown_approval_runs_global_without_loading_project() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(temp.path());
        std::fs::create_dir_all(&context.worktree_path).unwrap();
        let global = temp.path().join("global.json");
        let sentinel = temp.path().join("global-ran");
        write(
            &global,
            &format!(r#"{{"teardown":["touch {}"]}}"#, sentinel.display()),
        );
        write(
            &context.project_path.join(".muxy/worktree.json"),
            "{invalid",
        );

        run_teardown(
            &context,
            None,
            &options(global),
            &crate::subprocess::Deadline::new(Duration::from_secs(5)),
        )
        .unwrap();

        assert!(sentinel.exists());
    }
}
