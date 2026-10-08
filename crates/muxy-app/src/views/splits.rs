pub(crate) mod drag;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Bounds, Context, CursorStyle, DispatchPhase, HitboxBehavior, Hsla,
    InteractiveElement, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, ParentElement,
    Pixels, Point, SharedString, Styled, Window, canvas, div, point, px, relative,
};
use muxy_app_core::{Axis, Branch, GroupId, Layout, ProjectId, Tab, TabGroup, TabId};

use crate::model::AppModel;

#[derive(Clone, Default)]
pub(crate) struct SplitResizeState(Rc<RefCell<Option<SplitResize>>>);

/// What a divider splits: a tab's panes, or a project's tab groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplitOwner {
    Tab(TabId),
    Groups(ProjectId),
}

#[derive(Clone)]
struct SplitResize {
    owner: SplitOwner,
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

    fn apply(&self, model: &mut AppModel, pointer: Point<Pixels>) -> bool {
        let ratio = self.ratio_at(pointer);
        match self.owner {
            SplitOwner::Tab(tab) => {
                model.visible_tabs().contains(&tab)
                    && model.state.set_ratio(tab, &self.path, ratio).is_ok()
            }
            SplitOwner::Groups(project) => {
                model.state.current_project().id == project
                    && model
                        .state
                        .set_group_ratio(project, &self.path, ratio)
                        .is_ok()
            }
        }
    }
}

impl SplitResizeState {
    pub(crate) fn active(&self) -> bool {
        self.0.borrow().is_some()
    }
    pub(crate) fn end(&self) -> bool {
        self.0.borrow_mut().take().is_some()
    }
    fn resizing(&self, owner: SplitOwner, path: &[Branch]) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|resize| resize.owner == owner && resize.path == path)
    }
    fn cursor(&self) -> Option<CursorStyle> {
        self.0
            .borrow()
            .as_ref()
            .map(|resize| resize_cursor(resize.axis))
    }
}

fn resize_cursor(axis: Axis) -> CursorStyle {
    match axis {
        Axis::Horizontal => CursorStyle::ResizeLeftRight,
        Axis::Vertical => CursorStyle::ResizeUpDown,
    }
}

pub(crate) fn render(
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let tab = model.tab(model.active_tab()?)?;
    let project = model.state.current_project();
    let content = match project.groups() {
        Some(groups) => {
            let width = f32::from(window.viewport_size().width) - model.sidebar_width();
            let widths: HashMap<_, _> = groups
                .layout()
                .rects()
                .into_iter()
                .map(|(group, rect)| (group, width * rect[2]))
                .collect();
            group_node(groups.layout(), Vec::new(), project.id, &widths, model, cx)
        }
        None => tab_content(tab, model, cx),
    };
    Some(
        div()
            .relative()
            .size_full()
            .child(content)
            .child(resize_tracker(model, cx))
            .child(super::tab_strip::drag::track_pointer(
                model.tab_drag.cells.clone(),
                super::tab_strip::drag::Source::Strip,
                cx,
            ))
            .into_any_element(),
    )
}

