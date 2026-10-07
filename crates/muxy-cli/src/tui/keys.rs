//! Keys: Ctrl-B commands, scroll and resize modes, and dialogs. Everything
//! else goes to the focused terminal.

use muxy_app_core::Direction;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{focused_pane, scroll, to_terminal};
use crate::input::Input;
use crate::state::Result;
use crate::ui::{self, Mode, Overlay, Ui};
use crate::worker::{Action, Closing, Worker, lock};

pub(super) fn key(key: KeyEvent, worker: &Worker, ui: &mut Ui) -> Result {
    if ui.overlay != Overlay::None {
        return overlay(key, worker, ui);
    }
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let control_b = control && matches!(key.code, KeyCode::Char('b' | 'B'));
    match ui.mode {
        Mode::Prefix => {
            ui.mode = Mode::Normal;
            if control_b {
                to_terminal(worker, ui, Input::Bytes(vec![0x02]))
            } else {
                prefix(key, worker, ui)
            }
        }
        Mode::Normal | Mode::Scroll if control_b => {
            ui.mode = Mode::Prefix;
            Ok(())
        }
        Mode::Scroll => scrolling(key, worker, ui),
        Mode::Resize => resizing(key, worker, ui),
        Mode::Normal => to_terminal(worker, ui, Input::Key(key)),
    }
}

fn direction(code: KeyCode) -> Option<Direction> {
    match code {
        KeyCode::Left | KeyCode::Char('h') => Some(Direction::Left),
        KeyCode::Right | KeyCode::Char('l') => Some(Direction::Right),
        KeyCode::Up | KeyCode::Char('k') => Some(Direction::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(Direction::Down),
        _ => None,
    }
}

fn prefix(key: KeyEvent, worker: &Worker, ui: &mut Ui) -> Result {
    let action =
        match key.code {
            KeyCode::Char('c') => Action::New(None),
            KeyCode::Char('n') => Action::CycleTab(true),
            KeyCode::Char('p') => Action::CycleTab(false),
            KeyCode::Char(number @ '1'..='9') => Action::SelectTab(number as usize - '1' as usize),
            KeyCode::Char('0') => Action::SelectTab(9),
            KeyCode::Char('%' | 'v') => Action::New(Some(Direction::Right)),
            KeyCode::Char('"' | '-') => Action::New(Some(Direction::Down)),
            KeyCode::Char('o') | KeyCode::Tab => Action::CyclePane(true),
            KeyCode::BackTab => Action::CyclePane(false),
            KeyCode::Char('z') => Action::Zoom,
            KeyCode::Char('d') => Action::Detach,
            KeyCode::Char('x') => Action::CheckClose,
            KeyCode::Char('(' | ')') => {
                let shared = lock(&worker.shared);
                let next = shared.catalog.as_ref().zip(shared.state.as_ref()).and_then(
                    |(catalog, state)| {
                        crate::projects::cycle(
                            catalog,
                            state.active,
                            key.code == KeyCode::Char(')'),
                        )
                    },
                );
                drop(shared);
                match next {
                    Some(project) => Action::Open(project),
                    None => return Ok(()),
                }
            }
            KeyCode::Char('s' | 'g') => {
                let shared = lock(&worker.shared);
                let selected = shared
                    .catalog
                    .as_ref()
                    .zip(shared.state.as_ref())
                    .and_then(|(catalog, state)| {
                        ui::project_choices(catalog, "")
                            .iter()
                            .position(|entry| entry.project.id == state.active)
                    })
                    .unwrap_or(0);
                ui.overlay = Overlay::Projects {
                    filter: String::new(),
                    selected,
                };
                return Ok(());
            }
            KeyCode::Char('w') => {
                ui.overlay = Overlay::Sessions(0);
                return worker.send(Action::ListSessions);
            }
            KeyCode::Char('?') => {
                ui.overlay = Overlay::Help(0);
                return Ok(());
            }
            KeyCode::Char('b') => {
                ui.sidebar_shown = !ui.sidebar_shown;
                return Ok(());
            }
            KeyCode::Char('m') => {
                ui.mouse = !ui.mouse;
                lock(&worker.shared).say(if ui.mouse {
                    "Mouse on"
                } else {
                    "Mouse off: the terminal selects text; Ctrl-B m turns it back on"
                });
                return Ok(());
            }
            KeyCode::Char('[') => {
                ui.mode = Mode::Scroll;
                return Ok(());
            }
            KeyCode::PageUp => {
                ui.mode = Mode::Scroll;
                let Some(pane) = focused_pane(&lock(&worker.shared)) else {
                    return Ok(());
                };
                let rows = page_rows(worker);
                scroll(worker, pane, Some(rows));
                return Ok(());
            }
            KeyCode::Char('r') => {
                ui.mode = Mode::Resize;
                return Ok(());
            }
            code => match direction(code) {
                Some(direction)
                    if key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    Action::Resize(direction)
                }
                Some(direction) => Action::Focus(direction),
                None => return Ok(()),
            },
        };
    worker.send(action)
}

