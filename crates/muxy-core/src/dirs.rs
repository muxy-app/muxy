use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;

use crate::release::{Channel, Version};

pub fn muxy_dir() -> io::Result<PathBuf> {
    let linux = cfg!(target_os = "linux");
    let directory = resolve_directory(
        env::var_os("MUXY_DIR"),
        env::var_os("HOME"),
        env::var_os("XDG_STATE_HOME"),
        directory_name(env!("CARGO_PKG_VERSION"), cfg!(debug_assertions), linux),
        linux,
    )?;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)?;
    Ok(directory)
}

/// The folder where agents build extensions. It sits outside the profile, so
/// removing an extension from Muxy never deletes the work, and beside rather
/// than in the `extensions` folder that Muxy 1.x left under `~/.config/muxy`.
pub fn extension_workspace() -> io::Result<PathBuf> {
    resolve_extension_workspace(
        env::var_os("MUXY_DIR"),
        env::var_os("HOME"),
        env::var_os("XDG_CONFIG_HOME"),
        directory_name(env!("CARGO_PKG_VERSION"), cfg!(debug_assertions), true),
    )
}

fn resolve_extension_workspace(
    override_path: Option<OsString>,
    home: Option<OsString>,
    config_home: Option<OsString>,
    name: &str,
) -> io::Result<PathBuf> {
    const FOLDER: &str = "extension-workspace";
    if let Some(path) = override_path.filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path).join(FOLDER));
    }
    if let Some(config) = config_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Ok(config.join(name).join(FOLDER));
    }
    let home = home
        .filter(|home| !home.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home).join(".config").join(name).join(FOLDER))
}

/// Releases of each channel and local builds never share data. Stable is
/// `Muxy 2` on macOS because 1.x keeps its data in `Muxy`.
fn directory_name(version: &str, debug: bool, linux: bool) -> &'static str {
    let channel = Version::parse(version)
        .filter(|_| !debug)
        .map(Version::channel);
    match (channel, linux) {
        (None, false) => "Muxy Dev",
        (None, true) => "muxy-dev",
        (Some(Channel::Beta), false) => "Muxy Beta",
        (Some(Channel::Beta), true) => "muxy-beta",
        (Some(Channel::Stable), false) => "Muxy 2",
        (Some(Channel::Stable), true) => "muxy",
    }
}

fn resolve_directory(
    override_path: Option<OsString>,
    home: Option<OsString>,
    state_home: Option<OsString>,
    name: &str,
    linux: bool,
) -> io::Result<PathBuf> {
    if let Some(path) = override_path {
        return if path.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MUXY_DIR must not be empty",
            ))
        } else {
            Ok(PathBuf::from(path))
        };
    }
    if linux && let Some(state_home) = state_home {
        let state_home = PathBuf::from(state_home);
        if !state_home.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "XDG_STATE_HOME must be absolute",
            ));
        }
        return Ok(state_home.join(name));
    }
    let home = home
        .filter(|home| !home.is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home)
        .join(if linux {
            ".local/state"
        } else {
            "Library/Application Support"
        })
        .join(name))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_extension_workspace_sits_under_the_config_folder() {
        let workspace = |override_path: Option<&str>, config: Option<&str>| {
            resolve_extension_workspace(
                override_path.map(Into::into),
                Some("/home/me".into()),
                config.map(Into::into),
                "muxy-beta",
            )
            .unwrap()
        };
        assert_eq!(
            workspace(None, None),
            PathBuf::from("/home/me/.config/muxy-beta/extension-workspace")
        );
        assert_eq!(
            workspace(None, Some("/xdg")),
            PathBuf::from("/xdg/muxy-beta/extension-workspace")
        );
        assert_eq!(
            workspace(None, Some("relative")),
            PathBuf::from("/home/me/.config/muxy-beta/extension-workspace")
        );
        assert_eq!(
            workspace(Some("/profile"), Some("/xdg")),
            PathBuf::from("/profile/extension-workspace")
        );
        assert!(resolve_extension_workspace(None, None, None, "muxy").is_err());
    }

    use super::*;

    #[test]
    fn channels_and_development_builds_do_not_share_data_on_either_platform() -> io::Result<()> {
        for linux in [false, true] {
            for (version, debug, macos_name, linux_name) in [
                ("2.0.0-beta-0", true, "Muxy Dev", "muxy-dev"),
                ("2.0.0-beta-0", false, "Muxy Dev", "muxy-dev"),
                ("2.0.0-beta.1001", true, "Muxy Dev", "muxy-dev"),
                ("2.0.0-beta.1001", false, "Muxy Beta", "muxy-beta"),
                ("2.0.0", true, "Muxy Dev", "muxy-dev"),
                ("2.0.0", false, "Muxy 2", "muxy"),
            ] {
                let expected = if linux {
                    format!(".local/state/{linux_name}")
                } else {
                    format!("Library/Application Support/{macos_name}")
                };
                assert_eq!(
                    resolve_directory(
                        None,
                        Some("/home/test".into()),
                        None,
                        directory_name(version, debug, linux),
                        linux
                    )?,
                    PathBuf::from("/home/test").join(expected)
                );
            }
        }
        Ok(())
    }
}
