use crate::socket::commands::CommandResult;
use crate::socket::commands::target::{self, ResolvedTarget};
use crate::views::window::MainWindow;
use muxy_core::workspace::{CloseMode, Edge, Tab, TabKind};
use std::path::PathBuf;

pub enum PaneCommand {
    Immediate(CommandResult),
    Surface(SurfaceCommand),
}

pub struct SurfaceCommand {
    pub pane_id: String,
    pub operation: SurfaceOperation,
}

pub enum SurfaceOperation {
    SendText(String),
    SendBytes(Vec<u8>),
    ReadScreen(usize),
}

pub fn handle(head: &str, parts: &[&str], window: &mut MainWindow) -> Option<PaneCommand> {
    let result = match head {
        "split-right" => split(parts, window, Edge::Right),
        "split-down" => split(parts, window, Edge::Bottom),
        "send" => surface(parts, window, SurfaceKind::Send),
        "send-keys" => surface(parts, window, SurfaceKind::Keys),
        "read-screen" => surface(parts, window, SurfaceKind::Read),
        "close-pane" => immediate(close(parts, window)),
        "rename-pane" => immediate(rename(parts, window)),
        "list-panes" => immediate(Ok(CommandResult::reply(list(window)))),
        _ => return None,
    };
    Some(result)
}

fn immediate(result: Result<CommandResult, String>) -> PaneCommand {
    PaneCommand::Immediate(
        result.unwrap_or_else(|error| CommandResult::reply(format!("error:{error}"))),
    )
}

fn split(parts: &[&str], window: &mut MainWindow, edge: Edge) -> PaneCommand {
    immediate(split_result(parts, window, edge))
}

fn split_result(
    parts: &[&str],
    window: &mut MainWindow,
    edge: Edge,
) -> Result<CommandResult, String> {
    let parsed = target::parse_flags(&parts[1..]);
    let target = target::resolve(&window.state, &parsed)?;
    let (from_pane, command) = parse_split_request(&parsed.remaining);
    let context = split_context(window, target.as_ref(), from_pane.as_deref())?;
    let workspace = &window.state.tab_workspaces.states()[context.state_index];
    let project_path = workspace
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
        .ok_or_else(|| "no active workspace".to_owned())?;
    let launch_directory = context
        .source_tab_id
        .as_deref()
        .and_then(|tab_id| {
            window
                .terminal_runtime
                .surfaces
                .handle(tab_id)
                .and_then(|handle| handle.metadata().working_directory.clone())
                .or_else(|| {
                    workspace
                        .tab(tab_id)
                        .and_then(|tab| tab.project_path.clone())
                })
        })
        .unwrap_or_else(|| project_path.clone());
    let mut tab = Tab::new(TabKind::Terminal);
    tab.project_path = Some(project_path.clone());
    let pane_id = tab.id.clone();
    let previous = window.state.tab_workspaces.clone();
    let created = window.state.tab_workspaces.states_mut()[context.state_index]
        .split_area(&context.area_id, edge, tab)
        .is_some();
    if !created {
        return Err("split succeeded but could not determine new pane ID".to_owned());
    }
    if let Err(error) = window.state.tab_workspaces.save() {
        window.state.tab_workspaces = previous;
        return Err(error.to_string());
    }
    window
        .terminal_runtime
        .surfaces
        .queue_launch_directory(pane_id.clone(), PathBuf::from(launch_directory));
    if let Some(command) = command
        .map(|command| command.trim().to_owned())
        .filter(|command| !command.is_empty())
    {
        window.terminal_runtime.surfaces.queue_launch_command(
            pane_id.clone(),
            crate::terminal::LaunchCommand {
                command,
                keeps_shell_open: true,
            },
        );
    }
    Ok(CommandResult::changed(target::display_id(&pane_id)))
}