/// Rows in a page of the focused pane: its height less one, so a line of
/// context stays in view.
fn page_rows(worker: &Worker) -> isize {
    let shared = lock(&worker.shared);
    focused_pane(&shared)
        .and_then(|pane| shared.views.get(&pane))
        .map_or(1, |view| {
            isize::try_from(view.viewport.rows.max(2) - 1).unwrap_or(1)
        })
}

fn scrolling(key: KeyEvent, worker: &Worker, ui: &mut Ui) -> Result {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let Some(pane) = focused_pane(&lock(&worker.shared)) else {
        ui.mode = Mode::Normal;
        return Ok(());
    };
    let height = page_rows(worker);
    let rows = match key.code {
        KeyCode::Char('f') if control => Some(-height),
        KeyCode::Char('u') if control => Some(height / 2),
        KeyCode::Char('d') if control => Some(-height / 2),
        KeyCode::Up | KeyCode::Char('k') => Some(1),
        KeyCode::Down | KeyCode::Char('j') => Some(-1),
        KeyCode::PageUp => Some(height),
        KeyCode::PageDown | KeyCode::Char(' ') => Some(-height),
        KeyCode::Home | KeyCode::Char('g') => None,
        KeyCode::End | KeyCode::Char('G' | 'q') | KeyCode::Esc => {
            leave_scroll(worker, ui, pane);
            return Ok(());
        }
        KeyCode::Char('y') | KeyCode::Enter => {
            if let Some(selection) = ui.selection.filter(|selection| selection.pane == pane) {
                super::mouse::copy(worker, selection)?;
            }
            leave_scroll(worker, ui, pane);
            return Ok(());
        }
        _ => return Ok(()),
    };
    scroll(worker, pane, rows);
    Ok(())
}

fn leave_scroll(worker: &Worker, ui: &mut Ui, pane: muxy_app_core::PaneId) {
    ui.mode = Mode::Normal;
    ui.selection = None;
    if let Some(view) = lock(&worker.shared).views.get_mut(&pane) {
        view.scroll.bottom();
    }
}

fn resizing(key: KeyEvent, worker: &Worker, ui: &mut Ui) -> Result {
    if let Some(direction) = direction(key.code) {
        return worker.send(Action::Resize(direction));
    }
    if matches!(
        key.code,
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q' | 'r')
    ) {
        ui.mode = Mode::Normal;
    }
    Ok(())
}

