use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{SshTarget, program};

impl SshTarget {
    pub fn login(&self) -> io::Result<String> {
        login_from(&self.config()?).ok_or_else(|| {
            io::Error::other(format!("no SSH user or host for {}", self.destination))
        })
    }

    pub fn has_own_route(&self) -> io::Result<bool> {
        Ok(route_from(&self.config()?))
    }

    pub fn default_identity(&self) -> io::Result<Option<PathBuf>> {
        let config = self.config()?;
        Ok(std::env::home_dir().and_then(|home| identity_from(&config, &home)))
    }

    fn config(&self) -> io::Result<String> {
        if self.is_custom_command() {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Custom commands manage their own SSH settings and authentication",
            ));
        }
        let program = program(self.program.as_deref(), std::env::var_os("MUXY_SSH"))?;
        let output = Command::new(program)
            .args(["-G", "--", &self.destination])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "could not read the SSH settings for {}",
                self.destination
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn route_from(config: &str) -> bool {
    config.lines().any(|line| {
        line.split_once(' ').is_some_and(|(key, value)| {
            matches!(key, "proxyjump" | "proxycommand") && value.trim() != "none"
        })
    })
}

fn login_from(config: &str) -> Option<String> {
    let value = |key: &str| {
        config.lines().find_map(|line| {
            let (name, value) = line.split_once(' ')?;
            (name == key).then(|| value.trim().to_owned())
        })
    };
    let user = value("user")?;
    let host = value("hostkeyalias").or_else(|| value("hostname"))?;
    Some(format!("{user}@{host}"))
}

fn identity_from(config: &str, home: &Path) -> Option<PathBuf> {
    config.lines().find_map(|line| {
        let value = line.strip_prefix("identityfile ")?.trim();
        let path = if let Some(relative) = value
            .strip_prefix("~/")
            .or_else(|| value.strip_prefix("%d/"))
        {
            home.join(relative)
        } else if value.contains('%') || value.starts_with('~') || value == "none" {
            return None;
        } else {
            PathBuf::from(value)
        };
        path.is_file().then_some(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jump_hosts_and_proxy_commands_in_ssh_config_are_their_own_route() {
        assert!(route_from("user me\nproxyjump bastion.example\nport 22\n"));
        assert!(route_from("proxycommand nc -X 5 %h %p\n"));
        assert!(!route_from("user me\nproxycommand none\nport 22\n"));
        assert!(!route_from("user me\nhostname box\n"));
    }

    #[test]
    fn login_uses_the_first_values_and_prefers_the_host_key_alias() {
        for (config, expected) in [
            ("hostname box\nuser dev\n", Some("dev@box")),
            (
                "user  dev  \nhostname box\nhostkeyalias known-box\nuser other\nhostkeyalias other-box\n",
                Some("dev@known-box"),
            ),
            (
                "user dev\nhostname first\nhostname second\n",
                Some("dev@first"),
            ),
            ("user dev\nhostkeyalias known-box\n", Some("dev@known-box")),
            ("hostname box\n", None),
            ("user dev\n", None),
        ] {
            assert_eq!(login_from(config).as_deref(), expected, "{config}");
        }
    }

    #[test]
    fn identities_expand_home_and_select_the_first_existing_file() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let home = directory.path();
        let first = home.join("first key");
        let second = home.join("second key");
        std::fs::write(&first, "first")?;
        std::fs::write(&second, "second")?;
        for prefix in ["~/", "%d/"] {
            let config = format!(
                "identityfile none\nidentityfile ~other/key\nidentityfile %h/key\nidentityfile {prefix}missing\nidentityfile {prefix}.\nidentityfile {prefix}first key\nidentityfile {prefix}second key\n"
            );
            assert_eq!(identity_from(&config, home), Some(first.clone()));
        }
        let config = format!("identityfile {}\n", second.display());
        assert_eq!(identity_from(&config, home), Some(second.clone()));
        std::fs::remove_file(second)?;
        assert_eq!(identity_from(&config, home), None);
        Ok(())
    }

    #[cfg(unix)]
    fn config_program(directory: &Path, body: &str) -> io::Result<PathBuf> {
        use std::os::unix::fs::PermissionsExt;

        let path = directory.join("ssh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(path)
    }

    #[cfg(unix)]
    #[test]
    fn configuration_queries_use_the_selected_program_and_destination() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let program = config_program(
            directory.path(),
            r#"[ "$#" = 3 ] && [ "$1" = '-G' ] && [ "$2" = '--' ] && [ "$3" = 'work-box' ] || exit 1
if read -r line; then exit 2; fi
printf 'user dev\nhostname box\nhostkeyalias known-box\nproxyjump bastion\n'
printf 'identityfile %s.key\n' "$0""#,
        )?;
        let identity = program.with_extension("key");
        std::fs::write(&identity, "key")?;
        let target = SshTarget::new("work-box")?.with_program(program);
        assert_eq!(target.login()?, "dev@known-box");
        assert!(target.has_own_route()?);
        assert_eq!(
            target.default_identity()?,
            std::env::home_dir().map(|_| identity)
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn configuration_query_failures_keep_their_error_kind_and_context() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let failed = config_program(
            directory.path(),
            "printf 'user dev\\nhostname box\\n'; exit 7",
        )?;
        let target = SshTarget::new("work-box")?.with_program(failed);
        for result in [
            target.login().map(|_| ()),
            target.has_own_route().map(|_| ()),
            target.default_identity().map(|_| ()),
        ] {
            assert_eq!(
                result.err().map(|error| (error.kind(), error.to_string())),
                Some((
                    io::ErrorKind::Other,
                    "could not read the SSH settings for work-box".into()
                ))
            );
        }
        let empty = config_program(directory.path(), "exit 0")?;
        let target = target.with_program(empty);
        assert_eq!(
            target
                .login()
                .err()
                .map(|error| (error.kind(), error.to_string())),
            Some((
                io::ErrorKind::Other,
                "no SSH user or host for work-box".into()
            ))
        );
        assert!(!target.has_own_route()?);
        assert_eq!(target.default_identity()?, None);
        let missing = target.with_program(directory.path().join("missing"));
        assert_eq!(
            missing.login().err().map(|error| error.kind()),
            Some(io::ErrorKind::NotFound)
        );
        Ok(())
    }
}
