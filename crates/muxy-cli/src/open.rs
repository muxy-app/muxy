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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_existing_folders_are_opened() {
        let file = std::env::current_exe().expect("test executable");
        for word in [
            Path::new("/muxy-missing-folder"),
            Path::new("muxy-missing-folder"),
            &file,
        ] {
            let error = run(word).expect_err("not a folder");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains("not a command or a folder"));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_app_is_the_bundle_that_ships_this_executable() {
        assert_eq!(
            containing_app(Path::new("/Applications/Muxy Beta.app/Contents/MacOS/muxy")),
            Some(Path::new("/Applications/Muxy Beta.app"))
        );
        for executable in [
            "/Users/me/.local/bin/muxy",
            "/Applications/Muxy Beta.app/Contents/Resources/muxy",
            "/Applications/Muxy Beta/Contents/MacOS/muxy",
            "/muxy",
        ] {
            assert_eq!(containing_app(Path::new(executable)), None, "{executable}");
        }
    }
}
