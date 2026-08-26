use super::CommandResult;
use super::target::{
    display_id, find_project, find_worktree, is_directory, parse_flags, preferred_worktree,
    resolve, standardize_path,
};
use crate::state::AppState;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use muxy_core::store::{HOME_PROJECT_ID, Project, Worktree};
use std::path::Path;

pub enum ProjectCommand {
    Immediate(CommandResult),
    Refresh(Box<Project>),
}

fn decode(value: &str) -> Option<String> {
    String::from_utf8(STANDARD.decode(value).ok()?).ok()
}

fn decode_optional(parts: &[&str], index: usize, label: &str) -> Result<Option<String>, String> {
    let Some(value) = parts.get(index) else {
        return Ok(None);
    };
    let Some(value) = decode(value) else {
        return Err(format!("error:invalid {label}"));
    };
    Ok((!value.is_empty()).then_some(value))
}

fn resolve_project<'a>(state: &'a AppState, identifier: Option<&str>) -> Option<&'a Project> {
    match identifier.filter(|identifier| !identifier.is_empty()) {
        Some(identifier) => find_project(state, identifier),
        None => state.active_project(),
    }
}

fn choose_worktree(state: &AppState, project: &Project) -> Result<Worktree, String> {
    preferred_worktree(state, project)
        .cloned()
        .ok_or_else(|| format!("error:no worktree for project {}", project.name))
}

fn select(state: &mut AppState, project_id: &str, worktree_id: &str) -> bool {
    state.try_select_worktree(project_id, worktree_id)
}

pub fn ensure_project_context(state: &mut AppState, project_id: &str) -> Result<Worktree, String> {
    let Some(project) = state.workspace.project(project_id).cloned() else {
        return Err(format!("error:project not found {project_id}"));
    };
    if !state.worktrees.contains_key(project_id) {
        let Some(worktrees) =
            muxy_api::worktrees::load_or_create_primary(&project.id, &project.name, &project.path)
        else {
            return Err(format!(
                "error:could not load worktrees for project {}",
                project.name
            ));
        };
        state.worktrees.insert(project.id.clone(), worktrees);
    }
    let worktree = choose_worktree(state, &project)?;
    if !select(state, &project.id, &worktree.id) {
        return Err("error:could not save project workspace".to_owned());
    }
    Ok(worktree)
}

