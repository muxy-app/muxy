use super::{Close, Composer, ComposerEvent, Insert, Submit};
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, FontWeight, MouseButton, Pixels, Point, Size, size};
use gpui::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_app_core::settings::{ComposerPosition, ComposerPresentation};
use muxy_ui::{
    components::{ButtonInteraction, SymbolGlyph},
    l10n::{tr_key, translate},
    panel::{PanelAction, PanelChrome, PanelControl, PanelFrame, PanelStyle},
    tr,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ResizeEdge {
    Leading,
    Trailing,
    Bottom,
}

pub(super) struct SizeMotion {
    start: Size<Pixels>,
    target: Size<Pixels>,
    began: std::time::Instant,
}

#[derive(Clone, Copy)]
enum Control {
    Metadata,
    Toolbar,
    Dismiss,
}

impl Composer {
    pub(crate) fn floating_size(&self, available: Size<Pixels>) -> Size<Pixels> {
        let m = self.metrics;
        let desired = if self.settings.expanded {
            size(m.scaled(880.0), m.scaled(620.0))
        } else {
            size(
                m.scaled(self.settings.floating_width),
                m.scaled(self.settings.floating_height),
            )
        };
        size(
            desired
                .width
                .max(m.scaled(360.0))
                .min((available.width - m.scaled(40.0)).max(px(0.0))),
            desired
                .height
                .max(m.scaled(180.0))
                .min((available.height - m.scaled(40.0)).max(px(0.0))),
        )
    }

    fn glyph_button(
        &self,
        id: &'static str,
        symbol: &'static str,
        label: &'static str,
        control: Control,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let m = self.metrics;
        let theme = self.theme.clone();
        let (box_size, glyph_size, color) = match control {
            Control::Metadata => (m.control_small(), m.font_caption(), theme.fg_dim),
            Control::Toolbar => (m.control_large(), m.font_footnote(), theme.fg_muted),
            Control::Dismiss => (m.font_micro(), m.font_micro(), theme.fg_dim),
        };
        div()
            .id(id)
            .group(id)
            .debug_selector(move || id.into())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(box_size)
            .when(id == "composer-more", |button| button.w(m.control_small()))
            .when(id == "composer-more", |button| {
                let anchor = self.menu_anchor.clone();
                button.relative().child(
                    gpui::canvas(move |bounds, _, _| anchor.set(bounds), |_, (), _, _| {})
                        .absolute()
                        .size_full(),
                )
            })
            .rounded(m.radius_xl())
            .child(SymbolGlyph::new(symbol, glyph_size, color))
            .tooltip(move |_, cx| {
                cx.new(|_| {
                    muxy_ui::components::Tooltip::new(
                        translate(label),
                        theme.raised(),
                        theme.fg,
                        theme.border,
                        theme.bg,
                    )
                })
                .into()
            })
            .button_interaction(cx.listener(move |view, _, window, cx| action(view, window, cx)))
            .into_any_element()
    }

    fn header(&self, cx: &Context<Self>) -> AnyElement {
        let placement = self.placement();
        PanelChrome::new(
            tr!("Rich Input"),
            Some(
                SymbolGlyph::new(
                    "keyboard",
                    self.metrics.font_footnote(),
                    self.theme.fg_muted,
                )
                .into_any_element(),
            ),
            self.header_focus[0].clone(),
            self.panel_control(
                "composer-position",
                PanelControl::Move(placement.position),
                1,
                cx,
            ),
            self.panel_control("composer-pin", PanelControl::Mode(placement.mode), 2, cx),
            self.panel_control("composer-close", PanelControl::Close, 3, cx),
            PanelStyle::new(self.theme.clone(), self.metrics),
        )
        .with_trailing_action(
            self.panel_action(
                "composer-broadcast",
                tr_key!("Broadcast to Split Panes"),
                if self.settings.broadcast {
                    "dot.radiowaves.left.and.right"
                } else {
                    "antenna.radiowaves.left.and.right.slash"
                },
                4,
                |view, cx| {
                    view.settings.broadcast = !view.settings.broadcast;
                    view.preferences(cx);
                },
                cx,
            )
            .selected(self.settings.broadcast),
        )
        .with_trailing_action(self.panel_action(
            "composer-floating",
            tr_key!("Use Floating Composer"),
            "rectangle.on.rectangle",
            5,
            |view, cx| {
                view.settings.presentation = ComposerPresentation::Floating;
                view.preferences(cx);
            },
            cx,
        ))
        .into_any_element()
    }

    fn panel_control(
        &self,
        id: &'static str,
        control: PanelControl,
        index: usize,
        cx: &Context<Self>,
    ) -> PanelAction {
        let view = cx.weak_entity();
        PanelAction::control(
            id,
            control,
            self.header_focus[index].clone(),
            move |_, cx| {
                let _ = view.update(cx, |view, cx| {
                    match control {
                        PanelControl::Close => {
                            cx.emit(ComposerEvent::Close);
                            return;
                        }
                        PanelControl::Move(_) => {
                            view.settings.position =
                                if view.settings.position == ComposerPosition::Right {
                                    ComposerPosition::Bottom
                                } else {
                                    ComposerPosition::Right
                                };
                        }
                        PanelControl::Mode(_) => view.settings.pinned = !view.settings.pinned,
                    }
                    view.preferences(cx);
                });
            },
        )
    }

    fn panel_action(
        &self,
        id: &'static str,
        label: &'static str,
        symbol: &'static str,
        index: usize,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> PanelAction {
        let view = cx.weak_entity();
        PanelAction::symbol(
            id,
            translate(label),
            symbol,
            self.header_focus[index].clone(),
            move |_, cx| {
                let _ = view.update(cx, |view, cx| action(view, cx));
            },
        )
    }

    fn metadata(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = self.metrics;
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(m.spacing3())
            .h(m.scaled(22.0))
            .child(
                div()
                    .size(m.scaled(6.0))
                    .rounded_full()
                    .bg(self.theme.diff_add)
                    .flex_none(),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(m.font_caption())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.theme.fg_muted)
                    .child(self.context_label.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(m.font_caption())
                    .text_color(self.theme.fg_dim)
                    .child(tr!("· active pane")),
            )
            .child(div().flex_1())
            .child(self.glyph_button(
                "composer-expand",
                if self.settings.expanded {
                    "arrow.down.right.and.arrow.up.left"
                } else {
                    "arrow.up.left.and.arrow.down.right"
                },
                if self.settings.expanded {
                    tr_key!("Collapse Composer")
                } else {
                    tr_key!("Expand Composer")
                },
                Control::Metadata,
                |view, _, cx| {
                    view.settings.expanded = !view.settings.expanded;
                    view.preferences(cx);
                },
                cx,
            ))
            .child(self.glyph_button(
                "composer-close",
                "xmark",
                tr_key!("Close Composer"),
                Control::Metadata,
                |_, _, cx| cx.emit(ComposerEvent::Close),
                cx,
            ))
            .into_any_element()
    }

    fn error_row(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let error = self.error.as_ref()?;
        let m = self.metrics;
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(m.spacing4())
                .h(m.scaled(26.0))
                .px(m.spacing1())
                .text_size(m.font_caption())
                .text_color(self.theme.diff_remove)
                .child(
                    SymbolGlyph::new(
                        "exclamationmark.circle.fill",
                        m.font_caption(),
                        self.theme.diff_remove,
                    )
                    .regular(),
                )
                .child(div().min_w_0().flex_1().truncate().child(error.clone()))
                .child(self.glyph_button(
                    "composer-dismiss-error",
                    "xmark",
                    tr_key!("Dismiss Error"),
                    Control::Dismiss,
                    |view, _, cx| {
                        view.error = None;
                        cx.notify();
                    },
                    cx,
                ))
                .into_any_element(),
        )
    }

    fn attachments(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = self.metrics;
        let mut row = div()
            .id("composer-attachments")
            .flex()
            .flex_none()
            .items_center()
            .gap(m.spacing3())
            .h(m.scaled(28.0))
            .px(m.spacing1())
            .overflow_x_scroll();
        for (index, path) in self.draft.file_attachments.iter().enumerate() {
            let name = std::path::Path::new(path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            row = row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(m.spacing2())
                    .px(m.spacing4())
                    .py(m.spacing2())
                    .rounded(m.radius_md())
                    .bg(self.theme.surface)
                    .text_color(self.theme.fg_muted)
                    .text_size(m.font_xs())
                    .child(SymbolGlyph::new("doc", m.font_xs(), self.theme.fg_muted).regular())
                    .child(name)
                    .child(
                        div()
                            .id(("composer-remove-file", index))
                            .button_interaction(cx.listener(move |view, _, _, cx| {
                                if index < view.draft.file_attachments.len() {
                                    view.draft.file_attachments.remove(index);
                                    cx.emit(ComposerEvent::Changed);
                                    cx.notify();
                                }
                            }))
                            .child(SymbolGlyph::new(
                                "xmark",
                                m.font_micro(),
                                self.theme.fg_muted,
                            )),
                    ),
            );
        }
        row.into_any_element()
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = self.metrics;
        let enabled = !self.sending;
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(m.spacing2())
            .pt(m.spacing4())
            .child(self.glyph_button(
                "composer-attach",
                "plus",
                tr_key!("Attach File"),
                Control::Toolbar,
                |_, _, cx| cx.emit(ComposerEvent::Attach),
                cx,
            ))
            .child(self.glyph_button(
                "composer-more",
                "ellipsis",
                tr_key!("More Composer Actions"),
                Control::Toolbar,
                |_, _, cx| cx.emit(ComposerEvent::Menu),
                cx,
            ))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(m.font_xs())
                    .text_color(self.theme.fg_dim)
                    .child(if self.settings.broadcast {
                        tr!("All split panes")
                    } else {
                        tr!("Active pane")
                    }),
            )
            .child(div().flex_1().min_w(m.spacing4()))
            .child(
                div()
                    .id("composer-send")
                    .debug_selector(|| "composer-send".into())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(m.control_large())
                    .rounded(m.radius_xl())
                    .bg(self.theme.accent)
                    .child(
                        SymbolGlyph::new("arrow.up", m.font_body(), self.theme.accent_foreground)
                            .bold(),
                    )
                    .when(enabled, |button| {
                        button
                            .button_interaction(cx.listener(|view, _, _, cx| view.submit(true, cx)))
                    }),
            )
            .into_any_element()
    }

    fn body(&self, cx: &mut Context<Self>) -> AnyElement {
        let m = self.metrics;
        div()
            .id("composer-body")
            .debug_selector(|| "composer-body".into())
            .key_context("Composer")
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .p(m.scaled(14.0))
            .on_action(cx.listener(|view, _: &Submit, _, cx| view.submit(true, cx)))
            .on_action(cx.listener(|view, _: &Insert, _, cx| view.submit(false, cx)))
            .on_action(cx.listener(|_, _: &Close, _, cx| cx.emit(ComposerEvent::Close)))
            .on_action(cx.listener(
                |view, _: &crate::views::workspace::IncreaseFontSize, _, cx| view.zoom(1.0, cx),
            ))
            .on_action(cx.listener(
                |view, _: &crate::views::workspace::DecreaseFontSize, _, cx| view.zoom(-1.0, cx),
            ))
            .on_drop(cx.listener(|_, paths: &gpui::ExternalPaths, _, cx| {
                cx.emit(ComposerEvent::Files(paths.paths().to_vec()));
            }))
            .when(
                self.settings.presentation == ComposerPresentation::Floating,
                |body| body.child(self.metadata(cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .id("composer-editor")
                            .debug_selector(|| "composer-editor".into())
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .px(m.spacing1())
                            .py(m.spacing6())
                            .child(self.input.clone()),
                    )
                    .children(
                        self.error_row(cx)
                            .map(|error| div().pt(m.spacing2()).child(error)),
                    ),
            )
            .when(!self.draft.file_attachments.is_empty(), |body| {
                body.child(self.attachments(cx))
            })
            .child(self.toolbar(cx))
            .into_any_element()
    }

    pub(super) fn render_composer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.settings.presentation == ComposerPresentation::Floating {
            let progress = self.visibility.sample(window);
            let scale = 0.98 + progress * 0.02;
            let dimensions = self.animated_size(window, cx);
            let dimensions = size(dimensions.width * scale, dimensions.height * scale);
            let metrics = self.metrics;
            self.metrics = muxy_ui::theme::Metrics::new(f32::from(metrics.scaled(1.0)) * scale);
            let mut style = self.input_style;
            style.font_size *= scale;
            style.line_height *= scale;
            self.input
                .update(cx, |input, cx| input.set_style(style, cx));
            let body = self.body(cx);
            let result = self.floating(body, dimensions, cx);
            self.metrics = metrics;
            return result;
        }
        let style = self.input_style;
        self.input
            .update(cx, |input, cx| input.set_style(style, cx));
        let body = self.body(cx);
        let owner = cx.weak_entity();
        PanelFrame::new(
            self.placement(),
            self.sizing(window),
            self.header(cx),
            body,
            move |dimension, _, cx| {
                let _ = owner.update(cx, |view, cx| {
                    if view.settings.position == ComposerPosition::Right {
                        view.settings.width = dimension;
                    } else {
                        view.settings.height = dimension;
                    }
                    view.preferences(cx);
                });
            },
            PanelStyle::new(self.theme.clone(), self.metrics),
        )
        .into_any_element()
    }

    fn floating(
        &mut self,
        body: AnyElement,
        dimensions: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let m = self.metrics;
        div()
            .id("floating-composer-card")
            .debug_selector(|| "floating-composer-card".into())
            .relative()
            .flex()
            .flex_col()
            .w(dimensions.width)
            .h(dimensions.height)
            .rounded(m.scaled(18.0))
            .bg(self.theme.raised())
            .shadow(muxy_ui::theme::Elevation::Elevated.shadow(self.theme.bg))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(Self::resize_listener(cx))
            .child(body)
            .children(
                [
                    ResizeEdge::Leading,
                    ResizeEdge::Trailing,
                    ResizeEdge::Bottom,
                ]
                .into_iter()
                .map(|edge| self.resize_edge(edge, dimensions, cx)),
            )
            .into_any_element()
    }

    fn animated_size(&mut self, window: &Window, cx: &Context<Self>) -> Size<Pixels> {
        let now = cx.background_executor().now();
        let target = self.floating_size(window.viewport_size());
        if self.floating_dimensions != Some(target) {
            if let Some(start) = self.floating_dimensions {
                self.size_motion = self.floating_resize.is_none().then_some(SizeMotion {
                    start,
                    target,
                    began: now,
                });
            }
            self.floating_dimensions = Some(target);
        }
        let Some(motion) = &self.size_motion else {
            return target;
        };
        let progress = (now.duration_since(motion.began).as_secs_f32() / 0.14).min(1.0);
        if progress >= 1.0 {
            self.size_motion = None;
            return target;
        }
        let eased = 1.0 - (1.0 - progress).powi(3);
        window.request_animation_frame();
        size(
            motion.start.width + (motion.target.width - motion.start.width) * eased,
            motion.start.height + (motion.target.height - motion.start.height) * eased,
        )
    }

    fn resize_listener(cx: &Context<Self>) -> impl IntoElement {
        let owner = cx.weak_entity();
        gpui::canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                let owner_move = owner.clone();
                window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, window, cx| {
                    if phase != gpui::DispatchPhase::Bubble {
                        return;
                    }
                    let _ = owner_move.update(cx, |view, cx| {
                        let Some((start, dimensions, edge)) = view.floating_resize else {
                            return;
                        };
                        if event.pressed_button != Some(MouseButton::Left) {
                            view.floating_resize = None;
                            return;
                        }
                        view.resize_floating(start, dimensions, edge, event.position, window, cx);
                    });
                });
                window.on_mouse_event(move |_: &gpui::MouseUpEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Bubble {
                        let _ = owner.update(cx, |view, _| view.floating_resize = None);
                    }
                });
            },
        )
        .absolute()
        .size_full()
    }

    fn resize_edge(
        &self,
        edge: ResizeEdge,
        dimensions: Size<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let m = self.metrics;
        let group = match edge {
            ResizeEdge::Leading => "composer-leading-grip",
            ResizeEdge::Trailing => "composer-trailing-grip",
            ResizeEdge::Bottom => "composer-bottom-grip",
        };
        let active = self
            .floating_resize
            .is_some_and(|(_, _, active)| active == edge);
        let grip = div()
            .rounded_full()
            .bg(self.theme.fg_muted)
            .opacity(if active { 0.8 } else { 0.0 })
            .w(m.scaled(if edge == ResizeEdge::Bottom {
                28.0
            } else {
                3.0
            }))
            .h(m.scaled(if edge == ResizeEdge::Bottom {
                3.0
            } else {
                28.0
            }))
            .group_hover(group, |style| style.opacity(0.8));
        let handle = div()
            .id(match edge {
                ResizeEdge::Leading => "composer-resize-leading",
                ResizeEdge::Trailing => "composer-resize-trailing",
                ResizeEdge::Bottom => "composer-resize-bottom",
            })
            .absolute()
            .group(group)
            .flex()
            .items_center()
            .justify_center()
            .child(grip)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &gpui::MouseDownEvent, _, cx| {
                    view.size_motion = None;
                    view.floating_resize = Some((event.position, dimensions, edge));
                    cx.stop_propagation();
                }),
            );
        match edge {
            ResizeEdge::Leading => handle
                .top_0()
                .bottom_0()
                .left_0()
                .w(m.resize_handle_hit_area())
                .cursor_ew_resize(),
            ResizeEdge::Trailing => handle
                .top_0()
                .bottom_0()
                .right_0()
                .w(m.resize_handle_hit_area())
                .cursor_ew_resize(),
            ResizeEdge::Bottom => handle
                .bottom_0()
                .left_0()
                .right_0()
                .h(m.resize_handle_hit_area())
                .cursor_ns_resize(),
        }
        .into_any_element()
    }

    fn resize_floating(
        &mut self,
        start: Point<Pixels>,
        dimensions: Size<Pixels>,
        edge: ResizeEdge,
        current: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let scale = f32::from(self.metrics.scaled(1.0));
        self.settings.expanded = false;
        self.settings.floating_width = f32::from(dimensions.width) / scale;
        self.settings.floating_height = f32::from(dimensions.height) / scale;
        match edge {
            ResizeEdge::Leading => {
                self.settings.floating_width -= 2.0 * f32::from(current.x - start.x) / scale;
            }
            ResizeEdge::Trailing => {
                self.settings.floating_width += 2.0 * f32::from(current.x - start.x) / scale;
            }
            ResizeEdge::Bottom => {
                self.settings.floating_height += 2.0 * f32::from(current.y - start.y) / scale;
            }
        }
        let dimensions = self.floating_size(window.viewport_size());
        self.settings.floating_width = (f32::from(dimensions.width) / scale).max(1.0);
        self.settings.floating_height = (f32::from(dimensions.height) / scale).max(1.0);
        self.preferences(cx);
    }
}