fn surface(parts: &[&str], window: &MainWindow, kind: SurfaceKind) -> PaneCommand {
    let usage = match kind {
        SurfaceKind::Send => "usage send|paneID|text",
        SurfaceKind::Keys => "usage send-keys|paneID|key",
        SurfaceKind::Read => "usage read-screen|paneID[|lines]",
    };
    let minimum = if matches!(kind, SurfaceKind::Read) {
        2
    } else {
        3
    };
    if parts.len() < minimum {
        return immediate(Err(usage.to_owned()));
    }
    let pane_id = parts[1];
    if !valid_uuid(pane_id) {
        return immediate(Err("invalid pane ID".to_owned()));
    }
    let Some((_, _, canonical_id)) = locate_pane(window, pane_id) else {
        return immediate(Err(format!("pane not found {pane_id}")));
    };
    let operation = match kind {
        SurfaceKind::Send => SurfaceOperation::SendText(parts[2..].join("|")),
        SurfaceKind::Keys => match key_bytes(parts[2]) {
            Some(bytes) => SurfaceOperation::SendBytes(bytes.to_vec()),
            None => {
                return immediate(Err(format!("unsupported key {}", parts[2])));
            }
        },
        SurfaceKind::Read => {
            let lines = parts
                .get(2)
                .and_then(|lines| lines.parse::<isize>().ok())
                .unwrap_or(50)
                .clamp(1, 500) as usize;
            SurfaceOperation::ReadScreen(lines)
        }
    };
    PaneCommand::Surface(SurfaceCommand {
        pane_id: canonical_id,
        operation,
    })
}

fn close(parts: &[&str], window: &mut MainWindow) -> Result<CommandResult, String> {
    if parts.len() < 2 {
        return Err("usage close-pane|paneID".to_owned());
    }
    if !valid_uuid(parts[1]) {
        return Err("invalid pane ID".to_owned());
    }
    let Some((state_index, _, pane_id)) = locate_pane(window, parts[1]) else {
        return Err(format!("pane not found {}", parts[1]));
    };
    let previous = window.state.tab_workspaces.clone();
    let removed = window.state.tab_workspaces.states_mut()[state_index]
        .close_tab(&pane_id, CloseMode::Single);
    if removed.is_empty() {
        return Ok(CommandResult::reply("ok"));
    }
    if let Err(error) = window.state.tab_workspaces.save() {
        window.state.tab_workspaces = previous;
        return Err(error.to_string());
    }
    for pane_id in removed {
        if let Some(handle) = window.terminal_runtime.surfaces.handle(&pane_id) {
            handle.request_close();
        }
    }
    Ok(CommandResult::changed("ok"))
}

fn rename(parts: &[&str], window: &mut MainWindow) -> Result<CommandResult, String> {
    if parts.len() < 3 {
        return Err("usage rename-pane|paneID|title".to_owned());
    }
    if !valid_uuid(parts[1]) {
        return Err("invalid pane ID".to_owned());
    }
    let Some((state_index, _, pane_id)) = locate_pane(window, parts[1]) else {
        return Err(format!("pane not found {}", parts[1]));
    };
    let previous = window.state.tab_workspaces.clone();
    let tab = window.state.tab_workspaces.states_mut()[state_index]
        .tab_mut(&pane_id)
        .ok_or_else(|| "could not rename pane".to_owned())?;
    tab.custom_title = Some(parts[2..].join("|"));
    if let Err(error) = window.state.tab_workspaces.save() {
        window.state.tab_workspaces = previous;
        return Err(error.to_string());
    }
    Ok(CommandResult::changed("ok"))
}

fn list(window: &MainWindow) -> String {
    let mut lines = Vec::new();
    for workspace in window.state.tab_workspaces.states() {
        let focused_area = workspace.focused_area_id.as_deref();
        let Some(root) = workspace.root.as_ref() else {
            continue;
        };
        for area_id in root.area_ids() {
            let Some(area) = root.area_by_id(&area_id) else {
                continue;
            };
            for tab in &area.tabs {
                if tab.kind != TabKind::Terminal {
                    continue;
                }
                let handle = window.terminal_runtime.surfaces.handle(&tab.id);
                let title = tab
                    .custom_title
                    .as_deref()
                    .or_else(|| handle.and_then(|handle| handle.metadata().title.as_deref()))
                    .unwrap_or_else(|| tab.title());
                let directory = handle
                    .and_then(|handle| handle.metadata().working_directory.as_deref())
                    .or(tab.project_path.as_deref())
                    .unwrap_or_default();
                let focused = focused_area == Some(area_id.as_str())
                    && area.active_tab_id.as_deref() == Some(tab.id.as_str());
                lines.push(format!(
                    "{}\t{}\t{}\t{}",
                    target::display_id(&tab.id),
                    title,
                    directory,
                    focused
                ));
            }
        }
    }
    lines.join("\n")
}

pub fn perform_surface(window: &MainWindow, command: &SurfaceCommand) -> Option<String> {
    match &command.operation {
        SurfaceOperation::SendText(text) => window
            .terminal_runtime
            .surfaces
            .send_text(&command.pane_id, text)
            .then(|| "ok".to_owned()),
        SurfaceOperation::SendBytes(bytes) => window
            .terminal_runtime
            .surfaces
            .send_bytes(&command.pane_id, bytes)
            .then(|| "ok".to_owned()),
        SurfaceOperation::ReadScreen(lines) => window
            .terminal_runtime
            .surfaces
            .read_screen_text(&command.pane_id, *lines),
    }
}

