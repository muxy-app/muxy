//! Moving a tab, a pane, or a whole tab window anywhere: into another tab's
//! splits, among any project's tabs, onto a project, or out into a window of
//! its own. Every window records where its drop zones are, and a move looks
//! them up under the pointer in whichever window that is.

mod ghost;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AnyWindowHandle, Bounds, Context, CursorStyle, DispatchPhase, Hsla, IntoElement,
    MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, SharedString, Size,
    Styled, Window, WindowHandle, WindowId, canvas, div, point, px, size,
};
use muxy_app_core::{Axis, PaneDrop, PaneId, ProjectId, TabId, TabPlace, TabWindowId};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use crate::model::AppModel;
use crate::model::tab_windows::dropped_bounds;
use crate::views::splits::drag::{DropTarget, Placement};

/// Who records a zone, so each can replace its own when it draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Strip,
    Sidebar,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ZoneKind {
    /// A tab's cell or row. A drop goes before or after it, along `axis`.
    Tab {
        project: ProjectId,
        tab: TabId,
        axis: Axis,
    },
    /// A tab strip: a drop past its last tab goes last.
    Strip(ProjectId),
    /// A project's row: a drop becomes its last tab.
    Project(ProjectId),
}

#[derive(Clone, Copy, Debug)]
struct Zone {
    owner: Owner,
    window: WindowId,
    bounds: Bounds<Pixels>,
    kind: ZoneKind,
}

#[derive(Default)]
struct ZoneMap {
    zones: Vec<Zone>,
    /// Each window drawn, with its system window number where it has one.
    windows: Vec<(WindowId, AnyWindowHandle, Option<isize>)>,
}

/// Where tabs and projects can take a drop, in every window.
#[derive(Clone, Default)]
pub(crate) struct Zones(Rc<RefCell<ZoneMap>>);

impl Zones {
    pub(crate) fn clear(&self, window: WindowId, owner: Owner) {
        self.0
            .borrow_mut()
            .zones
            .retain(|zone| zone.window != window || zone.owner != owner);
    }

    fn add(&self, zone: Zone) {
        self.0.borrow_mut().zones.push(zone);
    }

    /// Records that `window` draws, so a move can find it by its number.
    pub(crate) fn note_window(&self, window: &Window) {
        let id = window.window_handle().window_id();
        let number = if cfg!(test) {
            None
        } else {
            muxy_ui::screen::window_number(window)
        };
        let mut map = self.0.borrow_mut();
        map.windows.retain(|(known, ..)| *known != id);
        map.windows.push((id, window.window_handle(), number));
    }

    pub(crate) fn forget_window(&self, window: WindowId) {
        let mut map = self.0.borrow_mut();
        map.windows.retain(|(known, ..)| *known != window);
        map.zones.retain(|zone| zone.window != window);
    }

    fn numbered(&self, number: isize) -> Option<(WindowId, AnyWindowHandle)> {
        self.0
            .borrow()
            .windows
            .iter()
            .find(|(.., known)| *known == Some(number))
            .map(|(id, handle, _)| (*id, *handle))
    }

    fn number(&self, window: WindowId) -> Option<isize> {
        self.0
            .borrow()
            .windows
            .iter()
            .find(|(id, ..)| *id == window)
            .and_then(|(.., number)| *number)
    }

    /// The zone under `position` in `window`: a tab first, then a project,
    /// then a strip.
    fn at(&self, window: WindowId, position: Point<Pixels>) -> Option<Zone> {
        let map = self.0.borrow();
        let hits = || {
            map.zones
                .iter()
                .filter(move |zone| zone.window == window && zone.bounds.contains(&position))
        };
        hits()
            .find(|zone| matches!(zone.kind, ZoneKind::Tab { .. }))
            .or_else(|| hits().find(|zone| matches!(zone.kind, ZoneKind::Project(_))))
            .or_else(|| hits().find(|zone| matches!(zone.kind, ZoneKind::Strip(_))))
            .copied()
    }

    /// Where `project`'s last tab in `window`'s strip ends.
    fn strip_end(&self, window: WindowId, project: ProjectId) -> Option<Bounds<Pixels>> {
        self.0
            .borrow()
            .zones
            .iter()
            .filter(|zone| {
                zone.window == window
                    && matches!(zone.kind, ZoneKind::Tab { project: owner, axis: Axis::Horizontal, .. } if owner == project)
            })
            .map(|zone| zone.bounds)
            .max_by(|a, b| f32::from(a.right()).total_cmp(&f32::from(b.right())))
    }
}

