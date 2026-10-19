use std::{cell::RefCell, collections::HashMap, io, rc::Rc};

use gpui::{App, AppContext, AsyncApp, Global, WeakEntity, Window, WindowId};

pub use crate::dialog_types::{
    ADDITIONAL_PROMPT_LIMIT, ConfirmationResponse, ConsentResponse, DialogOptions, PromptCheckbox,
    PromptChoices, PromptPicker, PromptResponse,
};
use crate::{
    controls::Style,
    theme::{ColorScheme, Metrics, Theme},
    tr,
};

#[cfg(target_os = "linux")]
mod picker;
#[cfg(windows)]
#[path = "system_picker.rs"]
mod picker;
mod view;

pub use picker::{FolderPicker, choose_file, choose_folder, choose_folder_with};

#[derive(Default)]
struct Dialogs {
    active: HashMap<WindowId, WeakEntity<view::Dialog>>,
    pending: Option<Request>,
}

impl Global for Dialogs {}

#[derive(Default)]
struct Answer {
    index: Option<usize>,
    prompt: String,
    picked: Option<usize>,
    checked: Vec<bool>,
}

type Callback = Box<dyn FnOnce(Answer)>;

#[derive(Clone, Default)]
struct Completion(Rc<RefCell<Option<Callback>>>);

impl Completion {
    fn complete(&self, answer: Answer) {
        let callback = self.0.borrow_mut().take();
        if let Some(callback) = callback {
            callback(answer);
        }
    }

    fn pending(&self) -> bool {
        self.0.borrow().is_some()
    }
}

struct Request {
    choices: PromptChoices,
    input: Option<(String, String)>,
    input_limit: Option<usize>,
    cancel: Option<usize>,
    default: Option<usize>,
    completion: Completion,
    theme: Theme,
    metrics: Metrics,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            choices: PromptChoices::default(),
            input: None,
            input_limit: None,
            cancel: None,
            default: None,
            completion: Completion::default(),
            theme: Theme::from_scheme(&ColorScheme::default()),
            metrics: Metrics::new(1.0),
        }
    }
}

pub struct Confirmation {
    view: WeakEntity<view::Dialog>,
    completion: Completion,
    cx: AsyncApp,
}

impl std::fmt::Debug for Confirmation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Confirmation")
            .field("pending", &self.completion.pending())
            .finish_non_exhaustive()
    }
}

impl Drop for Confirmation {
    fn drop(&mut self) {
        let Some(callback) = self.completion.0.borrow_mut().take() else {
            return;
        };
        let view = self.view.clone();
        self.cx
            .spawn(async move |cx| {
                let _ = view.update(cx, view::Dialog::dismiss);
                callback(Answer::default());
            })
            .detach();
    }
}

pub type PromptConfirmation = Confirmation;

