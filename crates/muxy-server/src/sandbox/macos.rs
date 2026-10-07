use std::ffi::{OsStr, OsString};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use libproc::bsd_info::BSDInfo;
use libproc::proc_pid::pidinfo;
use libproc::processes::{ProcFilter, pids_by_type};
use muxy_protocol::{ErrorCode, SandboxInfo, SandboxNetwork, ServerPath};
use muxy_terminal::pty::{ProcessMonitor, Pty, SpawnRequest};
use rustix::process::{Pid, Signal, kill_process};
use serde_json::json;

use crate::{ServerError, ServerSettings};

use super::NONO_VERSION;

const RUNTIME: &[&str] = &[
    "/bin",
    "/usr/bin",
    "/usr/lib",
    "/usr/share",
    "/System/Library",
    "/System/Cryptexes",
    "/etc/ssl",
    "/etc/hosts",
    "/etc/resolv.conf",
    "/private/var/db/timezone/zoneinfo",
    "/dev/fd",
];

const BOOTSTRAP: &str = r#"
for fd in /dev/fd/*; do
    fd=${fd##*/}
    case "$fd" in
        0|1|2) ;;
        ''|*[!0-9]*) exit 125 ;;
        *) eval "exec $fd>&-" || exit 125 ;;
    esac
done
unset fd
umask 077
printf '%s' "$$" > "$1" || exit 125
shift
exec "$@"
"#;

fn failed(message: impl Into<String>) -> ServerError {
    ServerError::new(ErrorCode::SpawnFailed, message)
}

fn path(value: &ServerPath) -> &Path {
    Path::new(OsStr::from_bytes(&value.0))
}

fn text(path: &Path) -> Result<&str, ServerError> {
    path.to_str()
        .ok_or_else(|| failed("nono requires UTF-8 filesystem paths"))
}

fn canonical(path: &Path) -> Result<PathBuf, ServerError> {
    path.canonicalize()
        .map_err(|error| failed(format!("{}: {error}", path.display())))
}

fn process(pid: u32) -> Option<BSDInfo> {
    pidinfo::<BSDInfo>(i32::try_from(pid).ok()?, 0).ok()
}

fn identity(pid: u32) -> Option<(u64, u64)> {
    process(pid).map(|info| (info.pbi_start_tvsec, info.pbi_start_tvusec))
}

struct Runtime {
    directory: PathBuf,
    child: u32,
    shell: u32,
    started: Option<(u64, u64)>,
    shell_started: Option<(u64, u64)>,
    stopped: bool,
}

impl Runtime {
    fn new() -> Result<Self, ServerError> {
        let directory = std::env::temp_dir().join(format!(
            "muxy-sandbox-{}",
            muxy_protocol::OperationId::new()
        ));
        DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|error| failed(error.to_string()))?;
        Ok(Self {
            directory: canonical(&directory)?,
            child: 0,
            shell: 0,
            started: None,
            shell_started: None,
            stopped: false,
        })
    }

    fn directory(&self, name: &str) -> Result<PathBuf, ServerError> {
        let path = self.directory.join(name);
        DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|error| failed(error.to_string()))?;
        Ok(path)
    }
}

impl ProcessMonitor for Runtime {
    fn shell_pid(&self) -> u32 {
        self.shell
    }

    fn foreground_pid(&self) -> Option<u32> {
        process(self.shell).and_then(|info| (info.e_tpgid > 0).then_some(info.e_tpgid))
    }

    fn terminate(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        let mut descendants = Vec::new();
        for (pid, start) in [(self.child, self.started), (self.shell, self.shell_started)] {
            if pid != 0 && start.is_some() && identity(pid) == start && !descendants.contains(&pid)
            {
                descendants.push(pid);
            }
        }
        let mut index = 0;
        while index < descendants.len() && descendants.len() < 4096 {
            let children = pids_by_type(ProcFilter::ByParentProcess {
                ppid: descendants[index],
            })
            .unwrap_or_default();
            for child in children {
                if !descendants.contains(&child) {
                    descendants.push(child);
                }
            }
            index += 1;
        }
        let processes: Vec<_> = descendants
            .into_iter()
            .filter_map(|pid| identity(pid).map(|start| (pid, start)))
            .collect();
        for &(pid, start) in processes.iter().rev() {
            if identity(pid) == Some(start)
                && let Some(pid) = Pid::from_raw(pid.cast_signed())
            {
                let _ = kill_process(pid, Signal::TERM);
            }
        }
        let deadline = Instant::now() + Duration::from_millis(300);
        while Instant::now() < deadline
            && processes
                .iter()
                .any(|&(pid, start)| identity(pid) == Some(start))
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        for (pid, start) in processes {
            if identity(pid) == Some(start)
                && let Some(pid) = Pid::from_raw(pid.cast_signed())
            {
                let _ = kill_process(pid, Signal::KILL);
            }
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.terminate();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn verify_version(executable: &Path) -> Result<(), ServerError> {
    let mut child = Command::new(executable)
        .arg("--version")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| failed(format!("Could not run nono: {error}")))?;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| failed(error.to_string()))?
        {
            let output = child
                .wait_with_output()
                .map_err(|error| failed(error.to_string()))?;
            if status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .split_whitespace()
                    .nth(1)
                    == Some(NONO_VERSION)
            {
                return Ok(());
            }
            return Err(failed(format!(
                "This Muxy build requires nono {NONO_VERSION}"
            )));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failed("nono version check timed out"));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), ServerError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| failed(error.to_string()))
}