struct SplitContext {
    state_index: usize,
    area_id: String,
    source_tab_id: Option<String>,
}

fn split_context(
    window: &MainWindow,
    target: Option<&ResolvedTarget>,
    from_pane: Option<&str>,
) -> Result<SplitContext, String> {
    let target_index = match target {
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
            }),
        None => None,
    };
    if let Some(pane_id) = from_pane.filter(|pane_id| valid_uuid(pane_id))
        && let Some((state_index, area_id, source_tab_id)) = locate_pane(window, pane_id)
        && target_index.is_none_or(|target_index| target_index == state_index)
    {
        return Ok(SplitContext {
            state_index,
            area_id,
            source_tab_id: Some(source_tab_id),
        });
    }
    let state_index = match target_index {
        Some(index) => index,
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
                .ok_or_else(|| "no active workspace".to_owned())?
        }
    };
    let workspace = &window.state.tab_workspaces.states()[state_index];
    let area_id = workspace
        .focused_area_id
        .clone()
        .ok_or_else(|| "no focused area".to_owned())?;
    let source_tab_id = workspace
        .area(&area_id)
        .and_then(|area| area.active_tab_id.clone());
    Ok(SplitContext {
        state_index,
        area_id,
        source_tab_id,
    })
}

fn locate_pane(window: &MainWindow, pane_id: &str) -> Option<(usize, String, String)> {
    window
        .state
        .tab_workspaces
        .states()
        .iter()
        .enumerate()
        .find_map(|(state_index, workspace)| {
            let tab = workspace
                .root
                .as_ref()?
                .tabs()
                .into_iter()
                .find(|tab| tab.id.eq_ignore_ascii_case(pane_id))?;
            let area_id = workspace.area_containing_tab(&tab.id)?.id.clone();
            Some((state_index, area_id, tab.id.clone()))
        })
}

pub(crate) fn split_has_startup_command(parts: &[&str]) -> bool {
    let parsed = target::parse_flags(&parts[1..]);
    parse_split_request(&parsed.remaining)
        .1
        .is_some_and(|command| !command.trim().is_empty())
}

fn parse_split_request(parts: &[String]) -> (Option<String>, Option<String>) {
    let Some(first) = parts.first() else {
        return (None, None);
    };
    if first.is_empty() || valid_uuid(first) {
        return (
            Some(first.clone()),
            (parts.len() >= 2).then(|| parts[1..].join("|")),
        );
    }
    if parts.len() >= 2 && parts.last().is_some_and(|value| valid_uuid(value)) {
        return (
            parts.last().cloned(),
            Some(parts[..parts.len() - 1].join("|")),
        );
    }
    (None, Some(parts.join("|")))
}

fn key_bytes(key: &str) -> Option<&'static [u8]> {
    match key.to_ascii_lowercase().as_str() {
        "escape" | "esc" => Some(b"\x1b"),
        "enter" | "return" => Some(b"\r"),
        "tab" => Some(b"\t"),
        "ctrl+c" | "ctrl-c" => Some(b"\x03"),
        "ctrl+d" | "ctrl-d" => Some(b"\x04"),
        "ctrl+z" | "ctrl-z" => Some(b"\x1a"),
        "backspace" => Some(b"\x7f"),
        _ => None,
    }
}

pub(super) fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

#[derive(Clone, Copy)]
enum SurfaceKind {
    Send,
    Keys,
    Read,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_request_supports_every_retained_form() {
        let pane = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE";
        assert_eq!(parse_split_request(&[]), (None, None));
        assert_eq!(
            parse_split_request(&[pane.to_owned(), "echo|ok".to_owned()]),
            (Some(pane.to_owned()), Some("echo|ok".to_owned()))
        );
        assert_eq!(
            parse_split_request(&["echo".to_owned(), "ok".to_owned(), pane.to_owned()]),
            (Some(pane.to_owned()), Some("echo|ok".to_owned()))
        );
        assert_eq!(
            parse_split_request(&["echo".to_owned(), "ok".to_owned()]),
            (None, Some("echo|ok".to_owned()))
        );
    }

    #[test]
    fn keys_and_pane_ids_are_strict() {
        assert_eq!(key_bytes("CTRL-C"), Some(b"\x03".as_slice()));
        assert!(key_bytes("space").is_none());
        assert!(valid_uuid("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"));
        assert!(!valid_uuid("AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEZ"));
    }
}
