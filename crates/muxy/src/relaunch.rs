use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};

const HELPER_ARGUMENT: &str = "--p8-relaunch-helper";
const READY_BYTE: u8 = b'R';
const COMMIT_BYTE: u8 = b'C';
const READY_TIMEOUT: Duration = Duration::from_secs(2);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct PreparedRelaunch {
    child: Child,
    input: Option<ChildStdin>,
    committed: bool,
}

impl PreparedRelaunch {
    pub fn prepare() -> Result<Self, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("failed to resolve the Muxy executable: {error}"))?;
        let target = relaunch_target(&executable);
        let mut child = Command::new(&executable)
            .arg(HELPER_ARGUMENT)
            .arg(std::process::id().to_string())
            .arg(target.kind)
            .arg(target.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("failed to prepare the restart helper: {error}"))?;
        let Some(input) = child.stdin.take() else {
            stop_child(&mut child);
            return Err("restart helper commit pipe is unavailable".to_owned());
        };
        let Some(mut output) = child.stdout.take() else {
            stop_child(&mut child);
            return Err("restart helper readiness pipe is unavailable".to_owned());
        };
        let output_flags = unsafe { libc::fcntl(output.as_raw_fd(), libc::F_GETFL) };
        if output_flags < 0
            || unsafe {
                libc::fcntl(
                    output.as_raw_fd(),
                    libc::F_SETFL,
                    output_flags | libc::O_NONBLOCK,
                )
            } < 0
        {
            stop_child(&mut child);
            return Err("failed to configure restart helper readiness".to_owned());
        }
        let deadline = Instant::now() + READY_TIMEOUT;
        let mut ready = [0u8; 1];
        loop {
            match output.read(&mut ready) {
                Ok(1) if ready[0] == READY_BYTE => break,
                Ok(0) => {
                    stop_child(&mut child);
                    return Err("restart helper exited before becoming ready".to_owned());
                }
                Ok(_) => {
                    stop_child(&mut child);
                    return Err("restart helper sent an invalid readiness byte".to_owned());
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    stop_child(&mut child);
                    return Err(format!("failed to read restart helper readiness: {error}"));
                }
            }
            if Instant::now() >= deadline {
                stop_child(&mut child);
                return Err("restart helper did not become ready in time".to_owned());
            }
        }
        Ok(Self {
            child,
            input: Some(input),
            committed: false,
        })
    }

    pub fn commit(mut self) -> Result<(), String> {
        if muxy_core::prefs::is_test_process()
            && std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").is_some()
            && matches!(
                std::env::var("MUXY_TEST_P8_RESTART_FAILURE").as_deref(),
                Ok("enable-commit" | "disable-commit")
            )
        {
            return Err("injected restart commit-pipe failure".to_owned());
        }
        let mut input = self
            .input
            .take()
            .ok_or_else(|| "restart helper commit pipe is closed".to_owned())?;
        input
            .write_all(&[COMMIT_BYTE])
            .and_then(|_| input.flush())
            .map_err(|error| format!("failed to commit restart: {error}"))?;
        self.committed = true;
        Ok(())
    }
}

fn stop_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for PreparedRelaunch {
    fn drop(&mut self) {
        self.input.take();
        if self.committed {
            return;
        }
        let deadline = Instant::now() + Duration::from_millis(250);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct RelaunchTarget {
    kind: &'static str,
    path: PathBuf,
}

fn relaunch_target(executable: &Path) -> RelaunchTarget {
    #[cfg(target_os = "macos")]
    if let Some(app) = executable
        .parent()
        .filter(|path| path.file_name() == Some(OsStr::new("MacOS")))
        .and_then(Path::parent)
        .filter(|path| path.file_name() == Some(OsStr::new("Contents")))
        .and_then(Path::parent)
        .filter(|path| path.extension() == Some(OsStr::new("app")))
    {
        return RelaunchTarget {
            kind: "app",
            path: app.to_path_buf(),
        };
    }
    RelaunchTarget {
        kind: "executable",
        path: executable.to_path_buf(),
    }
}

pub fn run_helper_if_requested() -> bool {
    let mut arguments = std::env::args_os();
    let _ = arguments.next();
    if arguments.next().as_deref() != Some(OsStr::new(HELPER_ARGUMENT)) {
        return false;
    }
    if let Err(error) = run_helper(arguments.collect()) {
        eprintln!("Muxy restart helper failed: {error}");
    }
    true
}

fn run_helper(arguments: Vec<OsString>) -> Result<(), String> {
    let [old_pid, kind, path] = arguments.as_slice() else {
        return Err("restart helper received invalid arguments".to_owned());
    };
    let old_pid = old_pid
        .to_str()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid > 0)
        .ok_or_else(|| "restart helper received an invalid process ID".to_owned())?;
    std::io::stdout()
        .write_all(&[READY_BYTE])
        .and_then(|_| std::io::stdout().flush())
        .map_err(|error| format!("restart helper readiness failed: {error}"))?;
    let mut commit = [0u8; 1];
    match std::io::stdin().read_exact(&mut commit) {
        Ok(()) if commit[0] == COMMIT_BYTE => {}
        Ok(()) => return Err("restart helper received an invalid commit byte".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
        Err(error) => return Err(format!("restart helper commit failed: {error}")),
    }
    wait_for_exit(old_pid)?;
    let mut command = match kind.to_str() {
        Some("app") if cfg!(target_os = "macos") => {
            let mut command = Command::new("/usr/bin/open");
            command.arg("-n");
            if muxy_core::prefs::is_test_process()
                && std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").is_some()
            {
                for key in [
                    "MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY",
                    "MUXY_TEST_P8_SESSION_SOCKET_PATH",
                    "MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST",
                    "MUXY_TEST_P8_DISABLE_RESTART",
                    "MUXY_TEST_P8_ENABLE_RESTART",
                    "MUXY_TEST_P8_RECOVERY_ACTION",
                    "MUXY_TEST_P8_RESTART_FAILURE",
                    "HOME",
                    "CFFIXED_USER_HOME",
                    "TMPDIR",
                    "XDG_CONFIG_HOME",
                ] {
                    if let Some(value) = std::env::var_os(key) {
                        let mut assignment = OsString::from(key);
                        assignment.push("=");
                        assignment.push(value);
                        command.arg("--env").arg(assignment);
                    }
                }
            }
            command.arg(path);
            command
        }
        Some("executable") => Command::new(path),
        _ => return Err("restart helper received an invalid target".to_owned()),
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to relaunch Muxy: {error}"))?;
    Ok(())
}

fn wait_for_exit(process_id: u32) -> Result<(), String> {
    let process_id = i32::try_from(process_id)
        .map_err(|_| "restart helper process ID is out of range".to_owned())?;
    let deadline = Instant::now() + EXIT_TIMEOUT;
    loop {
        let result = unsafe { libc::kill(process_id, 0) };
        if result != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("old Muxy process did not exit in time".to_owned());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbundled_target_relaunches_the_executable() {
        let target = relaunch_target(Path::new("/tmp/muxy"));
        assert_eq!(target.kind, "executable");
        assert_eq!(target.path, Path::new("/tmp/muxy"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn bundled_target_relaunches_the_containing_app() {
        let target = relaunch_target(Path::new("/tmp/Muxy.app/Contents/MacOS/muxy"));
        assert_eq!(target.kind, "app");
        assert_eq!(target.path, Path::new("/tmp/Muxy.app"));
    }
}
