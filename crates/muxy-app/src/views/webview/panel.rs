use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
    Window, div, px,
};
use muxy_ui::floating::FloatingCard;
use muxy_ui::panel::{
    PanelAction, PanelChrome, PanelControl, PanelFrame, PanelId, PanelMode, PanelPosition,
    PanelSizeBounds, PanelSizing, PanelStyle,
};

use crate::model::AppModel;

impl AppModel {
    pub(crate) fn webview_panel_content(
        &self,
        mut content: AnyElement,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (available, floating_space) = self.webview_panel_space(window, cx);
        let mut panels: Vec<_> = self
            .webviews
            .panels
            .iter()
            .filter(|(id, _)| self.panels.placement(id).is_some())
            .collect();
        panels.sort_by_key(|(_, panel)| panel.placement.mode == PanelMode::Pinned);
        for (id, panel) in panels {
            let right = panel.placement.position == PanelPosition::Right;
            let pinned = panel.placement.mode == PanelMode::Pinned;
            let dimension = self.webviews.panel_size.get(panel.placement.position);
            let sizing = PanelSizing::new(
                &panel.placement,
                dimension,
                panel_size_bounds(if pinned { available } else { floating_space }, right),
                panel.resize.clone(),
            );
            let extent = px(sizing.layout().dimension() + 1.0);
            let style = PanelStyle::new(self.theme.clone(), self.metrics);
            let chrome = self
                .webview_panel_chrome(id, cx)
                .map_or_else(|| div().into_any_element(), IntoElement::into_any_element);
            let resize_id = id.clone();
            let model = cx.weak_entity();
            let frame = PanelFrame::new(
                panel.placement.clone(),
                sizing,
                chrome,
                panel.surface.view.clone(),
                move |dimension, _, cx| {
                    let _ = model.update(cx, |model, cx| {
                        if let Some(panel) = model.webviews.panels.get(&resize_id) {
                            model
                                .webviews
                                .panel_size
                                .set(panel.placement.position, dimension);
                            cx.notify();
                        }
                    });
                },
                style,
            )
            .grip(self.webview_grip(
                Some(panel.surface.view.entity_id()),
                panel.placement.position.resize_cursor(),
            ));
            let body = div()
                .relative()
                .flex()
                .size_full()
                .min_w_0()
                .min_h_0()
                .when(!right, Styled::flex_col)
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .overflow_hidden()
                        .child(content),
                );
            content = if pinned {
                body.child(frame).into_any_element()
            } else {
                let close_id = id.clone();
                body.child(
                    div()
                        .id(gpui::SharedString::from(format!(
                            "floating-{}",
                            id.as_str()
                        )))
                        .absolute()
                        .right_0()
                        .bottom_0()
                        .max_w_full()
                        .max_h_full()
                        .when(right, |view| view.top_0().w(extent))
                        .when(!right, |view| view.left_0().h(extent))
                        .on_mouse_down_out(cx.listener(move |model, _, _, cx| {
                            if model.overlay.is_none() {
                                model.dismiss_webview_panel(&close_id, cx);
                            }
                        }))
                        .child(frame)
                        .child(self.webview_occlusion_except(Some(panel.surface.view.entity_id()))),
                )
                .into_any_element()
            };
        }
        content
    }

    /// A detached extension panel: a card of its own, or the bare page,
    /// floating over the window. Its hit regions block what is under them and
    /// its drag regions move it.
    pub(crate) fn detached_webview_panel(
        &self,
        id: &PanelId,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        use muxy_app_core::extensions::panels::{PanelChrome as Chrome, Rect};
        let panel = self.webviews.panels.get(id)?;
        let (frame, _) = self.detached_webview_geometry(id, window)?;
        let entity = panel.surface.view.entity_id();
        let framed = panel.chrome == Chrome::Standard;
        let regions = panel.pointer_regions();
        let header = if framed {
            self.webview_panel_chrome(id, cx)
        } else {
            None
        };
        let model = cx.weak_entity();
        let rect = |rect: &Rect| {
            div()
                .absolute()
                .left(px(rect.x))
                .top(px(rect.y))
                .w(px(rect.width))
                .h(px(rect.height))
        };
        let blockers = regions.unwrap_or_default().iter().map(|region| {
            rect(region)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .into_any_element()
        });
        let drags = panel
            .drag_regions
            .iter()
            .filter(|_| panel.movable)
            .enumerate()
            .map(|(index, region)| {
                let id = id.clone();
                let model = model.clone();
                rect(region)
                    .id(gpui::SharedString::from(format!(
                        "{}-drag-{index}",
                        id.as_str()
                    )))
                    .occlude()
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        let _ = model.update(cx, |model, cx| {
                            model.begin_floating_drag(&id, None, event.position, window, cx);
                        });
                        window.prevent_default();
                        cx.stop_propagation();
                    })
                    .child(self.webview_grip(Some(entity), gpui::CursorStyle::Arrow))
                    .into_any_element()
            });
        let header = header.map(|chrome| Self::detached_header(id, panel.movable, chrome, cx));
        let content = div()
            .relative()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .children(blockers)
            .child(panel.surface.view.clone())
            .children(drags);
        let resize_id = id.clone();
        let card = FloatingCard::new(
            gpui::SharedString::from(format!("detached-{}", id.as_str())),
            frame,
            self.theme.clone(),
            self.metrics,
        )
        .unit(1.0)
        .framed(framed)
        .resizable(panel.resizable)
        .interactive(regions.is_none())
        .on_resize({
            let model = model.clone();
            move |edges, pointer, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    model.begin_floating_drag(&resize_id, edges, pointer, window, cx);
                });
            }
        })
        .on_press({
            let id = id.clone();
            move |_, cx| {
                let _ = model.update(cx, |model, cx| model.raise_floating(&id, cx));
            }
        })
        .grip(self.webview_grips(Some(entity)))
        .children(header)
        .child(content)
        .child(if framed || !(panel.transparent || regions.is_some()) {
            self.webview_occlusion_except(Some(entity))
        } else {
            self.webview_layer(entity)
        });
        Some(card.into_any_element())
    }

    /// Hides every native page, and every browser, under app content drawn
    /// after their occlusions were applied, such as a tooltip.
    ///
    /// The pages are held weakly: GPUI can keep a tooltip, and so this
    /// closure, alive after its element is gone, and a page it kept would
    /// stay on screen where it was last placed.
    fn native_cover(&self, cx: &gpui::App) -> impl Fn(gpui::Bounds<gpui::Pixels>) + 'static {
        let pages: Vec<_> = self
            .webviews
            .panes
            .values()
            .map(|surface| &surface.view)
            .chain(
                self.webviews
                    .panels
                    .values()
                    .map(|panel| &panel.surface.view),
            )
            .chain(
                self.webviews
                    .sidebar
                    .iter()
                    .map(|sidebar| &sidebar.surface.view),
            )
            .map(|view| std::rc::Rc::downgrade(&view.read(cx).native))
            .collect();
        let browsers: Vec<_> = self
            .browsers
            .views
            .values()
            .map(|surface| std::rc::Rc::downgrade(&surface.view.read(cx).native))
            .collect();
        move |bounds| {
            for page in pages.iter().filter_map(std::rc::Weak::upgrade) {
                page.occlude_also(bounds);
            }
            for browser in browsers.iter().filter_map(std::rc::Weak::upgrade) {
                browser.occlude_also(bounds);
            }
        }
    }

    /// A framed detached panel's header, which moves the panel when dragged.
    fn detached_header(
        id: &PanelId,
        movable: bool,
        chrome: PanelChrome,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = id.clone();
        let model = cx.weak_entity();
        div()
            .flex_none()
            .occlude()
            .when(movable, |header| {
                header.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    let _ = model.update(cx, |model, cx| {
                        model.begin_floating_drag(&id, None, event.position, window, cx);
                    });
                    window.prevent_default();
                    cx.stop_propagation();
                })
            })
            .child(chrome)
            .into_any_element()
    }

    fn webview_panel_space(
        &self,
        window: &Window,
        cx: &gpui::App,
    ) -> (gpui::Size<f32>, gpui::Size<f32>) {
        let mut available = gpui::size(
            (f32::from(window.viewport_size().width) - self.sidebar_width()).max(0.0),
            (f32::from(window.viewport_size().height) - 160.0).max(0.0),
        );
        if let Some(composer) = &self.composer.view
            && self.settings.composer.presentation
                == muxy_app_core::settings::ComposerPresentation::Panel
            && self.settings.composer.pinned
        {
            let layout = composer.read(cx).sizing(window).layout();
            available.width = (available.width
                - layout.consumed_width()
                - if layout.consumed_width() > 0.0 {
                    1.0
                } else {
                    0.0
                })
            .max(0.0);
            available.height = (available.height
                - layout.consumed_height()
                - if layout.consumed_height() > 0.0 {
                    1.0
                } else {
                    0.0
                })
            .max(0.0);
        }
        let mut floating_space = available;
        let pinned_space = available.map(|dimension| (dimension - 100.0).max(0.0));
        for (id, panel) in &self.webviews.panels {
            if self.panels.placement(id).is_some() && panel.placement.mode == PanelMode::Pinned {
                let right = panel.placement.position == PanelPosition::Right;
                let desired = self.webviews.panel_size.get(panel.placement.position);
                let bounds = panel_size_bounds(pinned_space, right);
                let extent = bounds.clamp(desired) + 1.0;
                if right {
                    floating_space.width = (floating_space.width - extent).max(0.0);
                } else {
                    floating_space.height = (floating_space.height - extent).max(0.0);
                }
            }
        }
        (pinned_space, floating_space)
    }

    /// main's panel header: icon, title, header buttons, and the position,
    /// pin, and close controls, each hideable; `None` when nothing shows.
    fn webview_panel_chrome(&self, id: &PanelId, cx: &Context<Self>) -> Option<PanelChrome> {
        use muxy_app_core::extensions::PanelControl as Hidden;
        let panel = &self.webviews.panels[id];
        let extension = self.extensions.registry.enabled(&panel.owner);
        let definition = extension.and_then(|extension| extension.manifest.panel(&panel.kind));
        let hide_topbar = definition.is_some_and(|definition| definition.hide_topbar);
        let hides = |control| {
            hide_topbar
                || match &panel.hidden_controls {
                    Some(hidden) => hidden.contains(&control),
                    None => definition.is_some_and(|definition| definition.hides(control)),
                }
        };
        if hide_topbar {
            return None;
        }
        let icon = extension
            .zip(definition.and_then(|definition| definition.icon.as_ref()))
            .filter(|_| !hides(Hidden::Icon))
            .map(|(extension, icon)| {
                self.extension_icon(
                    &extension.directory,
                    icon,
                    self.metrics.font_footnote(),
                    self.theme.fg_muted,
                )
            });
        let title = panel.title.clone().filter(|_| !hides(Hidden::Title));
        let buttons = definition.map_or(&[][..], |definition| &definition.header_buttons[..]);
        if icon.is_none()
            && title.is_none()
            && buttons.is_empty()
            && [Hidden::Position, Hidden::Pin, Hidden::Close, Hidden::Detach]
                .into_iter()
                .all(hides)
        {
            return None;
        }
        let mut chrome = PanelChrome::new(
            title.unwrap_or_default(),
            icon,
            panel.controls[0].clone(),
            self.webview_panel_action(id, PanelControl::Move(panel.placement.position), 1, cx),
            self.webview_panel_action(id, PanelControl::Mode(panel.placement.mode), 2, cx),
            self.webview_panel_action(id, PanelControl::Close, 3, cx),
            PanelStyle::new(self.theme.clone(), self.metrics),
        );
        chrome = chrome.cover_tooltips(self.native_cover(cx));
        if hides(Hidden::Position) || panel.detached {
            chrome = chrome.without_move_action();
        }
        if hides(Hidden::Pin) || panel.detached {
            chrome = chrome.without_mode_action();
        }
        if !hides(Hidden::Detach) {
            chrome = chrome.with_detach_action(self.webview_panel_action(
                id,
                PanelControl::Detach(panel.detached),
                4,
                cx,
            ));
        }
        if hides(Hidden::Close) {
            chrome = chrome.without_close_action();
        }
        for (index, item) in buttons.iter().enumerate() {
            let Some(focus) = panel.header_controls.get(index) else {
                break;
            };
            let owner = panel.owner.clone();
            let command = item.command.clone();
            let key = crate::model::extensions::RunCommand::key(&owner, &command, cx);
            let model = cx.weak_entity();
            let root = extension
                .map(|extension| extension.directory.clone())
                .unwrap_or_default();
            chrome = chrome.with_trailing_action(
                PanelAction::element(
                    gpui::SharedString::from(format!("extension-panel-{}-{}", owner, item.id)),
                    item.tooltip.clone().unwrap_or_else(|| item.id.clone()),
                    self.extension_icon(
                        &root,
                        &item.icon,
                        self.metrics.font_emphasis(),
                        self.theme.fg_muted,
                    ),
                    focus.clone(),
                    move |window, cx| {
                        let _ = model.update(cx, |model, cx| {
                            model.run_extension_command(&owner, &command, window, cx);
                        });
                    },
                )
                .shortcut(key),
            );
        }
        Some(chrome)
    }

    fn webview_panel_action(
        &self,
        id: &PanelId,
        control: PanelControl,
        index: usize,
        cx: &Context<Self>,
    ) -> PanelAction {
        let panel = &self.webviews.panels[id];
        let id = id.clone();
        let model = cx.weak_entity();
        PanelAction::control(
            gpui::SharedString::from(format!("{}-{index}", id.as_str())),
            control,
            panel.controls[index].clone(),
            move |window, cx| {
                let _ = model.update(cx, |model, cx| match control {
                    PanelControl::Close => model.dismiss_webview_panel(&id, cx),
                    PanelControl::Move(_) => model.move_webview_panel(&id, false, cx),
                    PanelControl::Mode(_) => model.move_webview_panel(&id, true, cx),
                    PanelControl::Detach(_) => model.toggle_webview_panel_detached(&id, window, cx),
                });
            },
        )
    }
}

fn panel_size_bounds(available: gpui::Size<f32>, right: bool) -> PanelSizeBounds {
    let maximum = (if right {
        available.width
    } else {
        available.height
    } - 1.0)
        .max(0.0)
        .min(if right { 800.0 } else { 600.0 });
    PanelSizeBounds::new(
        if right { 240.0_f32 } else { 120.0_f32 }.min(maximum),
        maximum,
    )
}