pub fn handle(head: &str, parts: &[&str], state: &mut AppState) -> Option<ProjectCommand> {
    Some(match head {
        "list-projects" => {
            let reply = state
                .workspace
                .projects
                .iter()
                .map(|project| {
                    format!(
                        "{}\t{}\t{}\t{}",
                        display_id(&project.id),
                        project.name,
                        project.path,
                        state.active_project_id.as_deref() == Some(project.id.as_str())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            ProjectCommand::Immediate(CommandResult::reply(reply))
        }
        "switch-project" => {
            if parts.len() < 2 {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:usage switch-project|name-or-id-or-path",
                )));
            }
            let identifier = parts[1..].join("|");
            let parsed = parse_flags(&[&identifier, "--project", &identifier]);
            let display_identifier = parsed.remaining.join("|");
            let target = match resolve(state, &parsed) {
                Ok(Some(target)) => target,
                Ok(None) => unreachable!(),
                Err(error) if error == "worktree not found " => {
                    let name = find_project(state, &identifier)
                        .map_or(display_identifier.as_str(), |project| project.name.as_str());
                    return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                        "error:no worktree for project {name}"
                    ))));
                }
                Err(error) => {
                    return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                        "error:{error}"
                    ))));
                }
            };
            if !select(state, &target.project_id, &target.worktree_id) {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:could not save project workspace",
                )));
            }
            ProjectCommand::Immediate(CommandResult::changed("ok"))
        }
        "list-worktrees" => {
            let identifier = (parts.len() >= 2).then(|| parts[1..].join("|"));
            let Some(project) = resolve_project(state, identifier.as_deref()) else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:project not found{}",
                    identifier
                        .as_deref()
                        .filter(|identifier| !identifier.is_empty())
                        .map_or(String::new(), |identifier| format!(" {identifier}"))
                ))));
            };
            let reply = state
                .worktrees
                .get(&project.id)
                .into_iter()
                .flatten()
                .map(|worktree| {
                    format!(
                        "{}\t{}\t{}\t{}\t{}",
                        display_id(&worktree.id),
                        worktree.name,
                        worktree.path,
                        worktree.branch.as_deref().unwrap_or_default(),
                        state.active_project_id.as_deref() == Some(project.id.as_str())
                            && state.prefs.active_worktree_ids.get(&project.id)
                                == Some(&worktree.id)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            ProjectCommand::Immediate(CommandResult::reply(reply))
        }
        "switch-worktree" => {
            if parts.len() < 2 {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:usage switch-worktree|name-or-id-or-path[|project]",
                )));
            }
            let project_identifier = (parts.len() >= 3).then(|| parts[2..].join("|"));
            let Some(project) = resolve_project(state, project_identifier.as_deref()).cloned()
            else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:project not found{}",
                    project_identifier
                        .as_deref()
                        .filter(|identifier| !identifier.is_empty())
                        .map_or(String::new(), |identifier| format!(" {identifier}"))
                ))));
            };
            let Some(worktree) = state
                .worktrees
                .get(&project.id)
                .and_then(|worktrees| find_worktree(worktrees, parts[1]))
                .cloned()
            else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:worktree not found {}",
                    parts[1]
                ))));
            };
            if !select(state, &project.id, &worktree.id) {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:could not save project workspace",
                )));
            }
            ProjectCommand::Immediate(CommandResult::changed("ok"))
        }
        "refresh-worktrees" => {
            let identifier = (parts.len() >= 2).then(|| parts[1..].join("|"));
            let Some(project) = resolve_project(state, identifier.as_deref()).cloned() else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:project not found{}",
                    identifier
                        .as_deref()
                        .filter(|identifier| !identifier.is_empty())
                        .map_or(String::new(), |identifier| format!(" {identifier}"))
                ))));
            };
            ProjectCommand::Refresh(Box::new(project))
        }
        "create-project" => {
            if parts.len() < 2 {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:usage create-project|<base64-path>[|createIfMissing][|base64-name][|base64-workspace]",
                )));
            }
            let Some(raw_path) = decode(parts[1]) else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:invalid create-project path",
                )));
            };
            let Some(path) = standardize_path(&raw_path) else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:invalid create-project path",
                )));
            };
            let create_if_missing = parts.get(2) == Some(&"true");
            let name = match decode_optional(parts, 3, "create-project name") {
                Ok(name) => name,
                Err(error) => {
                    return Some(ProjectCommand::Immediate(CommandResult::reply(error)));
                }
            };
            let workspace_identifier = match decode_optional(parts, 4, "create-project workspace") {
                Ok(identifier) => identifier.map(|identifier| identifier.trim().to_owned()),
                Err(error) => {
                    return Some(ProjectCommand::Immediate(CommandResult::reply(error)));
                }
            };
            let target_group = workspace_identifier
                .as_deref()
                .map(|identifier| {
                    state
                        .workspace
                        .groups
                        .resolve(identifier)
                        .filter(|group| group.is_local)
                        .map(|group| group.id.clone())
                        .ok_or_else(|| format!("error:workspace not found '{identifier}'"))
                })
                .transpose();
            let target_group = match target_group {
                Ok(group) => group.or_else(|| {
                    state
                        .workspace
                        .active_group_id
                        .as_deref()
                        .filter(|id| state.workspace.groups.is_local(id))
                        .map(str::to_owned)
                }),
                Err(error) => {
                    return Some(ProjectCommand::Immediate(CommandResult::reply(error)));
                }
            };
            if !Path::new(&path).exists() {
                if !create_if_missing {
                    return Some(ProjectCommand::Immediate(CommandResult::reply(
                        "error:path does not exist, use --create to create it",
                    )));
                }
                if std::fs::create_dir_all(&path).is_err() {
                    return Some(ProjectCommand::Immediate(CommandResult::reply(
                        "error:could not create directory",
                    )));
                }
            } else if !is_directory(&path) {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:path is not a directory",
                )));
            }
            let existing = state
                .workspace
                .contains_path(&path)
                .map(|project| project.id.clone());
            let created = existing.is_none();
            let project_id = match existing.or_else(|| {
                let default_name = Path::new(&path)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| path.clone());
                state.workspace.add(default_name, path.clone())
            }) {
                Some(id) => id,
                None => {
                    return Some(ProjectCommand::Immediate(CommandResult::reply(
                        "error:could not open project",
                    )));
                }
            };
            if created
                && !state
                    .workspace
                    .update(&project_id, |project| project.worktrees_enabled = true)
            {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:could not save project changes",
                )));
            }
            if let Some(name) = name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                && !state
                    .workspace
                    .update(&project_id, |project| project.name = name.to_owned())
            {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:could not save project changes",
                )));
            }
            if let Err(error) = ensure_project_context(state, &project_id) {
                return Some(ProjectCommand::Immediate(CommandResult::reply(error)));
            }
            if let Some(group_id) = target_group {
                let group_result = state
                    .workspace
                    .groups
                    .try_add_project(&project_id, &group_id);
                match group_result {
                    Ok(true) => {}
                    result => {
                        let error = match result {
                            Err(error) => {
                                format!("error:could not save workspace changes: {error}")
                            }
                            Ok(_) => "error:project cannot be added to workspace".to_owned(),
                        };
                        return Some(ProjectCommand::Immediate(CommandResult::reply(error)));
                    }
                }
            }
            let project = state
                .workspace
                .project(&project_id)
                .expect("created project");
            ProjectCommand::Immediate(CommandResult::changed(format!(
                "ok\t{}\t{}\t{}",
                display_id(&project.id),
                project.name,
                project.path
            )))
        }
        "attach-project" => {
            if parts.len() < 3 {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:usage attach-project|<base64-project>|<base64-workspace>",
                )));
            }
            let Some(project_identifier) = decode(parts[1]) else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:invalid attach-project project identifier",
                )));
            };
            let Some(workspace_identifier) = decode(parts[2]) else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:invalid attach-project workspace identifier",
                )));
            };
            let Some(project) = find_project(state, &project_identifier).cloned() else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:project not found {project_identifier}"
                ))));
            };
            if project.id.eq_ignore_ascii_case(HOME_PROJECT_ID) || project.is_remote() {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:the home and remote projects cannot be attached to a workspace",
                )));
            }
            let Some(group) = state
                .workspace
                .groups
                .resolve(&workspace_identifier)
                .filter(|group| group.is_local)
            else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:workspace not found '{workspace_identifier}'"
                ))));
            };
            let group_id = group.id.clone();
            let group_name = group.name.clone();
            match state
                .workspace
                .groups
                .try_add_project(&project.id, &group_id)
            {
                Ok(true) => ProjectCommand::Immediate(CommandResult::changed("ok")),
                Ok(false) => ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:project cannot be added to workspace '{group_name}'"
                ))),
                Err(error) => ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:could not save workspace changes: {error}"
                ))),
            }
        }
        "detach-project" => {
            if parts.len() < 2 {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:usage detach-project|<base64-project>",
                )));
            }
            let Some(identifier) = decode(parts[1]) else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:invalid detach-project project identifier",
                )));
            };
            let Some(project) = find_project(state, &identifier).cloned() else {
                return Some(ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:project not found {identifier}"
                ))));
            };
            if project.id.eq_ignore_ascii_case(HOME_PROJECT_ID) || project.is_remote() {
                return Some(ProjectCommand::Immediate(CommandResult::reply(
                    "error:the home and remote projects cannot be detached from a workspace",
                )));
            }
            match state
                .workspace
                .groups
                .try_remove_project_everywhere(&project.id)
            {
                Ok(()) => ProjectCommand::Immediate(CommandResult::changed("ok")),
                Err(error) => ProjectCommand::Immediate(CommandResult::reply(format!(
                    "error:could not save workspace changes: {error}"
                ))),
            }
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(command: &str, state: &mut AppState) -> String {
        let parts: Vec<&str> = command.split('|').collect();
        match handle(parts[0], &parts, state).expect("recognized") {
            ProjectCommand::Immediate(result) => result.reply,
            ProjectCommand::Refresh(_) => "refresh".to_owned(),
        }
    }

    #[test]
    fn optional_base64_fields_distinguish_empty_and_invalid() {
        assert_eq!(decode_optional(&["cmd", ""], 1, "field"), Ok(None));
        assert_eq!(
            decode_optional(&["cmd", "%%%"], 1, "field"),
            Err("error:invalid field".to_owned())
        );
    }

    #[test]
    fn create_project_trims_the_workspace_identifier() {
        let temp = tempfile::tempdir().expect("temp dir");
        let project_path = temp.path().join("project");
        std::fs::create_dir(&project_path).unwrap();
        let mut state = super::super::test_state(temp.path());
        let project = Project::new(
            "Project".to_owned(),
            project_path.to_string_lossy().into_owned(),
            0,
        );
        let project_id = project.id.clone();
        state.workspace = muxy_core::store::Workspace::for_tests(vec![project]);
        state.workspace.groups =
            muxy_core::store::Groups::load_from(temp.path().join("project-groups.json"));
        state.worktrees.insert(
            project_id,
            vec![muxy_core::store::worktrees::primary(
                "Project",
                &project_path.to_string_lossy(),
            )],
        );
        let group_id = state.workspace.groups.add("Team".to_owned());
        let command = format!(
            "create-project|{}|true||{}",
            STANDARD.encode(project_path.to_string_lossy().as_bytes()),
            STANDARD.encode(" Team ")
        );

        let response = reply(&command, &mut state);
        assert!(response.starts_with("ok\t"), "{response}");
        let project_id = response.split('\t').nth(1).unwrap();
        assert_eq!(
            state.workspace.groups.group_id_containing(project_id),
            Some(group_id.as_str())
        );
    }

    #[test]
    fn every_project_handler_has_deterministic_empty_state_output() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut state = super::super::test_state(temp.path());
        let cases = [
            ("list-projects", ""),
            (
                "switch-project",
                "error:usage switch-project|name-or-id-or-path",
            ),
            ("list-worktrees", "error:project not found"),
            (
                "switch-worktree",
                "error:usage switch-worktree|name-or-id-or-path[|project]",
            ),
            ("refresh-worktrees", "error:project not found"),
            (
                "create-project",
                "error:usage create-project|<base64-path>[|createIfMissing][|base64-name][|base64-workspace]",
            ),
            (
                "attach-project",
                "error:usage attach-project|<base64-project>|<base64-workspace>",
            ),
            (
                "detach-project",
                "error:usage detach-project|<base64-project>",
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(reply(command, &mut state), expected, "{command}");
        }
    }
}
