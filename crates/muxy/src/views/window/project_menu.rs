use crate::command::Command;
use crate::views::menu::Item;
use muxy_core::store::{Group, Project, Worktree};

pub fn items(
    project: &Project,
    groups: &[&Group],
    worktrees: &[Worktree],
    active_worktree_id: Option<&str>,
) -> Vec<Item> {
    let id = project.id.clone();

    if project.is_home() {
        return vec![
            Item::action("Copy Path", Command::CopyPath(id)),
            Item::Separator,
            Item::action("Hide Home", Command::HideHome),
        ];
    }

    let mut items = Vec::new();

    if !project.is_remote() {
        let label = if project.is_pinned { "Unpin" } else { "Pin" };
        items.push(Item::action(label, Command::TogglePin(id.clone())));
        items.push(Item::Separator);
    }

    items.push(Item::action("Set Logo...", Command::PickLogo(id.clone())));
    if project.logo.is_some() {
        items.push(Item::action("Remove Logo", Command::RemoveLogo(id.clone())));
    }
    items.push(Item::action(
        "Set Icon...",
        Command::OpenSymbolPicker(id.clone()),
    ));
    if project.icon.is_some() {
        items.push(Item::action("Remove Icon", Command::RemoveIcon(id.clone())));
    }
    items.push(Item::action(
        "Set Icon Color...",
        Command::OpenColorPicker(id.clone()),
    ));
    if project.icon_color.is_some() {
        items.push(Item::action(
            "Reset Icon Color",
            Command::ResetIconColor(id.clone()),
        ));
    }

    items.push(Item::Separator);
    items.push(Item::action(
        "Rename Project",
        Command::StartRename(id.clone()),
    ));

    if project.is_git_repo {
        items.push(Item::Separator);
        items.push(
            Item::action("Worktrees", Command::ToggleWorktrees(id.clone()))
                .checked(project.worktrees_enabled),
        );
        if project.worktrees_enabled {
            items.push(Item::action(
                "Refresh Worktrees",
                Command::RefreshWorktrees(id.clone()),
            ));
            items.push(Item::action(
                "New Worktree…",
                Command::NewWorktree(id.clone()),
            ));
            if worktrees.len() > 1 {
                let switch = worktrees
                    .iter()
                    .map(|worktree| {
                        Item::action(
                            if worktree.is_primary {
                                "primary".to_owned()
                            } else {
                                worktree.name.clone()
                            },
                            Command::SelectWorktree {
                                project_id: id.clone(),
                                worktree_id: worktree.id.clone(),
                            },
                        )
                        .checked(
                            active_worktree_id
                                .is_some_and(|active| worktree.id.eq_ignore_ascii_case(active)),
                        )
                    })
                    .collect();
                items.push(Item::submenu("Switch Worktree", switch));
            }
            if let Some(worktree) = active_worktree_id.and_then(|active| {
                worktrees
                    .iter()
                    .find(|worktree| worktree.id.eq_ignore_ascii_case(active))
            }) && project.can_remove_worktree(worktree)
            {
                items.push(
                    Item::action(
                        "Remove Worktree…",
                        Command::RemoveWorktree {
                            project_id: id.clone(),
                            worktree_id: worktree.id.clone(),
                        },
                    )
                    .destructive(),
                );
            }
        }
    }

    let memberships: Vec<Item> = groups
        .iter()
        .filter(|group| group.is_local)
        .map(|group| {
            let is_member = group
                .project_ids
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(&id));
            Item::action(
                &group.name,
                Command::MoveProjectToWorkspace {
                    project_id: id.clone(),
                    group_id: group.id.clone(),
                },
            )
            .checked(is_member)
        })
        .collect();
    if !project.is_remote() && !memberships.is_empty() {
        items.push(Item::Separator);
        items.push(Item::submenu("Move to Workspace", memberships));
    }

    items.push(Item::Separator);
    items.push(Item::action("Copy Path", Command::CopyPath(id.clone())));
    items.push(Item::Separator);
    items.push(Item::action("Remove Project", Command::RemoveProject(id)).destructive());

    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::store::worktrees::{Source, Worktree};

    fn action_labels(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .filter_map(|item| match item {
                Item::Action {
                    label, disabled, ..
                } if !disabled => Some(label.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn project_menu_exposes_refresh_new_switch_and_secondary_remove_commands() {
        let mut project = Project::new("Project".into(), "/repo".into(), 0);
        project.id = "PROJECT".into();
        project.is_git_repo = true;
        project.worktrees_enabled = true;
        let worktrees = vec![
            Worktree {
                id: "PRIMARY".into(),
                name: "Project".into(),
                path: "/repo".into(),
                branch: Some("main".into()),
                source: Source::Muxy,
                is_primary: true,
                created_at: 1.0,
                last_active_at: None,
            },
            Worktree {
                id: "FEATURE".into(),
                name: "Feature".into(),
                path: "/feature".into(),
                branch: Some("feature".into()),
                source: Source::Muxy,
                is_primary: false,
                created_at: 2.0,
                last_active_at: None,
            },
        ];
        let menu = items(&project, &[], &worktrees, Some("FEATURE"));
        let labels = action_labels(&menu);
        assert!(labels.contains(&"Refresh Worktrees".to_owned()));
        assert!(labels.contains(&"New Worktree…".to_owned()));
        assert!(labels.contains(&"Remove Worktree…".to_owned()));
        let switch = menu
            .iter()
            .find_map(|item| match item {
                Item::Submenu { label, items, .. } if label.as_ref() == "Switch Worktree" => {
                    Some(items)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(switch.len(), 2);

        let primary_menu = items(&project, &[], &worktrees, Some("PRIMARY"));
        assert!(!action_labels(&primary_menu).contains(&"Remove Worktree…".to_owned()));

        project.remote_workspace_id = Some("REMOTE".into());
        let remote_menu = items(&project, &[], &worktrees, Some("FEATURE"));
        assert!(!action_labels(&remote_menu).contains(&"Remove Worktree…".to_owned()));
    }
}
