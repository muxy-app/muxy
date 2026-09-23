use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

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
    let mut arguments: Vec<std::ffi::OsString> = Vec::new();
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

fn github_credential_helper() -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let gh = github_cli()?;
    let mut value = b"credential.https://github.com.helper=!'".to_vec();
    for byte in gh.as_os_str().as_bytes() {
        if *byte == b'\'' {
            value.extend_from_slice(b"'\\''");
        } else {
            value.push(*byte);
        }
    }
    value.extend_from_slice(b"' auth git-credential");
    Some(std::ffi::OsString::from_vec(value))
}

/// Finds `gh` beyond the system PATH that a server started from the app inherits,
/// checking Homebrew's folders before asking the login shell.
pub(super) fn github_cli() -> Option<PathBuf> {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    find_executable(
        "gh",
        std::env::split_paths(&inherited)
            .chain(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from))
            .chain(
                std::iter::once_with(crate::exec::login_path)
                    .flatten()
                    .flat_map(std::env::split_paths),
            ),
    )
}

/// Stops at the first match, so later directories are only computed when needed.
pub(super) fn find_executable(
    name: &str,
    directories: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    directories
        .into_iter()
        .map(|directory| directory.join(name))
        .find(|path| {
            path.is_absolute()
                && path.metadata().is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
        })
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
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .process_group(0);
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
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_EDITOR", "true")
        .env("GIT_MERGE_AUTOEDIT", "no");
    command
}

pub(super) fn capture(command: Command) -> Result<Vec<u8>> {
    capture_with(command, Duration::from_secs(20), None, false).map(|output| output.0)
}

pub(super) fn ignored(path: &Path, input: Vec<u8>) -> Result<Vec<u8>> {
    capture_process(
        git_command(
            path,
            &["--no-literal-pathspecs", "check-ignore", "-z", "--stdin"],
        ),
        Duration::from_secs(20),
        None,
        true,
        Some(input),
    )
    .map(|output| output.0)
}

pub(super) fn capture_with(
    command: Command,
    timeout: Duration,
    truncate_stdout: Option<usize>,
    allow_difference: bool,
) -> Result<(Vec<u8>, bool)> {
    capture_process(command, timeout, truncate_stdout, allow_difference, None)
}

fn capture_process(
    mut command: Command,
    timeout: Duration,
    truncate_stdout: Option<usize>,
    allow_difference: bool,
    input: Option<Vec<u8>>,
) -> Result<(Vec<u8>, bool)> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = command.spawn().map_err(error)?;
    let id = child.id();
    let input_task = input
        .zip(child.stdin.take())
        .map(|(bytes, mut pipe)| std::thread::spawn(move || pipe.write_all(&bytes)));
    let (send, receive) = mpsc::sync_channel(16);
    for (stream, pipe) in [
        (
            0,
            Box::new(
                child
                    .stdout
                    .take()
                    .ok_or_else(|| error("missing Git stdout"))?,
            ) as Box<dyn Read + Send>,
        ),
        (
            1,
            Box::new(
                child
                    .stderr
                    .take()
                    .ok_or_else(|| error("missing Git stderr"))?,
            ) as Box<dyn Read + Send>,
        ),
    ] {
        forward_output(stream, pipe, send.clone());
    }
    drop(send);
    let deadline = Instant::now() + timeout;
    let mut output = [Vec::new(), Vec::new()];
    let result = (|| {
        loop {
            if Instant::now() >= deadline {
                return Err(error("Git command timed out; refresh before retrying"));
            }
            match receive.recv_timeout(Duration::from_millis(10)) {
                Ok((stream, bytes)) => {
                    let bytes = bytes.map_err(error)?;
                    if stream == 0
                        && let Some(limit) = truncate_stdout
                        && output[0].len() + bytes.len() > limit
                    {
                        output[0].extend_from_slice(&bytes[..limit - output[0].len()]);
                        terminate_group(id);
                        return Ok((std::mem::take(&mut output[0]), true));
                    }
                    if output[stream].len() + bytes.len() > LIMIT {
                        return Err(error("Git output exceeds the limit"));
                    }
                    output[stream].extend(bytes);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if let Some(status) = child.try_wait().map_err(error)? {
                        return if status.success() || allow_difference && status.code() == Some(1) {
                            Ok((std::mem::take(&mut output[0]), false))
                        } else {
                            let message = String::from_utf8_lossy(&output[1]);
                            Err(error(if message.trim().is_empty() {
                                format!("Command failed with {status}")
                            } else {
                                message.trim().to_owned()
                            }))
                        };
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    })();
    if result.is_err() {
        terminate_group(id);
    }
    let _ = child.wait();
    if let Some(task) = input_task {
        let written = task
            .join()
            .map_err(|_| error("Git input worker failed"))?
            .map_err(error);
        if result.is_ok() {
            written?;
        }
    }
    result
}

fn terminate_group(id: u32) {
    let _ = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{id}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn forward_output(
    stream: usize,
    mut pipe: Box<dyn Read + Send>,
    send: mpsc::SyncSender<(usize, std::io::Result<Vec<u8>>)>,
) {
    std::thread::spawn(move || {
        let mut buffer = [0; 8192];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if send.send((stream, Ok(buffer[..n].to_vec()))).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = send.send((stream, Err(error)));
                    break;
                }
            }
        }
    });
}
