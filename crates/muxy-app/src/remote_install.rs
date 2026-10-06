use std::path::{Path, PathBuf};

use muxy_ui::tr;

#[derive(Clone, Debug)]
pub(crate) enum Source {
    Release(String),
    Development(PathBuf),
}

impl Source {
    pub(crate) fn current() -> Self {
        crate::model::released_version().map_or_else(
            || {
                Self::Development(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../../target/linux-dev/muxy-dev-linux-x86_64.tar.gz"),
                )
            },
            |version| Self::Release(version.into()),
        )
    }

    pub(crate) fn command(&self) -> String {
        match self {
            Self::Release(version) => format!(
                "sh -c 'curl -fsSL {}/v{version}/install-muxy.sh | sh -s -- --version {version}'",
                crate::updater::RELEASES
            ),
            Self::Development(_) => {
                let installer = include_str!("../../../scripts/install-muxy.sh");
                format!(
                    "sh -c '{}' muxy-install --dev-archive",
                    installer.replace('\'', "'\\''")
                )
            }
        }
    }

    pub(crate) fn archive(&self) -> Option<&Path> {
        match self {
            Self::Release(_) => None,
            Self::Development(archive) => Some(archive),
        }
    }

    pub(crate) fn description(&self, name: &str) -> String {
        match self {
            Self::Release(version) => tr!(
                "Muxy %@ is downloaded from GitHub and installed in ~/.local/bin on %@. It needs curl and glibc 2.35 or newer there.",
                version,
                name
            ),
            Self::Development(archive) => tr!(
                "The development build at %@ is uploaded to %@. It installs muxy-server and its muxy helper in ~/.local/bin, then tests the connection. It requires x86_64 Linux with glibc 2.35 or newer.",
                archive.display().to_string(),
                name
            ),
        }
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_installer_survives_shell_quoting() {
        let source = Source::Development(PathBuf::from("unused"));
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", &format!("{} --help", source.command())])
            .stdin(std::process::Stdio::null())
            .output()
            .expect("installer help");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("Usage: install-muxy.sh"));
    }
}
