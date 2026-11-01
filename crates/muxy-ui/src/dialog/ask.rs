//! Asking the user a question and waiting for the answer.

use std::io;

use gpui::{AnyWindowHandle, AsyncApp};

use super::{PromptChoices, PromptResponse};
use crate::controls::Style;

/// A question for [`ask`], answered by confirming or declining.
#[derive(Clone, Copy, Debug)]
pub enum Question<'a> {
    /// A warning answered with `confirm` or Cancel, with an optional checkbox
    /// that starts unchecked.
    Confirm {
        title: &'a str,
        message: &'a str,
        confirm: &'a str,
        checkbox: Option<&'a str>,
    },
    /// A choice between `confirm` and `decline`, with a checkbox that starts
    /// unchecked.
    Choose {
        title: &'a str,
        message: &'a str,
        confirm: &'a str,
        decline: &'a str,
        checkbox: &'a str,
    },
    /// A confirmation that also takes typed instructions and `choices`.
    Prompt {
        title: &'a str,
        message: &'a str,
        confirm: &'a str,
        choices: &'a PromptChoices,
        style: Style<'a>,
    },
}

impl PromptResponse {
    /// Whether the first checkbox was ticked.
    pub fn ticked(&self) -> bool {
        self.checked.first() == Some(&true)
    }
}

/// Shows `question` on `window` and waits for the answer: `None` when it was
/// declined or dismissed.
pub async fn ask(
    window: AnyWindowHandle,
    question: Question<'_>,
    cx: &mut AsyncApp,
) -> io::Result<Option<PromptResponse>> {
    #[cfg(any(test, feature = "test-support"))]
    if cx
        .update(|cx| cx.has_global::<Simulated>())
        .unwrap_or_default()
    {
        return simulated(window, question, cx).await;
    }
    let (sender, receiver) = async_channel::bounded(1);
    let _open_until_answered: Box<dyn std::any::Any> = window
        .update(cx, |_, window, cx| match question {
            Question::Confirm {
                title,
                message,
                confirm,
                checkbox,
            } => {
                let has_checkbox = checkbox.is_some();
                super::confirm(
                    window,
                    cx,
                    title,
                    message,
                    confirm,
                    checkbox,
                    move |answer| {
                        let _ = sender.try_send(match answer {
                            super::ConfirmationResponse::Confirmed { dont_ask_again } => {
                                Some(PromptResponse {
                                    checked: has_checkbox
                                        .then_some(dont_ask_again)
                                        .into_iter()
                                        .collect(),
                                    ..PromptResponse::default()
                                })
                            }
                            super::ConfirmationResponse::Cancelled => None,
                        });
                    },
                )
                .map(box_handle)
            }
            Question::Choose {
                title,
                message,
                confirm,
                decline,
                checkbox,
            } => super::confirm_with_checkbox(
                window,
                cx,
                title,
                message,
                [confirm, decline],
                checkbox,
                move |answer| {
                    let _ = sender.try_send(answer.map(|checked| PromptResponse {
                        checked: vec![checked],
                        ..PromptResponse::default()
                    }));
                },
            )
            .map(box_handle),
            Question::Prompt {
                title,
                message,
                confirm,
                choices,
                style,
            } => super::confirm_with_prompt(
                window,
                cx,
                title,
                message,
                confirm,
                choices,
                style,
                move |answer| {
                    let _ = sender.try_send(answer);
                },
            )
            .map(box_handle),
        })
        .map_err(|error| io::Error::other(error.to_string()))??;
    Ok(receiver.recv().await.ok().flatten())
}

fn box_handle<T: 'static>(handle: T) -> Box<dyn std::any::Any> {
    Box::new(handle)
}

#[cfg(any(test, feature = "test-support"))]
struct Simulated;

#[cfg(any(test, feature = "test-support"))]
impl gpui::Global for Simulated {}

/// Sends every later [`ask`] through `Window::prompt`, so a test answers it
/// with `simulate_prompt_answer`: the confirm or decline label, or
/// [`ticked_label`] to confirm with the checkbox ticked.
#[cfg(any(test, feature = "test-support"))]
pub fn simulate(cx: &mut gpui::App) {
    cx.set_global(Simulated);
}

/// The simulated answer that confirms with the checkbox ticked.
#[cfg(any(test, feature = "test-support"))]
pub fn ticked_label(confirm: &str, checkbox: &str) -> String {
    format!("{confirm} + {checkbox}")
}

