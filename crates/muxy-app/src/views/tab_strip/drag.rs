use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, Div, IntoElement, MouseButton, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Styled, canvas, point, size,
};
use muxy_app_core::{Direction, Project, ProjectId, ProjectStatus, TabId};

use crate::model::AppModel;

#[derive(Default)]
pub(crate) struct TabDragState {
    gesture: Option<TabDrag>,
    /// Tab cells of the titlebar and group strips. Cleared on every render.
    pub(crate) cells: TabBounds,
    /// Each group's strip, under the tab the group shows. Cleared on every
    /// render.
    pub(crate) strips: TabBounds,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Strip,
    Sidebar,
}

/// Where a dragged tab lands when released away from its own strip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TabDrop {
    /// A new group on the `edge` side of the group showing `beside`.
    Split { beside: TabId, edge: Direction },
    /// The group holding `group`, before `before` or after its last tab.
    Into { group: TabId, before: Option<TabId> },
}

struct TabDrag {
    project: ProjectId,
    tab: TabId,
    /// The tab `tab`'s group showed before pressing `tab` selected it.
    previous: Option<TabId>,
    origin: Point<Pixels>,
    source: Source,
    active: bool,
    last_target: Option<TabId>,
    drop: Option<(TabDrop, Bounds<Pixels>)>,
}

/// Share of a group, from each edge, that splits instead of joining it.
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

    fn is_from(&self, source: Source) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|drag| drag.source == source)
    }

    pub(crate) fn end(&mut self) -> bool {
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

    /// Where the dragged tab would land.
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
        let layout_changed = self.layout_drag.cancel();
        let tab_changed = self.tab_drag.pending();
        self.tab_drag.end();
        if tab_changed || layout_changed {
            cx.notify();
        }
        #[cfg(target_os = "macos")]
        if let Some(drag) = &self.window_drag {
            drag.cancel();
        }
    }

    /// Where `tab` would land if dropped at `position`, outside any strip tab.
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
        let (shown, bounds, _) = self
            .layout_drag
            .geometry
            .at(position)
            .filter(|(shown, ..)| visible.contains(shown))?;
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

    /// Joining the group holding `target`, previewed over the group's panes.
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

/// The edge of `bounds` whose zone holds `position`, nearest first.
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

/// The half of `bounds` along `edge`.
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

/// Records where the strip's tabs, `ids` in order, were laid out.
pub(super) fn measure_tabs(cells: Div, ids: Vec<TabId>, targets: TabBounds) -> Div {
    cells.on_children_prepainted(move |bounds, window, _| {
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

fn move_pointer(
    model: &mut AppModel,
    position: Point<Pixels>,
    bounds: &TabBounds,
    cx: &mut Context<AppModel>,
) {
    let pending = model.tab_drag.pending();
    model.tab_drag.cancel_unavailable(
        model.state.current_project(),
        model.overlay.is_some() || model.close_prompt.is_some(),
    );
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
    let project = model.state.current_project();
    let hovered = bounds.borrow().iter().find_map(|(id, bounds)| {
        (*id != tab
            && bounds.contains(&position)
            && project.tabs.iter().any(|candidate| candidate.id == *id))
        .then_some(*id)
    });
    let (target, drop) = match hovered {
        Some(target) if source == Source::Sidebar || project.group_tabs(tab).contains(&target) => {
            (Some(target), None)
        }
        Some(target) => (None, model.join_group(tab, target, Some(target))),
        None => (None, model.tab_drop_at(tab, position)),
    };
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
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = moving.update(cx, |model, cx| {
                    if !model.tab_drag.is_from(source) {
                        return;
                    }
                    if event.pressed_button == Some(MouseButton::Left) {
                        move_pointer(model, event.position, &move_bounds, cx);
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
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                let _ = ending.update(cx, |model, cx| {
                    if !model.tab_drag.is_from(source) {
                        return;
                    }
                    move_pointer(model, event.position, &end_bounds, cx);
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
