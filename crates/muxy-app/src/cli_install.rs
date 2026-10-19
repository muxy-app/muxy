#[cfg(unix)]
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, symlink};
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;

use crate::model::AppModel;
use gpui::{Context, SharedString};
use muxy_core::capability::{Capability, Feature};
use muxy_ui::tr;

/// The installer links the `muxy` that was installed with the running app.
pub(crate) const CAPABILITY: Capability = Capability::of(cfg!(unix));

impl AppModel {
    pub(crate) fn install_command_line(&mut self, cx: &mut Context<Self>) {
        if !self.require(Feature::CommandLineTool, cx) {
            return;
        }
        let window = self.window;
        cx.spawn(async move |_, cx| {
            let result = cx.background_executor().spawn(async { install() }).await;
            let response = window.update(cx, |_, window, cx| {
                let (title, message) = installation_message(result);
                window.prompt(
                    gpui::PromptLevel::Info,
                    &title,
                    Some(&message),
                    &[tr!("OK").as_ref()],
                    cx,
                )
            });
            if let Ok(answer) = response {
                let _ = answer.await;
            }
        })
        .detach();
    }
}

fn installation_message(result: io::Result<PathBuf>) -> (SharedString, String) {
    match result {
        Ok(path) => {
            let on_path = std::env::var_os("PATH").is_some_and(|value| {
                std::env::split_paths(&value).any(|entry| Some(entry.as_path()) == path.parent())
            });
            let guidance = if on_path {
                tr!("Run muxy --help in your terminal.")
            } else {
                tr!(
                    "Add ~/.local/bin to your PATH. For this shell, run:\nexport PATH=\"$HOME/.local/bin:$PATH\""
                )
            };
            (
                tr!("Command Line Tool Installed"),
                tr!("Installed %@.\n\n%@", path.display().to_string(), &guidance).to_string(),
            )
        }
        Err(error) => (
            tr!("Could Not Install Command Line Tool"),
            error.to_string(),
        ),
    }
}

#[cfg(windows)]
fn install() -> io::Result<PathBuf> {
    Err(io::Error::other(
        Feature::CommandLineTool.unsupported_reason().into_owned(),
    ))
}

#[cfg(unix)]
fn install() -> io::Result<PathBuf> {
    let executable = muxy_core::executable::current_path()?;
    let target = installed_tool(&executable)?;
    for executable in [&target, &target.with_file_name("muxy-server")] {
        if crate::server::read_build_info(executable)? != muxy_protocol::BuildInfo::current() {
            return Err(io::Error::other(
                tr!(
                    "The installed app changed. Reopen it before installing the command line tool."
                )
                .to_string(),
            ));
        }
    }
    let home = std::env::home_dir()
        .ok_or_else(|| io::Error::other(tr!("Home directory unavailable").to_string()))?;
    let destination = home.join(".local/bin/muxy");
    if target != destination {
        install_link(&target, &destination)?;
    }
    Ok(destination)
}

/// The `muxy` installed with the app at `executable`: on Linux the app, the
/// tool and the server sit in one directory. An app run from a temporary
/// folder is not installed, and a link into it would not last.
#[cfg(all(unix, not(target_os = "macos")))]
fn installed_tool(executable: &Path) -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(root) = muxy_core::bundle::linux_root(executable) {
        if root.join("current/muxy-app").canonicalize()? != executable {
            return Err(io::Error::other(
                tr!(
                    "The installed app changed. Reopen it before installing the command line tool."
                )
                .to_string(),
            ));
        }
        return Ok(root.join("current/muxy"));
    }
    let app_name = muxy_core::release::Channel::current().app_name();
    let temporary = [Path::new("/tmp"), Path::new("/var/tmp")]
        .into_iter()
        .chain([std::env::temp_dir().as_path()])
        .any(|folder| executable.starts_with(folder));
    if temporary || executable.file_name().is_none_or(|name| name != "muxy-app") {
        return Err(io::Error::other(
            tr!(
                "Install %@ and open it from there before installing its command line tool.",
                app_name
            )
            .to_string(),
        ));
    }
    Ok(executable.with_file_name("muxy"))
}

/// The `muxy` inside the app bundle at `executable`.
#[cfg(target_os = "macos")]
fn installed_tool(executable: &Path) -> io::Result<PathBuf> {
    let app_name = muxy_core::release::Channel::current().app_name();
    let bundle = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| {
            io::Error::other(
                tr!(
                    "Install and open %@.app before installing its command line tool.",
                    app_name
                )
                .to_string(),
            )
        })?;
    if executable.file_name().is_none_or(|name| name != "muxy-app")
        || executable.parent() != Some(bundle.join("Contents/MacOS").as_path())
        || bundle.starts_with("/Volumes")
        || bundle.starts_with("/tmp")
        || bundle.starts_with("/private/tmp")
        || bundle.starts_with("/private/var/folders")
        || bundle.components().any(|part| {
            part.as_os_str() == "AppTranslocation"
                || part
                    .as_os_str()
                    .to_string_lossy()
                    .starts_with(".muxy-beta-update-")
        })
        || bundle
            .file_name()
            .is_some_and(|name| name == "previous.app")
    {
        return Err(io::Error::other(
            tr!(
                "Move %@.app to Applications and reopen it before installing its command line tool.",
                app_name
            )
            .to_string(),
        ));
    }
    Ok(bundle.join("Contents/MacOS/muxy"))
}