#[cfg(any(test, feature = "test-support"))]
async fn simulated(
    window: AnyWindowHandle,
    question: Question<'_>,
    cx: &mut AsyncApp,
) -> io::Result<Option<PromptResponse>> {
    let cancel = crate::tr!("Cancel");
    let (title, message, confirm, decline, checkbox, choices) = match question {
        Question::Confirm {
            title,
            message,
            confirm,
            checkbox,
        } => (title, message, confirm, cancel.as_ref(), checkbox, None),
        Question::Choose {
            title,
            message,
            confirm,
            decline,
            checkbox,
        } => (title, message, confirm, decline, Some(checkbox), None),
        Question::Prompt {
            title,
            message,
            confirm,
            choices,
            ..
        } => (
            title,
            message,
            confirm,
            cancel.as_ref(),
            None,
            Some(choices),
        ),
    };
    let ticked = checkbox.map(|checkbox| ticked_label(confirm, checkbox));
    let mut buttons = vec![confirm, decline];
    buttons.extend(ticked.as_deref());
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                title,
                Some(message),
                &buttons,
                cx,
            )
        })
        .map_err(|error| io::Error::other(error.to_string()))?
        .await;
    let checked = |ticked: bool| checkbox.map(|_| ticked).into_iter().collect();
    Ok(match answer {
        Ok(0) => Some(match choices {
            Some(choices) => PromptResponse {
                prompt: String::new(),
                picked: choices.picker.as_ref().map(|picker| picker.selected),
                checked: choices
                    .checkboxes
                    .iter()
                    .map(|checkbox| checkbox.checked)
                    .collect(),
            },
            None => PromptResponse {
                checked: checked(false),
                ..PromptResponse::default()
            },
        }),
        Ok(2) => Some(PromptResponse {
            checked: checked(true),
            ..PromptResponse::default()
        }),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, IntoElement, Render, TestAppContext, VisualTestContext, Window, div};

    struct Host;

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn answer(
        question: Question<'static>,
        button: &str,
        cx: &mut VisualTestContext,
    ) -> Option<PromptResponse> {
        let answer = std::rc::Rc::new(std::cell::RefCell::new(None));
        let result = answer.clone();
        cx.update(|window, cx| {
            let window = window.window_handle();
            cx.spawn(async move |cx| {
                *result.borrow_mut() = Some(ask(window, question, cx).await.expect("asked"));
            })
            .detach();
        });
        cx.run_until_parked();
        cx.simulate_prompt_answer(button);
        cx.run_until_parked();
        answer.take().expect("answered")
    }

    #[gpui::test]
    fn the_test_hook_answers_with_the_chosen_button(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Host);
        cx.update(|_, cx| simulate(cx));
        let confirm = Question::Confirm {
            title: "Close Tab?",
            message: "A process is running.",
            confirm: "Close",
            checkbox: Some("Don't ask again"),
        };
        assert_eq!(answer(confirm, "Cancel", cx), None);
        assert_eq!(
            answer(confirm, "Close", cx),
            Some(PromptResponse {
                checked: vec![false],
                ..PromptResponse::default()
            })
        );
        let ticked = ticked_label("Close", "Don't ask again");
        assert!(answer(confirm, &ticked, cx).is_some_and(|answer| answer.ticked()));
        let choose = Question::Choose {
            title: "Muxy Update",
            message: "Install?",
            confirm: "Update",
            decline: "Not Now",
            checkbox: "Also restart the server",
        };
        assert_eq!(answer(choose, "Not Now", cx), None);
        assert_eq!(
            answer(choose, "Update", cx),
            Some(PromptResponse {
                checked: vec![false],
                ..PromptResponse::default()
            })
        );
        let ticked = ticked_label("Update", "Also restart the server");
        assert!(answer(choose, &ticked, cx).is_some_and(|answer| answer.ticked()));
    }

    #[gpui::test]
    fn the_test_hook_confirms_a_prompt_with_its_starting_choices(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Host);
        cx.update(|_, cx| simulate(cx));
        let choices: &'static PromptChoices = Box::leak(Box::new(PromptChoices {
            picker: Some(super::super::PromptPicker {
                label: "AI Provider".into(),
                items: vec!["First".into(), "Second".into()],
                selected: 1,
            }),
            checkboxes: vec![
                super::super::PromptCheckbox {
                    label: "Include unstaged changes".into(),
                    checked: false,
                    enabled: true,
                },
                super::super::PromptCheckbox {
                    label: "Push after committing".into(),
                    checked: true,
                    enabled: true,
                },
            ],
        }));
        let theme: &'static crate::theme::Theme = Box::leak(Box::new(
            crate::theme::Theme::from_scheme(&crate::theme::ColorScheme::default()),
        ));
        let metrics: &'static crate::theme::Metrics =
            Box::leak(Box::new(crate::theme::Metrics::new(1.0)));
        let prompt = Question::Prompt {
            title: "Commit",
            message: "Commit the staged changes?",
            confirm: "Commit",
            choices,
            style: Style { theme, metrics },
        };
        assert_eq!(answer(prompt, "Cancel", cx), None);
        assert_eq!(
            answer(prompt, "Commit", cx),
            Some(PromptResponse {
                prompt: String::new(),
                picked: Some(1),
                checked: vec![false, true],
            })
        );
    }
}
