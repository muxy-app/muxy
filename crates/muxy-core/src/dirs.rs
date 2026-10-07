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

    #[test]
    fn linux_uses_absolute_xdg_without_requiring_home_and_macos_ignores_it() -> io::Result<()> {
        assert_eq!(
            resolve_directory(None, None, Some("/state".into()), "muxy-beta", true)?,
            PathBuf::from("/state/muxy-beta")
        );
        for state in ["relative", ""] {
            assert!(
                resolve_directory(
                    None,
                    Some("/home/test".into()),
                    Some(state.into()),
                    "muxy-dev",
                    true
                )
                .is_err()
            );
        }
        assert_eq!(
            resolve_directory(
                None,
                Some("/Users/test".into()),
                Some("relative".into()),
                "Muxy Dev",
                false
            )?,
            PathBuf::from("/Users/test/Library/Application Support/Muxy Dev")
        );
        Ok(())
    }
}
