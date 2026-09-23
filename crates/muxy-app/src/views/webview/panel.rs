use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, div, px,
};
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
            let dimension = if right { panel.width } else { panel.height };
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
                        if let Some(panel) = model.webviews.panels.get_mut(&resize_id) {
                            if panel.placement.position == PanelPosition::Right {
                                panel.width = dimension;
                            } else {
                                panel.height = dimension;
                            }
                            cx.notify();
                        }
                    });
                },
                style,
            );
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
                let desired = if right { panel.width } else { panel.height };
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
        let hides = |control| definition.is_some_and(|definition| definition.hides(control));
        if definition.is_some_and(|definition| definition.hide_topbar) {
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
            && [Hidden::Position, Hidden::Pin, Hidden::Close]
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
        if hides(Hidden::Position) {
            chrome = chrome.without_move_action();
        }
        if hides(Hidden::Pin) {
            chrome = chrome.without_mode_action();
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
            let model = cx.weak_entity();
            let root = extension
                .map(|extension| extension.directory.clone())
                .unwrap_or_default();
            chrome = chrome.with_trailing_action(PanelAction::element(
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
            ));
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
            move |_, cx| {
                let _ = model.update(cx, |model, cx| match control {
                    PanelControl::Close => model.dismiss_webview_panel(&id, cx),
                    PanelControl::Move(_) => model.move_webview_panel(&id, false, cx),
                    PanelControl::Mode(_) => model.move_webview_panel(&id, true, cx),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_sizing_respects_space_remaining_beside_composer() {
        let remaining = gpui::size(800.0 - 200.0 - 401.0, 400.0);
        let bounds = panel_size_bounds(remaining, true);
        assert!(bounds.clamp(360.0) + 1.0 <= remaining.width);
        assert!(bounds.minimum <= bounds.maximum);
        for available in [0.0, 20.0, 100.0, 640.0] {
            let bounds = panel_size_bounds(gpui::size(available, available), false);
            assert!(bounds.clamp(260.0) <= available);
            assert!(bounds.minimum <= bounds.maximum);
        }
    }
}
