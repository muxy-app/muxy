use std::error::Error;
use std::path::Path;
#[cfg(not(target_os = "macos"))]
use std::time::{Duration, Instant};

#[cfg(not(target_os = "macos"))]
use muxy_client::local::Detached;

#[cfg(not(target_os = "macos"))]
const HANDOFF_PATIENCE: Duration = Duration::from_secs(5);
#[cfg(not(target_os = "macos"))]
const START_PATIENCE: Duration = Duration::from_secs(10);
#[cfg(not(target_os = "macos"))]
const START_POLL: Duration = Duration::from_millis(50);

pub(crate) fn run(word: &Path) -> Result<(), Box<dyn Error>> {
    let folder = muxy_core::files::canonicalize(word)
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

#[cfg(not(target_os = "macos"))]
pub(crate) fn desktop_app(folder: Option<&Path>) -> bool {
    let Ok(executable) = muxy_core::executable::current_path() else {
        return false;
    };
    (cfg!(windows) || desktop_session(|name| std::env::var_os(name)))
        && launch(
            &muxy_core::executable::sibling(&executable, "muxy-app"),
            folder,
        )
}

#[cfg(not(target_os = "macos"))]
fn desktop_session(variable: impl Fn(&str) -> Option<std::ffi::OsString>) -> bool {
    ["WAYLAND_DISPLAY", "DISPLAY"]
        .iter()
        .any(|name| variable(name).is_some_and(|value| !value.is_empty()))
}

#[cfg(not(target_os = "macos"))]
fn launch(app: &Path, folder: Option<&Path>) -> bool {
    let socket = muxy_core::dirs::muxy_dir()
        .ok()
        .map(|profile| muxy_app_core::browser::api::socket_path(&profile));
    let handoff = socket.as_deref().is_some_and(app_answers);
    let arguments: Vec<_> = folder
        .map(|folder| folder.as_os_str().to_owned())
        .into_iter()
        .collect();
    let Ok(mut child) = muxy_client::local::spawn_detached(app, &arguments) else {
        return false;
    };
    if handoff {
        handed_over(&mut child, HANDOFF_PATIENCE)
    } else {
        started(&mut child, socket.as_deref(), START_PATIENCE)
    }
}

#[cfg(not(target_os = "macos"))]
fn app_answers(socket: &Path) -> bool {
    muxy_protocol::transport::LocalStream::connect(socket).is_ok()
}

#[cfg(not(target_os = "macos"))]
fn handed_over(child: &mut Detached, patience: Duration) -> bool {
    match wait_timeout(child, patience) {
        Ok(Some(status)) => status.success(),
        _ => true,
    }
}

#[cfg(not(target_os = "macos"))]
fn started(child: &mut Detached, socket: Option<&Path>, patience: Duration) -> bool {
    let deadline = Instant::now() + patience;
    while Instant::now() < deadline {
        match wait_timeout(child, START_POLL) {
            Ok(Some(status)) => return status.success(),
            Ok(None) if socket.is_some_and(app_answers) => return true,
            Ok(None) => {}
            Err(_) => return true,
        }
    }
    true
}

#[cfg(not(target_os = "macos"))]
fn wait_timeout(
    child: &mut Detached,
    timeout: Duration,
) -> std::io::Result<Option<std::process::ExitStatus>> {
    #[cfg(unix)]
    {
        muxy_core::process::wait_timeout(child, timeout)
    }
    #[cfg(windows)]
    {
        child.wait_timeout(timeout)
    }
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
    #[cfg(unix)]
    use std::io;
    #[cfg(unix)]
    use std::os::unix::net::UnixListener;
    #[cfg(unix)]
    use std::process::{Child, Command};

    use super::*;

    #[cfg(unix)]
    const PATIENCE: Duration = Duration::from_secs(10);

    #[cfg(unix)]
    fn shell(script: &str) -> io::Result<Child> {
        Command::new("sh").args(["-c", script]).spawn()
    }

    #[cfg(unix)]
    fn stop(mut child: Child) -> io::Result<()> {
        child.kill()?;
        child.wait().map(drop)
    }

    #[cfg(unix)]
    #[test]
    fn a_launch_answers_with_how_it_exited() -> io::Result<()> {
        assert!(!handed_over(&mut shell("sleep 0.6; exit 1")?, PATIENCE));
        assert!(!started(&mut shell("sleep 0.6; exit 1")?, None, PATIENCE));
        assert!(handed_over(&mut shell("exit 0")?, PATIENCE));
        assert!(started(&mut shell("exit 0")?, None, PATIENCE));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_launch_still_running_at_the_deadline_counts_as_opened() -> io::Result<()> {
        let patience = Duration::from_millis(100);
        let mut running = shell("exec sleep 30")?;
        assert!(handed_over(&mut running, patience));
        assert!(started(&mut running, None, patience));
        stop(running)
    }

    #[cfg(unix)]
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
