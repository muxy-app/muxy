use gpui::Context;
use muxy_app_core::{
    Direction, PaneId, ProjectStatus,
    opener::{OpenContext, OpenRequest, Target},
};

use super::AppModel;
use crate::views::{
    menu::{Command, Item},
    terminal::pane::PaneState,
};

impl AppModel {
    pub(super) fn opener_context(&self, pane: PaneId) -> Option<OpenContext> {
        let project = self.state.projects().iter().find(|project| {
            (self.is_quick_terminal(pane) && project.id == self.state.home().id)
                || project
                    .tabs
                    .iter()
                    .any(|tab| tab.panes.iter().any(|p| p.id == pane))
        })?;
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
        if let Target::File(file) = &target
            && self.settings.openers.file == "system.editor"
            && self.settings.openers.project_target.is_none()
            && let Ok(relative) = file.path.strip_prefix(&context.project_directory)
            && let Some(relative) = relative.to_str()
        {
            let opener = self.extensions.registry.active().find_map(|extension| {
                extension
                    .manifest
                    .file_openers
                    .iter()
                    .find(|opener| opener.matches(relative))
                    .map(|opener| (extension.name.clone(), opener.clone()))
            });
            if let Some((owner, opener)) = opener {
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
                    owner,
                    kind: opener.tab_type,
                    data,
                };
                match self.open_webview_tab(descriptor, opener.singleton, cx) {
                    Ok(_) => return,
                    Err(error) => {
                        self.fail(format!("Could not open file: {error}"), cx);
                        return;
                    }
                }
            }
        }
        let settings = self.settings.openers.clone();
        let result = crate::opener::submit(move || {
            crate::opener::open(&OpenRequest { target, context }, &settings)
        });
        match result {
            Ok(result) => cx
                .spawn(async move |model, cx| {
                    if let Ok(Err(error)) = result.recv().await {
                        let _ = model.update(cx, |model, cx| {
                            model.fail(format!("Could not open link: {error}"), cx);
                        });
                    }
                })
                .detach(),
            Err(error) => self.fail(format!("Could not open link: {error}"), cx),
        }
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
        let project = self.state.current_project();
        let Some(tab) = project
            .tabs
            .iter()
            .find(|tab| tab.panes.iter().any(|pane| pane.id == id))
        else {
            return;
        };
        let pane = pane.view.read(cx);
        let mut copy = Item::action("Copy", Command::TerminalCopy(id));
        if pane.selection.is_none() {
            copy = copy.disabled();
        }
        let mut paste = Item::action("Paste", Command::TerminalPaste(id));
        if pane.state != PaneState::Live {
            paste = paste.disabled();
        }
        let mut all = Item::action("Select All", Command::TerminalSelectAll(id));
        if pane.displayed_grid().is_none() {
            all = all.disabled();
        }
        let mut output = Item::action(
            "Select Command Output",
            Command::TerminalSelectCommandOutput(id),
        );
        if pane.state != PaneState::Live {
            output = output.disabled();
        }
        let mut detach = Item::action("Detach Terminal", Command::DetachTerminal(id));
        if !self.can_detach_terminal(id) {
            detach = detach.disabled();
        }
        let mut split_right =
            Item::action("Split Right", Command::SplitPane(id, Direction::Right)).separated();
        let mut split_down = Item::action("Split Down", Command::SplitPane(id, Direction::Down));
        let mut zoom = Item::action(
            if tab.zoomed == Some(id) {
                "Restore Pane"
            } else {
                "Maximize Pane"
            },
            Command::ToggleZoomPane(id),
        );
        let mut close = Item::action("Close Pane", Command::ClosePane(id));
        if project.status() == ProjectStatus::Missing {
            split_right = split_right.disabled();
            split_down = split_down.disabled();
            close = close.disabled();
        }
        if tab.panes.len() < 2 && tab.zoomed.is_none() {
            zoom = zoom.disabled();
        }
        if tab.pinned && tab.panes.len() == 1 {
            close = close.disabled();
        }
        let items = vec![
            copy,
            paste,
            all,
            output,
            split_right,
            split_down,
            zoom,
            detach.separated(),
            close,
        ];
        let model = cx.entity().downgrade();
        let window = self.window;
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
