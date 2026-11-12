//! Tab, pane, and project controls beyond main's verbs: the actions the tab,
//! pane, and project menus offer, addressed by ID.

use gpui::Context;
use muxy_app_core::settings::ProjectOrder;
use muxy_app_core::{
    Direction, FloatScope, PaneContent, PaneId, ProjectId, ProjectStatus, TabCloseScope, TabId,
    TabPlace, TabSide,
};
use serde_json::{Value, json};

use super::{AppModel, Call, workspace::parse_color};

fn direction(value: &Value) -> Result<Direction, String> {
    match value.as_str() {
        None | Some("right") => Ok(Direction::Right),
        Some("left") => Ok(Direction::Left),
        Some("up") => Ok(Direction::Up),
        Some("down") => Ok(Direction::Down),
        Some(other) => Err(format!("invalid direction '{other}'")),
    }
}

fn float_scope(value: &Value) -> Result<FloatScope, String> {
    match value.as_str() {
        None | Some("project") => Ok(FloatScope::Project),
        Some("allProjects") => Ok(FloatScope::AllProjects),
        Some(other) => Err(format!("invalid scope '{other}'")),
    }
}

fn project_order(value: &Value) -> Result<ProjectOrder, String> {
    Ok(match value.as_str().unwrap_or("") {
        "manual" => ProjectOrder::Manual,
        "name" => ProjectOrder::Name,
        "nameDescending" => ProjectOrder::NameDescending,
        "recentlyActive" => ProjectOrder::RecentlyActive,
        "dateAdded" => ProjectOrder::DateAdded,
        other => return Err(format!("invalid order '{other}'")),
    })
}

impl AppModel {
    fn tab_arg(&self, args: &Value) -> Result<TabId, String> {
        let identifier = args["tabId"].as_str().unwrap_or("");
        identifier
            .parse::<TabId>()
            .ok()
            .filter(|tab| self.tab(*tab).is_some())
            .ok_or_else(|| format!("tab not found {identifier}"))
    }

    fn pane_arg(&self, args: &Value) -> Result<PaneId, String> {
        let identifier = args["paneID"].as_str().unwrap_or("");
        identifier
            .parse::<PaneId>()
            .ok()
            .filter(|pane| self.state.pane_tab(*pane).is_some())
            .ok_or_else(|| format!("pane not found {identifier}"))
    }

    fn terminal_arg(&self, args: &Value) -> Result<PaneId, String> {
        let pane = self.pane_arg(args)?;
        self.state
            .placed_pane(pane)
            .filter(|placed| matches!(placed.pane.content, PaneContent::Terminal { .. }))
            .map(|_| pane)
            .ok_or_else(|| format!("pane {pane} is not a terminal"))
    }

    /// A move's target: a project's active worktree, or the worktree named.
    fn destination(&self, args: &Value, source: ProjectId) -> Result<ProjectId, String> {
        let identifier = args["destination"].as_str().unwrap_or("");
        let project = self
            .find_project(identifier)
            .ok_or_else(|| format!("project not found {identifier}"))?;
        let target = if project.parent_id.is_some() {
            project.id
        } else {
            self.preferred_worktree(project.id)
        };
        if !self
            .move_destinations(source)
            .iter()
            .any(|(id, _)| *id == target)
        {
            return Err(format!("cannot move into project {identifier}"));
        }
        Ok(target)
    }

    /// Shows the pane's tab, or raises its window, and focuses the pane.
    /// Answers whether it became the main window's active pane.
    fn reveal_pane(&mut self, pane: PaneId, cx: &mut Context<Self>) -> Result<bool, String> {
        let project = self.state.pane_project(pane).map(|project| project.id);
        let tab = self.state.pane_tab(pane);
        let (Some(project), Some(tab)) = (project, tab) else {
            return Err(format!("pane not found {pane}"));
        };
        if self.raise_pane_window(pane, cx) {
            return Ok(false);
        }
        self.select_project(project, cx);
        self.select_tab(tab, cx);
        self.focus_pane(pane, cx);
        Ok(self.active_pane() == Some(pane))
    }

    /// `agents.focus`: jumps to an agent's pane, as the agents panel does.
    pub(super) fn focus_extension_agent(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let pane = self.pane_arg(&call.args)?;
        self.reveal_pane(pane, cx).map(|_| Value::Null)
    }

    fn activate_pane(&mut self, pane: PaneId, cx: &mut Context<Self>) -> Result<(), String> {
        if self.reveal_pane(pane, cx)? {
            Ok(())
        } else {
            Err(format!("pane {pane} is not in the main window"))
        }
    }

    fn edited(
        &mut self,
        edit: impl FnOnce(&mut muxy_app_core::AppState) -> Result<(), muxy_app_core::AppError>,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if self.edit_tab(edit, cx) {
            Ok(Value::Null)
        } else {
            Err("could not save tabs".into())
        }
    }