fn show(
    window: &mut Window,
    cx: &mut App,
    title: &str,
    message: &str,
    buttons: &[String],
    mut request: Request,
    callback: impl FnOnce(Answer) + 'static,
) -> io::Result<Confirmation> {
    if !cx.has_global::<Dialogs>() {
        cx.set_global(Dialogs::default());
        cx.set_prompt_builder(|_, title, message, buttons, handle, window, cx| {
            let request = cx
                .global_mut::<Dialogs>()
                .pending
                .take()
                .unwrap_or_default();
            let view = cx.new(|cx| view::Dialog::new(title, message, buttons, request, cx));
            cx.global_mut::<Dialogs>()
                .active
                .insert(window.window_handle().window_id(), view.downgrade());
            handle.with_view(view, window, cx)
        });
    }
    let id = window.window_handle().window_id();
    cx.global_mut::<Dialogs>()
        .active
        .retain(|_, view| view.upgrade().is_some());
    let active = cx
        .global::<Dialogs>()
        .active
        .get(&id)
        .and_then(WeakEntity::upgrade);
    if active.is_some_and(|view| view.read(cx).pending()) {
        return Err(io::Error::other("an application dialog is already open"));
    }
    let completion = Completion(Rc::new(RefCell::new(Some(Box::new(callback)))));
    request.completion = completion.clone();
    cx.global_mut::<Dialogs>().pending = Some(request);
    let _response = window.prompt(
        gpui::PromptLevel::Warning,
        title,
        Some(message),
        &buttons.iter().map(String::as_str).collect::<Vec<_>>(),
        cx,
    );
    let view = cx
        .global::<Dialogs>()
        .active
        .get(&id)
        .cloned()
        .ok_or_else(|| io::Error::other("could not open dialog"))?;
    window.refresh();
    Ok(Confirmation {
        view,
        completion,
        cx: cx.to_async(),
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "Confirmation fields and GPUI context"
)]
pub fn confirm(
    window: &mut Window,
    cx: &mut App,
    title: &str,
    message: &str,
    confirm_label: &str,
    suppression_label: Option<&str>,
    on_complete: impl FnOnce(ConfirmationResponse) + 'static,
) -> io::Result<Confirmation> {
    let choices = PromptChoices {
        checkboxes: suppression_label
            .into_iter()
            .map(|label| PromptCheckbox {
                label: label.into(),
                checked: false,
                enabled: true,
            })
            .collect(),
        ..PromptChoices::default()
    };
    show(
        window,
        cx,
        title,
        message,
        &[confirm_label.into(), tr!("Cancel").to_string()],
        Request {
            choices,
            cancel: Some(1),
            default: Some(0),
            ..Request::default()
        },
        move |answer| {
            on_complete(if answer.index == Some(0) {
                ConfirmationResponse::Confirmed {
                    dont_ask_again: answer.checked.first() == Some(&true),
                }
            } else {
                ConfirmationResponse::Cancelled
            });
        },
    )
}

pub fn confirm_with_checkbox(
    window: &mut Window,
    cx: &mut App,
    title: &str,
    message: &str,
    labels: [&str; 2],
    checkbox_label: &str,
    on_complete: impl FnOnce(Option<bool>) + 'static,
) -> io::Result<Confirmation> {
    show(
        window,
        cx,
        title,
        message,
        &labels.map(str::to_owned),
        Request {
            choices: PromptChoices {
                picker: None,
                checkboxes: vec![PromptCheckbox {
                    label: checkbox_label.into(),
                    checked: false,
                    enabled: true,
                }],
            },
            cancel: Some(1),
            default: Some(0),
            ..Request::default()
        },
        move |answer| {
            on_complete((answer.index == Some(0)).then(|| answer.checked.first() == Some(&true)));
        },
    )
}

pub fn consent(
    window: &mut Window,
    cx: &mut App,
    title: &str,
    message: &str,
    block_label: &str,
    on_complete: impl FnOnce(ConsentResponse) + 'static,
) -> io::Result<Confirmation> {
    show(
        window,
        cx,
        title,
        message,
        &[
            tr!("Allow & Remember").to_string(),
            tr!("Allow").to_string(),
            tr!("Cancel").to_string(),
            tr!("Deny & Remember").to_string(),
        ],
        Request {
            choices: PromptChoices {
                picker: None,
                checkboxes: vec![PromptCheckbox {
                    label: block_label.into(),
                    checked: false,
                    enabled: true,
                }],
            },
            cancel: Some(2),
            default: Some(2),
            ..Request::default()
        },
        move |answer| {
            on_complete(
                match (answer.index, answer.checked.first() == Some(&true)) {
                    (None | Some(2), _) => ConsentResponse::Cancel,
                    (_, true) => ConsentResponse::Block,
                    (Some(0), false) => ConsentResponse::AllowAndRemember,
                    (Some(1), false) => ConsentResponse::Allow,
                    (Some(3), false) => ConsentResponse::DenyAndRemember,
                    _ => ConsentResponse::Cancel,
                },
            );
        },
    )
}

#[allow(clippy::too_many_arguments, reason = "Prompt fields and GPUI context")]
pub fn confirm_with_prompt(
    window: &mut Window,
    cx: &mut App,
    title: &str,
    message: &str,
    confirm_label: &str,
    choices: &PromptChoices,
    style: Style<'_>,
    on_complete: impl FnOnce(Option<PromptResponse>) + 'static,
) -> io::Result<PromptConfirmation> {
    show(
        window,
        cx,
        title,
        message,
        &[confirm_label.into(), tr!("Cancel").to_string()],
        Request {
            choices: choices.clone(),
            input: Some((String::new(), tr!("Additional instructions").to_string())),
            input_limit: Some(ADDITIONAL_PROMPT_LIMIT),
            cancel: Some(1),
            default: Some(0),
            theme: style.theme.clone(),
            metrics: *style.metrics,
            ..Request::default()
        },
        move |answer| {
            on_complete((answer.index == Some(0)).then_some(PromptResponse {
                prompt: answer.prompt,
                picked: answer.picked,
                checked: answer.checked,
            }));
        },
    )
}

pub fn present(
    window: &mut Window,
    cx: &mut App,
    options: DialogOptions,
    on_complete: impl FnOnce(Option<String>) + 'static,
) -> io::Result<Confirmation> {
    let cancel = options
        .buttons
        .iter()
        .position(|label| Some(label) == options.cancel_button.as_ref());
    let default = options
        .buttons
        .iter()
        .position(|label| Some(label) == options.default_button.as_ref())
        .or(Some(0));
    let input = options.input.is_some();
    let labels = options.buttons.clone();
    show(
        window,
        cx,
        &options.title,
        &options.message,
        &options.buttons,
        Request {
            input: options.input,
            cancel,
            default,
            ..Request::default()
        },
        move |answer| {
            on_complete(
                answer
                    .index
                    .filter(|index| Some(*index) != cancel)
                    .and_then(|index| {
                        if input {
                            Some(answer.prompt)
                        } else {
                            labels.get(index).cloned()
                        }
                    }),
            );
        },
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "Regression fixture setup")]
mod tests;
