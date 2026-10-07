use std::io;
use std::path::Path;

pub(crate) fn run(word: &Path) -> io::Result<()> {
    let folder = word
        .canonicalize()
        .ok()
        .filter(|path| path.is_dir())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} is not a command or a folder; run muxy --help",
                    word.display()
                ),
            )
        })?;
    open(&folder)
}

#[cfg(target_os = "macos")]
fn open(folder: &Path) -> io::Result<()> {
    use std::process::{Command, Stdio};
    let mut open = Command::new("/usr/bin/open");
    match containing_app(&muxy_core::executable::current_path()?) {
        Some(app) => open.arg("-a").arg(app),
        None => open.args(["-b", "com.muxy-beta.app"]),
    };
    let status = open
        .arg(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            "could not open the Muxy desktop app; install Muxy Beta.app",
        ))
    }
}

#[cfg(not(target_os = "macos"))]
fn open(_: &Path) -> io::Result<()> {
    Err(io::Error::other(
        "opening a folder needs the Muxy desktop app, which runs on macOS",
    ))
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
