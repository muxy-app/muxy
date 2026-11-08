//! The layer of panels floating over the window, back to front.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, DispatchPhase, InteractiveElement, IntoElement, MouseButton,
    MouseMoveEvent, MouseUpEvent, ParentElement, Styled, Window, canvas, div,
};

use std::collections::HashMap;

use muxy_app_core::TabWindowId;

use crate::model::AppModel;

/// `floats` holds the floating tabs' cards, drawn beforehand as their own
/// surfaces.
pub(crate) fn layer(
    model: &AppModel,
    mut floats: HashMap<TabWindowId, AnyElement>,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let members = model.floating_members();
    let dragging = model.floating.drag.is_some()
        || matches!(
            model.agents_panel.drag,
            Some(crate::model::agents_panel::Drag::Floating(_))
        );
    let cursor = model.floating.drag.as_ref().map(|(_, drag)| {
        if drag.is_resize() {
            gpui::CursorStyle::Arrow
        } else {
            gpui::CursorStyle::ClosedHand
        }
    });
    div()
        .absolute()
        .inset_0()
        .children(members.iter().filter_map(|id| {
            if id.as_str() == crate::model::agents_panel::PANEL {
                super::agents_panel::floating(model, window, cx)
            } else if id.as_str() == crate::model::agent_sessions::PANEL {
                super::agent_sessions::floating(model, window, cx)
            } else if let Some(float) = crate::model::tab_windows::floating::panel_float(id) {
                floats.remove(&float)
            } else {
                model.detached_webview_panel(id, window, cx)
            }
        }))
        .when(dragging, |layer| {
            layer.child(
                div()
                    .id("floating-drag-cover")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .when_some(cursor, Styled::cursor),
            )
        })
        .child(drag_tracker(cx))
        .into_any_element()
}

fn drag_tracker(cx: &mut Context<AppModel>) -> AnyElement {
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
                    if model.floating.drag.is_none() {
                        return;
                    }
                    if event.pressed_button == Some(MouseButton::Left) {
                        model.move_floating_drag(event.position, window, cx);
                    } else {
                        model.finish_floating_drag(cx);
                    }
                    cx.stop_propagation();
                });
            });
            let ending = weak.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                    return;
                }
                let _ = ending.update(cx, |model, cx| {
                    if model.floating.drag.is_some() {
                        model.move_floating_drag(event.position, window, cx);
                        model.finish_floating_drag(cx);
                        cx.stop_propagation();
                    }
                });
            });
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}