/// Records the bounds of the element it fills as a zone of `kind`.
pub(crate) fn zone(zones: &Zones, owner: Owner, kind: ZoneKind) -> AnyElement {
    let zones = zones.clone();
    canvas(
        move |bounds, window, _| {
            let clip = window.content_mask().bounds;
            zones.add(Zone {
                owner,
                window: window.window_handle().window_id(),
                bounds: bounds.intersect(&clip),
                kind,
            });
        },
        |_, (), _, _| {},
    )
    .absolute()
    .inset_0()
    .into_any_element()
}

/// What is moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Moving {
    Tab(TabId),
    Pane(PaneId),
    Window(TabWindowId),
}

/// Where a move lands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Target {
    /// Among a project's listed tabs.
    Place(TabPlace),
    /// In a tab's splits.
    Pane(PaneDrop),
    /// In a window of its own.
    NewWindow,
    /// Somewhere it can't go, such as another computer's project.
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    /// Where a pane will be, over a tab's panes.
    Area,
    /// A line between tabs.
    Insert,
    /// A project's row.
    Project,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Indicator {
    window: WindowId,
    bounds: Bounds<Pixels>,
    mark: Mark,
}

/// How long an indicator glides to its next place.
const GLIDE: Duration = Duration::from_millis(140);
/// How far from the pointer the drag chip sits.
const CHIP_OFFSET: f32 = 14.0;

struct Gesture {
    moving: Moving,
    source: ProjectId,
    label: SharedString,
    icon: Icon,
    /// Where in the moving thing the pointer holds it, for a new window.
    grab: Point<Pixels>,
    /// The size a new window takes.
    size: Size<Pixels>,
    target: Option<Target>,
    indicator: Option<Indicator>,
    /// Where the indicator glides from, and since when.
    glide: Option<(Bounds<Pixels>, Instant)>,
    /// The pointer, in the window it is over.
    pointer: Option<(WindowId, Point<Pixels>)>,
}

#[derive(Default)]
pub(crate) struct MoveDragState {
    pub(crate) zones: Zones,
    gesture: Option<Gesture>,
    ghost: Option<WindowHandle<ghost::Ghost>>,
    ghost_opening: bool,
    carry: Option<Carry>,
}

/// A tab window carried by its titlebar, on systems where the app places
/// windows itself.
#[derive(Clone, Copy)]
struct Carry {
    window: TabWindowId,
    grab: Point<Pixels>,
    origin: Point<Pixels>,
    moved: bool,
}

impl MoveDragState {
    pub(crate) fn target(&self) -> Option<Target> {
        self.gesture.as_ref().and_then(|gesture| gesture.target)
    }
}

impl AppModel {
    /// Follows a move outside the reach of the tab strip or the tab's own
    /// splits. Returns whether it found somewhere to drop.
    pub(crate) fn track_move(
        &mut self,
        moving: Moving,
        window: &mut Window,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self
            .move_drag
            .gesture
            .as_ref()
            .is_none_or(|gesture| gesture.moving != moving)
        {
            let Some(gesture) = self.start_gesture(moving, window) else {
                return false;
            };
            self.move_drag.gesture = Some(gesture);
        }
        let (over, local, outside) = self.locate(moving, window, position, cx);
        let found = match over {
            Some(over) => self.target_in(moving, over, local),
            None if outside && self.can_open_window(moving) => Some((Target::NewWindow, None)),
            None => None,
        };
        let pointer = over.map(|over| (over, local));
        let Some(gesture) = &mut self.move_drag.gesture else {
            return false;
        };
        let (target, indicator) = found.map_or((None, None), |(target, indicator)| {
            (Some(target), indicator)
        });
        let changed = gesture.target != target
            || gesture.indicator != indicator
            || gesture.pointer != pointer;
        if gesture.indicator != indicator {
            gesture.glide = gesture
                .indicator
                .zip(indicator)
                .filter(|(from, to)| from.window == to.window && from.mark == to.mark)
                .map(|(from, _)| {
                    (
                        current_bounds(from.bounds, gesture.glide, gesture.indicator),
                        Instant::now(),
                    )
                });
            gesture.indicator = indicator;
        }
        gesture.target = target;
        gesture.pointer = pointer;
        let ghost = outside && target == Some(Target::NewWindow);
        self.place_ghost(ghost, cx);
        if changed {
            cx.notify();
        }
        target.is_some_and(|target| target != Target::Refused)
    }

