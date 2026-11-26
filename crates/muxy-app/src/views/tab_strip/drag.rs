use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, Div, IntoElement, MouseButton, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Styled, Window, canvas, point, px, size,
};
use muxy_app_core::{Axis, Direction, Project, ProjectId, ProjectStatus, TabId};

use crate::model::AppModel;
use crate::views::move_drag::Moving;

#[derive(Default)]
pub(crate) struct TabDragState {
    gesture: Option<TabDrag>,
    pub(crate) cells: TabBounds,
    pub(crate) strips: TabBounds,
    pub(crate) slides: Slides,
}

const SLIDE: Duration = Duration::from_millis(160);

#[derive(Default)]
struct Lane {
    moving: HashMap<TabId, (Pixels, Instant)>,
    layout: HashMap<TabId, Pixels>,
}

#[derive(Default)]
struct Lanes {
    horizontal: Lane,
    vertical: Lane,
}

impl Lanes {
    fn lane(&mut self, axis: Axis) -> &mut Lane {
        match axis {
            Axis::Horizontal => &mut self.horizontal,
            Axis::Vertical => &mut self.vertical,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Slides(Rc<RefCell<Lanes>>);

impl Slides {
    pub(crate) fn offset(&self, tab: TabId, axis: Axis) -> Pixels {
        self.0
            .borrow_mut()
            .lane(axis)
            .moving
            .get(&tab)
            .map_or(px(0.0), |(from, since)| sliding(*from, *since))
    }

    pub(crate) fn measure(
        &self,
        axis: Axis,
        tabs: &[TabId],
        bounds: &[Bounds<Pixels>],
        window: &Window,
    ) {
        let mut lanes = self.0.borrow_mut();
        let lane = lanes.lane(axis);
        for (tab, bounds) in tabs.iter().zip(bounds) {
            let start = match axis {
                Axis::Horizontal => bounds.left(),
                Axis::Vertical => bounds.top(),
            };
            lane.layout.insert(*tab, start);
        }
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let lane = lanes.lane(axis);
            lane.moving.retain(|_, (_, since)| since.elapsed() < SLIDE);
            if !lane.moving.is_empty() {
                window.request_animation_frame();
            }
        }
    }

    fn shift(&self, order: &[TabId], axis: Axis) {
        let mut lanes = self.0.borrow_mut();
        let lane = lanes.lane(axis);
        let starts: Vec<_> = order
            .iter()
            .filter_map(|tab| lane.layout.get(tab).copied())
            .collect();
        if starts.len() != order.len() {
            return;
        }
        let mut slots = starts.clone();
        slots.sort_by(|a, b| f32::from(*a).total_cmp(&f32::from(*b)));
        let now = Instant::now();
        for ((tab, start), slot) in order.iter().zip(starts).zip(slots) {
            let shown = start
                + lane
                    .moving
                    .get(tab)
                    .map_or(px(0.0), |(from, since)| sliding(*from, *since));
            let from = shown - slot;
            if f32::from(from).abs() >= 0.5 {
                lane.moving.insert(*tab, (from, now));
            } else {
                lane.moving.remove(tab);
            }
        }
    }

    pub(crate) fn clear(&self) {
        *self.0.borrow_mut() = Lanes::default();
    }
}

fn sliding(from: Pixels, since: Instant) -> Pixels {
    let t = (since.elapsed().as_secs_f32() / SLIDE.as_secs_f32()).clamp(0.0, 1.0);
    from * (1.0 - t).powi(3)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Strip,
    Sidebar,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TabDrop {
    Split { beside: TabId, edge: Direction },
    Into { group: TabId, before: Option<TabId> },
}

struct TabDrag {
    project: ProjectId,
    tab: TabId,
    previous: Option<TabId>,
    origin: Point<Pixels>,
    source: Source,
    active: bool,
    last_target: Option<TabId>,
    drop: Option<(TabDrop, Bounds<Pixels>)>,
}

const EDGE_ZONE: f32 = 0.3;

impl TabDragState {
    pub(crate) fn begin(
        &mut self,
        project: ProjectId,
        tab: TabId,
        previous: Option<TabId>,
        origin: Point<Pixels>,
        source: Source,
    ) {
        self.gesture = Some(TabDrag {
            project,
            tab,
            previous,
            origin,
            source,
            active: false,
            last_target: None,
            drop: None,
        });
    }

    pub(crate) fn pending(&self) -> bool {
        self.gesture.is_some()
    }

    pub(crate) fn lifted(&self) -> Option<TabId> {
        self.gesture
            .as_ref()
            .filter(|drag| drag.active)
            .map(|drag| drag.tab)
    }

    fn is_from(&self, source: Source) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|drag| drag.source == source)
    }

    pub(crate) fn end(&mut self) -> bool {
        self.slides.clear();
        self.gesture.take().is_some_and(|drag| drag.active)
    }

    pub(crate) fn cancel_unavailable(&mut self, project: &Project, blocked: bool) {
        if blocked
            || self.gesture.as_ref().is_some_and(|drag| {
                drag.project != project.id
                    || project.status() == ProjectStatus::Missing
                    || !project.tabs.iter().any(|tab| tab.id == drag.tab)
            })
        {
            self.end();
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.gesture.as_ref().is_some_and(|drag| drag.active)
    }

    #[cfg(test)]
    pub(crate) fn preview(&self) -> Option<Bounds<Pixels>> {
        self.gesture
            .as_ref()
            .filter(|drag| drag.active)
            .and_then(|drag| drag.drop)
            .map(|(_, bounds)| bounds)
    }

    pub(crate) fn clear_bounds(&self) {
        self.cells.borrow_mut().clear();
        self.strips.borrow_mut().clear();
    }
}

pub(crate) type TabBounds = Rc<RefCell<Vec<(TabId, Bounds<Pixels>)>>>;

impl AppModel {
    pub(crate) fn begin_tab_drag(
        &mut self,
        tab: TabId,
        position: Point<Pixels>,
        source: Source,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self
            .state
            .projects()
            .iter()
            .find(|project| project.tabs.iter().any(|candidate| candidate.id == tab))
        else {
            return;
        };
        let previous = match project.groups() {
            Some(groups) => groups.group_of(tab).map(muxy_app_core::TabGroup::selected),
            None => self.state.window().selected_tab.get(&project.id).copied(),
        };
        self.tab_drag
            .begin(project.id, tab, previous, position, source);
        cx.notify();
    }

    pub(crate) fn cancel_titlebar_drag(&mut self, cx: &mut Context<Self>) {
        self.cancel_tab_and_layout_drags(cx);
        #[cfg(test)]
        {
            self.window_move_cancels += 1;
        }
        #[cfg(target_os = "macos")]
        if let Some(drag) = &self.window_drag {
            drag.cancel();
        }
    }

    pub(crate) fn cancel_tab_and_layout_drags(&mut self, cx: &mut Context<Self>) {
        self.cancel_move(cx);
        let layout_changed = self.layout_drag.cancel();
        let tab_changed = self.tab_drag.pending();
        self.tab_drag.end();
        if tab_changed || layout_changed {
            cx.notify();
        }
    }

    fn tab_drop_at(
        &self,
        tab: TabId,
        position: Point<Pixels>,
    ) -> Option<(TabDrop, Bounds<Pixels>)> {
        let project = self.state.current_project();
        let visible = self.visible_tabs();
        let siblings = project.group_tabs(tab);
        let strip = self
            .tab_drag
            .strips
            .borrow()
            .iter()
            .find(|(shown, bounds)| visible.contains(shown) && bounds.contains(&position))
            .map(|(shown, _)| *shown);
        if let Some(shown) = strip {
            return self.join_group(tab, shown, None);
        }
        let crate::views::splits::drag::Placement {
            tab: shown, bounds, ..
        } = self
            .layout_drag
            .geometry
            .at(self.window.window_id(), position)
            .filter(|placement| visible.contains(&placement.tab))?;
        match edge_at(position, bounds) {
            None => self.join_group(tab, shown, None),
            Some(edge) => (!siblings.contains(&shown) || siblings.len() > 1).then(|| {
                (
                    TabDrop::Split {
                        beside: shown,
                        edge,
                    },
                    half(bounds, edge),
                )
            }),
        }
    }

    fn join_group(
        &self,
        tab: TabId,
        target: TabId,
        before: Option<TabId>,
    ) -> Option<(TabDrop, Bounds<Pixels>)> {
        let project = self.state.current_project();
        if project.group_tabs(tab).contains(&target) {
            return None;
        }
        let shown = project
            .groups()
            .and_then(|groups| groups.group_of(target))
            .map_or(target, muxy_app_core::TabGroup::selected);
        let (bounds, _) = self.layout_drag.geometry.get(shown)?;
        Some((
            TabDrop::Into {
                group: target,
                before,
            },
            bounds,
        ))
    }
}

fn edge_at(position: Point<Pixels>, bounds: Bounds<Pixels>) -> Option<Direction> {
    let width = f32::from(bounds.size.width).max(1.0);
    let height = f32::from(bounds.size.height).max(1.0);
    [
        (
            Direction::Left,
            f32::from(position.x - bounds.left()) / width,
        ),
        (
            Direction::Right,
            f32::from(bounds.right() - position.x) / width,
        ),
        (Direction::Up, f32::from(position.y - bounds.top()) / height),
        (
            Direction::Down,
            f32::from(bounds.bottom() - position.y) / height,
        ),
    ]
    .into_iter()
    .filter(|(_, distance)| *distance < EDGE_ZONE)
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .map(|(edge, _)| edge)
}

fn half(bounds: Bounds<Pixels>, edge: Direction) -> Bounds<Pixels> {
    let width = bounds.size.width / 2.0;
    let height = bounds.size.height / 2.0;
    match edge {
        Direction::Left => Bounds::new(bounds.origin, size(width, bounds.size.height)),
        Direction::Right => Bounds::new(
            point(bounds.left() + width, bounds.top()),
            size(width, bounds.size.height),
        ),
        Direction::Up => Bounds::new(bounds.origin, size(bounds.size.width, height)),
        Direction::Down => Bounds::new(
            point(bounds.left(), bounds.top() + height),
            size(bounds.size.width, height),
        ),
    }
}

pub(super) fn measure_tabs(cells: Div, ids: Vec<TabId>, targets: TabBounds, slides: Slides) -> Div {
    cells.on_children_prepainted(move |bounds, window, _| {
        slides.measure(Axis::Horizontal, &ids, &bounds, window);
        let clip = window.content_mask().bounds;
        let mut targets = targets.borrow_mut();
        targets.retain(|(id, _)| !ids.contains(id));
        targets.extend(
            ids.iter()
                .zip(bounds)
                .map(|(id, bounds)| (*id, bounds.intersect(&clip))),
        );
    })
}

fn crossed(
    position: Point<Pixels>,
    own: Bounds<Pixels>,
    other: Bounds<Pixels>,
    axis: Axis,
) -> bool {
    match axis {
        Axis::Horizontal if other.left() >= own.left() => position.x >= other.center().x,
        Axis::Horizontal => position.x <= other.center().x,
        Axis::Vertical if other.top() >= own.top() => position.y >= other.center().y,
        Axis::Vertical => position.y <= other.center().y,
    }
}

fn move_pointer(
    model: &mut AppModel,
    position: Point<Pixels>,
    bounds: &TabBounds,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) {
    let pending = model.tab_drag.pending();
    let covered = model.coverage().modal();
    model
        .tab_drag
        .cancel_unavailable(model.state.current_project(), covered);
    let Some(drag) = &mut model.tab_drag.gesture else {
        if pending {
            cx.notify();
        }
        return;
    };
    if !drag.active {
        if (position - drag.origin).magnitude() < 4.0 {
            return;
        }
        drag.active = true;
        cx.notify();
    }
    let (tab, source, last_target, previous) = (drag.tab, drag.source, drag.last_target, drag.drop);
    let axis = match source {
        Source::Strip => Axis::Horizontal,
        Source::Sidebar => Axis::Vertical,
    };
    let project = model.state.current_project();
    let snapshot = bounds.borrow().clone();
    let own = snapshot
        .iter()
        .find(|(id, _)| *id == tab)
        .map(|(_, bounds)| *bounds);
    let hovered = snapshot.iter().find_map(|(id, bounds)| {
        (*id != tab
            && bounds.contains(&position)
            && project.tabs.iter().any(|candidate| candidate.id == *id))
        .then_some((*id, *bounds))
    });
    let (target, drop, holding) = match hovered {
        Some((target, other))
            if source == Source::Sidebar || project.group_tabs(tab).contains(&target) =>
        {
            let reached = own.is_none_or(|own| crossed(position, own, other, axis));
            (reached.then_some(target), None, true)
        }
        Some((target, _)) => (None, model.join_group(tab, target, Some(target)), false),
        None => (None, model.tab_drop_at(tab, position), false),
    };
    let over_self = own.is_some_and(|own| own.contains(&position));
    if holding || over_self || drop.is_some() {
        let landing = target.and(hovered.map(|(_, bounds)| bounds));
        let local = drop.map(|(_, bounds)| bounds).or(landing).or(own);
        model.hold_move(Moving::Tab(tab), window, position, local, cx);
    } else {
        model.track_move(Moving::Tab(tab), window, position, cx);
    }
    if let Some(drag) = &mut model.tab_drag.gesture {
        drag.last_target = target;
        drag.drop = drop;
    }
    if drop != previous {
        cx.notify();
    }
    if target != last_target
        && let Some(target) = target
    {
        if source == Source::Sidebar
            && model.appearance.layout == muxy_app_core::settings::AppLayout::AgentsFocused
        {
            model.move_agent_tab(tab, target, cx);
        } else {
            model.move_tab(tab, target, cx);
        }
        let project = model.state.current_project();
        let order: Vec<_> = match source {
            Source::Strip => project.group_tabs(tab),
            Source::Sidebar => model.sidebar_tabs(project).map(|tab| tab.id).collect(),
        };
        model.tab_drag.slides.shift(&order, axis);
    }
}

pub(crate) fn track_pointer(
    bounds: TabBounds,
    source: Source,
    cx: &Context<AppModel>,
) -> AnyElement {
    let weak = cx.weak_entity();
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let moving = weak.clone();
            let move_bounds = bounds.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = moving.update(cx, |model, cx| {
                    if !model.tab_drag.is_from(source) {
                        return;
                    }
                    if event.pressed_button == Some(MouseButton::Left) {
                        move_pointer(model, event.position, &move_bounds, window, cx);
                        if model.tab_drag.is_active() {
                            cx.stop_propagation();
                        }
                    } else {
                        model.cancel_titlebar_drag(cx);
                    }
                });
            });
            let ending = weak.clone();
            let end_bounds = bounds.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                let _ = ending.update(cx, |model, cx| {
                    if !model.tab_drag.is_from(source) {
                        return;
                    }
                    move_pointer(model, event.position, &end_bounds, window, cx);
                    if model.move_drag.target().is_some() {
                        model.tab_drag.end();
                        model.finish_move(cx);
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    model.cancel_move(cx);
                    let drop = model
                        .tab_drag
                        .gesture
                        .as_ref()
                        .filter(|drag| drag.active)
                        .and_then(|drag| Some((drag.tab, drag.drop?.0, drag.previous)));
                    if model.tab_drag.end() {
                        cx.stop_propagation();
                    }
                    if let Some((tab, drop, previous)) = drop {
                        model.drop_tab(tab, drop, previous, cx);
                    }
                    cx.notify();
                });
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}
