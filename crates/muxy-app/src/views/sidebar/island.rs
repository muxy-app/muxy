use gpui::{
    AnyElement, Context, DispatchPhase, InteractiveElement, IntoElement, MouseDownEvent,
    MouseExitEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Styled, Window, canvas,
    div, px,
};
use muxy_ui::webview::Cover;

use crate::model::AppModel;

const EDGE: f32 = 20.0;
const NATIVE_EDGE: f32 = 8.0;

pub(crate) fn layer(
    model: &AppModel,
    window: &Window,
    cx: &Context<AppModel>,
) -> Option<AnyElement> {
    if !model.sidebar_auto_hides() {
        return None;
    }
    let m = model.metrics;
    let top = m.title_bar_height() + px(1.0);
    let margin = m.spacing4();
    let bottom = if model.appearance.status_bar_visible {
        m.status_bar_height()
    } else {
        px(0.0)
    };
    let width = px(model.sidebar_natural_width());
    Some(
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(top)
            .bottom(bottom)
            .child(edge(model, cx))
            .child(reach(model, margin + width + margin))
            .children(card(model, margin, width, window))
            .into_any_element(),
    )
}

fn edge(model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let bounds = model.sidebar_peek.edge.clone();
    let occlusions = model.webviews.occlusions.clone();
    let weak = cx.weak_entity();
    canvas(
        move |mut area, _, _| {
            bounds.set(Some(area));
            area.size.width = area.size.width.min(px(NATIVE_EDGE));
            occlusions.borrow_mut().push((
                None,
                Cover {
                    bounds: area,
                    pointer_only: true,
                    ..Cover::default()
                },
            ));
        },
        move |_, (), window, _| {
            let moving = weak.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = moving.update(cx, |model, cx| {
                    model.track_sidebar_peek(
                        Some(event.position),
                        event.pressed_button.is_some(),
                        cx,
                    );
                });
            });
            let pressing = weak.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = pressing.update(cx, |model, cx| {
                    model.press_outside_sidebar_peek(event.position, cx);
                });
            });
            let releasing = weak.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = releasing.update(cx, |model, cx| {
                    model.track_sidebar_peek(Some(event.position), false, cx);
                });
            });
            let leaving = weak.clone();
            window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let _ = leaving.update(cx, AppModel::sidebar_pointer_left);
            });
        },
    )
    .absolute()
    .left_0()
    .top_0()
    .bottom_0()
    .w(px(EDGE))
    .into_any_element()
}

fn reach(model: &AppModel, width: Pixels) -> AnyElement {
    let bounds = model.sidebar_peek.reach.clone();
    canvas(move |area, _, _| bounds.set(Some(area)), |_, (), _, _| ())
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(width)
        .into_any_element()
}

fn card(model: &AppModel, margin: Pixels, width: Pixels, window: &Window) -> Option<AnyElement> {
    let motion = model.sidebar_peek.motion.as_ref()?;
    let progress = if model.quick.reduce_motion() {
        1.0
    } else {
        motion.sample(window)
    };
    let m = model.metrics;
    let theme = &model.theme;
    let travel = width + margin * 2.0 + m.spacing9();
    let radius = m.radius_xl();
    let shadows = muxy_ui::theme::Elevation::Modal.shadow(theme.bg);
    let occlusions = model.webviews.occlusions.clone();
    let owner = model
        .webviews
        .sidebar
        .as_ref()
        .map(|sidebar| sidebar.surface.view.entity_id());
    let surface = div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(radius)
        .border_1()
        .border_color(theme.border)
        .bg(theme.raised())
        .shadow(shadows.clone())
        .child(model.cached_sidebar());
    Some(
        div()
            .id("sidebar-island")
            .debug_selector(|| "sidebar-island".into())
            .occlude()
            .absolute()
            .top(margin)
            .bottom(margin)
            .left(margin - travel * (1.0 - progress))
            .w(width)
            .child(
                canvas(
                    move |bounds, _, _| {
                        occlusions.borrow_mut().push((
                            owner,
                            Cover {
                                bounds,
                                radius,
                                shadows,
                                pointer_only: false,
                            },
                        ));
                    },
                    |_, (), _, _| (),
                )
                .absolute()
                .inset_0(),
            )
            .child(surface)
            .into_any_element(),
    )
}