    /// Forgets where a move would land, while the tab strip or the tab's own
    /// splits take it over.
    pub(crate) fn clear_move_target(&mut self, cx: &mut Context<Self>) {
        if let Some(gesture) = &mut self.move_drag.gesture
            && (gesture.target.is_some() || gesture.pointer.is_some())
        {
            gesture.target = None;
            gesture.indicator = None;
            gesture.glide = None;
            gesture.pointer = None;
            cx.notify();
        }
        self.place_ghost(false, cx);
    }

    /// Ends a move, applying it where it was dropped. Returns whether it
    /// landed anywhere.
    pub(crate) fn finish_move(&mut self, cx: &mut Context<Self>) -> bool {
        let gesture = self.move_drag.gesture.take();
        self.place_ghost(false, cx);
        let Some(gesture) = gesture else {
            return false;
        };
        cx.notify();
        let grab = Some(gesture.grab);
        let bounds = Some(dropped_bounds(gesture.size));
        match (gesture.target, gesture.moving) {
            (Some(Target::Place(place)), Moving::Tab(tab)) => {
                self.move_tab_to_place(tab, place, cx);
            }
            (Some(Target::Place(place)), Moving::Pane(pane)) => {
                self.move_pane_to_new_tab(pane, place, cx);
            }
            (Some(Target::Place(place)), Moving::Window(window)) => {
                self.return_tab_window(window, Some(place), cx);
            }
            (Some(Target::Pane(drop)), Moving::Pane(pane)) => self.move_pane_into(pane, drop, cx),
            (Some(Target::NewWindow), Moving::Tab(tab)) => {
                self.move_tab_to_window(tab, grab, bounds, cx);
            }
            (Some(Target::NewWindow), Moving::Pane(pane)) => {
                self.move_pane_to_window(pane, grab, bounds, cx);
            }
            (Some(Target::Refused), _) => {
                self.fail(
                    tr!("Tabs and panes move only between projects on the same computer.")
                        .to_string(),
                    cx,
                );
                return false;
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn cancel_move(&mut self, cx: &mut Context<Self>) {
        if self.move_drag.gesture.take().is_some() {
            cx.notify();
        }
        self.place_ghost(false, cx);
        if let Some(carry) = self.move_drag.carry.take()
            && let Some(handle) = self.tab_windows.handle(carry.window)
        {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| {
                    muxy_ui::screen::set_window_alpha(window, 1.0);
                });
            });
        }
    }

    fn start_gesture(&self, moving: Moving, window: &Window) -> Option<Gesture> {
        let tab = match moving {
            Moving::Tab(tab) => tab,
            Moving::Pane(pane) => self.state.pane_tab(pane)?,
            Moving::Window(id) => self.window_tab(id)?,
        };
        let source = self
            .state
            .projects()
            .iter()
            .find(|project| project.tabs.iter().any(|candidate| candidate.id == tab))?;
        let descriptor = self.tab(tab)?;
        let (label, icon) = match moving {
            Moving::Pane(pane) => {
                let pane = descriptor
                    .panes
                    .iter()
                    .find(|candidate| candidate.id == pane)?;
                (pane.title.clone(), pane_icon(&pane.content))
            }
            Moving::Tab(_) | Moving::Window(_) => (
                descriptor
                    .title(self.state.shown_pane(descriptor))
                    .to_owned(),
                descriptor
                    .panes
                    .first()
                    .map_or(Icon::Terminal, |pane| pane_icon(&pane.content)),
            ),
        };
        let area = self
            .layout_drag
            .geometry
            .placement(tab)
            .map(|placement| placement.bounds);
        let size = area.map_or_else(
            || window.viewport_size(),
            |area| {
                size(
                    area.size.width,
                    area.size.height + self.metrics.title_bar_height(),
                )
            },
        );
        Some(Gesture {
            moving,
            source: source.id,
            label: label.into(),
            icon,
            grab: point(
                px(CHIP_OFFSET.min(f32::from(size.width) / 2.0)),
                self.metrics.title_bar_height() / 2.0,
            ),
            size,
            target: None,
            indicator: None,
            glide: None,
            pointer: None,
        })
    }

