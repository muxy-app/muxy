//! Workspace verbs (`tabs`, `panes`, `projects`, `worktrees`, `workspaces`,
//! `agents`) with main's reply shapes and error messages.

use std::path::{Component, PathBuf};

use gpui::Context;
use muxy_app_core::{
    PaneContent, PaneId, Project, ProjectId, TabId, Workspace, WorkspaceId,
    webview::WebviewDescriptor,
};
use serde_json::{Value, json};

use super::{AppModel, Call};

/// Expands `~` and resolves `.`/`..` lexically, like `standardizedFileURL`.
pub(super) fn standardized(path: &str) -> PathBuf {
    let expanded = match (path.strip_prefix('~'), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            PathBuf::from(home).join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(path),
    };
    let mut normalized = PathBuf::new();
    for part in expanded.components() {
        match part {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => (),
            part => normalized.push(part),
        }
    }
    normalized
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |value, (index, byte)| {
                value | (u32::from(*byte) << (16 - 8 * index))
            });
        for index in 0..4 {
            text.push(if index <= chunk.len() {
                char::from(ALPHABET[(value >> (18 - 6 * index)) as usize & 63])
            } else {
                '='
            });
        }
    }
    text
}

fn key_bytes(key: &str) -> Result<&'static [u8], String> {
    Ok(match key.to_lowercase().as_str() {
        "escape" | "esc" => b"\x1b",
        "enter" | "return" => b"\r",
        "tab" => b"\t",
        "ctrl+c" | "ctrl-c" => b"\x03",
        "ctrl+d" | "ctrl-d" => b"\x04",
        "ctrl+z" | "ctrl-z" => b"\x1a",
        "backspace" => b"\x7f",
        _ => return Err(format!("unsupported key {key}")),
    })
}

