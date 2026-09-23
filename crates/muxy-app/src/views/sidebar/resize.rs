use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, DispatchPhase, InteractiveElement, IntoElement, MouseButton,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Styled, canvas, div, px,
};

use crate::model::AppModel;

#[derive(Clone, Copy)]
pub(crate) struct SidebarResize {
    pointer: Pixels,
    width: Pixels,
}

impl AppModel {
    fn move_sidebar_resize(&mut self, pointer: Pixels, cx: &mut Context<Self>) {
        let Some(resize) = self.sidebar_resize else {
            return;
        };
        let width = self.clamp_expanded_sidebar_width(resize.width + pointer - resize.pointer);
        if width != px(self.sidebar_width()) {
            self.appearance.sidebar_expanded_width =
                Some(f32::from(width) / f32::from(self.metrics.scaled(1.0)));
            cx.notify();
        }
    }

    pub(crate) fn finish_sidebar_resize(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_resize.take().is_none() {
            return;
        }
        if self.appearance.sidebar_expanded_width != self.settings.appearance.sidebar_expanded_width
        {
            self.save_appearance(cx);
        }
        cx.notify();
    }
}

pub(in crate::views) fn handle(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let weak = cx.weak_entity();
    let active = model.sidebar_resize.is_some();
    div()
        .id("sidebar-resize")
        .debug_selector(|| "sidebar-resize".into())
        .group("sidebar-resize")
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(model.sidebar_width()) - model.metrics.resize_handle_hit_area())
        .w(model.metrics.resize_handle_hit_area())
        .cursor_ew_resize()
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|model, event: &gpui::MouseDownEvent, window, cx| {
                if model.overlay.is_some() || model.close_prompt.is_some() {
                    return;
                }
                model.sidebar_resize = Some(SidebarResize {
                    pointer: event.position.x,
                    width: px(model.sidebar_width()),
                });
                window.prevent_default();
                cx.stop_propagation();
                cx.notify();
            }),
        )
        .child(
            div()
                .absolute()
                .right_0()
                .h_full()
                .w(px(1.0))
                .bg(model.theme.border)
                .when(active, |line| line.bg(model.theme.accent))
                .group_hover("sidebar-resize", |line| line.bg(model.theme.accent)),
        )
        .when(model.webviews.sidebar.is_some(), |handle| {
            handle.child(model.webview_occlusion())
        })
        .child(
            canvas(
                |_, _, _| (),
                move |_, (), window, _| {
                    let moving = weak.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase != DispatchPhase::Capture {
                            return;
                        }
                        let _ = moving.update(cx, |model, cx| {
                            if model.sidebar_resize.is_none() {
                                return;
                            }
                            if event.pressed_button == Some(MouseButton::Left) {
                                model.move_sidebar_resize(event.position.x, cx);
                            } else {
                                model.finish_sidebar_resize(cx);
                            }
                            cx.stop_propagation();
                        });
                    });
                    let ending = weak.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase != DispatchPhase::Capture || event.button != MouseButton::Left {
                            return;
                        }
                        let _ = ending.update(cx, |model, cx| {
                            if model.sidebar_resize.is_some() {
                                model.move_sidebar_resize(event.position.x, cx);
                                model.finish_sidebar_resize(cx);
                                cx.stop_propagation();
                            }
                        });
                    });
                },
            )
            .absolute()
            .size_full(),
        )
        .into_any_element()
}
