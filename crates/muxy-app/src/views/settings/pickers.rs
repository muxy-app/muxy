use gpui::{Context, InteractiveElement, IntoElement, ParentElement, Styled, div, px};
use muxy_ui::controls;
pub(crate) use muxy_ui::popover::PopoverAnchor as PickerAnchor;

use super::{SettingsEvent, SettingsView};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PickerKind {
    FontFamily,
    Language,
    Theme(bool),
    AiProvider(crate::repository_actions::Action),
}

impl PickerKind {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::FontFamily => "font-family",
            Self::Language => "composer-language",
            Self::Theme(false) => "light-theme",
            Self::Theme(true) => "dark-theme",
            Self::AiProvider(action) => super::ai::provider_id(action),
        }
    }
}

#[derive(Clone)]
pub(crate) struct PickerRequest {
    pub(crate) kind: PickerKind,
    pub(crate) anchor: PickerAnchor,
}

impl SettingsView {
    pub(super) fn picker(
        &self,
        kind: PickerKind,
        value: &str,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let anchor = self.picker_anchors[&kind].clone();
        let recorder = anchor.clone();
        div()
            .flex()
            .min_w(px(0.0))
            .debug_selector(move || format!("settings-picker-{}", kind.id()))
            .child(controls::picker_trigger(
                self.style(),
                kind.id(),
                value,
                (!self.compact).then_some(controls::CONTROL_WIDTH),
                false,
                cx.listener(move |pane, _, _, cx| {
                    pane.recording = None;
                    cx.emit(SettingsEvent::Picker(kind, anchor.clone()));
                }),
            ))
            .on_children_prepainted(move |bounds, window, _| {
                recorder.set(
                    bounds
                        .first()
                        .copied()
                        .filter(|bounds| window.content_mask().bounds.intersects(bounds)),
                );
            })
            .into_any_element()
    }
}

pub(crate) fn dropdown(
    picker: gpui::AnyElement,
    source: PickerRequest,
    cx: &mut Context<super::window::SettingsWindow>,
) -> gpui::AnyElement {
    let model = cx.weak_entity();
    muxy_ui::popover::anchored_popover(
        source.anchor,
        div()
            .debug_selector(|| "settings-dropdown".into())
            .child(picker)
            .into_any_element(),
        move |window, cx| {
            let _ = model.update(cx, |model, cx| {
                if model
                    .overlay
                    .as_ref()
                    .map(super::window::SettingsOverlay::source)
                    .is_some_and(|current| current.kind == source.kind)
                {
                    model.dismiss_overlay(window, cx);
                }
            });
        },
    )
}
