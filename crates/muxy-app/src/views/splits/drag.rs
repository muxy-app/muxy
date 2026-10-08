use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Styled, Window, canvas, px,
};
use muxy_app_core::{Layout, PaneId, ProjectId, TabId};

mod target;
pub(crate) use target::DropTarget;

use crate::model::AppModel;

#[derive(Default)]
pub(crate) struct LayoutDragState {
    pub(crate) geometry: TabGeometry,
    pane: Option<PaneDrag>,
    pub(crate) preview: Option<Preview>,
}

/// Where each shown tab's panes were laid out in the last frame, with the
/// scale factor. Cleared on every render.
#[derive(Clone, Debug, Default)]
pub(crate) struct TabGeometry(Rc<RefCell<Vec<Placement>>>);

type Placement = (TabId, Bounds<Pixels>, f32);

impl TabGeometry {
    pub(crate) fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    pub(crate) fn set(&self, tab: TabId, bounds: Bounds<Pixels>, scale: f32) {
        let mut entries = self.0.borrow_mut();
        entries.retain(|(id, ..)| *id != tab);
        entries.push((tab, bounds, scale));
    }

    pub(crate) fn get(&self, tab: TabId) -> Option<(Bounds<Pixels>, f32)> {
        self.0
            .borrow()
            .iter()
            .find(|(id, ..)| *id == tab)
            .map(|(_, bounds, scale)| (*bounds, *scale))
    }

    /// The tab whose panes are under `position`.
    pub(crate) fn at(&self, position: Point<Pixels>) -> Option<Placement> {
        self.0
            .borrow()
            .iter()
            .copied()
            .find(|(_, bounds, _)| bounds.contains(&position))
    }
}

struct PaneDrag {
    project: ProjectId,
    tab: TabId,
    pane: PaneId,
    origin: Point<Pixels>,
    active: bool,
    layout: Layout,
    geometry: (Bounds<Pixels>, f32),
    position: Point<Pixels>,
    hover: Point<Pixels>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DropIntent {
    pub(crate) source: PaneId,
    pub(crate) target: DropTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Preview {
    pub(crate) intent: DropIntent,
    pub(crate) bounds: Bounds<Pixels>,
}

impl LayoutDragState {
    pub(crate) fn active(&self) -> bool {
        self.pane.as_ref().is_some_and(|drag| drag.active)
    }

    pub(crate) fn cancel(&mut self) -> bool {
        let changed = self.pane.take().is_some();
        self.preview.take().is_some() || changed
    }
}

fn pane_rects(
    layout: &Layout,
    bounds: Bounds<Pixels>,
    scale: f32,
) -> Vec<(PaneId, Bounds<Pixels>)> {
    let mut result = Vec::new();
    visit_panes(layout, bounds, scale, &mut |pane, bounds| {
        result.push((pane, bounds));
    });
    result
}

fn visit_panes(
    layout: &Layout,
    bounds: Bounds<Pixels>,
    scale: f32,
    visit: &mut impl FnMut(PaneId, Bounds<Pixels>),
) {
    match layout {
        Layout::Leaf(pane) => visit(*pane, bounds),
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let [a, _, b] = super::split_bounds(bounds, *axis, *ratio, scale);
            visit_panes(first, a, scale, visit);
            visit_panes(second, b, scale, visit);
        }
    }
}

fn hit<T: Copy>(
    rects: &[(T, Bounds<Pixels>)],
    position: Point<Pixels>,
) -> Option<(T, Bounds<Pixels>)> {
    rects.iter().copied().find(|(_, bounds)| {
        bounds.size.width > px(0.0) && bounds.size.height > px(0.0) && bounds.contains(&position)
    })
}

impl AppModel {
    pub(crate) fn layout_drag_pending(&self) -> bool {
        self.tab_drag.pending() || self.layout_drag.pane.is_some()
    }

    pub(crate) fn cancel_layout_drag(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.layout_drag_pending() {
            return false;
        }
        self.cancel_titlebar_drag(cx);
        true
    }

    pub(crate) fn cancel_drag_on_escape(
        &mut self,
        event: &gpui::KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" && self.cancel_layout_drag(cx) {
            cx.stop_propagation();
        }
    }

    fn drag_blocked(&self) -> bool {
        self.overlay.is_some()
            || self.close_prompt.is_some()
            || self.split_resize.active()
            || self.sidebar_resize.is_some()
            || self.voice.view.is_some()
            || self.voice.closing.is_some()
            || (self.composer.view.is_some()
                && self.settings.composer.presentation
                    == muxy_app_core::settings::ComposerPresentation::Floating)
    }

    pub(crate) fn validate_layout_drag(&mut self, cx: &mut Context<Self>) {
        let unavailable = self.drag_blocked()
            || self.layout_drag.pane.as_ref().is_some_and(|drag| {
                drag.project != self.state.current_project().id
                    || !self.visible_panes().contains(&drag.pane)
                    || self.layout_drag.geometry.get(drag.tab) != Some(drag.geometry)
                    || self
                        .tab(drag.tab)
                        .is_none_or(|tab| tab.zoomed.is_some() || tab.layout != drag.layout)
            });
        if unavailable && self.layout_drag.cancel() {
            cx.notify();
        }
        if !self.layout_drag.active() {
            self.set_drop_preview(None, cx);
        }
    }

    fn set_drop_preview(&mut self, preview: Option<Preview>, cx: &mut Context<Self>) {
        if self.layout_drag.preview != preview {
            self.layout_drag.preview = preview;
            cx.notify();
        }
    }

