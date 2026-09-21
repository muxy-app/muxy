use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Release, Result, build_number, client, download, latest, replacement};

#[derive(Clone, Debug)]
pub(crate) struct Installation {
    bundle: PathBuf,
    team: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedUpdate {
    pub(crate) version: String,
    installation: Installation,
    staging: PathBuf,
    pub(crate) build: Option<muxy_protocol::BuildInfo>,
}

impl Installation {
    pub(crate) fn detect() -> Result<Self> {
        build_number(env!("CARGO_PKG_VERSION"))
            .ok_or("Automatic updates require an installed release of Muxy Beta")?;
        let executable = muxy_core::executable::current_path()?;
        let bundle = executable
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
            .ok_or("Automatic updates require an installed Muxy Beta.app")?
            .to_owned();
        if bundle.starts_with("/Volumes")
            || bundle
                .components()
                .any(|part| part.as_os_str() == "AppTranslocation")
        {
            return Err("Move Muxy Beta.app to Applications and reopen it before updating".into());
        }
        let details = Command::new("/usr/bin/codesign")
            .args(["--display", "--verbose=4"])
            .arg(&bundle)
            .output()?;
        if !details.status.success() {
            return Err("The installed beta has no valid developer signature".into());
        }
        let details = String::from_utf8(details.stderr)?;
        let team = details
            .lines()
            .find_map(|line| line.strip_prefix("TeamIdentifier="))
            .filter(|team| {
                team.len() == 10
                    && team
                        .bytes()
                        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            })
            .ok_or("Automatic updates require a Developer ID signed beta")?
            .to_owned();
        let installation = Self { bundle, team };
        installation.verify_app(&installation.bundle, env!("CARGO_PKG_VERSION"))?;
        Ok(installation)
    }

    pub(crate) fn latest() -> Result<Option<Release>> {
        let client = client()?;
        let (platform, arch) = if cfg!(target_arch = "aarch64") {
            ("macos-aarch64", "arm64")
        } else {
            ("macos-x86_64", "x86_64")
        };
        latest(&client, platform, arch)
    }

    pub(crate) fn prepare(&self, release: Release) -> Result<PreparedUpdate> {
        let client = client()?;
        let staging = tempfile::Builder::new()
            .prefix(".muxy-beta-update-")
            .tempdir_in(
                self.bundle
                    .parent()
                    .ok_or("Missing application directory")?,
            )?;
        let download_dir = tempfile::tempdir()?;
        let dmg = download_dir.path().join("update.dmg");
        download(&client, &release, &dmg)?;
        self.verify_signature(&dmg, false)?;
        let mount = MountedImage::attach(&dmg, download_dir.path())?;
        let source = mount.path.join("Muxy Beta.app");
        self.verify_app(&source, &release.version)?;
        let candidate = staging.path().join("Muxy Beta.app");
        run(Command::new("/usr/bin/ditto").arg(&source).arg(&candidate))?;
        self.verify_app(&candidate, &release.version)?;
        let build =
            crate::server::read_build_info(&candidate.join("Contents/MacOS/muxy-server")).ok();
        if build
            .as_ref()
            .is_some_and(|info| info.version != release.version)
        {
            return Err("The server version does not match the signed app".into());
        }
        Ok(PreparedUpdate {
            version: release.version,
            installation: self.clone(),
            staging: staging.keep(),
            build,
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
            .ok_or("Invalid pending update")?
            .to_owned();
        if build_number(&version) <= build_number(env!("CARGO_PKG_VERSION")) {
            std::fs::remove_file(path)?;
            return Ok(None);
        }
        let staging = PathBuf::from(
            record["staging"]
                .as_str()
                .ok_or("Invalid pending update path")?,
        );
        if !self.owns_staging(&staging) {
            return Err("Pending update is outside the installation directory".into());
        }
        let candidate = staging.join("Muxy Beta.app");
        self.verify_app(&candidate, &version)?;
        let build =
            crate::server::read_build_info(&candidate.join("Contents/MacOS/muxy-server")).ok();
        if build.as_ref().is_some_and(|build| build.version != version) {
            return Err("Pending update version mismatch".into());
        }
        Ok(Some((
            PreparedUpdate {
                version,
                installation: self.clone(),
                staging,
                build,
            },
            record["scheduled"].as_bool() == Some(true),
        )))
    }

    fn owns_staging(&self, path: &Path) -> bool {
        path.parent() == self.bundle.parent()
            && path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".muxy-beta-update-"))
            && std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.is_symlink())
    }

