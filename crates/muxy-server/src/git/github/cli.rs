use super::super::{Result, command, error};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(60);

/// Runs the GitHub CLI for a repository.
#[derive(Debug, Default)]
pub(super) struct Cli {
    /// Runs this GitHub CLI instead of the installed one.
    pub(super) executable: Option<PathBuf>,
}

impl Cli {
    fn command(&self, repository: &Path, args: &[&str]) -> Command {
        let executable = self
            .executable
            .clone()
            .or_else(command::github_cli)
            .unwrap_or_else(|| "gh".into());
        let mut command = Command::new(executable);
        command
            .args(args)
            .current_dir(repository)
            .env_remove("GH_REPO")
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .env("LC_ALL", "C")
            .env("NO_COLOR", "1");
        for (name, _) in std::env::vars_os() {
            let key = name.to_string_lossy();
            if key.starts_with("GIT_")
                && !matches!(
                    key.as_ref(),
                    "GIT_SSH" | "GIT_SSH_COMMAND" | "GIT_SSH_VARIANT" | "GIT_ASKPASS"
                )
            {
                command.env_remove(name);
            }
        }
        command.env("GIT_TERMINAL_PROMPT", "0");
        command
    }

    pub(super) fn run(&self, repository: &Path, args: &[&str]) -> Result<Vec<u8>> {
        command::capture_with(self.command(repository, args), TIMEOUT, None, false)
            .map(|output| output.0)
            .map_err(|cause| error(format!("GitHub CLI: {}", cause.message())))
    }

    /// Runs `args`, keeping at most `limit` bytes of output. Returns whether
    /// it was cut short.
    pub(super) fn run_bounded(
        &self,
        repository: &Path,
        args: &[&str],
        limit: usize,
    ) -> Result<(Vec<u8>, bool)> {
        command::capture_with(self.command(repository, args), TIMEOUT, Some(limit), false)
    }
}
