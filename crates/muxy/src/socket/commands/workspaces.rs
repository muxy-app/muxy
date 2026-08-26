use super::CommandResult;
use super::target::display_id;
use crate::state::AppState;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

fn decode(value: &str) -> Option<String> {
    String::from_utf8(STANDARD.decode(value).ok()?).ok()
}

fn field(parts: &[&str], index: usize, label: &str) -> Result<Option<String>, String> {
    let value = parts.get(index).copied().unwrap_or_default();
    let Some(decoded) = decode(value) else {
        return Err(format!("error:invalid {label}"));
    };
    Ok((!decoded.is_empty()).then_some(decoded))
}

pub fn handle(head: &str, parts: &[&str], state: &mut AppState) -> Option<CommandResult> {
    Some(match head {
        "list-workspaces" => {
            let reply = state
                .workspace
                .groups
                .all()
                .iter()
                .map(|group| {
                    format!(
                        "{}\t{}\t{}\t{}",
                        display_id(&group.id),
                        group.name,
                        group.project_count(),
                        state.workspace.active_group_id.as_deref() == Some(group.id.as_str())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            CommandResult::reply(reply)
        }
        "create-workspace" => {
            if parts.len() < 2 {
                return Some(CommandResult::reply(
                    "error:usage create-workspace|<base64-name>".to_owned(),
                ));
            }
            let name = match field(parts, 1, "create-workspace name") {
                Ok(name) => name
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "New Workspace".to_owned()),
                Err(error) => return Some(CommandResult::reply(error)),
            };
            match state.workspace.groups.try_add(name) {
                Ok(id) => {
                    state.workspace.select_group(Some(id.clone()));
                    CommandResult::changed(format!("ok\t{}", display_id(&id)))
                }
                Err(error) => CommandResult::reply(format!(
                    "error:could not save workspace creation: {error}"
                )),
            }
        }
        "switch-workspace" => {
            if parts.len() < 2 {
                return Some(CommandResult::reply(
                    "error:usage switch-workspace|<base64-name-or-id>".to_owned(),
                ));
            }
            let identifier = match field(parts, 1, "switch-workspace identifier") {
                Ok(Some(identifier)) => identifier,
                Ok(None) => String::new(),
                Err(error) => return Some(CommandResult::reply(error)),
            };
            let Some(group) = state.workspace.groups.resolve(&identifier) else {
                return Some(CommandResult::reply(format!(
                    "error:workspace not found '{identifier}'"
                )));
            };
            state.workspace.select_group(Some(group.id.clone()));
            CommandResult::changed("ok".to_owned())
        }
        "rename-workspace" => {
            if parts.len() < 3 {
                return Some(CommandResult::reply(
                    "error:usage rename-workspace|<base64-name-or-id>|<base64-new-name>".to_owned(),
                ));
            }
            let identifier = match field(parts, 1, "rename-workspace identifier") {
                Ok(Some(identifier)) => identifier,
                Ok(None) => String::new(),
                Err(error) => return Some(CommandResult::reply(error)),
            };
            let name = match field(parts, 2, "rename-workspace name") {
                Ok(Some(name)) if !name.trim().is_empty() => name.trim().to_owned(),
                Ok(_) => {
                    return Some(CommandResult::reply(
                        "error:name cannot be empty".to_owned(),
                    ));
                }
                Err(error) => return Some(CommandResult::reply(error)),
            };
            let Some(group_id) = state
                .workspace
                .groups
                .resolve(&identifier)
                .map(|group| group.id.clone())
            else {
                return Some(CommandResult::reply(format!(
                    "error:workspace not found '{identifier}'"
                )));
            };
            match state.workspace.groups.try_rename(&group_id, name) {
                Ok(true) => CommandResult::changed("ok".to_owned()),
                Ok(false) => {
                    CommandResult::reply(format!("error:workspace not found '{identifier}'"))
                }
                Err(error) => {
                    CommandResult::reply(format!("error:could not save workspace changes: {error}"))
                }
            }
        }
        "delete-workspace" => {
            if parts.len() < 2 {
                return Some(CommandResult::reply(
                    "error:usage delete-workspace|<base64-name-or-id>".to_owned(),
                ));
            }
            let identifier = match field(parts, 1, "delete-workspace identifier") {
                Ok(Some(identifier)) => identifier,
                Ok(None) => String::new(),
                Err(error) => return Some(CommandResult::reply(error)),
            };
            let Some(group) = state.workspace.groups.resolve(&identifier) else {
                return Some(CommandResult::reply(format!(
                    "error:workspace not found '{identifier}'"
                )));
            };
            if group.project_count() != 0 {
                return Some(CommandResult::reply(format!(
                    "error:workspace '{}' still contains projects",
                    group.name
                )));
            }
            let group_id = group.id.clone();
            match state.workspace.groups.try_remove(&group_id) {
                Ok(true) => {
                    if state.workspace.active_group_id.as_deref() == Some(group_id.as_str()) {
                        state.workspace.select_group(None);
                    }
                    CommandResult::changed("ok".to_owned())
                }
                Ok(false) => {
                    CommandResult::reply(format!("error:workspace not found '{identifier}'"))
                }
                Err(error) => CommandResult::reply(format!(
                    "error:could not save workspace deletion: {error}"
                )),
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
        handle(parts[0], &parts, state).expect("recognized").reply
    }

    #[test]
    fn decodes_utf8_base64_and_rejects_invalid_fields() {
        assert_eq!(decode("V29yayDwn5qA"), Some("Work 🚀".to_owned()));
        assert_eq!(decode("%%%"), None);
    }

    #[test]
    fn every_workspace_handler_has_deterministic_empty_state_output() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut state = super::super::test_state(temp.path());
        let cases = [
            ("list-workspaces", ""),
            (
                "create-workspace",
                "error:usage create-workspace|<base64-name>",
            ),
            (
                "switch-workspace",
                "error:usage switch-workspace|<base64-name-or-id>",
            ),
            (
                "rename-workspace",
                "error:usage rename-workspace|<base64-name-or-id>|<base64-new-name>",
            ),
            (
                "delete-workspace",
                "error:usage delete-workspace|<base64-name-or-id>",
            ),
        ];
        for (command, expected) in cases {
            assert_eq!(reply(command, &mut state), expected, "{command}");
        }
    }

    #[test]
    fn workspace_crud_reports_exact_success_and_persists() {
        let temp = tempfile::tempdir().expect("temp dir");
        let groups_path = temp.path().join("project-groups.json");
        std::fs::write(&groups_path, b"[]").expect("groups file");
        let mut state = super::super::test_state(temp.path());
        state.workspace.groups = muxy_core::store::Groups::load_from(&groups_path);

        let created = reply("create-workspace|V29yaw==", &mut state);
        let id = created.strip_prefix("ok\t").expect("created id").to_owned();
        assert_eq!(
            reply("list-workspaces", &mut state),
            format!("{id}\tWork\t0\ttrue")
        );
        assert_eq!(reply("rename-workspace|V29yaw==|TmV3", &mut state), "ok");
        assert_eq!(reply("switch-workspace|TmV3", &mut state), "ok");
        assert_eq!(reply("delete-workspace|TmV3", &mut state), "ok");
        assert_eq!(reply("list-workspaces", &mut state), "");
        assert_eq!(
            std::fs::read_to_string(groups_path).expect("saved groups"),
            "[]"
        );
    }
}