    /// The window under the pointer with the pointer in its coordinates, or
    /// whether the pointer is outside every window of the app.
    fn locate(
        &self,
        moving: Moving,
        window: &mut Window,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> (Option<WindowId>, Point<Pixels>, bool) {
        let here = window.window_handle().window_id();
        let Some(cursor) = cursor() else {
            let viewport = window.viewport_size();
            let inside = Bounds::new(Point::default(), viewport).contains(&position);
            return (inside.then_some(here), position, !inside);
        };
        let below = match moving {
            Moving::Window(id) => self
                .tab_windows
                .handle(id)
                .and_then(|handle| self.move_drag.zones.number(handle.window_id())),
            Moving::Tab(_) | Moving::Pane(_) => None,
        };
        let ghost = self
            .move_drag
            .ghost
            .and_then(|ghost| self.move_drag.zones.number(ghost.window_id()));
        let mut number = muxy_ui::screen::window_at(cursor, below);
        if number.is_some() && number == ghost {
            number = muxy_ui::screen::window_at(cursor, ghost);
        }
        let Some((id, handle)) = number.and_then(|number| self.move_drag.zones.numbered(number))
        else {
            return (None, position, number.is_none());
        };
        let local = if id == here {
            muxy_ui::screen::local(window, cursor)
        } else {
            handle
                .update(cx, |_, window, _| muxy_ui::screen::local(window, cursor))
                .ok()
                .flatten()
        };
        match local {
            Some(local) => (Some(id), local, false),
            None => (None, position, false),
        }
    }

    fn can_open_window(&self, moving: Moving) -> bool {
        match moving {
            Moving::Tab(tab) => self.tab(tab).is_some_and(|tab| !tab.detached()),
            Moving::Pane(pane) => self
                .state
                .pane_tab(pane)
                .and_then(|tab| self.tab(tab))
                .is_some_and(|tab| !tab.detached() || tab.panes.len() > 1),
            Moving::Window(_) => false,
        }
    }

    /// Where `moving` lands at `position` in `window`, and how to show it.
    fn target_in(
        &self,
        moving: Moving,
        window: WindowId,
        position: Point<Pixels>,
    ) -> Option<(Target, Option<Indicator>)> {
        let source = self.move_drag.gesture.as_ref()?.source;
        if let Some(zone) = self.move_drag.zones.at(window, position) {
            let (place, bounds, mark) = self.place_at(moving, zone, position)?;
            if !self.state.can_move_into(source, place.project) {
                return Some((
                    Target::Refused,
                    Some(Indicator {
                        window,
                        bounds: zone.bounds,
                        mark: Mark::Refused,
                    }),
                ));
            }
            return Some((
                Target::Place(place),
                Some(Indicator {
                    window,
                    bounds,
                    mark,
                }),
            ));
        }
        let Moving::Pane(pane) = moving else {
            return None;
        };
        let placement = self.layout_drag.geometry.at(window, position)?;
        if self.state.pane_tab(pane) == Some(placement.tab) {
            return None;
        }
        let tab = self.tab(placement.tab)?;
        if tab.zoomed.is_some() {
            return None;
        }
        let project = self.tab_project(placement.tab)?.id;
        if !self.state.can_move_into(source, project) {
            return Some((
                Target::Refused,
                Some(Indicator {
                    window,
                    bounds: placement.bounds,
                    mark: Mark::Refused,
                }),
            ));
        }
        let (drop, bounds) = pane_drop(pane, &tab.layout, placement, position)?;
        Some((
            Target::Pane(drop),
            Some(Indicator {
                window,
                bounds,
                mark: Mark::Area,
            }),
        ))
    }

    /// The place among tabs that `zone` offers at `position`, with the
    /// indicator's bounds and look.
    fn place_at(
        &self,
        moving: Moving,
        zone: Zone,
        position: Point<Pixels>,
    ) -> Option<(TabPlace, Bounds<Pixels>, Mark)> {
        let moving_tab = match moving {
            Moving::Tab(tab) => Some(tab),
            Moving::Window(id) => self.window_tab(id),
            Moving::Pane(_) => None,
        };
        match zone.kind {
            ZoneKind::Project(project) => {
                Some((TabPlace::end(project), zone.bounds, Mark::Project))
            }
            ZoneKind::Strip(project) => {
                let line = self
                    .move_drag
                    .zones
                    .strip_end(zone.window, project)
                    .map_or_else(
                        || line(zone.bounds, Axis::Horizontal, false),
                        |last| line(last, Axis::Horizontal, true),
                    );
                Some((TabPlace::end(project), line, Mark::Insert))
            }
            ZoneKind::Tab { project, tab, axis } => {
                if moving_tab == Some(tab) {
                    return None;
                }
                let after = match axis {
                    Axis::Horizontal => position.x > zone.bounds.center().x,
                    Axis::Vertical => position.y > zone.bounds.center().y,
                };
                let listed: Vec<_> = self
                    .state
                    .project(project)?
                    .listed_tabs()
                    .map(|tab| tab.id)
                    .filter(|id| Some(*id) != moving_tab)
                    .collect();
                let index = listed.iter().position(|id| *id == tab)?;
                let before = if after {
                    listed.get(index + 1).copied()
                } else {
                    Some(tab)
                };
                Some((
                    TabPlace { project, before },
                    line(zone.bounds, axis, after),
                    Mark::Insert,
                ))
            }
        }
    }

    /// Shows or hides the window that follows the pointer outside the app.
    fn place_ghost(&mut self, shown: bool, cx: &mut Context<Self>) {
        let cursor = cursor().filter(|_| can_place_windows());
        let gesture = self.move_drag.gesture.as_ref();
        match (shown, cursor, gesture, self.move_drag.ghost) {
            (true, Some(cursor), Some(_), Some(handle)) => {
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| {
                        muxy_ui::screen::move_window_to(window, cursor, ghost::GRAB);
                    });
                });
            }
            (true, Some(_), Some(gesture), None) if !self.move_drag.ghost_opening => {
                self.move_drag.ghost_opening = true;
                let chip = ghost::Chip {
                    label: gesture.label.clone(),
                    icon: gesture.icon,
                    theme: self.theme.clone(),
                    metrics: self.metrics,
                };
                let model = cx.weak_entity();
                cx.defer(move |cx| {
                    let opened = ghost::open(chip, cx);
                    let _ = model.update(cx, |model, cx| {
                        model.move_drag.ghost_opening = false;
                        match opened {
                            Some(handle) if model.move_drag.gesture.is_some() => {
                                model.move_drag.ghost = Some(handle);
                            }
                            Some(handle) => ghost::close(handle, cx),
                            None => {}
                        }
                    });
                });
            }
            (false, ..) | (_, None, ..) | (_, _, None, _) => {
                if let Some(handle) = self.move_drag.ghost.take() {
                    ghost::close(handle, cx);
                }
            }
            _ => {}
        }
    }

    /// Starts carrying tab window `id` by its titlebar, held at `grab`.
    pub(crate) fn begin_carry(
        &mut self,
        id: TabWindowId,
        grab: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !can_place_windows() {
            if !cfg!(test) {
                window.start_window_move();
            }
            return;
        }
        self.cancel_move(cx);
        self.move_drag.carry = Some(Carry {
            window: id,
            grab,
            origin: grab,
            moved: false,
        });
    }

    fn move_carry(
        &mut self,
        id: TabWindowId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(carry) = &mut self.move_drag.carry else {
            return;
        };
        if carry.window != id {
            return;
        }
        if !carry.moved && (position - carry.origin).magnitude() < 3.0 {
            return;
        }
        carry.moved = true;
        let grab = carry.grab;
        if let Some(cursor) = cursor() {
            muxy_ui::screen::move_window_to(window, cursor, grab);
        }
        let landing = self.track_move(Moving::Window(id), window, position, cx);
        muxy_ui::screen::set_window_alpha(window, if landing { 0.55 } else { 1.0 });
    }

    fn end_carry(&mut self, id: TabWindowId, window: &mut Window, cx: &mut Context<Self>) {
        if self.move_drag.carry.is_none_or(|carry| carry.window != id) {
            return;
        }
        self.move_drag.carry = None;
        muxy_ui::screen::set_window_alpha(window, 1.0);
        if !self.finish_move(cx) {
            self.tab_window_moved(id, window, cx);
        }
    }
}

