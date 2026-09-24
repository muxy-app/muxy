use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, Context, DispatchPhase, InteractiveElement, IntoElement, MouseButton,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, SharedString, Styled, canvas, div,
    point, px, relative,
};
use muxy_app_core::{Axis, Branch, Layout, TabId};

use crate::model::AppModel;

#[derive(Clone, Default)]
pub(crate) struct SplitResizeState(Rc<RefCell<Option<SplitResize>>>);

#[derive(Clone)]
struct SplitResize {
    tab: TabId,
    path: Vec<Branch>,
    axis: Axis,
    ratio: f32,
    pointer: Point<Pixels>,
    bounds: Bounds<Pixels>,
}

impl SplitResize {
    fn ratio_at(&self, pointer: Point<Pixels>) -> f32 {
        let (delta, extent) = match self.axis {
            Axis::Horizontal => (pointer.x - self.pointer.x, self.bounds.size.width),
            Axis::Vertical => (pointer.y - self.pointer.y, self.bounds.size.height),
        };
        (self.ratio + f32::from(delta) / (f32::from(extent) - 1.0).max(1.0)).clamp(0.15, 0.85)
    }
}

impl SplitResizeState {
    pub(crate) fn active(&self) -> bool {
        self.0.borrow().is_some()
    }
    pub(crate) fn end(&self) -> bool {
        self.0.borrow_mut().take().is_some()
    }
}

pub(crate) fn render(model: &AppModel, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    let tab = model
        .state
        .current_project()
        .tabs
        .iter()
        .find(|tab| Some(tab.id) == model.active_tab())?;
    if let Some(zoomed) = tab.zoomed {
        return Some(zoomed_frame(zoomed, model, cx));
    }
    let content = node(&tab.layout, tab.id, Vec::new(), model, cx);
    let state = model.split_resize.clone();
    let weak = cx.weak_entity();
    Some(
        div()
            .relative()
            .size_full()
            .child(content)
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, (), window, _| {
                        let state_move = state.clone();
                        let model_move = weak.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let Some(resize) = state_move.0.borrow().clone() else {
                                return;
                            };
                            let _ = model_move.update(cx, |model, cx| {
                                if model.active_tab() == Some(resize.tab)
                                    && model
                                        .state
                                        .set_ratio(
                                            resize.tab,
                                            &resize.path,
                                            resize.ratio_at(event.position),
                                        )
                                        .is_ok()
                                {
                                    cx.notify();
                                } else {
                                    model.split_resize.end();
                                }
                            });
                            cx.stop_propagation();
                        });
                        let state_end = state.clone();
                        let model_end = weak.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Capture
                                && event.button == MouseButton::Left
                                && state_end.end()
                            {
                                let _ = model_end.update(cx, AppModel::save_split_resize);
                                cx.stop_propagation();
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
            .into_any_element(),
    )
}

fn zoomed_frame(
    pane: muxy_app_core::PaneId,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let radius = model.metrics.radius_lg();
    let background = model.theme.bg;
    div()
        .debug_selector(|| "zoomed-pane-frame".into())
        .relative()
        .flex()
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .border(model.metrics.spacing7())
        .border_color(background)
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    window.paint_quad(
                        gpui::outline(bounds.dilate(radius), background, gpui::BorderStyle::Solid)
                            .corner_radii(radius * 2.0)
                            .border_widths(radius + px(1.0)),
                    );
                },
            )
            .absolute()
            .size_full(),
        )
        .child(
            div()
                .flex()
                .size_full()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .rounded(radius)
                .border(px(1.0))
                .border_color(model.theme.border_solid())
                .shadow(muxy_ui::theme::Elevation::Elevated.shadow(model.theme.bg))
                .overflow_hidden()
                .child(pane_element(pane, model, cx)),
        )
        .into_any_element()
}

fn node(
    layout: &Layout,
    tab: TabId,
    path: Vec<Branch>,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let (axis, ratio, first, second) = match layout {
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => (axis, ratio, first, second),
        Layout::Leaf(id) => {
            return pane_element(*id, model, cx);
        }
    };
    let mut first_path = path.clone();
    first_path.push(Branch::First);
    let mut second_path = path.clone();
    second_path.push(Branch::Second);
    let mut first = node(first, tab, first_path, model, cx);
    let mut second = node(second, tab, second_path, model, cx);
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let measured = bounds.clone();
    let state = model.split_resize.clone();
    let axis = *axis;
    let ratio = *ratio;
    let selector = format!("split-divider-{path:?}");
    let hit = div()
        .id(SharedString::from(format!("split-{tab}-{path:?}")))
        .debug_selector(move || selector.clone())
        .absolute()
        .on_mouse_down(MouseButton::Left, move |event, _, cx| {
            *state.0.borrow_mut() = Some(SplitResize {
                tab,
                path: path.clone(),
                axis,
                ratio,
                pointer: event.position,
                bounds: bounds.get(),
            });
            cx.stop_propagation();
        });
    let hit = match axis {
        Axis::Horizontal => hit
            .left(relative(ratio))
            .ml(px(-ratio - 2.5))
            .top_0()
            .w(px(6.0))
            .h_full()
            .cursor_ew_resize(),
        Axis::Vertical => hit
            .top(relative(ratio))
            .mt(px(-ratio - 2.5))
            .left_0()
            .h(px(6.0))
            .w_full()
            .cursor_ns_resize(),
    };
    let border = model.theme.border_solid();
    div()
        .relative()
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .child(
            canvas(
                move |bounds, window, cx| {
                    measured.set(bounds);
                    let [first_bounds, divider, second_bounds] =
                        split_bounds(bounds, axis, ratio, window.scale_factor());
                    first.layout_as_root(first_bounds.size.into(), window, cx);
                    first.prepaint_at(first_bounds.origin, window, cx);
                    second.layout_as_root(second_bounds.size.into(), window, cx);
                    second.prepaint_at(second_bounds.origin, window, cx);
                    (first, divider, second)
                },
                move |_, (mut first, divider, mut second), window, cx| {
                    first.paint(window, cx);
                    window.paint_quad(gpui::fill(divider, border));
                    second.paint(window, cx);
                },
            )
            .size_full(),
        )
        .child(hit)
        .into_any_element()
}

