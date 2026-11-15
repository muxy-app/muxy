use gpui::Context;
use muxy_app_core::{
    Direction, PaneId, Project, ProjectStatus, Tab,
    opener::{FileHandler, FileResolution, OpenContext, OpenRequest, Target, resolve_file},
};

use muxy_ui::tr;

use super::AppModel;
use crate::views::{
    menu::{Command, Item},
    terminal::pane::PaneState,
};

impl AppModel {
    pub(super) fn opener_context(&self, pane: PaneId) -> Option<OpenContext> {
        let project = self.state.pane_project(pane)?;
        Some(OpenContext {
            project: project.id,
            pane,
            server: project.server_id,
            directory: project.directory.clone(),
            project_directory: project.directory.clone(),
        })
    }

    pub(super) fn open_terminal_link(
        &mut self,
        pane: PaneId,
        target: Target,
        cx: &mut Context<Self>,
    ) {
        let Some(context) = self
            .terminal(&pane)
            .and_then(|pane| pane.view.read(cx).link_context())
        else {
            return;
        };
        if let Target::Url(url) = &target
            && self.browser_enabled()
            && self.settings.browser.open_terminal_links
        {
            self.open_url_in_browser(url.clone(), cx);
            return;
        }
        let request = OpenRequest { target, context };
        let Target::File(file) = &request.target else {
            self.open_system_link(request, cx);
            return;
        };
        let remote = (!request.context.server.is_local())
            .then(|| self.remote_files(request.context.server, cx).ok())
            .flatten();
        let (file, context) = (file.clone(), request.context.clone());
        let result = crate::opener::submit(move || {
            if context.server.is_local() {
                crate::opener::project_file(&file.path, &context.project_directory)
            } else {
                remote.and_then(|files| {
                    files.project_file(context.project, &file.path, &context.project_directory)
                })
            }
        });
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.fail(
                    tr!("Could not open link: %@", error.to_string()).to_string(),
                    cx,
                );
                return;
            }
        };
        cx.spawn(async move |model, cx| {
            if let Ok(relative) = result.recv().await {
                let _ = model.update(cx, |model, cx| {
                    model.route_terminal_file(request, relative, cx);
                });
            }
        })
        .detach();
    }

    pub(crate) fn route_terminal_file(
        &mut self,
        request: OpenRequest,
        relative: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !self.file_request_valid(&request) {
            return;
        }
        let Some(relative) = relative else {
            self.open_system_link(request, cx);
            return;
        };
        let handlers = self.extension_file_openers();
        match resolve_file(&relative, &handlers, &self.settings.openers) {
            FileResolution::System => self.open_system_link(request, cx),
            FileResolution::Handler(id) => self.open_file_with(&request, &relative, &id, cx),
            FileResolution::Choose(ids) => {
                let handlers = handlers
                    .into_iter()
                    .filter(|handler| ids.contains(&handler.id))
                    .collect();
                self.open_file_chooser(request, relative, handlers, cx);
            }
        }
    }

    pub(crate) fn file_request_valid(&self, request: &OpenRequest) -> bool {
        self.state
            .pane_project(request.context.pane)
            .is_some_and(|project| {
                project.id == request.context.project
                    && project.server_id == request.context.server
                    && project.directory == request.context.project_directory
            })
    }

    pub(crate) fn open_file_with(
        &mut self,
        request: &OpenRequest,
        relative: &str,
        id: &str,
        cx: &mut Context<Self>,
    ) {
        if !self.file_request_valid(request) {
            return;
        }
        if id == "system.finder" {
            self.open_system_link(request.clone(), cx);
            return;
        }
        let Some((owner, id)) = id.split_once(':') else {
            return;
        };
        let Some(opener) = self
            .extensions
            .registry
            .enabled(owner)
            .and_then(|extension| {
                extension
                    .manifest
                    .file_openers
                    .iter()
                    .find(|opener| opener.id == id)
            })
            .cloned()
        else {
            self.route_terminal_file(request.clone(), Some(relative.to_owned()), cx);
            return;
        };
        let Target::File(file) = &request.target else {
            return;
        };
        let mut data = serde_json::json!({
            "filePath": relative,
            "source": "terminal",
            "replaceable": false,
        });
        if let Some(line) = file.line {
            data["line"] = line.into();
        }
        if let Some(column) = file.column {
            data["column"] = column.into();
        }
        let descriptor = muxy_app_core::webview::WebviewDescriptor {
            owner: owner.to_owned(),
            kind: opener.tab_type,
            data,
        };
        if let Err(error) =
            self.open_webview_tab_in(request.context.project, descriptor, opener.singleton, cx)
        {
            self.fail(tr!("Could not open file: %@", &error).to_string(), cx);
        }
    }

    fn open_system_link(&mut self, request: OpenRequest, cx: &mut Context<Self>) {
        if let Target::File(file) = &request.target
            && !request.context.server.is_local()
        {
            self.remote_link_menu(request.context.pane, request.context.server, file, cx);
            return;
        }
        let settings = self.settings.openers.clone();
        let result = crate::opener::submit(move || crate::opener::open(&request, &settings));
        match result {
            Ok(result) => cx
                .spawn(async move |model, cx| {
                    if let Ok(Err(error)) = result.recv().await {
                        let _ = model.update(cx, |model, cx| {
                            model.fail(
                                tr!("Could not open link: %@", error.to_string()).to_string(),
                                cx,
                            );
                        });
                    }
                })
                .detach(),
            Err(error) => self.fail(
                tr!("Could not open link: %@", error.to_string()).to_string(),
                cx,
            ),
        }
    }

    pub(crate) fn extension_file_openers(&self) -> Vec<FileHandler> {
        self.extensions
            .registry
            .active()
            .flat_map(|extension| {
                extension
                    .manifest
                    .file_openers
                    .iter()
                    .map(|opener| FileHandler {
                        id: format!("{}:{}", extension.name, opener.id),
                        title: opener.title.as_ref().map_or_else(
                            || extension.name.clone(),
                            |title| format!("{} ({title})", extension.name),
                        ),
                        patterns: opener.registered_patterns().to_vec(),
                    })
            })
            .collect()
    }

    fn pane_move_items(&self, id: PaneId, project: &Project, tab: &Tab) -> Vec<Item> {
        let missing = project.status() == ProjectStatus::Missing;
        let alone = tab.panes.len() < 2;
        let mut to_tab = Item::action(
            tr!("Move to New Tab"),
            Command::MovePaneToTab(id, project.id),
        )
        .separated();
        let mut to_window = Item::action(tr!("Move to New Window"), Command::MovePaneToWindow(id));
        if missing || (alone && !tab.detached()) {
            to_tab = to_tab.disabled();
        }
        if missing || (alone && tab.detached() && !tab.floating()) {
            to_window = to_window.disabled();
        }
        let mut items = Vec::new();
        if tab.detached() {
            let label = if tab.floating() {
                tr!("Dock as Tab")
            } else {
                tr!("Move to Main Window")
            };
            items.push(
                Item::action(
                    label,
                    Command::Tab(tab.id, crate::views::tab_menu::Action::ReturnToMainWindow),
                )
                .separated(),
            );
        }
        items.extend([to_tab, to_window]);
        items.extend(self.float_items(id, project, tab));
        if !missing {
            items.extend(crate::views::tab_menu::move_to_project(
                &self.move_destinations(project.id),
                |project| Command::MovePaneToTab(id, project),
            ));
        }
        items
    }

    fn float_items(&self, id: PaneId, project: &Project, tab: &Tab) -> Vec<Item> {
        let Some(window) = tab.window.as_ref().filter(|window| window.is_floating()) else {
            let mut float = Item::action(tr!("Float Pane"), Command::FloatPane(id));
            if project.status() == ProjectStatus::Missing {
                float = float.disabled();
            }
            return vec![float];
        };
        let (label, scope) = match self.float_scope(window.id) {
            Some(muxy_app_core::FloatScope::AllProjects) => (
                tr!("Show Only in %@", self.project_title(project).as_str()),
                muxy_app_core::FloatScope::Project,
            ),
            _ => (
                tr!("Show in All Projects"),
                muxy_app_core::FloatScope::AllProjects,
            ),
        };
        vec![Item::action(label, Command::FloatScope(window.id, scope))]
    }

    pub(super) fn terminal_menu(
        &self,
        id: PaneId,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.terminal(&id) else {
            return;
        };
        let Some(project) = self.state.pane_project(id) else {
            return;
        };
        let Some(tab) = project
            .tabs
            .iter()
            .find(|tab| tab.panes.iter().any(|pane| pane.id == id))
        else {
            return;
        };
        let pane = pane.view.read(cx);
        let mut copy = Item::action(tr!("Copy"), Command::TerminalCopy(id));
        if pane.selection.is_none() {
            copy = copy.disabled();
        }
        let mut paste = Item::action(tr!("Paste"), Command::TerminalPaste(id));
        if pane.state != PaneState::Live {
            paste = paste.disabled();
        }
        let mut all = Item::action(tr!("Select All"), Command::TerminalSelectAll(id));
        if pane.displayed_grid().is_none() {
            all = all.disabled();
        }
        let mut output = Item::action(
            tr!("Select Command Output"),
            Command::TerminalSelectCommandOutput(id),
        );
        if pane.state != PaneState::Live {
            output = output.disabled();
        }
        let mut detach = Item::action(tr!("Detach Terminal"), Command::DetachTerminal(id));
        if !self.can_detach_terminal(id) {
            detach = detach.disabled();
        }
        let mut split_right =
            Item::action(tr!("Split Right"), Command::SplitPane(id, Direction::Right)).separated();
        let mut split_down =
            Item::action(tr!("Split Down"), Command::SplitPane(id, Direction::Down));
        let mut zoom = Item::action(
            if tab.zoomed == Some(id) {
                tr!("Restore Pane")
            } else {
                tr!("Maximize Pane")
            },
            Command::ToggleZoomPane(id),
        );
        let mut equalize = Item::action(tr!("Equalize Splits"), Command::EqualizeSplits(id));
        let mut close = Item::action(tr!("Close Pane"), Command::ClosePane(id));
        if project.status() == ProjectStatus::Missing {
            split_right = split_right.disabled();
            split_down = split_down.disabled();
            equalize = equalize.disabled();
            close = close.disabled();
        }
        if tab.panes.len() < 2 && tab.zoomed.is_none() {
            zoom = zoom.disabled();
        }
        if tab.panes.len() < 2 {
            equalize = equalize.disabled();
        }
        if tab.pinned && tab.panes.len() == 1 {
            close = close.disabled();
        }
        let mut items = vec![
            copy,
            paste,
            all,
            output,
            split_right,
            split_down,
            zoom,
            equalize,
        ];
        items.extend(self.pane_move_items(id, project, tab));
        items.extend([self.proxy_menu_item(id).separated(), detach, close]);
        let model = cx.entity().downgrade();
        let window = self.window_for_pane(id);
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    if model.grids.contains_key(&id) {
                        model.open_menu(items, position, window, cx);
                    }
                });
            });
        });
    }
}