    pub(crate) fn cleanup(&self, socket: &Path) -> Result<()> {
        let _server_lock = match crate::server::lock_for_update(socket) {
            Ok(lock) => lock,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let _bundle_lock = replacement::lock(
            self.bundle
                .parent()
                .ok_or("Missing application directory")?,
        )?;
        let client = muxy_client::Client::connect(socket)?;
        self.cleanup_retired(client.server_info())
    }

    fn cleanup_retired(&self, server: &muxy_protocol::ServerInfo) -> Result<()> {
        for entry in std::fs::read_dir(
            self.bundle
                .parent()
                .ok_or("Missing application directory")?,
        )? {
            let path = entry?.path();
            if !self.owns_staging(&path) {
                continue;
            }
            let Ok(bytes) = std::fs::read(path.join("retained.json")) else {
                continue;
            };
            let record: serde_json::Value = serde_json::from_slice(&bytes)?;
            if record["bundle"].as_str() == self.bundle.to_str()
                && record["committed"].as_bool() == Some(true)
                && record["instance"]
                    .as_u64()
                    .is_some_and(|instance| instance != server.instance)
                && path.join("previous.app").is_dir()
                && let Some(_lease) =
                    muxy_client::local::bundle::lock_unused(&path.join("previous.app"))?
            {
                std::fs::remove_dir_all(path)?;
            }
        }
        Ok(())
    }

    fn verify_signature(&self, path: &Path, app: bool) -> Result<()> {
        let mut requirement = format!(
            "anchor apple generic and certificate leaf[subject.OU] = \"{}\"",
            self.team
        );
        if app {
            requirement.push_str(" and identifier \"com.muxy-beta.app\"");
        }
        verify_code(path, &requirement)
    }

    fn verify_app(&self, app: &Path, version: &str) -> Result<()> {
        self.verify_signature(app, true)?;
        let plist = app.join("Contents/Info.plist");
        for (key, expected) in [
            ("CFBundleIdentifier", "com.muxy-beta.app"),
            ("CFBundleExecutable", "muxy-app"),
            ("MuxyVersion", version),
        ] {
            let actual = run(Command::new("/usr/bin/plutil")
                .args(["-extract", key, "raw", "-o", "-"])
                .arg(&plist))?;
            if actual.trim() != expected {
                return Err(format!("The signed beta has an unexpected {key}").into());
            }
        }
        let count = run(Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleVersion", "raw", "-o", "-"])
            .arg(&plist))?;
        if count.trim().parse::<u64>().ok() != build_number(version) {
            return Err("The signed beta build number does not match the update feed".into());
        }
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        for binary in ["muxy-app", "muxy", "muxy-server"] {
            run(Command::new("/usr/bin/lipo")
                .arg(app.join("Contents/MacOS").join(binary))
                .args(["-verify_arch", arch]))?;
        }
        let server = crate::server::read_build_info(&app.join("Contents/MacOS/muxy-server"))?;
        let client = crate::server::read_build_info(&app.join("Contents/MacOS/muxy"))?;
        if client != server || server.version != version {
            return Err("The bundled client and server do not match the release".into());
        }
        Ok(())
    }
}

impl PreparedUpdate {
    pub(crate) fn discard(&self, socket: &Path) -> Result<()> {
        if !self.installation.owns_staging(&self.staging) {
            return Ok(());
        }
        let _server_lock = match crate::server::lock_for_update(socket) {
            Ok(lock) => lock,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let _bundle_lock = replacement::lock(
            self.installation
                .bundle
                .parent()
                .ok_or("Missing application directory")?,
        )?;
        if self.installation.owns_staging(&self.staging)
            && !self.staging.join("retained.json").exists()
            && !self.staging.join("previous.app").exists()
        {
            std::fs::remove_dir_all(&self.staging)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn fixture() -> Result<Self> {
        let staging = tempfile::tempdir()?;
        Ok(Self {
            version: "2.0.0-beta-1234".into(),
            installation: Installation {
                bundle: staging.path().join("installed.app"),
                team: "TESTTEAM00".into(),
            },
            staging: staging.keep(),
            build: Some(muxy_protocol::BuildInfo::current()),
        })
    }

    pub(crate) fn compatible_with(&self, server: &muxy_protocol::ServerInfo) -> bool {
        self.build
            .as_ref()
            .is_some_and(|build| build.compatibility == server.build.compatibility)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        self.installation
            .verify_app(&self.candidate(), &self.version)?;
        self.installation
            .verify_app(&self.installation.bundle, env!("CARGO_PKG_VERSION"))?;
        let build =
            crate::server::read_build_info(&self.candidate().join("Contents/MacOS/muxy-server"))
                .ok();
        if build != self.build {
            return Err("The staged update metadata changed. Check for updates again.".into());
        }
        Ok(())
    }

    fn candidate(&self) -> PathBuf {
        self.staging.join("Muxy Beta.app")
    }

    pub(crate) fn persist(&self, path: &Path, scheduled: bool) -> Result<()> {
        let record = serde_json::json!({"version": self.version, "staging": self.staging, "scheduled": scheduled});
        write_record(path, &record)
    }

    fn retain_bundle(&self, server: &muxy_protocol::ServerInfo) -> Result<()> {
        write_record(
            &self.staging.join("retained.json"),
            &serde_json::json!({
                "bundle": self.installation.bundle, "committed": false, "instance": if server.build.version == env!("CARGO_PKG_VERSION") { server.instance } else { 0 }
            }),
        )
    }

    fn commit_retirement(&self) -> Result<()> {
        let path = self.staging.join("retained.json");
        let mut record: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        record["committed"] = true.into();
        write_record(&path, &record)
    }

    pub(crate) fn install(
        &self,
        _server_lock: std::fs::File,
        server: &muxy_protocol::ServerInfo,
    ) -> Result<()> {
        let _bundle_lock = replacement::lock(
            self.installation
                .bundle
                .parent()
                .ok_or("Missing application directory")?,
        )?;
        let candidate = self.staging.join("Muxy Beta.app");
        self.validate()?;
        let backup = self.staging.join("previous.app");
        self.retain_bundle(server)?;
        if let Err(error) =
            replacement::replace_and_restart(&self.installation.bundle, &candidate, &backup, || {
                restart(&self.installation.bundle)
            })
        {
            if backup.exists() {
                return Err(format!(
                    "{error}. The previous app is available at {}",
                    backup.display()
                )
                .into());
            }
            return Err(error);
        }
        if let Err(error) = self.commit_retirement() {
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr(),
                "Retaining update recovery files: {error}"
            );
        }
        Ok(())
    }
}

fn restart(bundle: &Path) -> Result<()> {
    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.1; done; /usr/bin/open \"$2\"",
            "muxy-beta-restart",
        ])
        .arg(std::process::id().to_string())
        .arg(bundle)
        .process_group(0)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

struct MountedImage {
    path: PathBuf,
}

impl MountedImage {
    fn attach(dmg: &Path, directory: &Path) -> Result<Self> {
        let path = directory.join("mount");
        std::fs::create_dir(&path)?;
        run(Command::new("/usr/bin/hdiutil")
            .args([
                "attach",
                "-readonly",
                "-nobrowse",
                "-noautoopen",
                "-mountpoint",
            ])
            .arg(&path)
            .arg(dmg)
            .stdin(std::process::Stdio::null()))?;
        Ok(Self { path })
    }
}

impl Drop for MountedImage {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/hdiutil")
            .args(["detach", "-force"])
            .arg(&self.path)
            .output();
    }
}

fn run(command: &mut Command) -> Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "{} failed: {}",
            command.get_program().to_string_lossy(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn verify_code(path: &Path, requirement: &str) -> Result<()> {
    run(Command::new("/usr/bin/codesign")
        .args([
            "--verify",
            "--strict",
            "--deep",
            "-R",
            &format!("={requirement}"),
        ])
        .arg(path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_verification_checks_literal_requirement_and_nested_code() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let app = directory.path().join("Muxy Beta.app");
        let binaries = app.join("Contents/MacOS");
        std::fs::create_dir_all(&binaries)?;
        std::fs::write(
            app.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.muxy-beta.app</string>
<key>CFBundleExecutable</key><string>muxy-app</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>"#,
        )?;
        for binary in ["muxy-app", "muxy", "muxy-server"] {
            let path = binaries.join(binary);
            std::fs::copy("/usr/bin/true", &path)?;
            run(Command::new("/usr/bin/codesign")
                .args(["--force", "--sign", "-"])
                .arg(&path))?;
        }
        run(Command::new("/usr/bin/codesign")
            .args(["--force", "--sign", "-"])
            .arg(&app))?;
        verify_code(&app, "identifier \"com.muxy-beta.app\"")?;
        assert!(verify_code(&app, "identifier \"com.muxy.app\"").is_err());
        let installation = Installation {
            bundle: app.clone(),
            team: "TESTTEAM00".into(),
        };
        assert!(installation.verify_signature(&app, true).is_err());
        std::fs::write(binaries.join("muxy"), b"changed helper")?;
        assert!(verify_code(&app, "identifier \"com.muxy-beta.app\"").is_err());
        Ok(())
    }
}

fn write_record(path: &Path, value: &serde_json::Value) -> Result<()> {
    use std::io::Write;
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing update directory")?)?;
    serde_json::to_writer(file.as_file_mut(), value)?;
    file.flush()?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod smoke;

#[cfg(test)]
mod retirement_tests {
    use super::*;

    #[test]
    fn discarded_downloads_respect_installation_locks_and_recovery_files() -> Result<()> {
        let root = tempfile::tempdir()?;
        let staging = root.path().join(".muxy-beta-update-superseded");
        std::fs::create_dir(&staging)?;
        let update = PreparedUpdate {
            version: "2.0.0-beta-10".into(),
            installation: Installation {
                bundle: root.path().join("Muxy Beta.app"),
                team: "TESTTEAM00".into(),
            },
            staging: staging.clone(),
            build: None,
        };
        let socket = root.path().join("server.sock");
        let lock = crate::server::lock_for_update(&socket)?;
        update.discard(&socket)?;
        assert!(staging.exists());
        drop(lock);
        std::fs::write(staging.join("retained.json"), b"{}")?;
        update.discard(&socket)?;
        assert!(staging.exists());
        std::fs::remove_file(staging.join("retained.json"))?;
        std::fs::create_dir(staging.join("previous.app"))?;
        update.discard(&socket)?;
        assert!(staging.exists());
        std::fs::remove_dir(staging.join("previous.app"))?;
        update.discard(&socket)?;
        assert!(!staging.exists());
        Ok(())
    }

    #[test]
    fn only_committed_unused_bundles_are_retired() -> Result<()> {
        let root = tempfile::tempdir()?;
        let installation = Installation {
            bundle: root.path().join("Muxy Beta.app"),
            team: "TESTTEAM00".into(),
        };
        let server = muxy_protocol::ServerInfo::current();
        let mut directories = Vec::new();
        for (name, committed, instance) in [
            ("active", true, server.instance),
            ("recovery", false, 0),
            ("unused", true, 0),
        ] {
            let path = root.path().join(format!(".muxy-beta-update-{name}"));
            std::fs::create_dir_all(path.join("previous.app"))?;
            write_record(
                &path.join("retained.json"),
                &serde_json::json!({"bundle": installation.bundle, "committed": committed, "instance": instance}),
            )?;
            directories.push(path);
        }
        let socket = root.path().join("server.sock");
        let lock = crate::server::lock_for_update(&socket)?;
        installation.cleanup(&socket)?;
        assert!(directories.iter().all(|path| path.exists()));
        lock.unlock()?;
        drop(lock);
        assert!(installation.cleanup(&socket).is_err());
        assert!(directories.iter().all(|path| path.exists()));
        installation.cleanup_retired(&server)?;
        assert!(directories[0].exists());
        assert!(directories[1].exists());
        assert!(!directories[2].exists());
        Ok(())
    }
}
