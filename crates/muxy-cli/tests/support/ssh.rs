//! A fake remote computer for SSH tests. Its fake ssh runs the remote command
//! on this computer, with the remote's own home, PATH, and Muxy profile.
//!
//! Include it with `#[path = "support/ssh.rs"]` next to `mod support;`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use std::{fs, thread};

use muxy_client::{Client, SshTarget};

pub(super) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) struct FakeRemote {
    directory: tempfile::TempDir,
}

impl FakeRemote {
    /// A remote with Muxy on its PATH.
    pub(super) fn new() -> Result<Self> {
        Self::with_script("")
    }

    /// Runs `script` in the fake ssh before the remote command, to print
    /// noise, change PATH, or fail the way ssh does.
    pub(super) fn with_script(script: &str) -> Result<Self> {
        // Short, so the server's socket path fits.
        let directory = tempfile::Builder::new()
            .prefix("muxy-ssh-")
            .tempdir_in("/tmp")?;
        let remote = Self { directory };
        fs::create_dir(remote.home())?;
        fs::write(
            remote.profile().join("server.toml"),
            "default_shell = \"/bin/sh\"\nshell_integration = false\n",
        )?;
        let ssh = format!(
            "#!/bin/sh\n\
             export HOME={home} MUXY_DIR={profile} PATH={bin}:/usr/bin:/bin SHELL=/bin/sh\n\
             unset MUXY_SERVER_BIN MUXY_PANE_ID XDG_STATE_HOME\n\
             {script}\n\
             for last; do :; done\n\
             exec /bin/sh -c \"$last\"\n",
            home = quote(&remote.home()),
            profile = quote(remote.profile()),
            bin = quote(
                super::support::binary()
                    .parent()
                    .ok_or("binary directory")?
            ),
        );
        install(&remote.ssh(), &ssh)?;
        Ok(remote)
    }

    pub(super) fn target(&self) -> Result<SshTarget> {
        Ok(SshTarget::new("box")?.with_program(self.ssh()))
    }

    /// Runs `muxy` as the fake ssh would, on the remote side.
    pub(super) fn muxy(&self, args: &[&str]) -> Command {
        let mut command = Command::new(super::support::binary());
        command
            .args(args)
            .env("HOME", self.home())
            .env("MUXY_DIR", self.profile())
            .env_remove("MUXY_SERVER_BIN")
            .env_remove("MUXY_PANE_ID");
        command
    }

    pub(super) fn ssh(&self) -> PathBuf {
        self.directory.path().join("fake-ssh")
    }

    pub(super) fn home(&self) -> PathBuf {
        self.directory.path().join("home")
    }

    pub(super) fn profile(&self) -> &Path {
        self.directory.path()
    }

    pub(super) fn socket(&self) -> PathBuf {
        self.profile().join("server.sock")
    }
}

impl Drop for FakeRemote {
    /// Stops any server a bridge started there.
    fn drop(&mut self) {
        if let Ok(client) = Client::connect_with_timeout(&self.socket(), Duration::from_millis(100))
        {
            let _ = client.stop_server();
            let deadline = Instant::now() + Duration::from_secs(5);
            while self.socket().exists() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', r"'\''"))
}

/// Writes the script from another process: a test thread that forks while
/// this one holds the file open for writing makes running it fail with
/// ETXTBSY on Linux.
fn install(path: &Path, script: &str) -> Result {
    let status = Command::new("/bin/sh")
        .args([
            "-c",
            r#"printf '%s' "$1" > "$2" && chmod 755 "$2""#,
            "sh",
            script,
        ])
        .arg(path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("could not write {}: {status}", path.display()).into())
    }
}
