use gpui::{Context, FocusHandle, Window};
use muxy_app_core::{PaneId, ProjectStatus, ServerId};
use muxy_ui::panel::PanelResizeState;
use muxy_ui::tr;

use super::{AppModel, Quitting};
use crate::views::menu::{Command, Item};
use crate::views::terminal::pane::PaneState;

pub(crate) struct PinnedTerminal {
    pub(crate) resize: PanelResizeState,
    pub(crate) controls: [FocusHandle; 3],
    pub(crate) focused: bool,
}

impl PinnedTerminal {
    pub(crate) fn new(cx: &mut Context<AppModel>) -> Self {
        Self {
            resize: PanelResizeState::default(),
            controls: std::array::from_fn(|_| cx.focus_handle()),
            focused: false,
        }
    }
}

impl AppModel {
    pub(crate) fn is_pinned_terminal(&self, pane: PaneId) -> bool {
        self.state
            .pinned_terminal()
            .is_some_and(|pinned| pinned.id == pane)
    }

    pub(crate) fn pinned_terminal_shown(&self) -> bool {
        self.state.pinned_terminal().is_some() && !self.appearance.pinned_terminal_hidden
    }

    pub(crate) fn can_pin_terminal(&self, pane: PaneId) -> bool {
        self.can_detach_terminal(pane) && self.state.pane_server(pane) == Some(ServerId::local())
    }

    pub(crate) fn pin_terminal(&mut self, pane: PaneId, cx: &mut Context<Self>) {
        if !self.can_pin_terminal(pane) {
            return;
        }
        let previous = self.state.clone();
        if let Err(error) = self.state.pin_pane(pane) {
            self.state = previous;
            self.fail(error.to_string(), cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        if self.appearance.pinned_terminal_hidden {
            self.appearance.pinned_terminal_hidden = false;
            self.save_appearance(cx);
        }
        self.pinned.focused = false;
        self.split_resize.end();
        self.dismiss_overlay(cx);
        self.focus_requested = true;
        self.changed(cx);
    }

    pub(crate) fn unpin_terminal(&mut self, cx: &mut Context<Self>) {
        if self.quitting != Quitting::Idle || self.state.pinned_terminal().is_none() {
            return;
        }
        let current = self.state.current_project();
        let project =
            if current.server_id.is_local() && current.status() == ProjectStatus::Available {
                current.id
            } else {
                self.state.home().id
            };
        let previous = self.state.clone();
        if let Err(error) = self.state.unpin_pane(project) {
            self.state = previous;
            self.fail(error.to_string(), cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        self.pinned.focused = false;
        self.pinned.resize.end();
        self.dismiss_overlay(cx);
        self.focus_requested = true;
        self.changed(cx);
    }

    pub(crate) fn close_pinned_terminal(&mut self, cx: &mut Context<Self>) {
        if self.state.pinned_terminal().is_none() {
            return;
        }
        self.state.close_pinned_terminal();
        self.pinned.focused = false;
        self.pinned.resize.end();
        if self.save(cx) {
            self.discard_pending(ServerId::local(), cx);
        }
        self.focus_requested = true;
        self.sync_visible(cx);
        cx.notify();
    }

    pub(super) fn restore_pinned_terminal(
        &mut self,
        sessions: &[muxy_protocol::SessionInfo],
        cx: &mut Context<Self>,
    ) {
        let stale = self.state.pinned_terminal().is_some_and(|pane| {
            self.pane_session(pane.id)
                .is_some_and(|id| !sessions.iter().any(|session| session.id == id))
        });
        if stale {
            self.close_pinned_terminal(cx);
        }
    }

    pub(crate) fn toggle_pinned_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() || self.state.pinned_terminal().is_none() {
            return;
        }
        self.appearance.pinned_terminal_hidden = !self.appearance.pinned_terminal_hidden;
        self.save_appearance(cx);
        self.sync_visible(cx);
        if self.pinned_terminal_shown() {
            self.focus_pinned_terminal(window, cx);
        } else {
            self.pinned.focused = false;
            self.pinned.resize.end();
            self.focus_active(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn focus_pinned_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() || self.state.pinned_terminal().is_none() {
            return;
        }
        if self.appearance.pinned_terminal_hidden {
            self.appearance.pinned_terminal_hidden = false;
            self.save_appearance(cx);
            self.sync_visible(cx);
        }
        let Some(pane) = self
            .state
            .pinned_terminal()
            .and_then(|pinned| self.terminal(&pinned.id))
        else {
            return;
        };
        self.blur_webviews(cx);
        pane.focus(window, cx);
        self.pinned.focused = true;
        self.focus_requested = false;
        cx.notify();
    }

    pub(super) fn pinned_terminal_menu(
        &self,
        id: PaneId,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.terminal(&id) else {
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
        let items = vec![
            copy,
            paste,
            all,
            Item::action(tr!("Move to Tab"), Command::UnpinTerminal).separated(),
            Item::action(tr!("Hide Pinned Terminal"), Command::HidePinnedTerminal),
            Item::action(tr!("Close Terminal"), Command::ClosePinnedTerminal),
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