fn environment(request: &SpawnRequest, sandbox: &SandboxInfo) -> Vec<(OsString, OsString)> {
    let permitted = [
        "TERM",
        "COLORTERM",
        "TERM_PROGRAM",
        "LANG",
        "LC_CTYPE",
        "LC_ALL",
    ];
    request
        .env
        .iter()
        .filter(|(name, _)| {
            name.to_str().is_some_and(|name| {
                permitted.contains(&name)
                    || sandbox
                        .spec
                        .policy
                        .environment
                        .iter()
                        .any(|approved| approved == name)
            })
        })
        .cloned()
        .collect()
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep fail-closed preparation, launch and readiness in one sequence"
)]
pub(crate) fn spawn(
    mut request: SpawnRequest,
    settings: &ServerSettings,
    sandbox: &SandboxInfo,
) -> Result<Pty, ServerError> {
    let executable = settings
        .sandbox
        .as_ref()
        .and_then(|settings| settings.executable.as_ref())
        .ok_or_else(|| {
            failed(
                "Set the nono executable in Server settings before creating a sandboxed terminal",
            )
        })?;
    let executable = canonical(path(executable))?;
    let workspace = canonical(path(&sandbox.spec.workspace))?;
    let cwd = canonical(&request.cwd)?;
    let host_home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|path| path.canonicalize().ok());
    if workspace == Path::new("/")
        || host_home
            .as_ref()
            .is_some_and(|home| home.starts_with(&workspace))
        || !workspace.is_dir()
        || !cwd.starts_with(&workspace)
        || executable.starts_with(&workspace)
    {
        return Err(failed(
            "Choose a workspace inside your home or a project directory; it must contain the starting folder and exclude the nono executable",
        ));
    }
    verify_version(&executable)?;
    let mut runtime = Runtime::new()?;
    if runtime.directory.starts_with(&workspace) {
        return Err(failed(
            "The workspace must not contain sandbox runtime state",
        ));
    }
    let launcher = runtime.directory("launcher")?;
    let home = runtime.directory("home")?;
    let temporary = runtime.directory("tmp")?;
    let hooks = runtime.directory("hooks")?;
    write(
        &hooks.join("muxy.zsh"),
        include_bytes!("../../shell/muxy.zsh"),
    )?;
    write(
        &hooks.join("muxy.bash"),
        include_bytes!("../../shell/muxy.bash"),
    )?;
    write(
        &hooks.join("muxy.fish"),
        include_bytes!("../../shell/muxy.fish"),
    )?;
    if settings.shell_integration {
        write(
            &home.join(".zshrc"),
            b"source \"$MUXY_SHELL_INTEGRATION_DIR/muxy.zsh\"\n",
        )?;
    }
    let mut grants = Vec::new();
    let mut bins = vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")];
    for root in RUNTIME
        .iter()
        .map(Path::new)
        .chain(sandbox.spec.policy.read_paths.iter().map(path))
        .chain(std::iter::once(request.program.as_path()))
    {
        if !root.exists() && RUNTIME.contains(&root.to_str().unwrap_or_default()) {
            continue;
        }
        let requested = root;
        let root = canonical(root)?;
        if root == Path::new("/")
            || runtime.directory.starts_with(&root)
            || host_home
                .as_ref()
                .is_some_and(|home| home.starts_with(&root))
        {
            return Err(failed(
                "A tool grant must not expose the user home or sandbox runtime state",
            ));
        }
        grants.push(json!({"path": text(requested)?, "access": "read", "type": if root.is_dir() {"directory"} else {"file"}}));
        if !RUNTIME.iter().any(|system| Path::new(system) == root) {
            if root.is_dir() {
                bins.push(if root.join("bin").is_dir() {
                    root.join("bin")
                } else {
                    root.clone()
                });
            } else if let Some(parent) = root.parent() {
                bins.push(parent.to_path_buf());
            }
        }
    }
    grants.push(json!({"path": text(&hooks)?, "access": "read"}));
    for root in [&workspace, &home, &temporary] {
        grants.push(json!({"path": text(root)?, "access": "readwrite"}));
    }
    for device in ["/dev/null", "/dev/tty"] {
        grants.push(json!({"path": device, "access": "readwrite", "type": "file"}));
    }
    let proxy = sandbox.spec.policy.network == SandboxNetwork::Domains;
    let manifest = json!({
        "version": "0.1.0", "filesystem": {"grants": grants},
        "network": {"mode": if proxy {"proxy"} else {"blocked"}, "dns": false, "allow_domains": sandbox.spec.policy.domains},
        "rollback": {"enabled": false}, "process": {"exec_strategy": if proxy {"supervised"} else {"direct"}}
    });
    let manifest_path = runtime.directory.join("policy.json");
    write(
        &manifest_path,
        &serde_json::to_vec(&manifest).map_err(|error| failed(error.to_string()))?,
    )?;
    let ready = home.join(".muxy-ready");
    let shell = request.program.clone();
    let mut args: Vec<OsString> = vec![
        if proxy { "run" } else { "wrap" }.into(),
        "--config".into(),
        manifest_path.into_os_string(),
        "--silent".into(),
    ];
    if proxy {
        args.extend(
            ["--no-audit", "--no-diagnostics", "--startup-timeout", "0"].map(OsString::from),
        );
    }
    args.extend([OsString::from("--"), OsString::from("/usr/bin/env")]);
    for (name, value) in [
        ("HOME", &home),
        ("XDG_CONFIG_HOME", &home),
        ("XDG_STATE_HOME", &home),
        ("XDG_CACHE_HOME", &home),
        ("TMPDIR", &temporary),
    ] {
        let mut item = OsString::from(format!("{name}="));
        item.push(value);
        args.push(item);
    }
    args.extend(["/bin/sh", "-c", BOOTSTRAP, "muxy-sandbox"].map(OsString::from));
    args.push(ready.clone().into_os_string());
    args.push(shell.clone().into_os_string());
    match shell.file_name().and_then(OsStr::to_str) {
        Some("zsh") if settings.shell_integration => args.push("-i".into()),
        Some("zsh") => args.extend(["-f", "-i"].map(OsString::from)),
        Some("bash") if settings.shell_integration => args.extend([
            OsString::from("--noprofile"),
            OsString::from("--rcfile"),
            hooks.join("muxy.bash").into_os_string(),
            OsString::from("-i"),
        ]),
        Some("fish") if settings.shell_integration => args.extend(
            [
                "--no-config",
                "-i",
                "-C",
                "source \"$MUXY_SHELL_INTEGRATION_DIR/muxy.fish\"",
            ]
            .map(OsString::from),
        ),
        Some("bash") => args.extend(["--noprofile", "--norc", "-i"].map(OsString::from)),
        Some("fish") => args.extend(["--no-config", "-i"].map(OsString::from)),
        _ => args.push("-i".into()),
    }
    request.env = environment(&request, sandbox);
    request.env.extend([
        ("HOME".into(), launcher.clone().into_os_string()),
        ("XDG_STATE_HOME".into(), launcher.clone().into_os_string()),
        ("XDG_CONFIG_HOME".into(), launcher.into_os_string()),
        ("TMPDIR".into(), temporary.into_os_string()),
        (
            "PATH".into(),
            std::env::join_paths(bins).map_err(|error| failed(error.to_string()))?,
        ),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
    ]);
    request.env.extend([
        (
            "MUXY_SHELL_INTEGRATION".into(),
            if settings.shell_integration { "1" } else { "0" }.into(),
        ),
        ("MUXY_SHELL_INTEGRATION_DIR".into(), hooks.into_os_string()),
    ]);
    for root in &sandbox.spec.policy.read_paths {
        let root = canonical(path(root))?;
        let sdk = root.join("SDKs/MacOSX.sdk");
        if sdk.is_dir() {
            request
                .env
                .push(("DEVELOPER_DIR".into(), root.into_os_string()));
            request.env.push(("SDKROOT".into(), sdk.into_os_string()));
            break;
        }
    }
    request.clear_env = true;
    request.program = executable;
    request.args = args;
    request.cwd = cwd;
    let mut pty = Pty::spawn(request).map_err(ServerError::spawn_failed)?;
    runtime.child = pty.child_pid();
    runtime.started = identity(runtime.child);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(value) = fs::read_to_string(&ready)
            && let Ok(pid) = value.parse::<u32>()
            && process(pid)
                .is_some_and(|info| pid == runtime.child || info.pbi_ppid == runtime.child)
        {
            runtime.shell = pid;
            runtime.shell_started = identity(pid);
            let _ = fs::remove_file(&ready);
            pty.set_monitor(Box::new(runtime));
            return Ok(pty);
        }
        if pty.try_wait().is_some() || Instant::now() >= deadline {
            runtime.terminate();
            let _ = pty.kill();
            return Err(failed(
                "nono could not start the sandboxed shell. Check the approved tool/runtime paths and backend version; no ordinary terminal was started.",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests;
