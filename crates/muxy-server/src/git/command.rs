use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use muxy_core::process::{self, Cap, Limits};

use super::{Result, error};

const LIMIT: usize = 4 * 1024 * 1024;

pub(super) fn run(path: &Path, args: &[impl AsRef<OsStr>]) -> Result<Vec<u8>> {
    capture(git_command(path, args))
}

pub(super) fn run_with_index(
    path: &Path,
    index: &Path,
    args: &[impl AsRef<OsStr>],
) -> Result<Vec<u8>> {
    let mut command = git_command(path, args);
    command.env("GIT_INDEX_FILE", index);
    capture(command)
}

pub(super) fn network(path: &Path, args: &[impl AsRef<OsStr>]) -> Result<Vec<u8>> {
    let mut arguments: Vec<OsString> = Vec::new();
    if let Some(helper) = github_credential_helper() {
        arguments.extend(["-c".into(), helper]);
    }
    arguments.extend(args.iter().map(|arg| arg.as_ref().to_owned()));
    capture_with(
        git_command(path, &arguments),
        Duration::from_secs(60),
        None,
        false,
    )
    .map(|output| output.0)
}

fn github_credential_helper() -> Option<OsString> {
    let gh = github_cli()?;
    let mut value = OsString::from("credential.https://github.com.helper=!'");
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let mut quoted = Vec::new();
        for byte in gh.as_os_str().as_bytes() {
            if *byte == b'\'' {
                quoted.extend_from_slice(b"'\\''");
            } else {
                quoted.push(*byte);
            }
        }
        value.push(OsString::from_vec(quoted));
    }
    #[cfg(windows)]
    value.push(gh.to_str()?.replace('\'', "'\\''"));
    value.push("' auth git-credential");
    Some(value)
}

pub(super) fn github_cli() -> Option<PathBuf> {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let homebrew: &[&str] = if cfg!(windows) {
        &[]
    } else {
        &["/opt/homebrew/bin", "/usr/local/bin"]
    };
    crate::exec::find_program(
        "gh",
        std::env::split_paths(&inherited)
            .chain(homebrew.iter().map(PathBuf::from))
            .chain(
                std::iter::once_with(crate::exec::login_path)
                    .flatten()
                    .flat_map(std::env::split_paths),
            )
            .filter(|directory| directory.is_absolute()),
    )
}

pub(super) fn diff(
    path: &Path,
    args: &[impl AsRef<OsStr>],
    no_index: bool,
) -> Result<(Vec<u8>, bool)> {
    capture_with(
        git_command(path, args),
        Duration::from_secs(20),
        Some(1024 * 1024),
        no_index,
    )
}

fn git_command(path: &Path, args: &[impl AsRef<OsStr>]) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-pager")
        .arg("--literal-pathspecs")
        .args(args)
        .current_dir(path)
        .env("LC_ALL", "C");
    without_relative_path(&mut command);
    without_inherited_git(&mut command);
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_EDITOR", "true")
        .env("GIT_MERGE_AUTOEDIT", "no");
    command
}

pub(super) fn without_relative_path(command: &mut Command) {
    if let Some(path) = std::env::var_os("PATH").and_then(|path| absolute_folders(&path)) {
        command.env("PATH", path);
    }
}

fn absolute_folders(path: &OsStr) -> Option<OsString> {
    let kept: Vec<PathBuf> = std::env::split_paths(path)
        .filter(|folder| folder.is_absolute())
        .collect();
    if kept.is_empty() {
        return Some(system_path());
    }
    std::env::join_paths(kept).ok()
}

#[cfg(unix)]
fn system_path() -> OsString {
    "/usr/local/bin:/usr/bin:/bin".into()
}

#[cfg(windows)]
fn system_path() -> OsString {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let root = PathBuf::from(root);
    std::env::join_paths([root.join("System32"), root])
        .unwrap_or_else(|_| r"C:\Windows\System32".into())
}

pub(super) fn without_inherited_git(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        let key = name.to_string_lossy();
        if key.starts_with("GIT_")
            && !matches!(
                key.as_ref(),
                "GIT_SSH" | "GIT_SSH_COMMAND" | "GIT_SSH_VARIANT" | "GIT_ASKPASS"
            )
        {
            command.env_remove(name);
        }
    }
    command.env("GIT_TERMINAL_PROMPT", "0");
}

pub(super) fn capture(command: Command) -> Result<Vec<u8>> {
    capture_with(command, Duration::from_secs(20), None, false).map(|output| output.0)
}

pub(super) fn ignored(path: &Path, input: &[u8]) -> Result<Vec<u8>> {
    let command = git_command(
        path,
        &["--no-literal-pathspecs", "check-ignore", "-z", "--stdin"],
    );
    let limits = Limits {
        stdin: Some(input),
        ..Limits::new(Duration::from_secs(20), LIMIT)
    };
    finish(command, limits, false, true).map(|output| output.0)
}

pub(super) fn capture_with(
    command: Command,
    timeout: Duration,
    truncate_stdout: Option<usize>,
    allow_difference: bool,
) -> Result<(Vec<u8>, bool)> {
    let limits = Limits {
        stdout: Cap::StopAt(truncate_stdout.unwrap_or(LIMIT)),
        ..Limits::new(timeout, LIMIT)
    };
    finish(command, limits, truncate_stdout.is_some(), allow_difference)
}

fn finish(
    mut command: Command,
    limits: Limits<'_>,
    truncates: bool,
    allow_difference: bool,
) -> Result<(Vec<u8>, bool)> {
    let output = process::run(&mut command, limits).map_err(error)?;
    if output.timed_out {
        return Err(error("Git command timed out; refresh before retrying"));
    }
    if output.stderr.truncated || output.stdout.truncated && !truncates {
        return Err(error("Git output exceeds the limit"));
    }
    if output.stdout.truncated {
        return Ok((output.stdout.bytes, true));
    }
    let status = output.status;
    if status.success() || allow_difference && status.code() == Some(1) {
        return Ok((output.stdout.bytes, false));
    }
    let message = String::from_utf8_lossy(&output.stderr.bytes);
    Err(error(if message.trim().is_empty() {
        format!("Command failed with {status}")
    } else {
        message.trim().to_owned()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_absolute_path_folders_are_kept() {
        let folders = [
            PathBuf::from(if cfg!(windows) { r"C:\bin" } else { "/usr/bin" }),
            PathBuf::new(),
            PathBuf::from("."),
            PathBuf::from("node_modules/.bin"),
            PathBuf::from(if cfg!(windows) {
                r"D:\tools"
            } else {
                "/opt/bin"
            }),
        ];
        let inherited = std::env::join_paths(&folders).unwrap_or_default();
        let kept = absolute_folders(&inherited).unwrap_or_default();
        assert_eq!(
            std::env::split_paths(&kept).collect::<Vec<_>>(),
            [folders[0].clone(), folders[4].clone()]
        );
        let relative = std::env::join_paths(&folders[1..4]).unwrap_or_default();
        let kept = absolute_folders(&relative).unwrap_or_default();
        assert!(!kept.is_empty());
        assert!(std::env::split_paths(&kept).all(|folder| folder.is_absolute()));
    }
}
