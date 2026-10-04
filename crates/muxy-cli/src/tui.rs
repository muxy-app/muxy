use std::io;
use std::sync::atomic::Ordering;

use muxy_app_core::{Direction, PaneId};
use muxy_client::{SshTarget, Start};
use muxy_protocol::CursorShape;
use ratatui::crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
};

use crate::state::Result;
use crate::target::Target;
use crate::{
    input, render,
    terminal::{self, Host},
    worker::{Action, Worker, lock},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Overlay {
    #[default]
    None,
    Projects(usize),
    Sessions(usize),
    Help,
    Confirm(PaneId),
}

/// Runs the TUI against this computer's server, or the one on `remote`.
pub(crate) fn run(remote: Option<SshTarget>) -> Result {
    terminal::require_interactive().map_err(|error| error.to_string())?;
    let target = Target::new(remote).map_err(|error| error.to_string())?;
    let executable = muxy_core::executable::current_path().map_err(|error| error.to_string())?;
    let _lease = muxy_client::local::bundle::acquire_runtime(&executable)
        .map_err(|error| error.to_string())?;
    // Another computer is reached before the terminal is taken over, so an
    // unreachable host or a refused login reports to the shell.
    let first = match target {
        Target::Local { .. } => None,
        Target::Ssh { .. } => Some(
            target
                .connect(Start::IfNeeded)
                .map_err(|error| target.explain(error).to_string())?,
        ),
    };
    let mut host = Host::enter().map_err(|error| error.to_string())?;
    let viewport = host.terminal.size().map_err(|error| error.to_string())?;
    let worker = Worker::start(
        target,
        first,
        ratatui::layout::Rect::new(0, 0, viewport.width, viewport.height),
    )?;
    let result = events(&mut host, &worker);
    drop(host);
    drop(worker);
    result
}

fn events(host: &mut Host, worker: &Worker) -> Result {
    let events = terminal::Events::start().map_err(|error| error.to_string())?;
    let mut prefix = false;
    let mut overlay = Overlay::None;
    let mut shape = None;
    loop {
        if host.stop.load(Ordering::Acquire) || terminal::require_interactive().is_err() {
            return Ok(());
        }
        host.resume_if_needed().map_err(|error| error.to_string())?;
        let size = host.terminal.size().map_err(|error| error.to_string())?;
        *lock(&worker.viewport) = ratatui::layout::Rect::new(0, 0, size.width, size.height);
        {
            let mut shared = lock(&worker.shared);
            if let Some(id) = shared.confirm.take() {
                overlay = Overlay::Confirm(id);
            }
            if let Some(result) = &shared.exit {
                return result.clone();
            }
        }
        if let Err(error) = report_focus(worker) {
            lock(&worker.shared).message = error;
        }
        let next_shape = {
            let shared = lock(&worker.shared);
            shared
                .state
                .as_ref()
                .and_then(|state| state.tab())
                .and_then(|tab| shared.views.get(&tab.focus))
                .map(|view| view.grid.cursor.shape)
        };
        if shape != next_shape {
            shape = next_shape;
            let style = match shape {
                Some(CursorShape::Bar) => cursor::SetCursorStyle::SteadyBar,
                Some(CursorShape::Underline) => cursor::SetCursorStyle::SteadyUnderScore,
                _ => cursor::SetCursorStyle::SteadyBlock,
            };
            execute!(io::stdout(), style).map_err(|error| error.to_string())?;
        }
        host.terminal
            .draw(|frame| render::draw(frame, &lock(&worker.shared), prefix, &overlay))
            .map_err(|error| error.to_string())?;
        let Some(event) = events.next().map_err(|error| error.to_string())? else {
            continue;
        };
        let result = match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                key_event(key, worker, &mut prefix, &mut overlay)
            }
            Event::Paste(text) => {
                prefix = false;
                if overlay == Overlay::None {
                    worker.typing(input::Input::Paste(text))
                } else {
                    Ok(())
                }
            }
            Event::FocusGained => host_focus(worker, true),
            Event::FocusLost => host_focus(worker, false),
            Event::Resize(_, _) | Event::Mouse(_) | Event::Key(_) => Ok(()),
        };
        if let Err(error) = result {
            lock(&worker.shared).message = error;
        }
    }
}

