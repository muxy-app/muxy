use serde_json::Value;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub const PROJECT_HOOKS_CHANGED: &str =
    "Project worktree hooks changed after approval. Review them and try again.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookKind {
    Setup,
    Teardown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandSource {
    Global,
    Project,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookCommand {
    pub command: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCommand {
    pub command: HookCommand,
    pub source: CommandSource,
}

impl ResolvedCommand {
    pub fn new(command: &str, name: Option<&str>, source: CommandSource) -> Self {
        Self {
            command: HookCommand {
                command: command.into(),
                name: name.map(str::to_owned),
            },
            source,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectHookApproval {
    pub commands: Vec<HookCommand>,
}

impl ProjectHookApproval {
    pub fn from_resolved(commands: &[ResolvedCommand]) -> Self {
        Self {
            commands: commands
                .iter()
                .filter(|command| command.source == CommandSource::Project)
                .filter_map(|command| {
                    let normalized = command.command.command.trim();
                    (!normalized.is_empty()).then(|| HookCommand {
                        command: normalized.to_owned(),
                        name: command.command.name.clone(),
                    })
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorktreeConfig {
    pub setup: Vec<HookCommand>,
    pub teardown: Vec<HookCommand>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorktreeConfigError {
    #[error("Could not read worktree hook config at {}.", path.display())]
    Unreadable { path: PathBuf },
    #[error("Invalid worktree hook config at {}.", path.display())]
    Invalid { path: PathBuf },
    #[error("{PROJECT_HOOKS_CHANGED}")]
    ProjectHooksChanged,
}

pub fn global_config_path(home: &Path, xdg_config_home: Option<&OsStr>) -> PathBuf {
    let root = xdg_config_home
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    root.join("muxy/worktree.json")
}

pub fn load(path: &Path) -> Result<Option<WorktreeConfig>, WorktreeConfigError> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read(path).map_err(|_| WorktreeConfigError::Unreadable {
        path: path.to_path_buf(),
    })?;
    let value: Value =
        serde_json::from_slice(&contents).map_err(|_| WorktreeConfigError::Invalid {
            path: path.to_path_buf(),
        })?;
    let Some(root) = value.as_object() else {
        return Err(WorktreeConfigError::Invalid {
            path: path.to_path_buf(),
        });
    };
    Ok(Some(WorktreeConfig {
        setup: decode_commands(root.get("setup"), path)?,
        teardown: decode_commands(root.get("teardown"), path)?,
    }))
}

pub fn resolved_commands(
    kind: HookKind,
    project_path: &Path,
    global_path: &Path,
    include_project: bool,
) -> Result<Vec<ResolvedCommand>, WorktreeConfigError> {
    let global = commands(load(global_path)?.as_ref(), kind, CommandSource::Global);
    if !include_project {
        return Ok(global);
    }
    let project_config_path = project_path.join(".muxy/worktree.json");
    let project = commands(
        load(&project_config_path)?.as_ref(),
        kind,
        CommandSource::Project,
    );
    Ok(match kind {
        HookKind::Setup => global.into_iter().chain(project).collect(),
        HookKind::Teardown => project.into_iter().chain(global).collect(),
    })
}

pub fn commands_for_execution(
    kind: HookKind,
    project_path: &Path,
    global_path: &Path,
    approval: Option<&ProjectHookApproval>,
) -> Result<Vec<ResolvedCommand>, WorktreeConfigError> {
    let Some(approval) = approval else {
        return resolved_commands(kind, project_path, global_path, false);
    };
    let commands = resolved_commands(kind, project_path, global_path, true)?;
    let current = ProjectHookApproval::from_resolved(&commands);
    if current != *approval {
        return Err(WorktreeConfigError::ProjectHooksChanged);
    }
    Ok(commands)
}

fn decode_commands(
    value: Option<&Value>,
    path: &Path,
) -> Result<Vec<HookCommand>, WorktreeConfigError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return invalid(path);
    };
    let mut commands = Vec::new();
    for value in values {
        let (command, name) = if let Some(command) = value.as_str() {
            (command, None)
        } else if let Some(object) = value.as_object() {
            let Some(command) = object.get("command").and_then(Value::as_str) else {
                return invalid(path);
            };
            let name = match object.get("name") {
                None | Some(Value::Null) => None,
                Some(Value::String(name)) => Some(name.to_owned()),
                Some(_) => return invalid(path),
            };
            (command, name)
        } else {
            return invalid(path);
        };
        let command = command.trim();
        if !command.is_empty() {
            commands.push(HookCommand {
                command: command.to_owned(),
                name,
            });
        }
    }
    Ok(commands)
}

fn commands(
    config: Option<&WorktreeConfig>,
    kind: HookKind,
    source: CommandSource,
) -> Vec<ResolvedCommand> {
    let commands = match (config, kind) {
        (Some(config), HookKind::Setup) => &config.setup,
        (Some(config), HookKind::Teardown) => &config.teardown,
        (None, _) => return Vec::new(),
    };
    commands
        .iter()
        .cloned()
        .map(|command| ResolvedCommand { command, source })
        .collect()
}

fn invalid<T>(path: &Path) -> Result<T, WorktreeConfigError> {
    Err(WorktreeConfigError::Invalid {
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &std::path::Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn worktree_config_normalizes_shapes_and_preserves_setup_and_teardown_order() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let global = temp.path().join("global.json");
        write(
            &global,
            r#"{"setup":[" global setup ",{"command":"   ","name":"blank"}],"teardown":[{"command":" global down ","name":"Global"}]}"#,
        );
        write(
            &project.join(".muxy/worktree.json"),
            r#"{"setup":[{"command":" project setup ","name":"Project"}],"teardown":[" project down "]}"#,
        );

        let setup = resolved_commands(HookKind::Setup, &project, &global, true).unwrap();
        assert_eq!(
            setup,
            vec![
                ResolvedCommand::new("global setup", None, CommandSource::Global),
                ResolvedCommand::new("project setup", Some("Project"), CommandSource::Project),
            ]
        );
        let teardown = resolved_commands(HookKind::Teardown, &project, &global, true).unwrap();
        assert_eq!(
            teardown,
            vec![
                ResolvedCommand::new("project down", None, CommandSource::Project),
                ResolvedCommand::new("global down", Some("Global"), CommandSource::Global),
            ]
        );
    }

    #[test]
    fn worktree_config_distinguishes_missing_invalid_and_unreadable_files() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing.json");
        assert_eq!(load(&missing).unwrap(), None);

        let invalid = temp.path().join("invalid.json");
        write(&invalid, r#"{"setup":[7]}"#);
        assert!(matches!(
            load(&invalid),
            Err(WorktreeConfigError::Invalid { .. })
        ));

        let unreadable = temp.path().join("directory.json");
        std::fs::create_dir(&unreadable).unwrap();
        assert!(matches!(
            load(&unreadable),
            Err(WorktreeConfigError::Unreadable { .. })
        ));

        let non_utf8 = temp.path().join("non-utf8.json");
        std::fs::write(&non_utf8, [0xff]).unwrap();
        assert!(matches!(
            load(&non_utf8),
            Err(WorktreeConfigError::Invalid { .. })
        ));
    }

    #[test]
    fn worktree_config_approval_is_exact_and_absent_approval_skips_project_loading() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        let global = temp.path().join("global.json");
        write(&global, r#"{"setup":["global"]}"#);
        write(
            &project.join(".muxy/worktree.json"),
            r#"{"setup":[{"command":"project","name":"Project"}]}"#,
        );
        let displayed = resolved_commands(HookKind::Setup, &project, &global, true).unwrap();
        let approval = ProjectHookApproval::from_resolved(&displayed);
        assert_eq!(
            commands_for_execution(HookKind::Setup, &project, &global, Some(&approval)).unwrap(),
            displayed
        );

        write(
            &project.join(".muxy/worktree.json"),
            r#"{"setup":[{"command":"changed","name":"Project"}]}"#,
        );
        assert_eq!(
            commands_for_execution(HookKind::Setup, &project, &global, Some(&approval))
                .unwrap_err()
                .to_string(),
            "Project worktree hooks changed after approval. Review them and try again."
        );
        write(&project.join(".muxy/worktree.json"), "{invalid");
        assert_eq!(
            commands_for_execution(HookKind::Setup, &project, &global, None).unwrap(),
            vec![ResolvedCommand::new("global", None, CommandSource::Global)]
        );
    }

    #[test]
    fn worktree_config_global_path_uses_nonempty_xdg_then_home() {
        assert_eq!(
            global_config_path(
                std::path::Path::new("/home/user"),
                Some(std::ffi::OsStr::new("/xdg"))
            ),
            std::path::Path::new("/xdg/muxy/worktree.json")
        );
        assert_eq!(
            global_config_path(
                std::path::Path::new("/home/user"),
                Some(std::ffi::OsStr::new(""))
            ),
            std::path::Path::new("/home/user/.config/muxy/worktree.json")
        );
    }

    #[test]
    fn worktree_config_approval_normalizes_direct_resolved_input() {
        let approval = ProjectHookApproval::from_resolved(&[
            ResolvedCommand::new("  project  ", Some("Name"), CommandSource::Project),
            ResolvedCommand::new("   ", Some("Blank"), CommandSource::Project),
            ResolvedCommand::new(" global ", None, CommandSource::Global),
        ]);

        assert_eq!(
            approval.commands,
            vec![HookCommand {
                command: "project".into(),
                name: Some("Name".into()),
            }]
        );
    }
}
