use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, PromptButton, Render, StatefulInteractiveElement, Styled,
    Subscription, Window, div, px,
};

use super::{Answer, Request};
use crate::{
    controls::{self, Style},
    text_input::{InputEvent, InputStyle, TextInput},
};

pub(super) struct Dialog {
    title: String,
    message: String,
    buttons: Vec<PromptButton>,
    request: Request,
    input: Option<Entity<TextInput>>,
    focus: FocusHandle,
    dismissed: bool,
    _input_subscription: Option<Subscription>,
}

impl Dialog {
    /// The card's background and one scaled point.
    #[cfg(test)]
    pub(super) fn look(&self) -> (gpui::Hsla, gpui::Pixels) {
        (self.request.theme.bg, self.request.metrics.scaled(1.0))
    }

    pub(super) fn new(
        title: &str,
        message: Option<&str>,
        buttons: &[PromptButton],
        mut request: Request,
        cx: &mut Context<Self>,
    ) -> Self {
        if request.cancel.is_none() {
            request.cancel = buttons
                .iter()
                .position(|button| matches!(button, PromptButton::Cancel(_)));
        }
        request.default = request
            .default
            .or_else(|| (!buttons.is_empty()).then_some(0));
        let input = request.input.as_ref().map(|(value, placeholder)| {
            cx.new(|cx| {
                let mut field =
                    TextInput::new(InputStyle::field(&request.theme, &request.metrics), cx);
                field.set_text(value.clone(), cx);
                field.set_placeholder(placeholder.clone());
                field
            })
        });
        let subscription = input.as_ref().map(|input| {
            cx.subscribe(input, |view: &mut Self, input, event, cx| match event {
                InputEvent::Submitted => view.finish(view.request.default, cx),
                InputEvent::Cancelled => view.finish(view.request.cancel, cx),
                InputEvent::Changed => {
                    let text = input.read(cx).text();
                    if let Some(limit) = view.request.input_limit
                        && let Some((boundary, _)) = text.char_indices().nth(limit)
                    {
                        let text = text[..boundary].to_owned();
                        input.update(cx, |input, cx| input.set_text(text, cx));
                    }
                }
            })
        });
        Self {
            title: title.into(),
            message: message.unwrap_or_default().into(),
            buttons: buttons.to_vec(),
            request,
            input,
            focus: cx.focus_handle(),
            dismissed: false,
            _input_subscription: subscription,
        }
    }

    pub(super) fn pending(&self) -> bool {
        !self.dismissed
    }

    pub(super) fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.finish(None, cx);
    }

    pub(super) fn finish(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        if self.dismissed {
            return;
        }
        self.dismissed = true;
        let answer = Answer {
            index,
            prompt: self
                .input
                .as_ref()
                .map(|input| input.read(cx).text().to_owned())
                .unwrap_or_default(),
            picked: self.request.choices.picker.as_ref().and_then(|picker| {
                (picker.selected < picker.items.len()).then_some(picker.selected)
            }),
            checked: self
                .request
                .choices
                .checkboxes
                .iter()
                .map(|checkbox| checkbox.checked)
                .collect(),
        };
        self.request.completion.complete(answer);
        cx.emit(gpui::PromptResponse(index.unwrap_or(usize::MAX)));
        cx.notify();
    }
}

impl Drop for Dialog {
    fn drop(&mut self) {
        self.request.completion.complete(Answer::default());
    }
}

impl Focusable for Dialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input
            .as_ref()
            .map_or_else(|| self.focus.clone(), |input| input.focus_handle(cx))
    }
}

impl EventEmitter<gpui::PromptResponse> for Dialog {}

impl Render for Dialog {
    #[allow(
        clippy::too_many_lines,
        reason = "Declarative dialog controls and event wiring"
    )]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.dismissed && !self.focus.contains_focused(window, cx) {
            window.focus(&self.focus_handle(cx));
        }
        let theme = &self.request.theme;
        let metrics = &self.request.metrics;
        let style = Style { theme, metrics };
        let mut body = div()
            .id("dialog-body")
            .flex()
            .flex_col()
            .gap(metrics.spacing5())
            .max_h(px(480.0))
            .overflow_y_scroll()
            .child(
                div()
                    .text_size(metrics.font_headline())
                    .child(self.title.clone()),
            )
            .child(div().child(self.message.clone()));
        if let Some(input) = &self.input {
            body = body.child(controls::text_field(style, "dialog-input", input, None));
        }
        if let Some(picker) = &self.request.choices.picker {
            body = body.child(div().child(picker.label.clone()));
            for (index, label) in picker.items.iter().enumerate() {
                let selected = picker.selected == index;
                let weak = cx.entity().downgrade();
                body = body.child(controls::button(
                    style,
                    &format!("dialog-choice-{index}"),
                    controls::Button::secondary(&format!(
                        "{} {label}",
                        if selected { "●" } else { "○" }
                    )),
                    move |_, _, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            if let Some(picker) = &mut view.request.choices.picker {
                                picker.selected = index;
                            }
                            cx.notify();
                        });
                    },
                ));
            }
        }
        for (index, checkbox) in self.request.choices.checkboxes.iter().enumerate() {
            let weak = cx.entity().downgrade();
            let checked = checkbox.checked;
            let enabled = checkbox.enabled;
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.spacing4())
                    .when(!enabled, |row| row.opacity(0.5))
                    .child(controls::toggle(
                        style,
                        &format!("dialog-checkbox-{index}"),
                        checked,
                        move |_, _, cx| {
                            if enabled {
                                let _ = weak.update(cx, |view, cx| {
                                    if let Some(checkbox) =
                                        view.request.choices.checkboxes.get_mut(index)
                                    {
                                        checkbox.checked = !checkbox.checked;
                                    }
                                    cx.notify();
                                });
                            }
                        },
                    ))
                    .child(checkbox.label.clone()),
            );
        }
        let mut buttons = div()
            .flex()
            .flex_wrap()
            .when(self.buttons.len() > 3, Styled::flex_col)
            .justify_end()
            .gap(metrics.spacing4());
        for (index, button) in self.buttons.iter().enumerate() {
            let weak = cx.entity().downgrade();
            buttons = buttons.child(controls::button(
                style,
                &format!("dialog-answer-{index}"),
                controls::Button::secondary(button.label()),
                move |_, _, cx| {
                    let _ = weak.update(cx, |view, cx| view.finish(Some(index), cx));
                },
            ));
        }
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.backdrop)
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .track_focus(&self.focus)
                    .w(px(520.0))
                    .max_w_full()
                    .p(metrics.spacing8())
                    .flex()
                    .flex_col()
                    .gap(metrics.spacing8())
                    .rounded(metrics.radius_lg())
                    .bg(theme.bg)
                    .text_color(theme.fg)
                    .text_size(metrics.font_emphasis())
                    .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => view.finish(view.request.cancel, cx),
                            "enter" if view.input.is_none() => {
                                view.finish(view.request.default, cx);
                            }
                            _ => return,
                        }
                        cx.stop_propagation();
                    }))
                    .child(body)
                    .child(buttons),
            )
    }
}
