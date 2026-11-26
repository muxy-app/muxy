mod ghost;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnimationExt as _, AnyElement, AnyWindowHandle, App, Bounds, Context, CursorStyle,
    DispatchPhase, Hsla, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement,
    Pixels, Point, SharedString, Size, Styled, Window, WindowHandle, WindowId, canvas, div, point,
    px, size,
};
use muxy_app_core::{Axis, PaneDrop, PaneId, ProjectId, TabId, TabPlace, TabWindowId};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use crate::model::AppModel;
use crate::model::tab_windows::dropped_bounds;
use crate::views::splits::drag::{DropTarget, Placement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Strip,
    Sidebar,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ZoneKind {
    Tab {
        project: ProjectId,
        tab: TabId,
        axis: Axis,
    },
    Strip(ProjectId),
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
    windows: Vec<(WindowId, AnyWindowHandle, Option<isize>)>,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Moving {
    Tab(TabId),
    Pane(PaneId),
    Window(TabWindowId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Target {
    Place(TabPlace),
    Pane(PaneDrop),
    NewWindow,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Area,
    Insert,
    Project,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Indicator {
    window: WindowId,
    bounds: Bounds<Pixels>,
    mark: Mark,
}

const GLIDE: Duration = Duration::from_millis(140);
const FADE: Duration = Duration::from_millis(120);
const CHIP_OFFSET: f32 = 14.0;
const CARRY_LANDING_ALPHA: f32 = 0.45;

struct Gesture {
    moving: Moving,
    source: ProjectId,
    grab: Point<Pixels>,
    size: Size<Pixels>,
    target: Option<Target>,
    indicator: Option<Indicator>,
    glide: Option<(Bounds<Pixels>, Instant)>,
    appeared: Instant,
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

#[derive(Clone, Copy)]
struct Carry {
    window: TabWindowId,
    grab: Point<Pixels>,
    origin: Point<Pixels>,
    moved: bool,
}

impl MoveDragState {
    pub(crate) fn carrying(&self) -> bool {
        self.carry.is_some()
    }

    pub(crate) fn target(&self) -> Option<Target> {
        self.gesture.as_ref().and_then(|gesture| gesture.target)
    }

    #[cfg(test)]
    pub(crate) fn lifted(&self) -> Option<Moving> {
        self.gesture.as_ref().map(|gesture| gesture.moving)
    }

    #[cfg(test)]
    pub(crate) fn marked(&self) -> Option<Bounds<Pixels>> {
        self.gesture
            .as_ref()
            .and_then(|gesture| gesture.indicator)
            .map(|indicator| indicator.bounds)
    }

    #[cfg(test)]
    pub(crate) fn pointer(&self) -> Option<(WindowId, Point<Pixels>)> {
        self.gesture.as_ref().and_then(|gesture| gesture.pointer)
    }
}

impl AppModel {
    pub(crate) fn track_move(
        &mut self,
        moving: Moving,
        window: &mut Window,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.lift(moving, window) {
            return false;
        }
        let (over, local, outside) = self.locate(moving, window, position, cx);
        let found = match over {
            Some(over) => self.target_in(moving, over, local),
            None if outside && self.can_open_window(moving) => Some((Target::NewWindow, None)),
            None => None,
        };
        let pointer = match over {
            Some(over) => Some((over, local)),
            None if !can_place_windows() && !matches!(moving, Moving::Window(_)) => {
                let viewport = window.viewport_size();
                Some((
                    window.window_handle().window_id(),
                    point(
                        position.x.clamp(px(0.0), viewport.width),
                        position.y.clamp(px(0.0), viewport.height),
                    ),
                ))
            }
            None => None,
        };
        let (target, indicator) = found.map_or((None, None), |(target, indicator)| {
            (Some(target), indicator)
        });
        let indicator = match (target, over) {
            (None, Some(over)) => self.home_marker(moving, over),
            _ => indicator,
        };
        self.show_gesture(target, indicator, pointer, cx);
        let ghost = outside && target == Some(Target::NewWindow);
        self.place_ghost(ghost, cx);
        target.is_some_and(|target| target != Target::Refused)
    }

    pub(crate) fn hold_move(
        &mut self,
        moving: Moving,
        window: &Window,
        position: Point<Pixels>,
        local: Option<Bounds<Pixels>>,
        cx: &mut Context<Self>,
    ) {
        if !self.lift(moving, window) {
            return;
        }
        let here = window.window_handle().window_id();
        let indicator = local
            .map(|bounds| Indicator {
                window: here,
                bounds,
                mark: Mark::Area,
            })
            .or_else(|| self.home_marker(moving, here));
        self.show_gesture(None, indicator, Some((here, position)), cx);
        self.place_ghost(false, cx);
    }

    fn home_marker(&self, moving: Moving, window: WindowId) -> Option<Indicator> {
        let bounds = match moving {
            Moving::Pane(pane) => {
                let tab = self.state.pane_tab(pane)?;
                let placement = self
                    .layout_drag
                    .geometry
                    .placement(tab)
                    .filter(|placement| placement.window == window)?;
                crate::views::splits::drag::pane_bounds(&self.tab(tab)?.layout, placement, pane)?
            }
            Moving::Tab(tab) if window == self.window.window_id() => self
                .tab_drag
                .cells
                .borrow()
                .iter()
                .find(|(id, _)| *id == tab)
                .map(|(_, bounds)| *bounds)
                .filter(|bounds| bounds.size.width > px(0.0))?,
            Moving::Tab(_) | Moving::Window(_) => return None,
        };
        Some(Indicator {
            window,
            bounds,
            mark: Mark::Area,
        })
    }

    fn lift(&mut self, moving: Moving, window: &Window) -> bool {
        if self
            .move_drag
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.moving == moving)
        {
            return true;
        }
        let Some(gesture) = self.start_gesture(moving, window) else {
            return false;
        };
        self.move_drag.gesture = Some(gesture);
        true
    }

    fn show_gesture(
        &mut self,
        target: Option<Target>,
        indicator: Option<Indicator>,
        pointer: Option<(WindowId, Point<Pixels>)>,
        cx: &mut Context<Self>,
    ) {
        let Some(gesture) = &mut self.move_drag.gesture else {
            return;
        };
        let changed = gesture.target != target
            || gesture.indicator != indicator
            || gesture.pointer != pointer;
        if gesture.indicator != indicator {
            let from = gesture
                .indicator
                .zip(indicator)
                .filter(|(from, to)| from.window == to.window && from.mark == to.mark)
                .map(|(from, _)| current_bounds(from.bounds, gesture.glide, gesture.indicator));
            gesture.glide = from.map(|from| (from, Instant::now()));
            if from.is_none() {
                gesture.appeared = Instant::now();
            }
            gesture.indicator = indicator;
        }
        gesture.target = target;
        gesture.pointer = pointer;
        if changed {
            cx.notify();
        }
    }

    pub(crate) fn settle_move(&mut self, cx: &mut Context<Self>) {
        if self.move_drag.gesture.is_some()
            && !self.tab_drag.is_active()
            && !self.layout_drag.active()
            && !self.move_drag.carrying()
        {
            self.cancel_move(cx);
        }
    }

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
            (Some(Target::Pane(drop)), Moving::Window(window)) => {
                let Some(pane) = self.window_pane(window) else {
                    return false;
                };
                self.move_pane_into(pane, drop, cx);
            }
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
        if let Moving::Pane(pane) = moving
            && !descriptor
                .panes
                .iter()
                .any(|candidate| candidate.id == pane)
        {
            return None;
        }
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
            grab: point(
                px(CHIP_OFFSET.min(f32::from(size.width) / 2.0)),
                self.metrics.title_bar_height() / 2.0,
            ),
            size,
            target: None,
            indicator: None,
            glide: None,
            appeared: Instant::now(),
            pointer: None,
        })
    }

    fn window_pane(&self, id: TabWindowId) -> Option<PaneId> {
        let tab = self.tab(self.window_tab(id)?)?;
        match tab.panes.as_slice() {
            [pane] => Some(pane.id),
            _ => None,
        }
    }

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
        let inside = |window: &Window| {
            muxy_ui::screen::local(window, cursor).map(|local| {
                let inside = Bounds::new(Point::default(), window.viewport_size()).contains(&local);
                (local, inside)
            })
        };
        let local = if id == here {
            inside(window)
        } else {
            handle
                .update(cx, |_, window, _| inside(window))
                .ok()
                .flatten()
        };
        match local {
            Some((local, true)) => (Some(id), local, false),
            Some((_, false)) => (None, position, true),
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
                .is_some_and(|tab| !tab.detached() || tab.floating() || tab.panes.len() > 1),
            Moving::Window(_) => false,
        }
    }

    fn target_in(
        &self,
        moving: Moving,
        window: WindowId,
        position: Point<Pixels>,
    ) -> Option<(Target, Option<Indicator>)> {
        let source = self.move_drag.gesture.as_ref()?.source;
        let over_float = self
            .layout_drag
            .geometry
            .at(window, position)
            .and_then(|placement| self.tab(placement.tab))
            .is_some_and(muxy_app_core::Tab::floating);
        if let Some(zone) = self
            .move_drag
            .zones
            .at(window, position)
            .filter(|_| !over_float)
        {
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
        let pane = match moving {
            Moving::Pane(pane) => pane,
            Moving::Window(id) => self.window_pane(id)?,
            Moving::Tab(_) => return None,
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
        if !self.state.can_move_into(source, project) || (tab.floating() && self.shows_page(pane)) {
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
        if let PaneDrop::Swap(anchor) = drop
            && self.pane_floats(pane)
            && self.shows_page(anchor)
        {
            return Some((
                Target::Refused,
                Some(Indicator {
                    window,
                    bounds,
                    mark: Mark::Refused,
                }),
            ));
        }
        Some((
            Target::Pane(drop),
            Some(Indicator {
                window,
                bounds,
                mark: Mark::Area,
            }),
        ))
    }

    fn place_at(
        &self,
        moving: Moving,
        zone: Zone,
        position: Point<Pixels>,
    ) -> Option<(TabPlace, Bounds<Pixels>, Mark)> {
        let moving_tab = match moving {
            Moving::Tab(tab) => Some(tab),
            Moving::Window(id) => self.window_tab(id),
            Moving::Pane(pane) => self
                .state
                .pane_tab(pane)
                .filter(|tab| self.tab(*tab).is_some_and(|tab| tab.panes.len() == 1)),
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

    fn place_ghost(&mut self, shown: bool, cx: &mut Context<Self>) {
        let cursor = cursor().filter(|_| can_place_windows());
        let gesture = self
            .move_drag
            .gesture
            .as_ref()
            .filter(|gesture| !matches!(gesture.moving, Moving::Window(_)));
        match (cursor, gesture, self.move_drag.ghost) {
            (Some(cursor), Some(_), Some(handle)) => {
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| {
                        muxy_ui::screen::move_window_to(window, cursor, ghost::GRAB);
                        muxy_ui::screen::set_window_alpha(window, if shown { 1.0 } else { 0.0 });
                    });
                });
            }
            (Some(_), Some(gesture), None) if shown && !self.move_drag.ghost_opening => {
                self.move_drag.ghost_opening = true;
                let chip = ghost::Chip {
                    model: cx.weak_entity(),
                    moving: gesture.moving,
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
                                let shown = model.move_drag.target() == Some(Target::NewWindow);
                                model.place_ghost(shown, cx);
                            }
                            Some(handle) => ghost::close(handle, cx),
                            None => {}
                        }
                    });
                });
            }
            (None, ..) | (_, None, _) => {
                if let Some(handle) = self.move_drag.ghost.take() {
                    ghost::close(handle, cx);
                }
            }
            _ => {}
        }
    }

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
        muxy_ui::screen::set_window_alpha(window, if landing { CARRY_LANDING_ALPHA } else { 1.0 });
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