fn screen_text(grid: &muxy_client::RunGrid, lines: usize) -> String {
    let rows: Vec<String> = (0..grid.history.len() + grid.rows.len())
        .filter_map(|index| grid.content_row(index))
        .map(|runs| {
            runs.iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect();
    let end = rows
        .iter()
        .rposition(|row| !row.is_empty())
        .map_or(0, |index| index + 1);
    rows[end.saturating_sub(lines)..end].join("\n")
}

impl AppModel {
    /// Resolves main's project selector: an ID, a case-insensitive name, or a path.
    pub(super) fn find_project(&self, identifier: &str) -> Option<&Project> {
        let path = standardized(identifier);
        let matches = |project: &&Project| {
            project.id.to_string().eq_ignore_ascii_case(identifier)
                || project.name.to_lowercase() == identifier.to_lowercase()
                || project.directory == path
        };
        let projects = self.state.projects();
        projects
            .iter()
            .filter(|project| project.parent_id.is_none())
            .find(matches)
            .or_else(|| projects.iter().find(matches))
    }

    /// `projects.list`: top-level projects, Home included.
    pub(super) fn project_list(&self) -> Vec<Value> {
        let root = super::events::root_of(self.state.current_project());
        self.state
            .projects()
            .iter()
            .filter(|project| project.parent_id.is_none())
            .enumerate()
            .map(|(order, project)| {
                json!({
                    "id": project.id.to_string(),
                    "name": project.name,
                    "path": project.directory.display().to_string(),
                    "isActive": project.id == root,
                    "sortOrder": order,
                    "iconColor": project.color.as_str(),
                    "icon": project.icon,
                    "logo": project.logo.as_ref().map(|logo| {
                        format!("data:image/png;base64,{}", base64(logo))
                    }),
                    "worktreesEnabled": !project.home,
                })
            })
            .collect()
    }

    /// Resolves main's workspace selector: an ID or a case-insensitive name.
    pub(super) fn find_workspace(&self, identifier: &str) -> Option<&Workspace> {
        let id = identifier.parse::<WorkspaceId>().ok();
        self.state.workspaces().iter().find(|workspace| {
            Some(workspace.id) == id || workspace.name.to_lowercase() == identifier.to_lowercase()
        })
    }

    fn workspace_list(&self) -> Vec<Value> {
        let active = self.state.active_workspace().map(|workspace| workspace.id);
        self.state
            .workspaces()
            .iter()
            .map(|workspace| {
                json!({
                    "id": workspace.id.to_string(),
                    "name": workspace.name,
                    "projectCount": workspace.projects.len(),
                    "isActive": Some(workspace.id) == active,
                })
            })
            .collect()
    }

    /// Membership belongs to top-level projects; a worktree resolves to its parent.
    fn member_project(&self, identifier: &str, action: &str) -> Result<ProjectId, String> {
        let project = self
            .find_project(identifier)
            .ok_or_else(|| format!("project not found {identifier}"))?;
        if project.home {
            return Err(format!(
                "home and SSH workspace projects cannot be {action} a workspace"
            ));
        }
        Ok(project.parent_id.unwrap_or(project.id))
    }

    fn mutable_project(&self, identifier: &str) -> Result<ProjectId, String> {
        let project = self
            .find_project(identifier)
            .ok_or_else(|| format!("project not found {identifier}"))?;
        if project.home {
            return Err("the home project cannot be modified".into());
        }
        Ok(project.id)
    }

    fn apply_project_edit(
        &mut self,
        edit: impl FnOnce(&mut muxy_app_core::AppState) -> Result<(), muxy_app_core::AppError>,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if self.edit_project(edit, cx) {
            Ok(Value::Null)
        } else {
            Err("could not save project changes".into())
        }
    }

    pub(super) fn projects_call(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let text = |field: &str| {
            call.args[field]
                .as_str()
                .ok_or_else(|| format!("missing argument '{field}'"))
        };
        match call.verb.as_str() {
            "projects.list" => Ok(Value::Array(self.project_list())),
            "projects.switch" => {
                let root = self
                    .find_project(text("identifier")?)
                    .map(|project| project.parent_id.unwrap_or(project.id))
                    .ok_or_else(|| {
                        format!("project not found {}", text("identifier").unwrap_or(""))
                    })?;
                self.select_project(self.preferred_worktree(root), cx);
                Ok(Value::Null)
            }
            "projects.add" => self
                .add_extension_project(text("path")?, cx)
                .map(|id| json!(id.to_string())),
            "projects.rename" => {
                let id = self.mutable_project(text("identifier")?)?;
                let name = text("name")?.trim().to_owned();
                if name.is_empty() {
                    return Err("name cannot be empty".into());
                }
                self.apply_project_edit(|state| state.rename_project(id, &name), cx)
            }
            "projects.setColor" => {
                let id = self.mutable_project(text("identifier")?)?;
                let color = match call.args["color"].as_str() {
                    None => muxy_app_core::Color::default(),
                    Some(color) => muxy_app_core::PROJECT_COLORS
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(color))
                        .map_or(color, |(_, hex)| *hex)
                        .parse()
                        .map_err(|_| format!("unknown color '{color}'"))?,
                };
                self.apply_project_edit(|state| state.set_project_color(id, color), cx)
            }
            "projects.setIcon" => {
                let id = self.mutable_project(text("identifier")?)?;
                let icon = call.args["icon"]
                    .as_str()
                    .map(str::trim)
                    .filter(|icon| !icon.is_empty())
                    .map(str::to_owned);
                self.apply_project_edit(|state| state.set_project_icon(id, icon), cx)
            }
            "projects.setLogo" => {
                let id = self.mutable_project(text("identifier")?)?;
                if !call.args["logo"].is_null() {
                    return Err("invalid project logo".into());
                }
                self.apply_project_edit(|state| state.set_project_logo(id, None), cx)
            }
            "projects.reorder" => self.reorder_projects(&call.args, cx),
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    /// `projects.attach`/`detach` and `workspaces.*`.
    pub(super) fn workspaces_call(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let text = |field: &str| {
            call.args[field]
                .as_str()
                .ok_or_else(|| format!("missing argument '{field}'"))
        };
        match call.verb.as_str() {
            "projects.attach" => {
                let project = self.member_project(text("identifier")?, "attached to")?;
                let (workspace, name) = self.workspace_named(text("workspace")?)?;
                if self.edit_workspaces(
                    |state| state.set_workspace_member(workspace, project, true),
                    cx,
                ) {
                    Ok(Value::Null)
                } else {
                    Err(format!("project cannot be added to workspace '{name}'"))
                }
            }
            "projects.detach" => {
                let project = self.member_project(text("identifier")?, "detached from")?;
                let workspaces: Vec<_> = self
                    .state
                    .workspaces()
                    .iter()
                    .filter(|workspace| workspace.projects.contains(&project))
                    .map(|workspace| workspace.id)
                    .collect();
                self.apply_workspace_edit(
                    |state| {
                        for workspace in workspaces {
                            state.set_workspace_member(workspace, project, false)?;
                        }
                        Ok(())
                    },
                    cx,
                )
            }
            "workspaces.list" => Ok(Value::Array(self.workspace_list())),
            "workspaces.create" => {
                let name = text("name")?.trim();
                let name = if name.is_empty() {
                    "New Workspace"
                } else {
                    name
                };
                let workspace = self
                    .create_workspace(name, None, cx)
                    .ok_or("could not save workspace changes")?;
                self.select_workspace(Some(workspace), cx);
                Ok(json!(workspace.to_string()))
            }
            "workspaces.switch" => {
                let (workspace, _) = self.workspace_named(text("identifier")?)?;
                self.select_workspace(Some(workspace), cx);
                Ok(Value::Null)
            }
            "workspaces.rename" => {
                let name = text("name")?.trim().to_owned();
                if name.is_empty() {
                    return Err("name cannot be empty".into());
                }
                let (workspace, _) = self.workspace_named(text("identifier")?)?;
                self.apply_workspace_edit(|state| state.rename_workspace(workspace, &name), cx)
            }
            "workspaces.delete" => {
                let identifier = text("identifier")?;
                let workspace = self
                    .find_workspace(identifier)
                    .ok_or_else(|| format!("workspace not found '{identifier}'"))?;
                if !workspace.projects.is_empty() {
                    return Err(format!(
                        "workspace '{}' still contains projects",
                        workspace.name
                    ));
                }
                let workspace = workspace.id;
                if self.edit_workspaces(|state| state.delete_workspace(workspace), cx) {
                    Ok(Value::Null)
                } else {
                    Err("could not save workspace deletion".into())
                }
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    /// Like main, a project created for `workspace` joins only that workspace
    /// and the sidebar follows it; `active` is the filter from before opening.
    pub(super) fn file_created_project(
        &mut self,
        project: ProjectId,
        workspace: WorkspaceId,
        active: Option<&Workspace>,
        cx: &mut Context<Self>,
    ) -> bool {
        let opened_into = active
            .filter(|active| active.id != workspace && !active.projects.contains(&project))
            .map(|active| active.id);
        !self
            .state
            .project(project)
            .is_some_and(|project| project.home)
            && self.edit_workspaces(
                |state| {
                    state.set_workspace_member(workspace, project, true)?;
                    if let Some(active) = opened_into {
                        state.set_workspace_member(active, project, false)?;
                    }
                    state.reveal_current_project();
                    Ok(())
                },
                cx,
            )
    }

    fn workspace_named(&self, identifier: &str) -> Result<(WorkspaceId, String), String> {
        self.find_workspace(identifier)
            .map(|workspace| (workspace.id, workspace.name.clone()))
            .ok_or_else(|| format!("workspace not found '{identifier}'"))
    }

    fn apply_workspace_edit(
        &mut self,
        edit: impl FnOnce(&mut muxy_app_core::AppState) -> Result<(), muxy_app_core::AppError>,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if self.edit_workspaces(edit, cx) {
            Ok(Value::Null)
        } else {
            Err("could not save workspace changes".into())
        }
    }

    /// `projects.add` and `projects.create`: reuse a project already opened at
    /// the path, otherwise add and activate a new one.
    pub(super) fn add_extension_project(
        &mut self,
        path: &str,
        cx: &mut Context<Self>,
    ) -> Result<ProjectId, String> {
        let directory = standardized(path);
        if let Some(existing) = self
            .state
            .projects()
            .iter()
            .find(|project| project.parent_id.is_none() && project.directory == directory)
            .map(|project| project.id)
        {
            self.join_active_workspace(existing, cx);
            self.select_project(existing, cx);
            return Ok(existing);
        }
        if !directory.is_dir() {
            return Err(format!("could not open project at path '{path}'"));
        }
        let mut added = None;
        if self.edit_project(
            |state| {
                added = Some(state.add_project(directory)?);
                Ok(())
            },
            cx,
        ) {
            added.ok_or_else(|| "could not open project".into())
        } else {
            Err(format!("could not open project at path '{path}'"))
        }
    }

    fn reorder_projects(&mut self, args: &Value, cx: &mut Context<Self>) -> Result<Value, String> {
        let mut order = Vec::new();
        for identifier in args["identifiers"].as_array().into_iter().flatten() {
            order.push(self.mutable_project(identifier.as_str().unwrap_or(""))?);
        }
        let expected: std::collections::BTreeSet<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| project.parent_id.is_none() && !project.home)
            .map(|project| project.id)
            .collect();
        if order.len() != expected.len()
            || order
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                != expected
        {
            return Err("identifiers must list every project exactly once".into());
        }
        self.apply_project_edit(
            |state| {
                for id in order.iter().rev() {
                    state.move_project(*id, 1)?;
                }
                Ok(())
            },
            cx,
        )
    }

    pub(super) fn tabs_call(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let project = self.state.current_project();
        match call.verb.as_str() {
            "tabs.list" => Ok(Value::Array(
                project
                    .tabs
                    .iter()
                    .enumerate()
                    .map(|(index, tab)| {
                        let webview = tab
                            .displayed_pane(self.state.window().active_pane)
                            .is_some_and(|pane| matches!(pane.content, PaneContent::Webview(_)));
                        json!({
                            "index": index,
                            "id": tab.id.to_string(),
                            "kind": if webview { "extensionWebView" } else { "terminal" },
                            "title": self.webview_title(tab, cx),
                            "isActive": Some(tab.id) == self.active_tab(),
                        })
                    })
                    .collect(),
            )),
            "tabs.switch" => {
                let identifier = call.args["identifier"].as_str().unwrap_or("");
                let owns = |tab: &&muxy_app_core::Tab| {
                    tab.id.to_string() == identifier
                        || tab
                            .panes
                            .iter()
                            .any(|pane| pane.id.to_string() == identifier)
                };
                let tab = if let Ok(index) = identifier.parse::<i64>() {
                    usize::try_from(index)
                        .ok()
                        .and_then(|index| project.tabs.get(index))
                } else {
                    project
                        .tabs
                        .iter()
                        .find(|tab| {
                            owns(tab)
                                || self.webview_title(tab, cx).to_lowercase()
                                    == identifier.to_lowercase()
                        })
                        .or_else(|| {
                            self.state
                                .projects()
                                .iter()
                                .flat_map(|p| &p.tabs)
                                .find(owns)
                        })
                }
                .map(|tab| tab.id)
                .ok_or_else(|| format!("tab not found {identifier}"))?;
                self.select_tab(tab, cx);
                Ok(Value::Null)
            }
            "tabs.new" => {
                self.new_tab(cx);
                Ok(json!(self.active_tab().map(|tab| tab.to_string())))
            }
            "tabs.next" | "tabs.previous" => {
                self.cycle_tab(call.verb == "tabs.next", cx);
                Ok(Value::Null)
            }
            "tabs.open" => self.extension_open_tab(&call.args, cx),
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    /// Validation main performs before asking for consent to open a tab.
    pub(super) fn check_tab_request(&self, args: &Value) -> Result<(), String> {
        match args["kind"].as_str() {
            Some("terminal") => self.tab_directory(args).map(|_| ()),
            Some("extensionWebView") => {
                let target = &args["extension"];
                let id = target["id"]
                    .as_str()
                    .ok_or("invalid open tab request: missing extension id")?;
                let tab_type = target["tabType"]
                    .as_str()
                    .ok_or("invalid open tab request: missing tabType")?;
                let extension = self
                    .extensions
                    .registry
                    .enabled(id)
                    .ok_or_else(|| format!("extension '{id}' is not loaded"))?;
                extension
                    .manifest
                    .tab_type(tab_type)
                    .map(|_| ())
                    .ok_or_else(|| format!("extension '{id}' has no tab type '{tab_type}'"))
            }
            Some("browser") => Err("browser tabs cannot be opened via this API yet".into()),
            _ => Err("invalid open tab request: unknown kind".into()),
        }
    }

    fn tab_directory(&self, args: &Value) -> Result<Option<PathBuf>, String> {
        let Some(relative) = args["directory"].as_str() else {
            return Ok(None);
        };
        let root = self
            .state
            .current_project()
            .directory
            .canonicalize()
            .map_err(|error| error.to_string())?;
        root.join(relative.trim_start_matches('/'))
            .canonicalize()
            .ok()
            .filter(|path| path.starts_with(&root) && path.is_dir())
            .map(Some)
            .ok_or_else(|| "directory must be an existing folder inside the worktree".into())
    }

    pub(super) fn extension_open_tab(
        &mut self,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        self.check_tab_request(args)?;
        if args["kind"] == "terminal" {
            let directory = self.tab_directory(args)?;
            let tab = self
                .state
                .open_terminal_tab(self.state.current_project().id)
                .map_err(|error| error.to_string())?;
            let pane = self
                .state
                .projects()
                .iter()
                .flat_map(|project| &project.tabs)
                .find(|candidate| candidate.id == tab)
                .and_then(|tab| tab.panes.first())
                .map(|pane| pane.id)
                .ok_or("could not create tab")?;
            if let Some(directory) = directory {
                self.initial_directories.insert(pane, directory);
            }
            if let Some(command) = args["command"]
                .as_str()
                .map(str::trim)
                .filter(|c| !c.is_empty())
            {
                self.extensions.startup.insert(pane, command.to_owned());
            }
            self.changed(cx);
            self.focus_requested = true;
            if self.connection == super::super::ConnectionState::Disconnected {
                self.connect(cx);
            }
            return Ok(json!(tab.to_string()));
        }
        let target = &args["extension"];
        let (id, tab_type) = (
            target["id"].as_str().unwrap_or(""),
            target["tabType"].as_str().unwrap_or(""),
        );
        let default = self
            .extensions
            .registry
            .enabled(id)
            .and_then(|extension| extension.manifest.tab_type(tab_type))
            .map(|kind| kind.default_data.clone())
            .unwrap_or_default();
        let pane = self.open_webview_tab(
            WebviewDescriptor {
                owner: id.into(),
                kind: tab_type.into(),
                data: if target["data"].is_null() {
                    default
                } else {
                    target["data"].clone()
                },
            },
            target["singleton"].as_bool().unwrap_or(false),
            cx,
        )?;
        Ok(json!(self.pane_tab(pane).map(|tab| tab.to_string())))
    }

    fn terminal_pane(&self, args: &Value) -> Result<(PaneId, TabId), String> {
        let identifier = args["paneID"].as_str().unwrap_or("");
        let pane: PaneId = identifier
            .parse()
            .map_err(|_| "invalid pane ID".to_owned())?;
        let tab = self
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .find(|tab| {
                tab.panes.iter().any(|candidate| {
                    candidate.id == pane
                        && matches!(candidate.content, PaneContent::Terminal { .. })
                })
            })
            .map(|tab| tab.id)
            .ok_or_else(|| format!("pane not found {identifier}"))?;
        Ok((pane, tab))
    }

    /// Consent for pane verbs comes after main's pane ID check.
    pub(super) fn check_pane_request(args: &Value) -> Result<(), String> {
        args["paneID"]
            .as_str()
            .and_then(|pane| pane.parse::<PaneId>().ok())
            .map(|_| ())
            .ok_or_else(|| "invalid pane ID".into())
    }

    pub(super) fn panes_call(
        &mut self,
        call: &Call,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        if call.verb == "panes.list" {
            let active = self.active_pane();
            return Ok(Value::Array(
                self.state
                    .projects()
                    .iter()
                    .flat_map(|project| project.tabs.iter().map(move |tab| (project, tab)))
                    .flat_map(|(project, tab)| {
                        tab.panes.iter().map(move |pane| (project, tab, pane))
                    })
                    .filter(|(_, _, pane)| matches!(pane.content, PaneContent::Terminal { .. }))
                    .map(|(project, tab, pane)| {
                        let directory = self
                            .terminal(&pane.id)
                            .and_then(|view| view.view.read(cx).directory())
                            .unwrap_or_else(|| project.directory.clone());
                        json!({
                            "id": pane.id.to_string(),
                            "title": tab.custom_title.clone().unwrap_or_else(|| pane.title.clone()),
                            "workingDirectory": directory.display().to_string(),
                            "isFocused": Some(pane.id) == active,
                        })
                    })
                    .collect(),
            ));
        }
        let (pane, tab) = self.terminal_pane(&call.args)?;
        let not_ready = || format!("pane surface not ready {pane} (waited 0.0s)");
        match call.verb.as_str() {
            "panes.send" | "panes.sendKeys" => {
                let bytes = if call.verb == "panes.send" {
                    call.args["text"]
                        .as_str()
                        .ok_or("missing argument 'text'")?
                        .as_bytes()
                        .to_vec()
                } else {
                    key_bytes(call.args["key"].as_str().unwrap_or(""))?.to_vec()
                };
                let channel = self
                    .terminal(&pane)
                    .and_then(|view| view.view.read(cx).channel())
                    .ok_or_else(not_ready)?;
                self.send(crate::boot::Work::Input(channel, bytes), cx);
                Ok(Value::Null)
            }
            "panes.readScreen" => {
                let lines = usize::try_from(call.args["lines"].as_u64().unwrap_or(50))
                    .unwrap_or(500)
                    .clamp(1, 500);
                let text = self
                    .terminal(&pane)
                    .and_then(|view| {
                        view.view
                            .read(cx)
                            .grid
                            .as_ref()
                            .map(|grid| screen_text(grid, lines))
                    })
                    .or_else(|| {
                        self.snapshots
                            .get(&pane)
                            .map(|grid| screen_text(grid, lines))
                    })
                    .ok_or_else(not_ready)?;
                Ok(json!(text))
            }
            "panes.close" => {
                self.close_pane(pane, cx);
                Ok(Value::Null)
            }
            "panes.rename" => {
                let title = call.args["title"]
                    .as_str()
                    .ok_or("missing argument 'title'")?;
                let title = Some(title.trim().to_owned()).filter(|title| !title.is_empty());
                let previous = self.state.clone();
                if self.state.set_tab_title(tab, title).is_err() || !self.save(cx) {
                    self.state = previous;
                    return Err("could not rename pane".into());
                }
                cx.notify();
                Ok(Value::Null)
            }
            _ => Err(format!("unsupported extension API: {}", call.verb)),
        }
    }

    pub(super) fn worktree_list(&self, project: ProjectId) -> Value {
        let current = self.state.current_project().id;
        let root = self
            .state
            .project(project)
            .map_or(project, super::events::root_of);
        Value::Array(
            self.state
                .projects()
                .iter()
                .filter(|p| p.id == root || p.parent_id == Some(root))
                .map(|p| {
                    json!({
                        "id": p.id.to_string(),
                        "name": p.name,
                        "path": p.directory.display().to_string(),
                        "branch": self.git.projects.get(&p.id).and_then(|r| r.summary.as_ref()).and_then(|s| s.branch.as_ref()),
                        "isActive": p.id == current,
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_keys_follow_main() {
        assert_eq!(standardized("/a/./b/../c"), PathBuf::from("/a/c"));
        assert_eq!(key_bytes("Ctrl+C").expect("key"), b"\x03");
        assert_eq!(key_bytes("Return").expect("key"), b"\r");
        assert_eq!(key_bytes("f1").unwrap_err(), "unsupported key f1");
        assert_eq!(base64(b"Muxy!"), "TXV4eSE=");
        assert_eq!(base64(b"ab"), "YWI=");
    }
}