/// The pointer on screen, where the system says. Tests have none, so they
/// never depend on where the real pointer is.
pub(crate) fn cursor() -> Option<muxy_ui::screen::ScreenPoint> {
    if cfg!(test) {
        None
    } else {
        muxy_ui::screen::cursor()
    }
}

/// Whether the app can place windows itself, so a window can follow the
/// pointer.
pub(crate) fn can_place_windows() -> bool {
    !cfg!(test) && muxy_ui::screen::can_place_windows()
}

/// Where a pane from another tab lands at `position` over `placement`'s
/// panes, with the bounds it would take.
fn pane_drop(
    pane: PaneId,
    layout: &muxy_app_core::Layout,
    placement: Placement,
    position: Point<Pixels>,
) -> Option<(PaneDrop, Bounds<Pixels>)> {
    let target =
        crate::views::splits::drag::target_at(layout, placement.bounds, placement.scale, position)?;
    let (drop, result) = match target {
        DropTarget::Swap(anchor) => {
            let mut result = layout.clone();
            result.replace(anchor, pane);
            (PaneDrop::Swap(anchor), result)
        }
        DropTarget::Dock {
            pane: anchor,
            edge,
            level,
        } => (
            PaneDrop::Dock {
                anchor,
                edge,
                level,
            },
            layout.inserted(pane, anchor, edge, level)?,
        ),
    };
    let bounds = crate::views::splits::drag::pane_bounds(&result, placement, pane)?;
    Some((drop, bounds))
}