/// Follows the pointer while a divider is dragged.
fn resize_tracker(model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let state = model.split_resize.clone();
    let weak = cx.weak_entity();
    canvas(
        |_, _, _| (),
        move |_, (), window, _| {
            if let Some(cursor) = state.cursor() {
                window.set_window_cursor_style(cursor);
            }
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
                    if event.pressed_button == Some(MouseButton::Left)
                        && resize.apply(model, event.position)
                    {
                        cx.notify();
                    } else {
                        model.split_resize.end();
                        model.save_split_resize(cx);
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
    .inset_0()
    .into_any_element()
}

/// A tab's panes, recording where they are laid out for pane and tab drops.
fn tab_content(tab: &Tab, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let id = tab.id;
    let content = match tab.zoomed {
        Some(zoomed) => zoomed_frame(zoomed, model, cx),
        None => node(&tab.layout, id, Vec::new(), model, cx),
    };
    let geometry = model.layout_drag.geometry.clone();
    div()
        .relative()
        .size_full()
        .child(content)
        .child(
            canvas(
                move |bounds, window, _| geometry.set(id, bounds, window.scale_factor()),
                |_, (), _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .into_any_element()
}

fn group_node(
    layout: &Layout<GroupId>,
    path: Vec<Branch>,
    project: ProjectId,
    widths: &HashMap<GroupId, f32>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    match layout {
        Layout::Leaf(id) => model
            .state
            .current_project()
            .groups()
            .and_then(|groups| groups.group(*id))
            .and_then(|group| {
                let width = widths.get(id).copied().unwrap_or_default();
                group_element(group, width, model, cx)
            })
            .unwrap_or_else(|| div().size_full().into_any_element()),
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            let mut first_path = path.clone();
            first_path.push(Branch::First);
            let mut second_path = path.clone();
            second_path.push(Branch::Second);
            let first = group_node(first, first_path, project, widths, model, cx);
            let second = group_node(second, second_path, project, widths, model, cx);
            split(
                SplitOwner::Groups(project),
                path,
                *axis,
                *ratio,
                [first, second],
                model,
            )
        }
    }
}

fn group_element(
    group: &TabGroup,
    width: f32,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let tab = model.tab(group.selected())?;
    Some(
        div()
            .debug_selector(move || format!("tab-group-{}", group.id()))
            .flex()
            .flex_col()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .child(super::tab_strip::group_strip(group, width, model, cx))
            .child(div().h(px(1.0)).flex_none().bg(model.theme.border))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(tab_content(tab, model, cx)),
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
    let first = node(first, tab, first_path, model, cx);
    let second = node(second, tab, second_path, model, cx);
    split(
        SplitOwner::Tab(tab),
        path,
        *axis,
        *ratio,
        [first, second],
        model,
    )
}

/// Lays out two children along `axis` with a draggable divider between them.
fn split(
    owner: SplitOwner,
    path: Vec<Branch>,
    axis: Axis,
    ratio: f32,
    [mut first, mut second]: [AnyElement; 2],
    model: &AppModel,
) -> AnyElement {
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let measured = bounds.clone();
    let divider = Rc::new(Cell::new(Bounds::default()));
    let measured_divider = divider.clone();
    let state = model.split_resize.clone();
    let resizing = state.resizing(owner, &path);
    let hoverable = !state.active();
    let (id, selector) = match owner {
        SplitOwner::Tab(tab) => (
            format!("split-{tab}-{path:?}"),
            format!("split-divider-{path:?}"),
        ),
        SplitOwner::Groups(project) => (
            format!("group-split-{project}-{path:?}"),
            format!("group-divider-{path:?}"),
        ),
    };
    let grip = model.metrics.resize_handle_hit_area();
    let hit = div()
        .id(SharedString::from(id))
        .debug_selector(move || selector.clone())
        .absolute()
        .block_mouse_except_scroll()
        .cursor(resize_cursor(axis))
        .child(highlight(divider, resizing, hoverable, model.theme.accent))
        .when(!model.webviews.panes.is_empty(), |hit| {
            hit.child(model.webview_grip(None, resize_cursor(axis)))
        })
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            *state.0.borrow_mut() = Some(SplitResize {
                owner,
                path: path.clone(),
                axis,
                ratio,
                pointer: event.position,
                bounds: bounds.get(),
            });
            window.refresh();
            cx.stop_propagation();
        });
    // Centered on the 1px divider, which starts at `(extent - 1) * ratio`.
    let offset = px(-ratio) - (grip - px(1.0)) * 0.5;
    let hit = match axis {
        Axis::Horizontal => hit
            .left(relative(ratio))
            .ml(offset)
            .top_0()
            .w(grip)
            .h_full(),
        Axis::Vertical => hit
            .top(relative(ratio))
            .mt(offset)
            .left_0()
            .h(grip)
            .w_full(),
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
                    measured_divider.set(divider);
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

/// Paints `divider` in the accent color while it is dragged, or while the
/// pointer is over this element and `hoverable`.
fn highlight(
    divider: Rc<Cell<Bounds<Pixels>>>,
    resizing: bool,
    hoverable: bool,
    accent: Hsla,
) -> AnyElement {
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            let hovered = hitbox.is_hovered(window);
            if resizing || (hoverable && hovered) {
                window.paint_quad(gpui::fill(divider.get(), accent));
            }
            let view = window.current_view();
            window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture && hitbox.is_hovered(window) != hovered {
                    cx.notify(view);
                }
            });
        },
    )
    .absolute()
    .size_full()
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
