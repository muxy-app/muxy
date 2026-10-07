//! The mouse: clicking and dragging the interface, selecting and copying
//! text, scrolling history, and passing events to apps that use the mouse.

use std::time::{Duration, Instant};

use muxy_app_core::{Axis, PaneId};
use muxy_protocol::{
    Modifiers, MouseAction, MouseButton as TerminalButton, MouseEvent as TerminalMouse,
    ScrollDirection,
};
use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{focused_pane, scroll};
use crate::clipboard::{self, Copied};
use crate::selection::{Selection, Unit};
use crate::state::Result;
use crate::ui::{Mode, Overlay, Target, Ui, layout, layout::Divider, panes};
use crate::worker::{Action, View, Worker, lock};

/// Clicks this close together count as a double or triple click.
const MULTI_CLICK: Duration = Duration::from_millis(400);
/// Rows scrolled per wheel notch.
const WHEEL_ROWS: isize = 3;
/// How often a selection dragged past a pane's edge scrolls it.
const AUTOSCROLL: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug)]
enum Drag {
    Divider {
        divider: Divider,
        at: u16,
    },
    Sidebar,
    Select {
        pane: PaneId,
        column: u16,
        row: u16,
    },
    /// A press the app under it receives, with the moves and release after it.
    App {
        pane: PaneId,
        button: TerminalButton,
    },
}

#[derive(Debug, Default)]
pub(super) struct Pointer {
    drag: Option<Drag>,
    click: Option<(Instant, Position, u8)>,
    /// The last motion sent to an app, to skip repeats within one cell.
    motion: Option<(PaneId, Position)>,
    scrolled: Option<Instant>,
}

impl Pointer {
    pub(super) fn handle(&mut self, event: MouseEvent, worker: &Worker, ui: &mut Ui) -> Result {
        let position = Position::new(event.column, event.row);
        ui.hover = Some(position);
        let mut target = ui.target(event.column, event.row);
        if ui.overlay != Overlay::None {
            // A dialog that opened mid-drag still ends the drag.
            if let MouseEventKind::Up(button) = event.kind
                && self.drag.is_some()
            {
                self.release(button, event, worker, ui)?;
            }
            if !dialog(event, target, worker, ui)? {
                return Ok(());
            }
            target = ui.target_beneath_dialog(event.column, event.row);
        }
        if matches!(event.kind, MouseEventKind::Down(_)) && ui.mode == Mode::Prefix {
            ui.mode = Mode::Normal;
        }
        match event.kind {
            MouseEventKind::Down(button) => self.press(button, event, target, worker, ui),
            MouseEventKind::Drag(button) => self.drag(button, event, worker, ui),
            MouseEventKind::Up(button) => self.release(button, event, worker, ui),
            MouseEventKind::Moved => self.moved(event, target, worker, ui),
            MouseEventKind::ScrollUp => wheel(true, event, target, worker, ui),
            MouseEventKind::ScrollDown => wheel(false, event, target, worker, ui),
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => Ok(()),
        }
    }

    /// Keeps scrolling while a selection is held past a pane's top or bottom.
    pub(super) fn tick(&mut self, worker: &Worker, ui: &mut Ui) {
        let Some(Drag::Select { pane, column, row }) = self.drag else {
            return;
        };
        let Some(inner) = ui
            .area(Target::Content(pane))
            .filter(|_| ui.overlay == Overlay::None)
        else {
            return;
        };
        // A pane can reach the screen's bottom edge, so its last row scrolls down too.
        let rows = if row < inner.y {
            1
        } else if row.saturating_add(1) >= inner.bottom() {
            -1
        } else {
            return;
        };
        if self
            .scrolled
            .is_some_and(|scrolled| scrolled.elapsed() < AUTOSCROLL)
        {
            return;
        }
        self.scrolled = Some(Instant::now());
        scroll(worker, pane, Some(rows));
        extend(worker, ui, pane, column, row);
    }