#[cfg(unix)]
fn install_link(target: &Path, destination: &Path) -> io::Result<()> {
    if !target.is_absolute() || !target.is_file() {
        return Err(io::Error::other(
            tr!("Bundled muxy executable is missing").to_string(),
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other(tr!("Missing installation directory").to_string()))?;
    fs::create_dir_all(parent)?;
    let _lock = muxy_client::local::try_lock(&parent.join(".muxy-cli-install.lock"))?;
    let receipt = parent.join(".muxy-cli-link.json");
    let managed: Vec<Vec<u8>> = match fs::read(&receipt) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let existing = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_symlink() => {
            let previous = fs::read_link(destination)?;
            if previous != target
                && !managed
                    .iter()
                    .any(|value| value == previous.as_os_str().as_bytes())
            {
                return Err(io::Error::other(
                    tr!(
                        "%@ is an unrelated link; move it before installing Muxy.",
                        destination.display().to_string()
                    )
                    .to_string(),
                ));
            }
            Some((metadata.dev(), metadata.ino(), previous))
        }
        Ok(_) => {
            return Err(io::Error::other(
                tr!(
                    "%@ already exists; move it before installing Muxy.",
                    destination.display().to_string()
                )
                .to_string(),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let staging = tempfile::Builder::new()
        .prefix(".muxy-link-")
        .tempdir_in(parent)?;
    let link = staging.path().join("muxy");
    symlink(target, &link)?;
    let mut targets = vec![target.as_os_str().as_bytes().to_vec()];
    if let Some((_, _, previous)) = &existing {
        targets.push(previous.as_os_str().as_bytes().to_vec());
    }
    let record = staging.path().join("receipt");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&record)?;
    serde_json::to_writer(&mut file, &targets)?;
    file.sync_all()?;
    let unchanged = match (fs::symlink_metadata(destination), &existing) {
        (Err(error), None) => error.kind() == io::ErrorKind::NotFound,
        (Ok(metadata), Some((device, inode, _))) => {
            (metadata.dev(), metadata.ino()) == (*device, *inode)
        }
        _ => false,
    };
    if !unchanged {
        return Err(io::Error::other(
            tr!("The command path changed during installation; try again.").to_string(),
        ));
    }
    fs::rename(&record, receipt)?;
    fs::rename(link, destination)?;
    fs::File::open(parent)?.sync_all()
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    #[test]
    fn installs_repairs_managed_links_and_refuses_unrelated_paths() -> io::Result<()> {
        let root = tempfile::tempdir()?;
        let target = root.path().join("App With Spaces/muxy");
        fs::create_dir(target.parent().ok_or_else(|| io::Error::other("parent"))?)?;
        fs::write(&target, b"executable")?;
        let destination = root.path().join("bin/muxy");
        install_link(&target, &destination)?;
        assert_eq!(fs::read_link(&destination)?, target);
        install_link(&target, &destination)?;
        let next = root.path().join("moved-muxy");
        fs::rename(&target, &next)?;
        install_link(&next, &destination)?;
        assert_eq!(fs::read_link(&destination)?, next);
        fs::remove_file(&destination)?;
        symlink("/usr/bin/false", &destination)?;
        assert!(install_link(&next, &destination).is_err());
        assert_eq!(
            fs::read_link(&destination)?,
            PathBuf::from("/usr/bin/false")
        );
        fs::remove_file(&destination)?;
        fs::write(&destination, b"unrelated")?;
        assert!(install_link(&next, &destination).is_err());
        assert_eq!(fs::read(&destination)?, b"unrelated");
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_tool_is_found_beside_an_installed_app_wherever_that_is() -> io::Result<()> {
        for folder in ["/usr/bin", "/opt/Muxy App/bin", "/home/me/.local/bin"] {
            let folder = Path::new(folder);
            assert_eq!(
                installed_tool(&folder.join("muxy-app"))?,
                folder.join("muxy")
            );
        }
        for running in [
            "/tmp/.mount_Muxy12/usr/bin/muxy-app",
            "/var/tmp/build/muxy-app",
            "/usr/bin/muxy",
        ] {
            assert!(installed_tool(Path::new(running)).is_err(), "{running}");
        }
        assert!(installed_tool(&std::env::temp_dir().join("muxy-app")).is_err());
        Ok(())
    }
}
