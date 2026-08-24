use crate::command::Command;
use crate::views::menu::Item;
use muxy_core::store::{Group, Project};

pub fn items(project: &Project, groups: &[&Group]) -> Vec<Item> {
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
            items.push(Item::label("Refresh Worktrees"));
            items.push(Item::label("New Worktree…"));
            items.push(Item::label("Switch Worktree…"));
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