    fn press(
        &mut self,
        button: MouseButton,
        event: MouseEvent,
        target: Option<Target>,
        worker: &Worker,
        ui: &mut Ui,
    ) -> Result {
        if let Some(Drag::App { pane, .. }) = self.drag {
            let button = terminal_button(button);
            return forward(
                worker,
                ui,
                pane,
                MouseAction::Press,
                Some(button),
                None,
                event,
            );
        }
        if button == MouseButton::Right {
            let at = Position::new(event.column, event.row);
            match target {
                Some(Target::Content(pane) | Target::Pane(pane)) => {
                    super::menu::pane(worker, ui, pane, at);
                }
                Some(Target::Tab(pane)) => {
                    // Below the tab bar, so the tab stays in view.
                    let below = Position::new(at.x, at.y.saturating_add(1));
                    super::menu::tab(worker, ui, pane, below);
                }
                _ => {}
            }
            return Ok(());
        }
        let left = button == MouseButton::Left;
        if !matches!(target, Some(Target::Content(_))) {
            ui.selection = None;
        }
        match target {
            Some(Target::Content(pane)) => return self.press_pane(button, event, pane, worker, ui),
            Some(Target::Pane(pane)) => return focus(worker, pane),
            Some(Target::Divider(divider)) if left => {
                self.drag = Some(Drag::Divider {
                    divider,
                    at: match divider.axis {
                        Axis::Horizontal => event.column,
                        Axis::Vertical => event.row,
                    },
                });
            }
            Some(Target::SidebarEdge) if left => self.drag = Some(Drag::Sidebar),
            Some(Target::HideSidebar) if left => ui.sidebar_shown = false,
            Some(Target::ShowSidebar) if left => ui.sidebar_shown = true,
            Some(Target::Help) if left => ui.overlay = Overlay::Help(0),
            Some(Target::Tab(pane)) if left => return focus(worker, pane),
            Some(Target::NewTab) if left => return worker.send(Action::New(None)),
            Some(Target::Existing) if left => {
                ui.overlay = Overlay::Sessions(0);
                return worker.send(Action::ListSessions);
            }
            Some(Target::Project(project)) if left => return worker.send(Action::Open(project)),
            Some(Target::Agent(session, project)) if left => {
                let pane = {
                    let shared = lock(&worker.shared);
                    shared
                        .state
                        .as_ref()
                        .and_then(|state| state.projects.get(&project))
                        .and_then(|layout| {
                            layout.tabs.iter().find_map(|tab| {
                                tab.panes
                                    .iter()
                                    .find(|(_, pane)| pane.session == Some(session))
                                    .map(|(id, _)| *id)
                            })
                        })
                };
                return worker.send(match pane {
                    Some(pane) => Action::SelectPane(project, pane),
                    None => Action::Open(project),
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn press_pane(
        &mut self,
        button: MouseButton,
        event: MouseEvent,
        pane: PaneId,
        worker: &Worker,
        ui: &mut Ui,
    ) -> Result {
        focus(worker, pane)?;
        let shift = event.modifiers.contains(KeyModifiers::SHIFT);
        let (reports, point) = {
            let shared = lock(&worker.shared);
            let Some(view) = shared.views.get(&pane) else {
                return Ok(());
            };
            let inner = ui.area(Target::Content(pane)).unwrap_or_default();
            (
                reports_mouse(view, shift),
                panes::point(view, inner, event.column, event.row),
            )
        };
        if reports {
            ui.selection = None;
            let button = terminal_button(button);
            self.drag = Some(Drag::App { pane, button });
            return forward(
                worker,
                ui,
                pane,
                MouseAction::Press,
                Some(button),
                None,
                event,
            );
        }
        if button != MouseButton::Left {
            return Ok(());
        }
        let unit = match self.clicks(Position::new(event.column, event.row)) {
            1 => Unit::Cell,
            2 => Unit::Word,
            _ => Unit::Line,
        };
        ui.selection = Some(Selection {
            pane,
            anchor: point,
            head: point,
            unit,
        });
        self.drag = Some(Drag::Select {
            pane,
            column: event.column,
            row: event.row,
        });
        Ok(())
    }

    /// Counts this press as the first, second, or third of a quick series.
    fn clicks(&mut self, position: Position) -> u8 {
        let now = Instant::now();
        let count = match self.click {
            Some((at, last, count))
                if now.duration_since(at) <= MULTI_CLICK
                    && last.y == position.y
                    && last.x.abs_diff(position.x) <= 1 =>
            {
                count % 3 + 1
            }
            _ => 1,
        };
        self.click = Some((now, position, count));
        count
    }

    fn drag(
        &mut self,
        button: MouseButton,
        event: MouseEvent,
        worker: &Worker,
        ui: &mut Ui,
    ) -> Result {
        match &mut self.drag {
            None => Ok(()),
            Some(Drag::Divider { divider, at }) => {
                let position = match divider.axis {
                    Axis::Horizontal => event.column,
                    Axis::Vertical => event.row,
                };
                if position == *at {
                    return Ok(());
                }
                *at = position;
                worker.send(Action::Ratio {
                    first: divider.first,
                    second: divider.second,
                    ratio: divider.ratio(event.column, event.row),
                })
            }
            Some(Drag::Sidebar) => {
                let width = event
                    .column
                    .saturating_add(1)
                    .saturating_sub(ui.chrome.sidebar.x);
                ui.sidebar_width = width.clamp(
                    *layout::SIDEBAR_WIDTHS.start(),
                    *layout::SIDEBAR_WIDTHS.end(),
                );
                Ok(())
            }
            Some(Drag::Select { pane, column, row }) => {
                let pane = *pane;
                *column = event.column;
                *row = event.row;
                extend(worker, ui, pane, event.column, event.row);
                Ok(())
            }
            Some(Drag::App { pane, button: held }) => {
                let (pane, held) = (*pane, *held);
                if terminal_button(button) != held || self.repeats(pane, event) {
                    return Ok(());
                }
                forward(
                    worker,
                    ui,
                    pane,
                    MouseAction::Motion,
                    Some(held),
                    None,
                    event,
                )
            }
        }
    }

    fn release(
        &mut self,
        button: MouseButton,
        event: MouseEvent,
        worker: &Worker,
        ui: &mut Ui,
    ) -> Result {
        self.motion = None;
        let button = terminal_button(button);
        if let Some(Drag::App { pane, button: held }) = self.drag
            && button != held
        {
            return forward(
                worker,
                ui,
                pane,
                MouseAction::Release,
                Some(button),
                None,
                event,
            );
        }
        match self.drag.take() {
            Some(Drag::Select { pane, .. }) => {
                if let Some(selection) = ui
                    .selection
                    .filter(|selection| selection.pane == pane && !selection.is_empty())
                {
                    copy(worker, selection)?;
                }
                Ok(())
            }
            Some(Drag::App { pane, button: held }) => forward(
                worker,
                ui,
                pane,
                MouseAction::Release,
                Some(held),
                None,
                event,
            ),
            Some(Drag::Divider { .. } | Drag::Sidebar) | None => Ok(()),
        }
    }

    fn moved(
        &mut self,
        event: MouseEvent,
        target: Option<Target>,
        worker: &Worker,
        ui: &Ui,
    ) -> Result {
        let Some(Target::Content(pane)) = target else {
            return Ok(());
        };
        if focused_pane(&lock(&worker.shared)) != Some(pane)
            || !reports(worker, pane, event)
            || self.repeats(pane, event)
        {
            return Ok(());
        }
        forward(worker, ui, pane, MouseAction::Motion, None, None, event)
    }

    /// Whether `event` is in the same cell as the last motion sent.
    fn repeats(&mut self, pane: PaneId, event: MouseEvent) -> bool {
        let at = (pane, Position::new(event.column, event.row));
        let repeated = self.motion == Some(at);
        self.motion = Some(at);
        repeated
    }
}

fn wheel(
    up: bool,
    event: MouseEvent,
    target: Option<Target>,
    worker: &Worker,
    ui: &mut Ui,
) -> Result {
    match target {
        Some(Target::Content(pane) | Target::Pane(pane)) => {
            if reports_wheel(worker, pane, event) {
                if let Some(view) = lock(&worker.shared).views.get_mut(&pane) {
                    view.scroll.bottom();
                }
                let direction = if up {
                    ScrollDirection::Up
                } else {
                    ScrollDirection::Down
                };
                return forward(
                    worker,
                    ui,
                    pane,
                    MouseAction::Scroll,
                    None,
                    Some(direction),
                    event,
                );
            }
            scroll(
                worker,
                pane,
                Some(if up { WHEEL_ROWS } else { -WHEEL_ROWS }),
            );
            Ok(())
        }
        Some(Target::SidebarList | Target::Project(_)) => {
            ui.sidebar_scroll = if up {
                ui.sidebar_scroll.saturating_sub(2)
            } else {
                ui.sidebar_scroll + 2
            };
            Ok(())
        }
        Some(Target::TabBar | Target::Tab(_) | Target::NewTab) => {
            worker.send(Action::CycleTab(!up))
        }
        _ => Ok(()),
    }
}

/// Clicks and the wheel while a dialog or menu is open. Returns whether the
/// event still belongs to what is under it: a right-click outside a menu
/// closes it and opens another there.
fn dialog(event: MouseEvent, target: Option<Target>, worker: &Worker, ui: &mut Ui) -> Result<bool> {
    let menu = matches!(ui.overlay, Overlay::Menu(_));
    match (event.kind, target) {
        (
            MouseEventKind::Down(MouseButton::Left | MouseButton::Right),
            Some(Target::Item(index)),
        ) => choose(index, worker, ui)?,
        (MouseEventKind::Down(MouseButton::Left), Some(Target::Confirm(yes))) => {
            super::keys::answer(worker, ui, yes)?;
        }
        (MouseEventKind::Down(_), Some(Target::Panel)) => {}
        (MouseEventKind::Down(button), _) => {
            ui.overlay = Overlay::None;
            return Ok(menu && button == MouseButton::Right);
        }
        (MouseEventKind::Moved, Some(Target::Item(index))) => match &mut ui.overlay {
            Overlay::Projects { selected, .. } | Overlay::Sessions(selected) => *selected = index,
            Overlay::Menu(menu) if menu.command(index).is_some() => menu.selected = Some(index),
            _ => {}
        },
        (MouseEventKind::ScrollUp | MouseEventKind::ScrollDown, _) => {
            let up = event.kind == MouseEventKind::ScrollUp;
            match &mut ui.overlay {
                Overlay::Projects { selected, .. }
                | Overlay::Sessions(selected)
                | Overlay::Help(selected) => {
                    *selected = if up {
                        selected.saturating_sub(1)
                    } else {
                        *selected + 1
                    };
                }
                Overlay::Menu(menu) => menu.step(!up),
                Overlay::None | Overlay::Confirm(_) | Overlay::Rename { .. } => {}
            }
            if let Overlay::Help(scroll) = &mut ui.overlay {
                *scroll = (*scroll).min(crate::ui::help_lines());
            }
            super::keys::clamp_selection(worker, ui);
        }
        _ => {}
    }
    Ok(false)
}

/// Runs a menu item, or opens a picker's row.
fn choose(index: usize, worker: &Worker, ui: &mut Ui) -> Result {
    let command = match &mut ui.overlay {
        Overlay::Menu(menu) => menu.command(index),
        Overlay::Projects { selected, .. } | Overlay::Sessions(selected) => {
            *selected = index;
            return super::keys::choose(worker, ui);
        }
        _ => None,
    };
    match command {
        Some(command) => super::menu::run(command, worker, ui),
        None => Ok(()),
    }
}

pub(super) fn focus(worker: &Worker, pane: PaneId) -> Result {
    let project = {
        let shared = lock(&worker.shared);
        let Some(state) = &shared.state else {
            return Ok(());
        };
        if state.tab().is_none_or(|tab| tab.focus == pane) {
            return Ok(());
        }
        state.active
    };
    worker.send(Action::SelectPane(project, pane))
}

/// Moves the end of the selection being dragged in `pane` to a cell.
fn extend(worker: &Worker, ui: &mut Ui, pane: PaneId, column: u16, row: u16) {
    let Some(inner) = ui.area(Target::Content(pane)) else {
        return;
    };
    let shared = lock(&worker.shared);
    if let (Some(view), Some(selection)) = (shared.views.get(&pane), &mut ui.selection)
        && selection.pane == pane
    {
        selection.head = panes::point(view, inner, column, row);
    }
}

/// Puts the selected text on the clipboard of the computer showing this UI.
pub(super) fn copy(worker: &Worker, selection: Selection) -> Result {
    let text = {
        let shared = lock(&worker.shared);
        let Some(view) = shared.views.get(&selection.pane) else {
            return Ok(());
        };
        selection.text(view.scroll.grid(&view.grid))
    };
    if text.is_empty() {
        return Ok(());
    }
    let count = text.chars().count();
    let message = match clipboard::copy(&text).map_err(|error| error.to_string())? {
        Copied::System => format!("Copied {count} characters"),
        Copied::Terminal => format!("Sent {count} characters to the terminal's clipboard"),
    };
    lock(&worker.shared).say(message);
    Ok(())
}

/// Whether clicks in `view` belong to its app. History shown in place of
/// the app's screen is the UI's to select.
fn reports_mouse(view: &View, shift: bool) -> bool {
    !shift
        && !view.ended
        && view.channel.is_some()
        && !view.scroll.is_active()
        && view.input.mouse_tracking
}

fn reports(worker: &Worker, pane: PaneId, event: MouseEvent) -> bool {
    let shift = event.modifiers.contains(KeyModifiers::SHIFT);
    lock(&worker.shared)
        .views
        .get(&pane)
        .is_some_and(|view| reports_mouse(view, shift))
}

/// Whether the wheel over `pane` belongs to its app rather than scrollback.
fn reports_wheel(worker: &Worker, pane: PaneId, event: MouseEvent) -> bool {
    let shift = event.modifiers.contains(KeyModifiers::SHIFT);
    lock(&worker.shared).views.get(&pane).is_some_and(|view| {
        !shift
            && !view.ended
            && view.channel.is_some()
            && !view.scroll.is_active()
            && (view.input.mouse_tracking || view.input.alternate_scroll)
    })
}

fn terminal_button(button: MouseButton) -> TerminalButton {
    match button {
        MouseButton::Left => TerminalButton::Left,
        MouseButton::Right => TerminalButton::Right,
        MouseButton::Middle => TerminalButton::Middle,
    }
}

/// Sends a mouse event to the app in `pane`, in the pane's own cells.
pub(super) fn forward(
    worker: &Worker,
    ui: &Ui,
    pane: PaneId,
    action: MouseAction,
    button: Option<TerminalButton>,
    scroll: Option<ScrollDirection>,
    event: MouseEvent,
) -> Result {
    let Some(inner) = ui.area(Target::Content(pane)) else {
        return Ok(());
    };
    let Some(channel) = lock(&worker.shared)
        .views
        .get(&pane)
        .and_then(|view| view.channel)
    else {
        return Ok(());
    };
    let mouse = TerminalMouse {
        action,
        button,
        column: event
            .column
            .saturating_sub(inner.x)
            .min(inner.width.saturating_sub(1)),
        row: event
            .row
            .saturating_sub(inner.y)
            .min(inner.height.saturating_sub(1)),
        scroll,
        modifiers: Modifiers {
            shift: event.modifiers.contains(KeyModifiers::SHIFT),
            alt: event.modifiers.contains(KeyModifiers::ALT),
            ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
        },
    };
    worker.mouse(channel, mouse)
}