fn split_bounds(
    bounds: Bounds<Pixels>,
    axis: Axis,
    ratio: f32,
    scale_factor: f32,
) -> [Bounds<Pixels>; 3] {
    let extent = match axis {
        Axis::Horizontal => bounds.size.width,
        Axis::Vertical => bounds.size.height,
    };
    let available = (extent - px(1.0)).max(px(0.0));
    let first =
        ((available * ratio * scale_factor).round() / scale_factor).clamp(px(0.0), available);
    let second = first + extent.min(px(1.0));
    let (first_end, divider_start, divider_end, second_start) = match axis {
        Axis::Horizontal => (
            point(bounds.left() + first, bounds.bottom()),
            point(bounds.left() + first, bounds.top()),
            point(bounds.left() + second, bounds.bottom()),
            point(bounds.left() + second, bounds.top()),
        ),
        Axis::Vertical => (
            point(bounds.right(), bounds.top() + first),
            point(bounds.left(), bounds.top() + first),
            point(bounds.right(), bounds.top() + second),
            point(bounds.left(), bounds.top() + second),
        ),
    };
    [
        Bounds::from_corners(bounds.origin, first_end),
        Bounds::from_corners(divider_start, divider_end),
        Bounds::from_corners(second_start, bounds.bottom_right()),
    ]
}

fn pane_element(id: muxy_app_core::PaneId, model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    if let Some(surface) = model.webviews.panes.get(&id) {
        return surface.view.clone().into_any_element();
    }
    if let Some(descriptor) = model
        .state
        .projects()
        .iter()
        .flat_map(|p| &p.tabs)
        .flat_map(|t| &t.panes)
        .find_map(|pane| match &pane.content {
            muxy_app_core::PaneContent::Webview(descriptor) if pane.id == id => Some(descriptor),
            _ => None,
        })
    {
        return div()
            .size_full()
            .id(SharedString::from(format!("webview-placeholder-{id}")))
            .debug_selector(|| "webview-placeholder".into())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |model, _, window, cx| {
                    model.focus_pane(id, cx);
                    model.focus_active(window, cx);
                }),
            )
            .child(super::webview::placeholder(
                &descriptor.owner,
                &descriptor.kind,
                &model.theme,
                model.metrics,
            ))
            .into_any_element();
    }
    let Some(pane) = model.grids.get(&id) else {
        return div().size_full().into_any_element();
    };
    pane.element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size};

    #[test]
    fn split_bounds_tile_the_parent_on_the_pixel_grid() {
        for scale in [1.0, 2.0] {
            for axis in [Axis::Horizontal, Axis::Vertical] {
                for extent in [0.0, 1.0, 601.0, 602.0] {
                    let extent = px(extent / scale);
                    let outer = Bounds::new(point(px(20.0), px(30.0)), size(extent, extent));
                    for ratio in [0.15, 0.37, 0.5, 0.61, 0.85] {
                        let [first, divider, second] = split_bounds(outer, axis, ratio, scale);
                        assert_eq!(first.origin, outer.origin);
                        assert_eq!(second.bottom_right(), outer.bottom_right());
                        match axis {
                            Axis::Horizontal => {
                                assert_eq!(first.right(), divider.left());
                                assert_eq!(divider.right(), second.left());
                                assert_eq!(divider.size.width, extent.min(px(1.0)));
                            }
                            Axis::Vertical => {
                                assert_eq!(first.bottom(), divider.top());
                                assert_eq!(divider.bottom(), second.top());
                                assert_eq!(divider.size.height, extent.min(px(1.0)));
                            }
                        }
                        for bounds in [first, divider, second] {
                            assert!(bounds.size.width >= px(0.0) && bounds.size.height >= px(0.0));
                            for edge in
                                [bounds.left(), bounds.right(), bounds.top(), bounds.bottom()]
                            {
                                assert_eq!(edge * scale, (edge * scale).round());
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn divider_uses_parent_pixels_retains_grab_offset_and_clamps() {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let resize = SplitResize {
                tab: TabId::new(),
                path: vec![],
                axis,
                ratio: 0.5,
                pointer: point(px(500.0), px(300.0)),
                bounds: Bounds::new(point(px(200.0), px(100.0)), size(px(601.0), px(401.0))),
            };
            assert!((resize.ratio_at(resize.pointer) - 0.5).abs() < f32::EPSILON);
            assert!((resize.ratio_at(point(px(560.0), px(340.0))) - 0.6).abs() < f32::EPSILON);
            assert!((resize.ratio_at(point(px(-1000.0), px(-1000.0))) - 0.15).abs() < f32::EPSILON);
            assert!((resize.ratio_at(point(px(2000.0), px(2000.0))) - 0.85).abs() < f32::EPSILON);
        }
    }
}