fn overlay(key: KeyEvent, worker: &Worker, ui: &mut Ui) -> Result {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    if key.code == KeyCode::Esc {
        ui.overlay = Overlay::None;
        return Ok(());
    }
    match &mut ui.overlay {
        Overlay::None => {}
        Overlay::Help(scroll) => match key.code {
            KeyCode::Enter | KeyCode::Char('q') => ui.overlay = Overlay::None,
            KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                *scroll = (*scroll + 1).min(ui::help_lines());
            }
            KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
            KeyCode::PageDown => *scroll = (*scroll + 10).min(ui::help_lines()),
            _ => {}
        },
        Overlay::Confirm(_) => match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => return answer(worker, ui, true),
            KeyCode::Char('n' | 'N') => ui.overlay = Overlay::None,
            _ => {}
        },
        Overlay::Rename { text, .. } => match key.code {
            KeyCode::Enter => return answer(worker, ui, true),
            KeyCode::Backspace => {
                text.pop();
            }
            KeyCode::Char('u') if control => text.clear(),
            KeyCode::Char(character) if !control => text.push(character),
            _ => {}
        },
        Overlay::Menu(menu) => match key.code {
            KeyCode::Up | KeyCode::Char('k') => menu.step(false),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => menu.step(true),
            KeyCode::Enter => {
                if let Some(command) = menu.selected.and_then(|index| menu.command(index)) {
                    return super::menu::run(command, worker, ui);
                }
            }
            _ => {}
        },
        Overlay::Projects { filter, selected } => match key.code {
            KeyCode::Up => *selected = selected.saturating_sub(1),
            KeyCode::Char('p' | 'k') if control => *selected = selected.saturating_sub(1),
            KeyCode::Down => *selected += 1,
            KeyCode::Char('n' | 'j') if control => *selected += 1,
            KeyCode::Backspace => {
                filter.pop();
                *selected = 0;
            }
            KeyCode::Char(character) if !control => {
                filter.push(character);
                *selected = 0;
            }
            KeyCode::Enter => return choose(worker, ui),
            _ => {}
        },
        Overlay::Sessions(selected) => match key.code {
            KeyCode::Up | KeyCode::Char('k') => *selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => *selected += 1,
            KeyCode::Enter => return choose(worker, ui),
            _ => {}
        },
    }
    clamp_selection(worker, ui);
    Ok(())
}

/// Closes a confirmation or the rename dialog, doing what it asked if `yes`.
pub(super) fn answer(worker: &Worker, ui: &mut Ui, yes: bool) -> Result {
    let overlay = std::mem::take(&mut ui.overlay);
    if !yes {
        return Ok(());
    }
    match overlay {
        Overlay::Confirm(Closing::Pane(pane)) => worker.send(Action::Close(pane)),
        Overlay::Confirm(Closing::Tab(pane)) => worker.send(Action::CloseTab(pane)),
        Overlay::Rename { pane, text } => worker.send(Action::RenameTab {
            pane,
            title: Some(text),
        }),
        _ => Ok(()),
    }
}

/// Keeps a picker's selection on one of its rows.
pub(super) fn clamp_selection(worker: &Worker, ui: &mut Ui) {
    let count = {
        let shared = lock(&worker.shared);
        match &ui.overlay {
            Overlay::Projects { filter, .. } => shared
                .catalog
                .as_ref()
                .map_or(0, |catalog| ui::project_choices(catalog, filter).len()),
            Overlay::Sessions(_) => shared.sessions.len(),
            _ => return,
        }
    };
    if let Overlay::Projects { selected, .. } | Overlay::Sessions(selected) = &mut ui.overlay {
        *selected = (*selected).min(count.saturating_sub(1));
    }
}

/// Opens what is selected in the project finder or the existing terminals.
pub(super) fn choose(worker: &Worker, ui: &mut Ui) -> Result {
    let action = {
        let shared = lock(&worker.shared);
        match &ui.overlay {
            Overlay::Projects { filter, selected } => shared.catalog.as_ref().and_then(|catalog| {
                let choices = ui::project_choices(catalog, filter);
                choices
                    .get((*selected).min(choices.len().saturating_sub(1)))
                    .map(|entry| Action::Open(entry.project.id))
            }),
            Overlay::Sessions(selected) => shared
                .sessions
                .get((*selected).min(shared.sessions.len().saturating_sub(1)))
                .cloned()
                .map(Action::Existing),
            _ => None,
        }
    };
    if let Some(action) = action {
        worker.send(action)?;
        ui.overlay = Overlay::None;
    }
    Ok(())
}