/// A thin line along the start or end edge of `bounds`, across `axis`.
fn line(bounds: Bounds<Pixels>, axis: Axis, end: bool) -> Bounds<Pixels> {
    const WIDTH: f32 = 3.0;
    match axis {
        Axis::Horizontal => {
            let x = if end { bounds.right() } else { bounds.left() };
            Bounds::new(
                point(x - px(WIDTH / 2.0), bounds.top() + px(4.0)),
                size(px(WIDTH), (bounds.size.height - px(8.0)).max(px(WIDTH))),
            )
        }
        Axis::Vertical => {
            let y = if end { bounds.bottom() } else { bounds.top() };
            Bounds::new(
                point(bounds.left() + px(6.0), y - px(WIDTH / 2.0)),
                size((bounds.size.width - px(12.0)).max(px(WIDTH)), px(WIDTH)),
            )
        }
    }
}

fn pane_icon(content: &muxy_app_core::PaneContent) -> Icon {
    match content {
        muxy_app_core::PaneContent::Terminal { .. } => Icon::Terminal,
        muxy_app_core::PaneContent::Settings | muxy_app_core::PaneContent::Servers => {
            Icon::Settings
        }
        muxy_app_core::PaneContent::Webview(_) => Icon::Puzzle,
        muxy_app_core::PaneContent::Browser(_) => Icon::Globe,
    }
}

