//! The terminal UI: connecting, the event loop, and focus reports.

mod keys;
mod menu;
mod mouse;

use std::io;
use std::sync::atomic::Ordering;

use muxy_app_core::PaneId;
use muxy_client::{SshTarget, Start};
use muxy_protocol::CursorShape;
use ratatui::crossterm::{
    cursor,
    event::{self, Event, KeyEventKind},
    execute,
};
use ratatui::layout::Rect;

use crate::input::Input;
use crate::scroll::HistoryRequest;
use crate::state::{Result, State};
use crate::target::Target;
use crate::ui::{self, Mode, Overlay, Ui, layout::Chrome};
use crate::{
    terminal::{self, Host},
    worker::{Shared, Worker, lock},
};

/// Runs the TUI against this computer's server, or the one on `remote`.
/// Another computer is reached before the terminal is taken over, so an
/// unreachable host or a refused login reports to the shell.
pub(crate) fn run(remote: Option<SshTarget>) -> Result {
    terminal::require_interactive().map_err(|error| error.to_string())?;
    let target = Target::new(remote).map_err(|error| error.to_string())?;
    let executable = muxy_core::executable::current_path().map_err(|error| error.to_string())?;
    let _lease = muxy_client::local::bundle::acquire_runtime(&executable)
        .map_err(|error| error.to_string())?;
    let first = match target {
        Target::Local { .. } => None,
        Target::Ssh { .. } => Some(
            target
                .connect(Start::IfNeeded)
                .map_err(|error| target.explain(error).to_string())?,
        ),
    };
    let mut host = Host::enter().map_err(|error| error.to_string())?;
    let size = host.terminal.size().map_err(|error| error.to_string())?;
    let ui = Ui::default();
    let body = Chrome::new(Rect::new(0, 0, size.width, size.height), ui.sidebar()).body;
    let worker = Worker::start(target, first, body)?;
    let result = events(&mut host, &worker, ui);
    drop(host);
    drop(worker);
    result
}

fn events(host: &mut Host, worker: &Worker, mut ui: Ui) -> Result {
    let events = terminal::Events::start().map_err(|error| error.to_string())?;
    let mut pointer = mouse::Pointer::default();
    let mut capturing = true;
    let mut shape = None;
    loop {
        if host.stop.load(Ordering::Acquire) || terminal::require_interactive().is_err() {
            return Ok(());
        }
        if host.resume_if_needed().map_err(|error| error.to_string())? {
            capturing = true;
        }
        if capturing != ui.mouse {
            capturing = ui.mouse;
            let result = if capturing {
                execute!(io::stdout(), event::EnableMouseCapture)
            } else {
                execute!(io::stdout(), event::DisableMouseCapture)
            };
            result.map_err(|error| error.to_string())?;
            pointer = mouse::Pointer::default();
            ui.hover = None;
        }
        let size = host.terminal.size().map_err(|error| error.to_string())?;
        *lock(&worker.viewport) =
            Chrome::new(Rect::new(0, 0, size.width, size.height), ui.sidebar()).body;
        {
            let mut shared = lock(&worker.shared);
            if let Some(id) = shared.confirm.take() {
                ui.overlay = Overlay::Confirm(id);
                ui.mode = Mode::Normal;
            }
            if let Some(result) = &shared.exit {
                return result.clone();
            }
            forget_closed(&shared, &mut ui);
        }
        if let Err(error) = report_focus(worker) {
            lock(&worker.shared).say(error);
        }
        let next_shape = {
            let shared = lock(&worker.shared);
            focused_pane(&shared)
                .and_then(|pane| shared.views.get(&pane))
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
        pointer.tick(worker, &mut ui);
        host.terminal
            .draw(|frame| ui::draw(frame, &lock(&worker.shared), &mut ui))
            .map_err(|error| error.to_string())?;
        let Some(event) = events.next().map_err(|error| error.to_string())? else {
            continue;
        };
        let result = match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => keys::key(key, worker, &mut ui),
            Event::Paste(text) => {
                if ui.mode == Mode::Prefix {
                    ui.mode = Mode::Normal;
                }
                if let Overlay::Rename { text: name, .. } = &mut ui.overlay {
                    name.extend(text.chars().filter(|character| !character.is_control()));
                    Ok(())
                } else if ui.overlay == Overlay::None && ui.mode == Mode::Normal {
                    to_terminal(worker, &mut ui, Input::Paste(text))
                } else {
                    Ok(())
                }
            }
            Event::Mouse(event) => pointer.handle(event, worker, &mut ui),
            Event::FocusGained => host_focus(worker, true),
            Event::FocusLost => host_focus(worker, false),
            // The terminal may have cut or moved what was drawn, even if it
            // is back to the size last drawn, so draw everything again.
            Event::Resize(_, _) => host.terminal.clear().map_err(|error| error.to_string()),
            Event::Key(_) => Ok(()),
        };
        if let Err(error) = result {
            lock(&worker.shared).say(error);
        }
    }
}