pub(crate) fn cursor() -> Option<muxy_ui::screen::ScreenPoint> {
    if cfg!(test) {
        None
    } else {
        muxy_ui::screen::cursor()
    }
}

pub(crate) fn can_place_windows() -> bool {
    !cfg!(test) && muxy_ui::screen::can_place_windows()
}

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

pub(super) fn label(moving: Moving, model: &AppModel, cx: &App) -> SharedString {
    let tab = match moving {
        Moving::Pane(id) => {
            let Some(pane) = model.state.pane(id) else {
                return SharedString::default();
            };
            return model
                .terminal(&id)
                .and_then(|terminal| terminal.view.read(cx).header_title())
                .unwrap_or_else(|| model.pane_title(pane, cx).to_owned())
                .into();
        }
        Moving::Tab(tab) => Some(tab),
        Moving::Window(id) => model.window_tab(id),
    };
    tab.and_then(|tab| model.tab(tab))
        .map(|tab| SharedString::from(model.webview_title(tab, cx).to_owned()))
        .unwrap_or_default()
}

pub(super) fn glyph(moving: Moving, model: &AppModel, cx: &App) -> AnyElement {
    let size = model.metrics.icon_md();
    let color = model.theme.fg_muted;
    let fallback = |pane: Option<&muxy_app_core::Pane>| {
        IconGlyph::new(
            pane.map_or(Icon::Terminal, |pane| pane_icon(&pane.content)),
            size,
            color,
        )
        .into_any_element()
    };
    match moving {
        Moving::Pane(id) => {
            let Some(pane) = model.state.pane(id) else {
                return fallback(None);
            };
            model.page_glyph(pane, size, color, cx).unwrap_or_else(|| {
                super::tab_activity::pane_glyph(pane, model, size, fallback(Some(pane)))
            })
        }
        Moving::Tab(_) | Moving::Window(_) => {
            let tab = match moving {
                Moving::Window(id) => model.window_tab(id),
                Moving::Tab(tab) => Some(tab),
                Moving::Pane(_) => None,
            };
            let Some(tab) = tab.and_then(|tab| model.tab(tab)) else {
                return fallback(None);
            };
            let shown = tab.displayed_pane(model.state.shown_pane(tab));
            let base = model
                .webview_glyph(tab, size, color, cx)
                .unwrap_or_else(|| fallback(shown));
            super::tab_activity::glyph(tab, model, size, base)
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

fn eased(since: Instant, duration: Duration) -> f32 {
    let elapsed = since.elapsed().as_secs_f32() / duration.as_secs_f32();
    let t = elapsed.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn progress(since: Instant) -> f32 {
    eased(since, GLIDE)
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

fn current_bounds(
    to: Bounds<Pixels>,
    glide: Option<(Bounds<Pixels>, Instant)>,
    indicator: Option<Indicator>,
) -> Bounds<Pixels> {
    let to = indicator.map_or(to, |indicator| indicator.bounds);
    glide.map_or(to, |(from, since)| lerp(from, to, progress(since)))
}

fn lifted_pane(model: &AppModel, gesture: &Gesture, here: WindowId) -> Option<Bounds<Pixels>> {
    let Moving::Pane(pane) = gesture.moving else {
        return None;
    };
    let tab = model.state.pane_tab(pane)?;
    let placement = model
        .layout_drag
        .geometry
        .placement(tab)
        .filter(|placement| placement.window == here)?;
    let layout = &model.tab(tab)?.layout;
    if layout.leaves().len() < 2 {
        return Some(placement.bounds);
    }
    crate::views::splits::drag::pane_bounds(layout, placement, pane)
}

pub(crate) fn overlay(model: &AppModel, window: &Window, cx: &App) -> AnyElement {
    let here = window.window_handle().window_id();
    let Some(gesture) = &model.move_drag.gesture else {
        return div().into_any_element();
    };
    let indicator = gesture
        .indicator
        .filter(|indicator| indicator.window == here);
    let glide = gesture.glide;
    let appeared = gesture.appeared;
    let theme = &model.theme;
    let accent = theme.accent;
    let refused = theme.danger;
    let scrim = theme.fg.opacity(0.05);
    let outline = theme.fg_muted.opacity(0.8);
    let lifted = gesture
        .pointer
        .and_then(|_| lifted_pane(model, gesture, here));
    let occlusions = model.webviews.occlusions.clone();
    let main = here == model.window.window_id();
    let painted = canvas(
        move |_, _, _| {
            if main && let Some(indicator) = indicator {
                occlusions
                    .borrow_mut()
                    .push((None, indicator.bounds.dilate(px(2.0)).into()));
            }
        },
        move |_, (), window, _| {
            if let Some(bounds) = lifted {
                window.paint_quad(gpui::fill(bounds, scrim));
                window.paint_quad(
                    gpui::outline(bounds.inset(px(3.0)), outline, gpui::BorderStyle::Dashed)
                        .border_widths(px(1.5))
                        .corner_radii(px(6.0)),
                );
            }
            let Some(indicator) = indicator else {
                return;
            };
            let bounds = current_bounds(indicator.bounds, glide, Some(indicator));
            let fading = appeared.elapsed() < FADE;
            if fading || glide.is_some_and(|(_, since)| since.elapsed() < GLIDE) {
                window.request_animation_frame();
            }
            let strength = eased(appeared, FADE);
            paint_indicator(
                bounds,
                indicator.mark,
                accent.opacity(strength),
                refused.opacity(strength),
                window,
            );
        },
    )
    .absolute()
    .size_full();
    let viewport = window.viewport_size();
    let chip = gesture
        .pointer
        .filter(|(window, _)| *window == here)
        .filter(|_| !matches!(gesture.moving, Moving::Window(_)))
        .map(|(_, position)| chip(gesture, position, viewport, main, model, cx));
    let cursor = if gesture.target == Some(Target::Refused) {
        CursorStyle::OperationNotAllowed
    } else {
        CursorStyle::ClosedHand
    };
    div()
        .absolute()
        .inset_0()
        .child(painted)
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
            let (bounds, radius) = if mark == Mark::Project {
                (bounds, px(6.0))
            } else {
                (inset(bounds, px(4.0)), px(8.0))
            };
            window.paint_quad(gpui::fill(bounds, accent.opacity(0.16)).corner_radii(radius));
            window.paint_quad(
                gpui::outline(bounds, accent.opacity(0.9), gpui::BorderStyle::Solid)
                    .border_widths(px(1.5))
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
            let bounds = inset(bounds, px(2.0));
            window.paint_quad(gpui::fill(bounds, refused.opacity(0.10)).corner_radii(px(6.0)));
            window.paint_quad(
                gpui::outline(bounds, refused.opacity(0.7), gpui::BorderStyle::Dashed)
                    .border_widths(px(1.5))
                    .corner_radii(px(6.0)),
            );
        }
    }
}

fn inset(bounds: Bounds<Pixels>, by: Pixels) -> Bounds<Pixels> {
    let by = by
        .min(bounds.size.width / 4.0)
        .min(bounds.size.height / 4.0)
        .max(px(0.0));
    bounds.inset(by)
}

enum Side {
    Start(Pixels),
    End(Pixels),
}

fn side(pointer: Pixels, extent: Pixels, viewport: Pixels) -> Side {
    let offset = px(CHIP_OFFSET);
    if pointer + offset + extent <= viewport {
        Side::Start(pointer + offset)
    } else if pointer - offset >= extent {
        Side::End(viewport - pointer + offset)
    } else {
        Side::Start((viewport - extent).max(px(0.0)))
    }
}

fn chip(
    gesture: &Gesture,
    position: Point<Pixels>,
    viewport: Size<Pixels>,
    main: bool,
    model: &AppModel,
    cx: &App,
) -> AnyElement {
    let reach = size(px(ghost::WIDTH), model.metrics.control_large());
    let across = side(position.x, reach.width, viewport.width);
    let down = side(position.y, reach.height, viewport.height);
    let occlusions = model.webviews.occlusions.clone();
    let body = div()
        .relative()
        .child(ghost::chip_body(
            label(gesture.moving, model, cx),
            glyph(gesture.moving, model, cx),
            &model.theme,
            model.metrics,
        ))
        .when(main, |body| {
            body.child(
                canvas(
                    move |bounds, _, _| {
                        occlusions.borrow_mut().push((None, bounds.into()));
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .inset_0(),
            )
        });
    let placed = div().absolute().flex().flex_col();
    let placed = match across {
        Side::Start(left) => placed.left(left),
        Side::End(right) => placed.items_end().right(right),
    };
    let placed = match down {
        Side::Start(top) => placed.top(top),
        Side::End(bottom) => placed.bottom(bottom),
    };
    placed
        .child(body.with_animation(
            "move-drag-chip",
            gpui::Animation::new(FADE),
            Styled::opacity,
        ))
        .into_any_element()
}

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
