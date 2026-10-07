use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use muxy_protocol::{ProjectDescriptor, WorktreeHook};
use serde::Deserialize;

use super::{Result, command, error, path};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Config {
    setup: Vec<Hook>,
    teardown: Vec<Hook>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Hook {
    Command(String),
    Named {
        command: String,
        name: Option<String>,
    },
}

fn global_path() -> Result<PathBuf> {
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| error("Home directory is unavailable"))?;
    Ok(root.join("muxy/worktree.json"))
}

fn load(path: &Path, teardown: bool, project: bool) -> Result<Vec<WorktreeHook>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(cause) => return Err(error(format!("Could not read {}: {cause}", path.display()))),
    };
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > 1024 * 1024 {
        return Err(error("Worktree hook configuration is too large"));
    }
    let config: Config = serde_json::from_slice(&bytes)
        .map_err(|cause| error(format!("Invalid {}: {cause}", path.display())))?;
    let hooks = if teardown {
        config.teardown
    } else {
        config.setup
    };
    let hooks: Vec<_> = hooks
        .into_iter()
        .filter_map(|hook| {
            let (command, name) = match hook {
                Hook::Command(command) => (command, None),
                Hook::Named { command, name } => (command, name),
            };
            let command = command.trim().to_owned();
            (!command.is_empty()).then_some(WorktreeHook {
                command,
                name,
                project,
            })
        })
        .collect();
    if hooks.len() > 128
        || hooks.iter().any(|hook| {
            hook.command.len() > 16 * 1024
                || hook.command.contains('\0')
                || hook.name.as_ref().is_some_and(|name| name.len() > 1024)
        })
    {
        return Err(error(
            "Worktree hook configuration exceeds the command limits",
        ));
    }
    Ok(hooks)
}

pub(super) fn resolve(
    repository: &Path,
    global: Option<&Path>,
    teardown: bool,
) -> Result<Vec<WorktreeHook>> {
    match global {
        Some(global) => resolve_at(repository, global, teardown),
        None => resolve_at(repository, &global_path()?, teardown),
    }
}

fn resolve_at(repository: &Path, global: &Path, teardown: bool) -> Result<Vec<WorktreeHook>> {
    let mut global = load(global, teardown, false)?;
    let mut project = load(&repository.join(".muxy/worktree.json"), teardown, true)?;
    let hooks = if teardown {
        project.append(&mut global);
        project
    } else {
        global.append(&mut project);
        global
    };
    if hooks.len() > 128 {
        return Err(error("Too many worktree hooks"));
    }
    Ok(hooks)
}

pub(super) fn approved(
    repository: &Path,
    global: Option<&Path>,
    teardown: bool,
    approved: &[WorktreeHook],
) -> Result<()> {
    if resolve(repository, global, teardown)? != approved {
        return Err(error(
            "Worktree hooks changed after approval. Review them and try again.",
        ));
    }
    Ok(())
}

pub(super) fn run(
    repository: &Path,
    project: &ProjectDescriptor,
    branch: &str,
    hooks: &[WorktreeHook],
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(300);
    let shell = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|shell| shell.is_absolute() && shell.is_file())
        .unwrap_or_else(|| "/bin/sh".into());
    for hook in hooks {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| error("Worktree hooks timed out"))?;
        let mut process = Command::new(&shell);
        process
            .args(["-c", &hook.command])
            .current_dir(path(&project.directory))
            .env("MUXY_PROJECT_PATH", repository)
            .env("MUXY_WORKTREE_ID", project.id.to_string())
            .env("MUXY_WORKTREE_PATH", path(&project.directory))
            .env("MUXY_WORKTREE_NAME", &project.name)
            .env("MUXY_WORKTREE_BRANCH", branch);
        if let Some(path) = crate::exec::login_path() {
            process.env("PATH", path);
        }
        command::capture_with(process, remaining, None, false).map_err(|cause| {
            error(format!(
                "Worktree hook failed ({}): {cause}",
                hook.name.as_deref().unwrap_or(&hook.command)
            ))
        })?;
    }
    Ok(())
}