/// Drops screen state that belonged to panes that are gone.
fn forget_closed(shared: &Shared, ui: &mut Ui) {
    let open = |pane: PaneId| {
        shared
            .state
            .as_ref()
            .and_then(State::tab)
            .is_some_and(|tab| tab.panes.contains_key(&pane))
    };
    if ui.selection.is_some_and(|selection| !open(selection.pane)) {
        ui.selection = None;
    }
    let in_project = |pane: PaneId| {
        shared
            .state
            .as_ref()
            .and_then(|state| state.projects.get(&state.active))
            .is_some_and(|project| project.tabs.iter().any(|tab| tab.panes.contains_key(&pane)))
    };
    let gone = match &ui.overlay {
        Overlay::Confirm(closing) => !open(closing.pane()),
        Overlay::Rename { pane, .. } => !in_project(*pane),
        _ => false,
    };
    if gone {
        ui.overlay = Overlay::None;
    }
    if matches!(ui.mode, Mode::Scroll | Mode::Resize) && focused_pane(shared).is_none() {
        ui.mode = Mode::Normal;
    }
}

pub(super) fn focused_pane(shared: &Shared) -> Option<PaneId> {
    shared
        .state
        .as_ref()
        .and_then(State::tab)
        .map(|tab| tab.focus)
}

/// Sends typing to the focused terminal, which brings it back from
/// scrollback first.
fn to_terminal(worker: &Worker, ui: &mut Ui, input: Input) -> Result {
    ui.selection = None;
    {
        let mut shared = lock(&worker.shared);
        if let Some(pane) = focused_pane(&shared)
            && let Some(view) = shared.views.get_mut(&pane)
        {
            view.scroll.bottom();
        }
    }
    worker.typing(input)
}

/// Scrolls `pane`'s history by `rows`, positive to go back, or to the top
/// when `rows` is `None`, fetching history from the server as needed.
fn scroll(worker: &Worker, pane: PaneId, rows: Option<isize>) {
    let mut shared = lock(&worker.shared);
    let Some(view) = shared
        .views
        .get_mut(&pane)
        .filter(|view| !view.ended && view.channel.is_some())
    else {
        return;
    };
    let height = usize::from(view.viewport.rows);
    let request: Option<HistoryRequest> = match rows {
        Some(rows) => view.scroll.by(rows, &view.grid, height),
        None => view.scroll.to(usize::MAX, &view.grid, height),
    };
    if let (Some(request), Some(channel)) = (request, view.channel) {
        worker.history.fetch(&mut shared, channel, request);
    }
}

fn host_focus(worker: &Worker, gained: bool) -> Result {
    lock(&worker.shared).host_unfocused = !gained;
    report_focus(worker)
}

fn report_focus(worker: &Worker) -> Result {
    let mut shared = lock(&worker.shared);
    let next = focused_pane(&shared)
        .and_then(|pane| shared.views.get(&pane))
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
