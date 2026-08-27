use crate::socket::commands::CommandResult;
use crate::socket::commands::target::{self, ResolvedTarget};
use crate::views::window::MainWindow;
use muxy_core::workspace::{CloseMode, Tab, TabKind, WorkspaceState};

pub fn handle(head: &str, parts: &[&str], window: &mut MainWindow) -> Option<CommandResult> {
    let result = match head {
        "list-tabs" => list(parts, window),
        "switch-tab" => switch_tab(parts, window),
        "new-tab" => new_tab(parts, window),
        "next-tab" => cycle(parts, window, 1),
        "previous-tab" => cycle(parts, window, -1),
        "tab-rename" => metadata(parts, window, Metadata::Title),
        "tab-set-color" => metadata(parts, window, Metadata::Color),
        "tab-set-icon" => metadata(parts, window, Metadata::Icon),
        "tab-pin" => pin(parts, window, true),
        "tab-unpin" => pin(parts, window, false),
        "tab-close" => close(parts, window),
        "tab-move" => move_tab(parts, window),
        _ => return None,
    };
    Some(result.unwrap_or_else(|error| CommandResult::reply(format!("error:{error}"))))
}

fn list(parts: &[&str], window: &MainWindow) -> Result<CommandResult, String> {
    let parsed = target::parse_flags(&parts[1..]);
    let target = target::resolve(&window.state, &parsed)?;
    let index = target_index(window, target.as_ref())?;
    let workspace = &window.state.tab_workspaces.states()[index];
    let lines = flat_tabs(workspace)
        .into_iter()
        .enumerate()
        .map(|(index, location)| {
            let active = workspace.focused_area_id.as_deref() == Some(location.area_id.as_str())
                && workspace
                    .area(&location.area_id)
                    .and_then(|area| area.active_tab_id.as_deref())
                    == Some(location.tab_id.as_str());
            let tab = workspace.tab(&location.tab_id).unwrap();
            format!(
                "{}\t{}\t{}\t{}\t{}",
                index,
                target::display_id(&tab.id),
                kind_name(tab.kind),
                tab.title(),
                active
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(CommandResult::reply(lines))
}

fn switch_tab(parts: &[&str], window: &mut MainWindow) -> Result<CommandResult, String> {
    let parsed = target::parse_flags(&parts[1..]);
    if parsed.remaining.is_empty() {
        return Err("usage switch-tab|index-or-id-or-title".to_owned());
    }
    let target = target::resolve(&window.state, &parsed)?;
    let index = target_index(window, target.as_ref())?;
    let identifier = parsed.remaining.join("|");
    let location = resolve_in(&window.state.tab_workspaces.states()[index], &identifier)
        .ok_or_else(|| format!("tab not found {identifier}"))?;
    mutate(window, |states| {
        states[index].select_tab(&location.area_id, &location.tab_id);
        Ok(())
    })?;
    Ok(CommandResult::changed("ok"))
}

fn new_tab(parts: &[&str], window: &mut MainWindow) -> Result<CommandResult, String> {
    let parsed = target::parse_flags(&parts[1..]);
    let target = target::resolve(&window.state, &parsed)?;
    let index = target_index(window, target.as_ref())?;
    let project_path = workspace_path(window, index)?;
    let mut tab = Tab::new(TabKind::Terminal);
    tab.project_path = Some(project_path.clone());
    let tab_id = tab.id.clone();
    mutate(window, |states| {
        states[index]
            .new_top_level_tab(tab)
            .ok_or_else(|| "could not create tab".to_owned())?;
        Ok(())
    })?;
    window
        .terminal_runtime
        .surfaces
        .queue_launch_directory(tab_id.clone(), project_path.into());
    Ok(CommandResult::changed(target::display_id(&tab_id)))
}

fn cycle(parts: &[&str], window: &mut MainWindow, delta: isize) -> Result<CommandResult, String> {
    let parsed = target::parse_flags(&parts[1..]);
    let target = target::resolve(&window.state, &parsed)?;
    let index = target_index(window, target.as_ref())?;
    let workspace = &window.state.tab_workspaces.states()[index];
    let locations = flat_tabs(workspace);
    if locations.is_empty() {
        return Ok(CommandResult::reply("ok"));
    }
    let current = locations
        .iter()
        .position(|location| {
            workspace.focused_area_id.as_deref() == Some(location.area_id.as_str())
                && workspace
                    .area(&location.area_id)
                    .and_then(|area| area.active_tab_id.as_deref())
                    == Some(location.tab_id.as_str())
        })
        .unwrap_or(0);
    let next = (current as isize + delta).rem_euclid(locations.len() as isize) as usize;
    let location = locations[next].clone();
    mutate(window, |states| {
        states[index].select_tab(&location.area_id, &location.tab_id);
        Ok(())
    })?;
    Ok(CommandResult::changed("ok"))
}

fn metadata(
    parts: &[&str],
    window: &mut MainWindow,
    metadata: Metadata,
) -> Result<CommandResult, String> {
    let usage = match metadata {
        Metadata::Title => "usage tab-rename|<index-or-id-or-title>[|title]",
        Metadata::Color => "usage tab-set-color|<index-or-id-or-title>[|color]",
        Metadata::Icon => "usage tab-set-icon|<index-or-id-or-title>[|sf-symbol]",
    };
    if parts.len() < 2 {
        return Err(usage.to_owned());
    }
    let (index, location) =
        locate(window, parts[1]).ok_or_else(|| format!("tab not found {}", parts[1]))?;
    let value = match metadata {
        Metadata::Title => parts
            .get(2..)
            .map(|values| values.join("|"))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        Metadata::Icon => parts
            .get(2)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        Metadata::Color => color(parts.get(2).copied().unwrap_or_default())?,
    };
    mutate(window, |states| {
        let tab = states[index]
            .tab_mut(&location.tab_id)
            .ok_or_else(|| "could not update tab".to_owned())?;
        match metadata {
            Metadata::Title => tab.custom_title = value,
            Metadata::Color => tab.color_id = value,
            Metadata::Icon => tab.custom_icon = value,
        }
        Ok(())
    })?;
    Ok(CommandResult::changed("ok"))
}

fn pin(parts: &[&str], window: &mut MainWindow, pinned: bool) -> Result<CommandResult, String> {
    if parts.len() < 2 {
        return Err(format!(
            "usage {}|<index-or-id-or-title>",
            if pinned { "tab-pin" } else { "tab-unpin" }
        ));
    }
    let (index, location) =
        locate(window, parts[1]).ok_or_else(|| format!("tab not found {}", parts[1]))?;
    mutate(window, |states| {
        states[index].set_tab_pinned(&location.tab_id, pinned);
        Ok(())
    })?;
    Ok(CommandResult::changed("ok"))
}

fn close(parts: &[&str], window: &mut MainWindow) -> Result<CommandResult, String> {
    if parts.len() < 2 {
        return Err("usage tab-close|<index-or-id-or-title>".to_owned());
    }
    let (index, location) =
        locate(window, parts[1]).ok_or_else(|| format!("tab not found {}", parts[1]))?;
    let mut removed = Vec::new();
    mutate(window, |states| {
        removed = states[index].close_tab(&location.tab_id, CloseMode::Single);
        Ok(())
    })?;
    for tab_id in removed {
        if let Some(handle) = window.terminal_runtime.surfaces.handle(&tab_id) {
            handle.request_close();
        }
    }
    Ok(CommandResult::changed("ok"))
}

fn move_tab(parts: &[&str], window: &mut MainWindow) -> Result<CommandResult, String> {
    if parts.len() < 3 {
        return Err("usage tab-move|<index-or-id-or-title>|<to-index>".to_owned());
    }
    let to_index = parts[2]
        .parse::<isize>()
        .map_err(|_| "usage tab-move|<index-or-id-or-title>|<to-index>".to_owned())?;
    if to_index < 0 {
        return Err("index out of range".to_owned());
    }
    let to_index = to_index as usize;
    let (index, location) =
        locate(window, parts[1]).ok_or_else(|| format!("tab not found {}", parts[1]))?;
    let workspace = &window.state.tab_workspaces.states()[index];
    let locations = flat_tabs(workspace);
    let target = locations
        .get(to_index)
        .ok_or_else(|| "index out of range".to_owned())?
        .clone();
    let tab = workspace.tab(&location.tab_id).unwrap();
    let target_tab = workspace.tab(&target.tab_id).unwrap();
    if tab.parent_id.is_none() {
        if target_tab.parent_id.is_some() {
            return Err("target index is not a top-level tab".to_owned());
        }
        if tab.pinned != target_tab.pinned {
            return Err("target index crosses the pinned tab boundary".to_owned());
        }
        let root_index = workspace
            .top_level_order
            .iter()
            .position(|id| id == &target.tab_id)
            .ok_or_else(|| format!("tab not found {}", parts[1]))?;
        mutate(window, |states| {
            states[index].reorder_top_level_tab(&location.tab_id, root_index);
            Ok(())
        })?;
    } else {
        if location.area_id != target.slot_area_id {
            return Err("target index is in a different pane".to_owned());
        }
        if tab.pinned != target_tab.pinned {
            return Err("target index crosses the pinned tab boundary".to_owned());
        }
        let target_slot = target.slot_index;
        mutate(window, |states| {
            states[index]
                .area_mut(&location.area_id)
                .ok_or_else(|| "target index is in a different pane".to_owned())?
                .reorder(&location.tab_id, target_slot);
            Ok(())
        })?;
    }
    Ok(CommandResult::changed("ok"))
}

fn mutate(
    window: &mut MainWindow,
    action: impl FnOnce(&mut [WorkspaceState]) -> Result<(), String>,
) -> Result<(), String> {
    let previous = window.state.tab_workspaces.clone();
    if let Err(error) = action(window.state.tab_workspaces.states_mut()) {
        window.state.tab_workspaces = previous;
        return Err(error);
    }
    if let Err(error) = window.state.persist_tab_workspaces() {
        window.state.tab_workspaces = previous;
        return Err(error.to_string());
    }
    Ok(())
}

fn target_index(window: &MainWindow, target: Option<&ResolvedTarget>) -> Result<usize, String> {
    match target {
        Some(target) => window
            .state
            .tab_workspaces
            .states()
            .iter()
            .position(|workspace| {
                workspace
                    .project_id
                    .eq_ignore_ascii_case(&target.project_id)
                    && workspace
                        .worktree_id
                        .as_deref()
                        .is_some_and(|id| id.eq_ignore_ascii_case(&target.worktree_id))
            })
            .ok_or_else(|| "no active workspace".to_owned()),
        None => {
            let active = window
                .state
                .active_tab_workspace()
                .ok_or_else(|| "no active project".to_owned())?;
            window
                .state
                .tab_workspaces
                .states()
                .iter()
                .position(|workspace| std::ptr::eq(workspace, active))
                .ok_or_else(|| "no active project".to_owned())
        }
    }
}

fn workspace_path(window: &MainWindow, index: usize) -> Result<String, String> {
    let workspace = &window.state.tab_workspaces.states()[index];
    workspace
        .worktree_path
        .clone()
        .or_else(|| {
            window
                .state
                .workspace
                .projects
                .iter()
                .find(|project| project.id.eq_ignore_ascii_case(&workspace.project_id))
                .map(|project| project.path.clone())
        })
        .ok_or_else(|| "no active workspace".to_owned())
}

#[derive(Clone)]
struct Location {
    area_id: String,
    tab_id: String,
    slot_area_id: String,
    slot_index: usize,
}

fn flat_tabs(workspace: &WorkspaceState) -> Vec<Location> {
    let Some(root) = workspace.root.as_ref() else {
        return Vec::new();
    };
    let traversed: Vec<Location> = root
        .area_ids()
        .into_iter()
        .flat_map(|area_id| {
            root.area_by_id(&area_id).into_iter().flat_map(move |area| {
                let area_id = area_id.clone();
                area.tabs
                    .iter()
                    .enumerate()
                    .map(move |(slot_index, tab)| Location {
                        area_id: area_id.clone(),
                        tab_id: tab.id.clone(),
                        slot_area_id: area_id.clone(),
                        slot_index,
                    })
            })
        })
        .collect();
    let mut roots: Vec<Location> = traversed
        .iter()
        .filter(|location| {
            workspace
                .tab(&location.tab_id)
                .is_some_and(|tab| tab.parent_id.is_none())
        })
        .cloned()
        .collect();
    roots.sort_by_key(|location| {
        workspace
            .top_level_order
            .iter()
            .position(|id| id == &location.tab_id)
            .unwrap_or(usize::MAX)
    });
    let mut roots = roots.into_iter();
    traversed
        .into_iter()
        .map(|slot| {
            if workspace
                .tab(&slot.tab_id)
                .is_some_and(|tab| tab.parent_id.is_none())
            {
                let mut root = roots.next().unwrap_or_else(|| slot.clone());
                root.slot_area_id = slot.slot_area_id;
                root.slot_index = slot.slot_index;
                root
            } else {
                slot
            }
        })
        .collect()
}

fn resolve_in(workspace: &WorkspaceState, identifier: &str) -> Option<Location> {
    let locations = flat_tabs(workspace);
    if let Ok(index) = identifier.parse::<usize>() {
        return locations.get(index).cloned();
    }
    locations.into_iter().find(|location| {
        workspace.tab(&location.tab_id).is_some_and(|tab| {
            tab.id.eq_ignore_ascii_case(identifier) || tab.title().eq_ignore_ascii_case(identifier)
        })
    })
}

fn locate(window: &MainWindow, identifier: &str) -> Option<(usize, Location)> {
    if super::panes::valid_uuid(identifier) {
        return window
            .state
            .tab_workspaces
            .states()
            .iter()
            .enumerate()
            .find_map(|(index, workspace)| {
                resolve_in(workspace, identifier).map(|tab| (index, tab))
            });
    }
    let index = target_index(window, None).ok()?;
    resolve_in(&window.state.tab_workspaces.states()[index], identifier).map(|tab| (index, tab))
}

fn color(value: &str) -> Result<Option<String>, String> {
    if value.is_empty() {
        return Ok(None);
    }
    const COLORS: [(&str, &str); 12] = [
        ("red", "#E5484D"),
        ("orange", "#F76B15"),
        ("amber", "#F5A623"),
        ("yellow", "#EBCB00"),
        ("lime", "#9BCD1E"),
        ("green", "#30A46C"),
        ("teal", "#12A594"),
        ("cyan", "#05A2C2"),
        ("blue", "#3E63DD"),
        ("indigo", "#5B5BD6"),
        ("violet", "#8E4EC6"),
        ("pink", "#D6409F"),
    ];
    COLORS
        .iter()
        .find(|(id, hex)| value == *id || value.eq_ignore_ascii_case(hex))
        .map(|(id, _)| Some((*id).to_owned()))
        .ok_or_else(|| format!("unknown color '{value}'"))
}

fn kind_name(kind: TabKind) -> &'static str {
    match kind {
        TabKind::Terminal => "terminal",
        TabKind::Browser => "browser",
        TabKind::ExtensionWebView => "extensionWebView",
    }
}

#[derive(Clone, Copy)]
enum Metadata {
    Title,
    Color,
    Icon,
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_core::workspace::Edge;

    fn tab(id: &str, title: &str) -> Tab {
        let mut tab = Tab::with_static_title(TabKind::Terminal, title);
        tab.id = id.to_owned();
        tab
    }

    #[test]
    fn flat_order_substitutes_ordered_roots_into_traversal_slots() {
        let mut workspace = WorkspaceState::new("project");
        workspace.new_top_level_tab(tab("a", "Alpha"));
        workspace.new_top_level_tab(tab("b", "Beta"));
        workspace.select_root_tab("a");
        workspace.split_focused_area(Edge::Right, tab("child", "Child"));
        assert_eq!(
            flat_tabs(&workspace)
                .into_iter()
                .map(|location| location.tab_id)
                .collect::<Vec<_>>(),
            ["a", "b", "child"]
        );
        assert_eq!(resolve_in(&workspace, "0").unwrap().tab_id, "a");
        assert_eq!(resolve_in(&workspace, "beta").unwrap().tab_id, "b");
    }

    #[test]
    fn colors_accept_ids_and_case_insensitive_hex_values() {
        assert_eq!(color("blue"), Ok(Some("blue".to_owned())));
        assert_eq!(color("#3e63dd"), Ok(Some("blue".to_owned())));
        assert_eq!(color(""), Ok(None));
        assert_eq!(color("purple"), Err("unknown color 'purple'".to_owned()));
    }

    #[test]
    fn socket_tab_mutations_use_the_navigation_persistence_seam() {
        let source = include_str!("tabs.rs");
        let direct = ["window.state.tab_workspaces", ".save()"].concat();
        let seam = ["window.state", ".persist_tab_workspaces()"].concat();
        assert!(!source.contains(&direct));
        assert!(source.contains(&seam));
    }
}
