use std::error::Error;
use std::path::Path;
#[cfg(not(target_os = "macos"))]
use std::time::{Duration, Instant};

/// More than a launch takes to reach the running app and hear back from it.
#[cfg(not(target_os = "macos"))]
const HANDOFF_PATIENCE: Duration = Duration::from_secs(5);
#[cfg(not(target_os = "macos"))]
const START_PATIENCE: Duration = Duration::from_secs(10);
#[cfg(not(target_os = "macos"))]
const START_POLL: Duration = Duration::from_millis(50);

/// Opens `word`, a folder, in the desktop app. Without it, or over SSH, the
/// folder opens in the terminal UI instead.
pub(crate) fn run(word: &Path) -> Result<(), Box<dyn Error>> {
    let folder = word
        .canonicalize()
        .ok()
        .filter(|path| path.is_dir())
        .ok_or_else(|| {
            format!(
                "{} is not a command or a folder; run muxy --help",
                word.display()
            )
        })?;
    if crate::terminal::over_ssh() || !desktop_app(Some(&folder)) {
        crate::tui::run(None, Some(&folder))?;
    }
    Ok(())
}

/// Opens the desktop app, with `folder` as a project if given, returning
/// false if it can't.
#[cfg(target_os = "macos")]
pub(crate) fn desktop_app(folder: Option<&Path>) -> bool {
    use std::process::{Command, Stdio};
    let Ok(executable) = muxy_core::executable::current_path() else {
        return false;
    };
    let mut open = Command::new("/usr/bin/open");
    match containing_app(&executable) {
        Some(app) => open.arg("-a").arg(app),
        None => open.args([
            "-b",
            muxy_core::release::Channel::current().bundle_identifier(),
        ]),
    };
    open.args(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// On Linux the app is installed beside this tool. It needs a desktop
/// session to open in.
#[cfg(not(target_os = "macos"))]
pub(crate) fn desktop_app(folder: Option<&Path>) -> bool {
    let Ok(executable) = muxy_core::executable::current_path() else {
        return false;
    };
    desktop_session(|name| std::env::var_os(name))
        && launch(&executable.with_file_name("muxy-app"), folder)
}

#[cfg(not(target_os = "macos"))]
fn desktop_session(variable: impl Fn(&str) -> Option<std::ffi::OsString>) -> bool {
    ["WAYLAND_DISPLAY", "DISPLAY"]
        .iter()
        .any(|name| variable(name).is_some_and(|value| !value.is_empty()))
}

/// Starts the app with `folder`, away from this terminal, and reports whether
/// it opened. An app that is already running takes the folder from the new one,
/// which then leaves with the answer. A new one that stays is opened once its
/// socket answers. Running out of time counts as opened, since an app too old
/// to have that socket never answers.
#[cfg(not(target_os = "macos"))]
fn launch(app: &Path, folder: Option<&Path>) -> bool {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let socket = muxy_core::dirs::muxy_dir()
        .ok()
        .map(|profile| muxy_app_core::browser::api::socket_path(&profile));
    let handoff = socket.as_deref().is_some_and(app_answers);
    let Ok(mut child) = Command::new(app)
        .args(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
    else {
        return false;
    };
    if handoff {
        handed_over(&mut child, HANDOFF_PATIENCE)
    } else {
        started(&mut child, socket.as_deref(), START_PATIENCE)
    }
}

/// Connecting without sending a request is harmless: the app replies with an
/// error and closes.
#[cfg(not(target_os = "macos"))]
fn app_answers(socket: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket).is_ok()
}

#[cfg(not(target_os = "macos"))]
fn handed_over(child: &mut std::process::Child, patience: Duration) -> bool {
    match muxy_core::process::wait_timeout(child, patience) {
        Ok(Some(status)) => status.success(),
        _ => true,
    }
}

#[cfg(not(target_os = "macos"))]
fn started(child: &mut std::process::Child, socket: Option<&Path>, patience: Duration) -> bool {
    let deadline = Instant::now() + patience;
    while Instant::now() < deadline {
        match muxy_core::process::wait_timeout(child, START_POLL) {
            Ok(Some(status)) => return status.success(),
            Ok(None) if socket.is_some_and(app_answers) => return true,
            Ok(None) => {}
            Err(_) => return true,
        }
    }
    true
}

#[cfg(target_os = "macos")]
fn containing_app(executable: &Path) -> Option<&Path> {
    let executables = executable.parent()?;
    let contents = executables.parent()?;
    let app = contents.parent()?;
    (executables.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app")
        .then_some(app)
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use std::ffi::OsString;
    use std::io;
    use std::os::unix::net::UnixListener;
    use std::process::{Child, Command};

    use super::*;

    const PATIENCE: Duration = Duration::from_millis(1500);

    fn shell(script: &str) -> io::Result<Child> {
        Command::new("sh").args(["-c", script]).spawn()
    }

    fn stop(mut child: Child) -> io::Result<()> {
        child.kill()?;
        child.wait().map(drop)
    }

    #[test]
    fn a_launch_answers_with_how_it_exited() -> io::Result<()> {
        assert!(!handed_over(&mut shell("sleep 0.6; exit 1")?, PATIENCE));
        assert!(!started(&mut shell("sleep 0.6; exit 1")?, None, PATIENCE));
        assert!(handed_over(&mut shell("exit 0")?, PATIENCE));
        assert!(started(&mut shell("exit 0")?, None, PATIENCE));
        Ok(())
    }

    #[test]
    fn a_launch_still_running_at_the_deadline_counts_as_opened() -> io::Result<()> {
        let patience = Duration::from_millis(100);
        let mut running = shell("exec sleep 30")?;
        assert!(handed_over(&mut running, patience));
        assert!(started(&mut running, None, patience));
        stop(running)
    }

    #[test]
    fn a_new_app_has_started_once_its_socket_answers() -> io::Result<()> {
        let folder = tempfile::tempdir()?;
        let socket = folder.path().join("app.sock");
        assert!(!app_answers(&socket));
        let _listener = UnixListener::bind(&socket)?;
        assert!(app_answers(&socket));

        let mut running = shell("exec sleep 30")?;
        let before = Instant::now();
        assert!(started(
            &mut running,
            Some(&socket),
            Duration::from_secs(20)
        ));
        assert!(before.elapsed() < Duration::from_secs(10));
        stop(running)
    }

    #[test]
    fn the_desktop_app_opens_only_in_a_desktop_session() {
        let session = |set: &'static [(&'static str, &'static str)]| {
            desktop_session(move |name| {
                set.iter()
                    .find(|(variable, _)| *variable == name)
                    .map(|(_, value)| OsString::from(value))
            })
        };
        assert!(session(&[("WAYLAND_DISPLAY", "wayland-0")]));
        assert!(session(&[("DISPLAY", ":0")]));
        assert!(!session(&[]), "a console or a service has no desktop");
        assert!(!session(&[("DISPLAY", ""), ("WAYLAND_DISPLAY", "")]));
    }
}
