use gpui::Context;
use muxy_app_core::{
    PaneId,
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
                    .find(|opener| {
                        opener["patterns"].as_array().is_some_and(|patterns| {
                            patterns
                                .iter()
                                .filter_map(serde_json::Value::as_str)
                                .any(|pattern| {
                                    pattern == "*"
                                        || pattern == relative
                                        || pattern
                                            .strip_prefix('*')
                                            .is_some_and(|suffix| relative.ends_with(suffix))
                                })
                        })
                    })
                    .map(|opener| (extension.name.clone(), opener.clone()))
            });
            if let Some((owner, opener)) = opener {
                let descriptor = muxy_app_core::webview::WebviewDescriptor {
                    owner,
                    kind: opener["tabType"].as_str().unwrap_or("").into(),
                    data: serde_json::json!({"filePath":relative,"line":file.line,"column":file.column,"replaceable":false}),
                };
                match self.open_webview_tab(
                    descriptor,
                    opener["singleton"].as_bool().unwrap_or(false),
                    cx,
                ) {
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
        let items = vec![copy, paste, all, output, detach];
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