fn key_event(key: KeyEvent, worker: &Worker, prefix: &mut bool, overlay: &mut Overlay) -> Result {
    if *overlay != Overlay::None {
        return picker_key(key, worker, overlay);
    }
    let control_b = key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('b' | 'B'));
    if *prefix {
        *prefix = false;
        if control_b {
            return worker.typing(input::Input::Bytes(vec![0x02]));
        }
        let action = match key.code {
            KeyCode::Char('c') => Action::New(None),
            KeyCode::Char('n') => Action::CycleTab(true),
            KeyCode::Char('p') => Action::CycleTab(false),
            KeyCode::Char(number @ '0'..='9') => Action::SelectTab(number as usize - '0' as usize),
            KeyCode::Char('%') => Action::New(Some(Direction::Right)),
            KeyCode::Char('"') => Action::New(Some(Direction::Down)),
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                let direction = match key.code {
                    KeyCode::Left => Direction::Left,
                    KeyCode::Right => Direction::Right,
                    KeyCode::Up => Direction::Up,
                    _ => Direction::Down,
                };
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    Action::Resize(direction)
                } else {
                    Action::Focus(direction)
                }
            }
            KeyCode::Char('z') => Action::Zoom,
            KeyCode::Char('d') => Action::Detach,
            KeyCode::Char('s') => {
                *overlay = Overlay::Projects(0);
                return Ok(());
            }
            KeyCode::Char('w') => {
                *overlay = Overlay::Sessions(0);
                return worker.send(Action::ListSessions);
            }
            KeyCode::Char('?') => {
                *overlay = Overlay::Help;
                return Ok(());
            }
            KeyCode::Char('x') => Action::CheckClose,
            _ => return Ok(()),
        };
        worker.send(action)
    } else if control_b {
        *prefix = true;
        Ok(())
    } else {
        worker.typing(input::Input::Key(key))
    }
}

fn picker_key(key: KeyEvent, worker: &Worker, overlay: &mut Overlay) -> Result {
    if key.code == KeyCode::Esc {
        *overlay = Overlay::None;
        return Ok(());
    }
    match *overlay {
        Overlay::None => {}
        Overlay::Help => {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char('q')) {
                *overlay = Overlay::None;
            }
        }
        Overlay::Confirm(id) => match key.code {
            KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                *overlay = Overlay::None;
                worker.send(Action::Close(id))?;
            }
            KeyCode::Char('n' | 'N') => *overlay = Overlay::None,
            _ => {}
        },
        Overlay::Projects(mut index) | Overlay::Sessions(mut index) => {
            let shared = lock(&worker.shared);
            let projects = matches!(*overlay, Overlay::Projects(_));
            let count = if projects {
                shared
                    .catalog
                    .as_ref()
                    .map_or(0, |catalog| catalog.projects.len())
            } else {
                shared.sessions.len()
            };
            index = index.min(count.saturating_sub(1));
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => index = index.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    index = (index + 1).min(count.saturating_sub(1));
                }
                KeyCode::Enter => {
                    let action = if projects {
                        shared
                            .catalog
                            .as_ref()
                            .and_then(|catalog| catalog.projects.get(index))
                            .map(|project| Action::Open(project.id))
                    } else {
                        shared.sessions.get(index).cloned().map(Action::Existing)
                    };
                    drop(shared);
                    if let Some(action) = action {
                        worker.send(action)?;
                        *overlay = Overlay::None;
                    }
                    return Ok(());
                }
                _ => {}
            }
            *overlay = if projects {
                Overlay::Projects(index)
            } else {
                Overlay::Sessions(index)
            };
        }
    }
    Ok(())
}

fn host_focus(worker: &Worker, gained: bool) -> Result {
    lock(&worker.shared).host_unfocused = !gained;
    report_focus(worker)
}

fn report_focus(worker: &Worker) -> Result {
    let mut shared = lock(&worker.shared);
    let next = shared
        .state
        .as_ref()
        .and_then(|state| state.tab())
        .and_then(|tab| shared.views.get(&tab.focus))
        .filter(|view| view.input.focus_events && !view.ended && !shared.host_unfocused)
        .and_then(|view| view.channel);
    if shared.focus == next {
        return Ok(());
    }
    let Some(client) = shared.client.clone() else {
        shared.focus = None;
        return Ok(());
    };
    let old = shared.focus.and_then(|channel| {
        shared
            .views
            .values()
            .any(|view| view.channel == Some(channel) && view.input.focus_events && !view.ended)
            .then_some(channel)
    });
    shared.focus = next;
    drop(shared);
    if let Some(channel) = old {
        worker
            .input
            .send(client.clone(), channel, b"\x1b[O".to_vec())?;
    }
    if let Some(channel) = next {
        worker.input.send(client, channel, b"\x1b[I".to_vec())?;
    }
    Ok(())
}
