//! User-owned, versioned desktop installations. Package-manager files are never replaced.

use std::path::{Path, PathBuf};
use std::process::Command;

use muxy_core::release;
use muxy_ui::tr;

use super::{Release, Result, client, download, latest};

pub(crate) const CAPABILITY: muxy_core::capability::Capability =
    muxy_core::capability::Capability::Supported;

#[derive(Clone, Debug)]
pub(crate) struct Installation {
    root: PathBuf,
    installer: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedUpdate {
    pub(crate) version: String,
    pub(crate) build: Option<muxy_protocol::BuildInfo>,
    installation: Installation,
}

impl Installation {
    pub(crate) fn detect() -> Result<Self> {
        let executable = muxy_core::executable::current_path()?;
        let root = muxy_core::bundle::linux_root(&executable).ok_or_else(|| {
            tr!("Automatic updates require a user-installed Linux desktop release").to_string()
        })?;
        let installation = Self {
            root: root.to_owned(),
            installer: executable.with_file_name("install.py"),
        };
        if installation.verify(env!("CARGO_PKG_VERSION"))? != muxy_protocol::BuildInfo::current()
            || root.join("current/muxy-app").canonicalize()? != executable
        {
            return Err(tr!("The installed app changed. Reopen it before updating.")
                .to_string()
                .into());
        }
        Ok(installation)
    }

    fn command(&self, action: &str) -> Command {
        let mut command = Command::new("/usr/bin/python3");
        command
            .arg(&self.installer)
            .arg(action)
            .arg("--root")
            .arg(&self.root)
            .stdin(std::process::Stdio::null());
        command
    }

    fn verify(&self, version: &str) -> Result<muxy_protocol::BuildInfo> {
        let output = run(self.command("verify").args(["--version", version]))?;
        Ok(serde_json::from_slice(&output)?)
    }

    pub(crate) fn latest() -> Result<Option<Release>> {
        let (platform, arch) = if cfg!(target_arch = "aarch64") {
            ("linux-aarch64", "arm64")
        } else {
            ("linux-x86_64", "x86_64")
        };
        latest(&client()?, platform, arch)
    }

    pub(crate) fn prepare(&self, release: Release) -> Result<PreparedUpdate> {
        let directory = tempfile::tempdir()?;
        let archive = directory.path().join("desktop.tar.gz");
        download(&client()?, &release, &archive)?;
        let checksum = release
            .sha256
            .ok_or_else(|| tr!("Missing desktop update checksum").to_string())?;
        run(self
            .command("stage")
            .args(["--version", &release.version, "--sha256", &checksum])
            .arg("--archive")
            .arg(&archive))?;
        Ok(PreparedUpdate {
            build: Some(self.verify(&release.version)?),
            version: release.version,
            installation: self.clone(),
        })
    }

    pub(crate) fn restore(&self, path: &Path) -> Result<Option<(PreparedUpdate, bool)>> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let record: serde_json::Value = serde_json::from_slice(&bytes)?;
        let version = record["version"]
            .as_str()
            .ok_or_else(|| tr!("Invalid pending update").to_string())?;
        if !release::is_newer(version, env!("CARGO_PKG_VERSION")) {
            std::fs::remove_file(path)?;
            return Ok(None);
        }
        if record["root"].as_str() != self.root.to_str() {
            return Err(tr!("Pending update belongs to another installation")
                .to_string()
                .into());
        }
        Ok(Some((
            PreparedUpdate {
                version: version.into(),
                build: Some(self.verify(version)?),
                installation: self.clone(),
            },
            record["scheduled"].as_bool() == Some(true),
        )))
    }

    pub(crate) fn cleanup(&self, _socket: &Path) -> Result<()> {
        run(&mut self.command("prune"))?;
        Ok(())
    }
}

impl PreparedUpdate {
    #[cfg(test)]
    pub(crate) fn fixture() -> Self {
        let root = crate::test_support::directory();
        Self {
            version: "2.0.0-beta.1234".into(),
            build: Some(muxy_protocol::BuildInfo::current()),
            installation: Installation {
                installer: root.join("install.py"),
                root,
            },
        }
    }

    pub(crate) fn compatible_with(&self, server: &muxy_protocol::ServerInfo) -> bool {
        self.build
            .as_ref()
            .is_some_and(|build| build.shares_protocol_with(&server.build))
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.installation.verify(env!("CARGO_PKG_VERSION"))?
            != muxy_protocol::BuildInfo::current()
            || Some(self.installation.verify(&self.version)?) != self.build
            || self
                .installation
                .root
                .join("current/muxy-app")
                .canonicalize()?
                != muxy_core::executable::current_path()?
        {
            return Err(
                tr!("The installed or staged update changed. Check for updates again.")
                    .to_string()
                    .into(),
            );
        }
        Ok(())
    }

    pub(crate) fn persist(&self, path: &Path, scheduled: bool) -> Result<()> {
        use std::io::Write;
        let directory = path
            .parent()
            .ok_or_else(|| tr!("Missing update directory").to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(directory)?;
        serde_json::to_writer(
            file.as_file_mut(),
            &serde_json::json!({"version": self.version, "root": self.installation.root, "scheduled": scheduled}),
        )?;
        file.flush()?;
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    }

    pub(crate) fn discard(&self, _socket: &Path) -> Result<()> {
        run(self
            .installation
            .command("discard")
            .args(["--version", &self.version]))?;
        Ok(())
    }

    pub(crate) fn install(
        &self,
        _server_lock: muxy_core::file_lock::FileLock,
        _server: &muxy_protocol::ServerInfo,
    ) -> Result<()> {
        self.validate()?;
        run(self.installation.command("activate").args([
            "--version",
            &self.version,
            "--current",
            env!("CARGO_PKG_VERSION"),
            "--pid",
            &std::process::id().to_string(),
        ]))?;
        Ok(())
    }
}

fn run(command: &mut Command) -> Result<Vec<u8>> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_owned()
            .into());
    }
    Ok(output.stdout)
}