/// How far a glide has come, eased so it slows into place.
fn progress(since: Instant) -> f32 {
    let elapsed = since.elapsed().as_secs_f32() / GLIDE.as_secs_f32();
    let t = elapsed.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn lerp(from: Bounds<Pixels>, to: Bounds<Pixels>, t: f32) -> Bounds<Pixels> {
    let mix = |a: Pixels, b: Pixels| a + (b - a) * t;
    Bounds::new(
        point(mix(from.left(), to.left()), mix(from.top(), to.top())),
        size(
            mix(from.size.width, to.size.width),
            mix(from.size.height, to.size.height),
        ),
    )
}

/// Where an indicator showing `to` is drawn now, while it glides there.
fn current_bounds(
    to: Bounds<Pixels>,
    glide: Option<(Bounds<Pixels>, Instant)>,
    indicator: Option<Indicator>,
) -> Bounds<Pixels> {
    let to = indicator.map_or(to, |indicator| indicator.bounds);
    glide.map_or(to, |(from, since)| lerp(from, to, progress(since)))
}

/// Draws a move's indicator, and the chip that follows the pointer, where
/// they fall in `window`.
pub(crate) fn overlay(model: &AppModel, window: &Window) -> AnyElement {
    let here = window.window_handle().window_id();
    let Some(gesture) = &model.move_drag.gesture else {
        return div().into_any_element();
    };
    let indicator = gesture
        .indicator
        .filter(|indicator| indicator.window == here);
    let glide = gesture.glide;
    let theme = &model.theme;
    let accent = theme.accent;
    let refused = theme.danger;
    let occlusions = model.webviews.occlusions.clone();
    let main = here == model.window.window_id();
    let painted = indicator.map(|indicator| {
        canvas(
            move |_, _, _| {
                if main {
                    occlusions
                        .borrow_mut()
                        .push((None, indicator.bounds.dilate(px(2.0))));
                }
            },
            move |_, (), window, _| {
                let bounds = current_bounds(indicator.bounds, glide, Some(indicator));
                if glide.is_some_and(|(_, since)| since.elapsed() < GLIDE) {
                    window.request_animation_frame();
                }
                paint_indicator(bounds, indicator.mark, accent, refused, window);
            },
        )
        .absolute()
        .size_full()
    });
    let chip = gesture
        .pointer
        .filter(|(window, _)| *window == here)
        .map(|(_, position)| chip(gesture, position, model));
    let cursor = if gesture.target == Some(Target::Refused) {
        CursorStyle::OperationNotAllowed
    } else {
        CursorStyle::ClosedHand
    };
    div()
        .absolute()
        .inset_0()
        .children(painted)
        .children(chip)
        .child(
            canvas(
                |_, _, _| (),
                move |_, (), window, _| window.set_window_cursor_style(cursor),
            )
            .absolute()
            .size_full(),
        )
        .into_any_element()
}

fn paint_indicator(
    bounds: Bounds<Pixels>,
    mark: Mark,
    accent: Hsla,
    refused: Hsla,
    window: &mut Window,
) {
    match mark {
        Mark::Area | Mark::Project => {
            let radius = if mark == Mark::Project {
                px(6.0)
            } else {
                px(2.0)
            };
            window.paint_quad(gpui::fill(bounds, accent.opacity(0.18)).corner_radii(radius));
            window.paint_quad(
                gpui::outline(bounds, accent.opacity(0.85), gpui::BorderStyle::Solid)
                    .border_widths(px(2.0))
                    .corner_radii(radius),
            );
        }
        Mark::Insert => {
            window.paint_quad(gpui::fill(bounds, accent).corner_radii(px(2.0)));
            let cap = px(7.0);
            for center in [bounds.origin, bounds.bottom_right()] {
                let dot = Bounds::new(center - point(cap / 2.0, cap / 2.0), size(cap, cap));
                let dot = if bounds.size.width < bounds.size.height {
                    Bounds::new(point(bounds.center().x - cap / 2.0, dot.top()), dot.size)
                } else {
                    Bounds::new(point(dot.left(), bounds.center().y - cap / 2.0), dot.size)
                };
                window.paint_quad(gpui::fill(dot, accent).corner_radii(cap / 2.0));
            }
        }
        Mark::Refused => {
            window.paint_quad(gpui::fill(bounds, refused.opacity(0.10)).corner_radii(px(4.0)));
            window.paint_quad(
                gpui::outline(bounds, refused.opacity(0.7), gpui::BorderStyle::Dashed)
                    .border_widths(px(2.0))
                    .corner_radii(px(4.0)),
            );
        }
    }
}

/// The chip beside the pointer: what moves, and where it will go.
fn chip(gesture: &Gesture, position: Point<Pixels>, model: &AppModel) -> AnyElement {
    let caption = match gesture.target {
        Some(Target::Refused) => Some((tr!("Only within this computer"), model.theme.danger)),
        Some(Target::Place(_)) => Some((
            match gesture.moving {
                Moving::Window(_) => tr!("Return to tabs"),
                Moving::Tab(_) => tr!("Move tab here"),
                Moving::Pane(_) => tr!("New tab"),
            },
            model.theme.accent,
        )),
        Some(Target::Pane(_)) => Some((tr!("Split here"), model.theme.accent)),
        Some(Target::NewWindow) => Some((tr!("New window"), model.theme.accent)),
        None => None,
    };
    div()
        .absolute()
        .left(position.x + px(CHIP_OFFSET))
        .top(position.y + px(CHIP_OFFSET))
        .child(ghost::chip_body(
            &gesture.label,
            gesture.icon,
            caption,
            &model.theme,
            model.metrics,
        ))
        .into_any_element()
}

/// Follows the pointer while a tab window is carried by its titlebar.
pub(crate) fn track_carry(id: TabWindowId, cx: &Context<AppModel>) -> AnyElement {
    let weak = cx.weak_entity();
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let moving = weak.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = moving.update(cx, |model, cx| {
                    if model.move_drag.carry.is_none_or(|carry| carry.window != id) {
                        return;
                    }
                    if event.pressed_button == Some(MouseButton::Left) {
                        model.move_carry(id, event.position, window, cx);
                        cx.stop_propagation();
                    } else {
                        model.end_carry(id, window, cx);
                    }
                });
            });
            let ending = weak.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                let _ = ending.update(cx, |model, cx| model.end_carry(id, window, cx));
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}
