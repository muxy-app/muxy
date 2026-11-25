use super::super::{Result, command, error};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub(super) struct Cli {
    pub(super) executable: Option<PathBuf>,
}

impl Cli {
    fn command(&self, repository: &Path, args: &[&str]) -> Result<Command> {
        let executable = self
            .executable
            .clone()
            .or_else(command::github_cli)
            .ok_or_else(|| error("GitHub CLI: gh is not installed"))?;
        let mut command = Command::new(executable);
        command
            .args(args)
            .current_dir(repository)
            .env_remove("GH_REPO")
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .env("LC_ALL", "C")
            .env("NO_COLOR", "1");
        command::without_relative_path(&mut command);
        command::without_inherited_git(&mut command);
        Ok(command)
    }

    pub(super) fn run(&self, repository: &Path, args: &[&str]) -> Result<Vec<u8>> {
        command::capture_with(self.command(repository, args)?, TIMEOUT, None, false)
            .map(|output| output.0)
            .map_err(|cause| error(format!("GitHub CLI: {}", cause.message())))
    }

    pub(super) fn run_bounded(
        &self,
        repository: &Path,
        args: &[&str],
        limit: usize,
    ) -> Result<(Vec<u8>, bool)> {
        command::capture_with(self.command(repository, args)?, TIMEOUT, Some(limit), false)
    }
}
