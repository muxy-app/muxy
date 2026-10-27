use std::error::Error;
use std::path::Path;

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

/// Starts the app with `folder`, away from this terminal. An app that is
/// already running takes the folder from the new one, which then leaves:
/// that, or an app still running after a moment, is a folder opened.
#[cfg(not(target_os = "macos"))]
fn launch(app: &Path, folder: Option<&Path>) -> bool {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
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
    let settled = Instant::now() + Duration::from_millis(500);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < settled => {
                std::thread::sleep(Duration::from_millis(10));
            }
            _ => return true,
        }
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

    use super::*;

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
