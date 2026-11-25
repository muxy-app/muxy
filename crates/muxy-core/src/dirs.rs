use std::env;
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use crate::release::{Channel, Version};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Platform {
    MacOs,
    Linux,
    Windows,
}

impl Platform {
    const CURRENT: Self = if cfg!(target_os = "linux") {
        Self::Linux
    } else if cfg!(windows) {
        Self::Windows
    } else {
        Self::MacOs
    };
}

pub fn muxy_dir() -> io::Result<PathBuf> {
    let platform = Platform::CURRENT;
    let directory = resolve_directory(
        env::var_os("MUXY_DIR"),
        crate::paths::home_dir().ok(),
        env::var_os("XDG_STATE_HOME"),
        env::var_os("LOCALAPPDATA"),
        directory_name(env!("CARGO_PKG_VERSION"), cfg!(debug_assertions), platform),
        platform,
    )?;
    crate::files::private_dir(&directory)?;
    Ok(directory)
}

pub fn extension_workspace() -> io::Result<PathBuf> {
    resolve_extension_workspace(
        env::var_os("MUXY_DIR"),
        crate::paths::home_dir().ok().map(PathBuf::into_os_string),
        env::var_os("XDG_CONFIG_HOME"),
        directory_name(
            env!("CARGO_PKG_VERSION"),
            cfg!(debug_assertions),
            Platform::Linux,
        ),
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

fn directory_name(version: &str, debug: bool, platform: Platform) -> &'static str {
    let channel = Version::parse(version)
        .filter(|_| !debug)
        .map(Version::channel);
    match (channel, platform == Platform::Linux) {
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
    home: Option<PathBuf>,
    state_home: Option<OsString>,
    local_app_data: Option<OsString>,
    name: &str,
    platform: Platform,
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
    let absolute = |variable: &str, value: OsString| {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            Ok(path.join(name))
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{variable} must be absolute"),
            ))
        }
    };
    match (platform, state_home) {
        (Platform::Windows, _) => {
            let local_app_data = local_app_data.ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set")
            })?;
            absolute("LOCALAPPDATA", local_app_data)
        }
        (Platform::Linux, Some(state_home)) => absolute("XDG_STATE_HOME", state_home),
        (Platform::Linux | Platform::MacOs, _) => {
            let home =
                home.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))?;
            Ok(home
                .join(if platform == Platform::Linux {
                    ".local/state"
                } else {
                    "Library/Application Support"
                })
                .join(name))
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
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

    const PLATFORMS: [Platform; 3] = [Platform::MacOs, Platform::Linux, Platform::Windows];

    fn root() -> PathBuf {
        PathBuf::from(if cfg!(windows) {
            r"C:\Users\test"
        } else {
            "/home/test"
        })
    }

    #[test]
    fn channels_and_development_builds_do_not_share_data_on_any_platform() -> io::Result<()> {
        let local_app_data = root().join("AppData").join("Local");
        for platform in PLATFORMS {
            for (version, debug, desktop_name, linux_name) in [
                ("2.0.0-beta-0", true, "Muxy Dev", "muxy-dev"),
                ("2.0.0-beta-0", false, "Muxy Dev", "muxy-dev"),
                ("2.0.0-beta.1001", true, "Muxy Dev", "muxy-dev"),
                ("2.0.0-beta.1001", false, "Muxy Beta", "muxy-beta"),
                ("2.0.0", true, "Muxy Dev", "muxy-dev"),
                ("2.0.0", false, "Muxy 2", "muxy"),
            ] {
                let expected = match platform {
                    Platform::MacOs => root()
                        .join("Library/Application Support")
                        .join(desktop_name),
                    Platform::Linux => root().join(".local/state").join(linux_name),
                    Platform::Windows => local_app_data.join(desktop_name),
                };
                assert_eq!(
                    resolve_directory(
                        None,
                        Some(root()),
                        None,
                        Some(local_app_data.clone().into()),
                        directory_name(version, debug, platform),
                        platform,
                    )?,
                    expected
                );
            }
        }
        Ok(())
    }

    #[test]
    fn an_override_wins_on_every_platform_and_must_not_be_empty() {
        for platform in PLATFORMS {
            assert_eq!(
                resolve_directory(Some("custom".into()), None, None, None, "Muxy 2", platform).ok(),
                Some(PathBuf::from("custom"))
            );
            assert_eq!(
                resolve_directory(Some("".into()), None, None, None, "Muxy 2", platform)
                    .err()
                    .map(|error| error.kind()),
                Some(io::ErrorKind::InvalidInput)
            );
        }
    }

    #[test]
    fn each_platform_reads_only_its_own_location() {
        let resolve = |platform| {
            resolve_directory(
                None,
                Some(root()),
                Some(root().join("state").into()),
                Some("relative".into()),
                "muxy",
                platform,
            )
            .map_err(|error| error.kind())
        };
        assert_eq!(
            resolve(Platform::Linux),
            Ok(root().join("state").join("muxy"))
        );
        assert_eq!(
            resolve(Platform::MacOs),
            Ok(root().join("Library/Application Support").join("muxy"))
        );
        assert_eq!(resolve(Platform::Windows), Err(io::ErrorKind::InvalidInput));
        assert_eq!(
            resolve_directory(None, Some(root()), None, None, "Muxy 2", Platform::Windows)
                .map_err(|error| error.kind()),
            Err(io::ErrorKind::NotFound)
        );
        assert_eq!(
            resolve_directory(None, None, None, None, "Muxy 2", Platform::MacOs)
                .map_err(|error| error.kind()),
            Err(io::ErrorKind::NotFound)
        );
    }
}
