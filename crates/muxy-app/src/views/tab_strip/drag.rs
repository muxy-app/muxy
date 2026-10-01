use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, Div, IntoElement, MouseButton, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Styled, canvas,
};
use muxy_app_core::{Project, ProjectId, ProjectStatus, TabId};

use crate::model::AppModel;

#[derive(Default)]
pub(crate) struct TabDragState {
    gesture: Option<TabDrag>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Titlebar,
    Sidebar,
}

struct TabDrag {
    project: ProjectId,
    tab: TabId,
    origin: Point<Pixels>,
    source: Source,
    active: bool,
    last_target: Option<TabId>,
}

impl TabDragState {
    pub(crate) fn begin(
        &mut self,
        project: ProjectId,
        tab: TabId,
        origin: Point<Pixels>,
        source: Source,
    ) {
        self.gesture = Some(TabDrag {
            project,
            tab,
            origin,
            source,
            active: false,
            last_target: None,
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
        self.tab_drag
            .begin(self.state.current_project().id, tab, position, source);
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
}

pub(super) fn measure_tabs(cells: Div, targets: TabBounds, model: &AppModel) -> Div {
    let ids: Vec<_> = model
        .state
        .current_project()
        .tabs
        .iter()
        .map(|tab| tab.id)
        .collect();
    cells.on_children_prepainted(move |bounds, window, _| {
        let clip = window.content_mask().bounds;
        *targets.borrow_mut() = ids
            .iter()
            .zip(bounds)
            .map(|(id, bounds)| (*id, bounds.intersect(&clip)))
            .collect();
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
    let target = bounds
        .borrow()
        .iter()
        .find_map(|(id, bounds)| (*id != drag.tab && bounds.contains(&position)).then_some(*id));
    if target != drag.last_target {
        drag.last_target = target;
        let tab = drag.tab;
        if let Some(target) = target {
            if drag.source == Source::Sidebar
                && model.appearance.layout == muxy_app_core::settings::AppLayout::AgentsFocused
            {
                model.move_agent_tab(tab, target, cx);
            } else {
                model.move_tab(tab, target, cx);
            }
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
                    if model.tab_drag.end() {
                        cx.stop_propagation();
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