    fn begin_pane_drag(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || !event.modifiers.platform || self.drag_blocked() {
            return;
        }
        let Some((tab, bounds, scale)) = self
            .layout_drag
            .geometry
            .at(event.position)
            .filter(|(tab, ..)| self.visible_tabs().contains(tab))
        else {
            return;
        };
        let Some(descriptor) = self
            .tab(tab)
            .filter(|tab| tab.zoomed.is_none() && tab.panes.len() > 1)
        else {
            return;
        };
        let Some((pane, _)) = hit(
            &pane_rects(&descriptor.layout, bounds, scale),
            event.position,
        ) else {
            return;
        };
        let layout = descriptor.layout.clone();
        self.layout_drag.cancel();
        self.tab_drag.end();
        self.focus_pane(pane, cx);
        self.focus_active(window, cx);
        self.layout_drag.pane = Some(PaneDrag {
            project: self.state.current_project().id,
            tab,
            pane,
            origin: event.position,
            active: false,
            layout,
            geometry: (bounds, scale),
            position: event.position,
            hover: event.position,
        });
        cx.notify();
        cx.stop_propagation();
    }

    fn move_pane_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.validate_layout_drag(cx);
        let Some(drag) = &mut self.layout_drag.pane else {
            return;
        };
        if !drag.active {
            if (position - drag.origin).magnitude() < 4.0 {
                return;
            }
            drag.active = true;
            cx.notify();
        }
        drag.position = position;
        let (bounds, scale) = drag.geometry;
        let target = target::target_at(&drag.layout, bounds, scale, position);
        if let Some(preview) = &self.layout_drag.preview {
            if target.as_ref() == Some(&preview.intent.target) {
                drag.hover = position;
                return;
            }
            if target.is_some() && (position - drag.hover).magnitude() <= 4.0 {
                return;
            }
        }
        drag.hover = position;
        let preview = target.and_then(|target| {
            let result = target.apply(&drag.layout, drag.pane)?;
            if result == drag.layout {
                return None;
            }
            let (_, bounds) = pane_rects(&result, bounds, scale)
                .into_iter()
                .find(|(pane, _)| *pane == drag.pane)?;
            Some(Preview {
                intent: DropIntent {
                    source: drag.pane,
                    target,
                },
                bounds,
            })
        });
        self.set_drop_preview(preview, cx);
    }

    fn release_pane_drag(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        self.validate_layout_drag(cx);
        let Some(drag) = &self.layout_drag.pane else {
            return;
        };
        if !drag.active {
            self.move_pane_drag(event.position, cx);
        } else if !target::within(event.position, drag.geometry.0)
            || (event.position - drag.position).magnitude() > 8.0
        {
            self.set_drop_preview(None, cx);
        }
        if let Some(drag) = self.layout_drag.pane.take() {
            if drag.active {
                self.finish_layout_drop(cx);
            } else if let Some(pane) = self.grids.get(&drag.pane) {
                pane.view
                    .update(cx, |pane, cx| pane.command_click(event.position, cx));
            }
            cx.notify();
            cx.stop_propagation();
        }
    }

    pub(crate) fn finish_layout_drop(&mut self, cx: &mut Context<Self>) {
        if let Some(preview) = self.layout_drag.preview.take() {
            self.apply_layout_drop(preview.intent, cx);
        }
    }
}

pub(crate) fn track_pointer(cx: &Context<AppModel>) -> AnyElement {
    let weak = cx.weak_entity();
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            let starting = weak.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture {
                    let _ =
                        starting.update(cx, |model, cx| model.begin_pane_drag(event, window, cx));
                }
            });
            let moving = weak.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = moving.update(cx, |model, cx| {
                    if model.layout_drag.pane.is_none() {
                        return;
                    }
                    if event.pressed_button != Some(MouseButton::Left) {
                        if model.layout_drag.cancel() {
                            cx.notify();
                        }
                        return;
                    }
                    model.move_pane_drag(event.position, cx);
                    cx.stop_propagation();
                });
            });
            let ending = weak.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                let _ = ending.update(cx, |model, cx| {
                    if model.layout_drag.pane.is_none() {
                        return;
                    }
                    model.release_pane_drag(event, cx);
                });
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

/// Paints the drop preview of a pane or tab drag.
pub(crate) fn overlay(model: &AppModel) -> AnyElement {
    let pane_preview = model
        .layout_drag
        .preview
        .as_ref()
        .map(|preview| preview.bounds);
    let expected = model
        .layout_drag
        .pane
        .as_ref()
        .map(|drag| (drag.tab, drag.geometry));
    let geometry = model.layout_drag.geometry.clone();
    let tab_preview = model.tab_drag.preview();
    let preview = move || {
        expected
            .filter(|(tab, expected)| geometry.get(*tab) == Some(*expected))
            .and(pane_preview)
            .or(tab_preview)
    };
    let painted = preview.clone();
    let accent = model.theme.accent;
    let fill = accent.opacity(0.18);
    let occlusions = model.webviews.occlusions.clone();
    canvas(
        move |_, _, _| {
            if let Some(bounds) = preview() {
                occlusions.borrow_mut().push((None, bounds));
            }
        },
        move |_, (), window, _| {
            if let Some(bounds) = painted() {
                window.paint_quad(gpui::fill(bounds, fill));
                window.paint_quad(
                    gpui::outline(bounds, accent.opacity(0.8), gpui::BorderStyle::Solid)
                        .border_widths(px(2.0)),
                );
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}
