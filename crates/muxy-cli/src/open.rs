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
    if crate::terminal::over_ssh() || !desktop(&folder) {
        crate::tui::run(None, Some(&folder))?;
    }
    Ok(())
}

/// Opens `folder` in the desktop app, returning false if it can't.
#[cfg(target_os = "macos")]
fn desktop(folder: &Path) -> bool {
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
    open.arg(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(target_os = "macos"))]
fn desktop(_: &Path) -> bool {
    false
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