    pub(super) fn tab_control(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if call.verb == "tabs.new" {
            return self.new_extension_tab(&call.args, cx);
        }
        let tab = self.tab_arg(&call.args)?;
        match call.verb.as_str() {
            "tabs.close" => {
                match call.args["scope"].as_str() {
                    None => self.close_tab(tab, cx),
                    Some(scope) => {
                        let scope = match scope {
                            "others" => TabCloseScope::Other,
                            "left" => TabCloseScope::Left,
                            "right" => TabCloseScope::Right,
                            other => return Err(format!("invalid scope '{other}'")),
                        };
                        self.close_tabs(tab, scope, cx);
                    }
                }
                Ok(Value::Null)
            }
            "tabs.rename" => {
                let title = call.args["title"]
                    .as_str()
                    .map(str::trim)
                    .filter(|title| !title.is_empty())
                    .map(str::to_owned);
                self.edited(|state| state.set_tab_title(tab, title), cx)
            }
            "tabs.setColor" => {
                let color = call.args["color"].as_str().map(parse_color).transpose()?;
                self.edited(|state| state.set_tab_color(tab, color), cx)
            }
            "tabs.pin" => {
                let pinned = call.args["pinned"].as_bool().unwrap_or(true);
                if self.tab(tab).is_some_and(|found| found.pinned == pinned) {
                    return Ok(Value::Null);
                }
                self.edited(|state| state.toggle_tab_pin(tab), cx)
            }
            "tabs.move" => self.move_extension_tab(tab, &call.args, cx),
            "tabs.moveToWindow" => {
                if self.tab(tab).is_some_and(muxy_app_core::Tab::detached) {
                    return Err("the tab already has a window".into());
                }
                self.move_tab_to_window(tab, None, None, cx);
                Ok(Value::Null)
            }
            "tabs.dock" => {
                let window = self
                    .tab(tab)
                    .and_then(|tab| tab.window.as_ref())
                    .map(|window| window.id)
                    .ok_or("the tab is in the main window")?;
                self.return_tab_window(window, None, cx);
                Ok(Value::Null)
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    fn new_extension_tab(&mut self, args: &Value, cx: &mut Context<Self>) -> Result<Value, String> {
        let before: std::collections::HashSet<TabId> = self
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .map(|tab| tab.id)
            .collect();
        if args["beside"].is_null() {
            self.new_tab(cx);
        } else {
            let anchor = self.tab_arg(&json!({"tabId": args["beside"]}))?;
            let side = match args["side"].as_str() {
                None | Some("right") => TabSide::Right,
                Some("left") => TabSide::Left,
                Some(other) => return Err(format!("invalid side '{other}'")),
            };
            self.new_tab_adjacent(anchor, side, cx);
        }
        self.state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .find(|tab| !before.contains(&tab.id))
            .map(|tab| json!(tab.id.to_string()))
            .ok_or_else(|| "could not open a tab".into())
    }

    fn move_extension_tab(
        &mut self,
        tab: TabId,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let project = self.tab_project(tab).ok_or("tab not found")?;
        let source = project.id;
        if !args["destination"].is_null() {
            let target = self.destination(args, source)?;
            self.move_tab_to_place(tab, TabPlace::end(target), cx);
            return Ok(Value::Null);
        }
        let to = args["index"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .ok_or("index or project is required")?;
        let from = project
            .tabs
            .iter()
            .position(|candidate| candidate.id == tab)
            .ok_or("tab not found")?;
        if to >= project.tabs.len() {
            return Err(format!("index {to} is out of range"));
        }
        self.transact(|state| state.move_tab(source, from, to), cx)
            .map_err(|error| error.to_string())?
            .ok_or("could not save tabs")?;
        self.changed(cx);
        Ok(Value::Null)
    }

    pub(super) fn pane_control(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let pane = self.pane_arg(&call.args)?;
        match call.verb.as_str() {
            "panes.focus" => self.reveal_pane(pane, cx).map(|_| Value::Null),
            "panes.split" => {
                let edge = direction(&call.args["direction"])?;
                self.activate_pane(pane, cx)?;
                self.split_pane(edge, cx);
                let created = self.active_pane().filter(|active| *active != pane);
                created
                    .map(|pane| json!(pane.to_string()))
                    .ok_or_else(|| "could not split the pane".into())
            }
            "panes.zoom" => {
                let zoomed = |model: &Self| {
                    model
                        .state
                        .placed_pane(pane)
                        .and_then(|placed| placed.tab)
                        .is_some_and(|tab| tab.zoomed == Some(pane))
                };
                let target = call.args["zoomed"]
                    .as_bool()
                    .unwrap_or_else(|| !zoomed(self));
                if zoomed(self) == target {
                    return Ok(Value::Null);
                }
                self.activate_pane(pane, cx)?;
                if zoomed(self) != target {
                    self.toggle_zoom_pane(cx);
                }
                Ok(Value::Null)
            }
            "panes.equalize" => {
                self.equalize_splits(pane, cx);
                Ok(Value::Null)
            }
            "panes.detach" => {
                let pane = self.terminal_arg(&call.args)?;
                if !self.can_detach_terminal(pane) {
                    return Err("the terminal cannot be detached".into());
                }
                self.detach_terminal(pane, cx);
                Ok(Value::Null)
            }
            "panes.restart" => {
                let pane = self.terminal_arg(&call.args)?;
                let proxy = call.args["proxy"]
                    .as_bool()
                    .unwrap_or_else(|| self.pane_uses_proxy(pane));
                if !self.can_restart_terminal(pane, proxy) {
                    return Err("the terminal cannot be restarted".into());
                }
                self.restart_terminal(pane, proxy, cx);
                Ok(Value::Null)
            }
            "panes.scrollToBottom" => {
                let pane = self.terminal_arg(&call.args)?;
                let view = self
                    .terminal(&pane)
                    .map(|view| view.view.clone())
                    .ok_or_else(|| format!("pane surface not ready {pane}"))?;
                view.update(
                    cx,
                    crate::views::terminal::pane::TerminalPane::scroll_to_bottom,
                );
                Ok(Value::Null)
            }
            "panes.float" => {
                let scope = float_scope(&call.args["scope"])?;
                self.float_pane(pane, scope, cx);
                Ok(Value::Null)
            }
            "panes.moveToTab" => {
                let source = self
                    .state
                    .pane_project(pane)
                    .map(|project| project.id)
                    .ok_or("pane not found")?;
                let target = if call.args["destination"].is_null() {
                    source
                } else {
                    self.destination(&call.args, source)?
                };
                self.move_pane_to_new_tab(pane, TabPlace::end(target), cx);
                Ok(Value::Null)
            }
            "panes.moveToWindow" => {
                self.move_pane_to_window(pane, None, None, cx);
                Ok(Value::Null)
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    pub(super) fn project_control(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        match call.verb.as_str() {
            "projects.next" | "projects.previous" => {
                self.cycle_project(call.verb == "projects.next", cx);
                Ok(Value::Null)
            }
            "projects.focus" => {
                let project = match call.args["identifier"].as_str() {
                    None => None,
                    Some(identifier) => Some(
                        self.find_project(identifier)
                            .map(super::events::root_of)
                            .ok_or_else(|| format!("project not found {identifier}"))?,
                    ),
                };
                self.set_project_focus(project, cx);
                Ok(Value::Null)
            }
            "projects.sort" => {
                self.appearance.sidebar_project_order = project_order(&call.args["order"])?;
                self.save_appearance(cx);
                Ok(Value::Null)
            }
            "projects.removed" => Ok(Value::Array(
                self.state
                    .removed_projects()
                    .iter()
                    .enumerate()
                    .map(|(index, removed)| {
                        json!({
                            "index": index,
                            "name": removed.name,
                            "path": removed.directory.display().to_string(),
                            "isLocal": removed.server_id.is_local(),
                        })
                    })
                    .collect(),
            )),
            "projects.restore" => {
                let index = call.args["index"]
                    .as_u64()
                    .and_then(|index| usize::try_from(index).ok())
                    .filter(|index| *index < self.state.removed_projects().len())
                    .ok_or("no removed project at that index")?;
                self.restore_removed_project(index, cx);
                Ok(Value::Null)
            }
            "projects.apps" => Ok(Value::Array(
                self.project_apps
                    .listed
                    .iter()
                    .flatten()
                    .map(|listed| json!({"id": listed.app.id, "name": listed.app.name}))
                    .collect(),
            )),
            "projects.openIn" => {
                let directory = self.local_project_directory(&call.args)?;
                let app = match call.args["app"].as_str() {
                    Some(app) => self
                        .project_apps
                        .listed
                        .iter()
                        .flatten()
                        .find(|listed| listed.app.id == app)
                        .map(|listed| listed.app.id.clone())
                        .ok_or_else(|| format!("app not found {app}"))?,
                    None => self
                        .default_project_app()
                        .map(|listed| listed.app.id.clone())
                        .ok_or("no app to open the project in")?,
                };
                self.open_directory_with(directory, app, cx);
                Ok(Value::Null)
            }
            "projects.reveal" => {
                let directory = self.local_project_directory(&call.args)?;
                cx.reveal_path(&directory);
                Ok(Value::Null)
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    fn local_project_directory(&self, args: &Value) -> Result<std::path::PathBuf, String> {
        let identifier = args["identifier"].as_str().unwrap_or("");
        let project = self
            .find_project(identifier)
            .ok_or_else(|| format!("project not found {identifier}"))?;
        if !project.server_id.is_local() {
            return Err("the project is on another computer".into());
        }
        if project.status() != ProjectStatus::Available {
            return Err("the project is not available".into());
        }
        Ok(project.directory.clone())
    }
}
