//! Right-click menus: what they offer for a terminal or a tab, and doing it.

use muxy_app_core::{Direction, PaneId};
use muxy_protocol::{MouseAction, MouseButton as TerminalButton};
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::mouse;
use crate::state::Result;
use crate::ui::menu::{Command, Item, Menu};
use crate::ui::{Mode, Overlay, Ui};
use crate::worker::{Action, Worker, lock};

/// Opens the menu for the terminal in `pane`.
pub(super) fn pane(worker: &Worker, ui: &mut Ui, pane: PaneId, at: Position) {
    let shared = lock(&worker.shared);
    let Some(tab) = shared.state.as_ref().and_then(crate::state::State::tab) else {
        return;
    };
    let live = shared
        .views
        .get(&pane)
        .filter(|view| !view.ended && view.channel.is_some());
    let mut items = vec![
        Some(Item::new("Split right", "^B %", Command::SplitRight(pane))),
        Some(Item::new("Split down", "^B \"", Command::SplitDown(pane))),
    ];
    if tab.panes.len() > 1 {
        let label = if tab.zoom { "Unzoom" } else { "Zoom" };
        items.push(Some(Item::new(label, "^B z", Command::Zoom(pane))));
    }
    items.push(None);
    if ui
        .selection
        .is_some_and(|selection| selection.pane == pane && !selection.is_empty())
    {
        items.push(Some(Item::new("Copy", "", Command::Copy)));
    }
    if live.is_some() {
        items.push(Some(Item::new(
            "Scroll back",
            "^B [",
            Command::ScrollBack(pane),
        )));
    }
    if live.is_some_and(|view| view.input.mouse_tracking) {
        items.push(Some(Item::new(
            "Right-click the app",
            "",
            Command::RightClick(pane, at),
        )));
    }
    items.push(Some(Item::new("New tab", "^B c", Command::NewTab)));
    items.push(None);
    items.push(Some(
        Item::new("Close pane", "^B x", Command::ClosePane(pane)).danger(),
    ));
    ui.overlay = Overlay::Menu(Menu::new(at, items));
}

/// Opens the menu for the current project's tab holding `pane`.
pub(super) fn tab(worker: &Worker, ui: &mut Ui, pane: PaneId, at: Position) {
    let shared = lock(&worker.shared);
    let Some(project) = shared
        .state
        .as_ref()
        .and_then(|state| state.projects.get(&state.active))
    else {
        return;
    };
    let Some(index) = project
        .tabs
        .iter()
        .position(|tab| tab.panes.contains_key(&pane))
    else {
        return;
    };
    let mut items = vec![
        Some(Item::new("New tab", "^B c", Command::NewTab)),
        None,
        Some(Item::new("Split right", "^B %", Command::SplitRight(pane))),
        Some(Item::new("Split down", "^B \"", Command::SplitDown(pane))),
        Some(Item::new("Rename…", "", Command::RenameTab(pane))),
    ];
    if index > 0 {
        items.push(Some(Item::new(
            "Move left",
            "",
            Command::MoveTab(pane, false),
        )));
    }
    if index + 1 < project.tabs.len() {
        items.push(Some(Item::new(
            "Move right",
            "",
            Command::MoveTab(pane, true),
        )));
    }
    items.push(None);
    items.push(Some(
        Item::new("Close tab", "", Command::CloseTab(pane)).danger(),
    ));
    ui.overlay = Overlay::Menu(Menu::new(at, items));
}

pub(super) fn run(command: Command, worker: &Worker, ui: &mut Ui) -> Result {
    ui.overlay = Overlay::None;
    match command {
        Command::SplitRight(pane) => {
            mouse::focus(worker, pane)?;
            worker.send(Action::New(Some(Direction::Right)))
        }
        Command::SplitDown(pane) => {
            mouse::focus(worker, pane)?;
            worker.send(Action::New(Some(Direction::Down)))
        }
        Command::Zoom(pane) => {
            mouse::focus(worker, pane)?;
            worker.send(Action::Zoom)
        }
        Command::Copy => match ui.selection {
            Some(selection) => mouse::copy(worker, selection),
            None => Ok(()),
        },
        Command::ScrollBack(pane) => {
            mouse::focus(worker, pane)?;
            ui.mode = Mode::Scroll;
            Ok(())
        }
        Command::RightClick(pane, at) => {
            for (kind, action) in [
                (MouseEventKind::Down(MouseButton::Right), MouseAction::Press),
                (MouseEventKind::Up(MouseButton::Right), MouseAction::Release),
            ] {
                let event = MouseEvent {
                    kind,
                    column: at.x,
                    row: at.y,
                    modifiers: KeyModifiers::NONE,
                };
                mouse::forward(
                    worker,
                    ui,
                    pane,
                    action,
                    Some(TerminalButton::Right),
                    None,
                    event,
                )?;
            }
            Ok(())
        }
        Command::ClosePane(pane) => {
            mouse::focus(worker, pane)?;
            worker.send(Action::CheckClose)
        }
        Command::NewTab => worker.send(Action::New(None)),
        Command::RenameTab(pane) => {
            let text = lock(&worker.shared)
                .state
                .as_ref()
                .and_then(|state| state.projects.get(&state.active))
                .and_then(|project| {
                    project
                        .tabs
                        .iter()
                        .find(|tab| tab.panes.contains_key(&pane))
                })
                .and_then(|tab| tab.title.clone())
                .unwrap_or_default();
            ui.overlay = Overlay::Rename { pane, text };
            Ok(())
        }
        Command::MoveTab(pane, forward) => worker.send(Action::MoveTab { pane, forward }),
        Command::CloseTab(pane) => {
            mouse::focus(worker, pane)?;
            worker.send(Action::CheckCloseTab)
        }
    }
}
