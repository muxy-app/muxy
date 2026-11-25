use gpui::{
    AnyElement, Bounds, Context, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    Styled, div,
};
use muxy_ui::controls;

use super::{SettingsEvent, SettingsView};
use crate::model::preferences::Change;

const VALUE_WIDTH: f32 = 52.0;
const FIELD_WIDTH: f32 = 72.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Commit {
    Terminal,
    Field,
    Quick,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Slider {
    pub(super) id: &'static str,
    pub(super) min: f32,
    pub(super) max: f32,
    pub(super) step: f32,
    pub(super) suffix: &'static str,
    pub(super) commit: Commit,
}

impl Slider {
    pub(super) const fn new(
        id: &'static str,
        range: (f32, f32),
        step: f32,
        suffix: &'static str,
        commit: Commit,
    ) -> Self {
        Self {
            id,
            min: range.0,
            max: range.1,
            step,
            suffix,
            commit,
        }
    }

    fn snap(&self, value: f32) -> f32 {
        if self.max - value < self.step / 2.0 {
            return self.max;
        }
        let snapped = self.min + ((value - self.min) / self.step).round() * self.step;
        snapped.clamp(self.min, self.max)
    }

    fn number(&self, value: f32) -> String {
        if self.step >= 1.0 {
            format!("{value:.0}")
        } else {
            format!("{value:.2}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        }
    }

    fn text(&self, value: f32) -> String {
        let number = self.number(value);
        if self.commit == Commit::Terminal && self.id.starts_with("adjust-") {
            format!("{number}{}", self.suffix)
        } else {
            number
        }
    }

    fn label(&self, value: f32) -> String {
        format!("{}{}", self.number(value), self.suffix)
    }
}

pub(super) struct Drag {
    slider: Slider,
    bounds: Bounds<Pixels>,
    value: Option<f32>,
}

impl SettingsView {
    pub(super) fn slider(&self, spec: Slider, value: f32, cx: &mut Context<Self>) -> AnyElement {
        let value = self.dragged(spec.id).unwrap_or(value);
        let label = div()
            .flex_none()
            .w(self.layout_metrics.scaled(VALUE_WIDTH))
            .child(spec.label(value));
        self.slider_with(spec, value, label.into_any_element(), cx)
    }

    pub(super) fn slider_field(
        &self,
        spec: Slider,
        value: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = self.dragged(spec.id).unwrap_or(value);
        let selector = format!("settings-field-{}", spec.id);
        let field = div()
            .flex_none()
            .debug_selector(move || selector.clone())
            .child(controls::text_field(
                self.style(),
                spec.id,
                &self.fields[spec.id],
                Some(FIELD_WIDTH),
            ))
            .into_any_element();
        self.slider_with(spec, value, field, cx)
    }

    fn slider_with(
        &self,
        spec: Slider,
        value: f32,
        beside: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let spec = Slider {
            min: spec.min.min(value),
            max: spec.max.max(value),
            ..spec
        };
        let id = spec.id;
        let control = controls::slider(
            self.style(),
            id,
            &super::control_name(id),
            value,
            (spec.min, spec.max),
            cx.listener(move |view, grab: &controls::Grab, _, cx| {
                view.finish_slider(cx);
                view.slider = Some(Drag {
                    slider: spec,
                    bounds: grab.bounds,
                    value: None,
                });
                view.move_slider(grab.position, cx);
            }),
            cx.listener(move |view, value: &f32, _, cx| {
                view.finish_slider(cx);
                view.slider = Some(Drag {
                    slider: spec,
                    bounds: Bounds::default(),
                    value: None,
                });
                view.set_slider(*value, cx);
                view.finish_slider(cx);
            }),
        );
        div()
            .debug_selector(move || format!("settings-slider-row-{id}"))
            .flex()
            .flex_none()
            .items_center()
            .gap(self.layout_metrics.scaled(8.0))
            .child(control)
            .child(beside)
            .into_any_element()
    }

    pub(super) fn dragging(&self) -> Option<&'static str> {
        self.slider.as_ref().map(|drag| drag.slider.id)
    }

    fn dragged(&self, id: &str) -> Option<f32> {
        self.slider
            .as_ref()
            .filter(|drag| drag.slider.id == id)
            .and_then(|drag| drag.value)
    }

    pub(super) fn move_slider(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = &self.slider else {
            return;
        };
        let spec = drag.slider;
        let fraction = controls::fraction_at(drag.bounds, position);
        self.set_slider(spec.min + fraction * (spec.max - spec.min), cx);
    }

    fn set_slider(&mut self, value: f32, cx: &mut Context<Self>) {
        let Some(drag) = &mut self.slider else {
            return;
        };
        let spec = drag.slider;
        let value = spec.snap(value);
        if drag.value == Some(value) {
            return;
        }
        drag.value = Some(value);
        match spec.commit {
            Commit::Terminal => {
                cx.emit(SettingsEvent::PreviewTerminal(spec.id, spec.text(value)));
            }
            Commit::Field => {
                if let Some(input) = self.fields.get(spec.id) {
                    input.update(cx, |input, cx| input.set_text(spec.text(value), cx));
                }
                self.dirty.remove(spec.id);
            }
            Commit::Quick => {}
        }
        cx.notify();
    }

    pub(super) fn finish_slider(&mut self, cx: &mut Context<Self>) {
        if let Some(change) = self.take_slider_change() {
            cx.emit(SettingsEvent::Change(change));
        }
    }

    pub(super) fn take_slider_change(&mut self) -> Option<Change> {
        let drag = self.slider.take()?;
        let value = drag.value?;
        let spec = drag.slider;
        match spec.commit {
            Commit::Terminal => Some(Change::Terminal(spec.id, spec.text(value))),
            Commit::Field => Some(Change::Field(spec.id, spec.text(value))),
            Commit::Quick => {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "Quick Terminal sliders stay within their small positive ranges"
                )]
                let value = value.round() as u16;
                Some(Change::QuickNumber(spec.id, value))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Commit, Slider};

    #[test]
    fn values_snap_to_steps_from_the_minimum_and_reach_both_ends() {
        let width = Slider::new("width", (640.0, 16384.0), 10.0, "", Commit::Field);
        assert!((width.snap(16383.0) - 16384.0).abs() < f32::EPSILON);
        assert!((width.snap(16376.0) - 16380.0).abs() < f32::EPSILON);
        assert!((width.snap(0.0) - 640.0).abs() < f32::EPSILON);
        assert!((width.snap(1203.0) - 1200.0).abs() < f32::EPSILON);
        let height = Slider::new("line", (1.1, 2.0), 0.05, "", Commit::Field);
        assert_eq!(height.text(height.snap(1.93)), "1.95");
    }
}
