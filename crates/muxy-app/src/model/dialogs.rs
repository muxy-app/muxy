//! Dialogs on the app's windows, shown one at a time. A dialog asked for
//! while another is open waits its turn.

use std::collections::VecDeque;

use gpui::{AnyWindowHandle, AsyncApp, Context, Task};
use muxy_ui::dialog::Parent;

use super::AppModel;

/// One dialog asked for, to dismiss it before it is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DialogId(u64);

type Show = Box<dyn FnOnce(&mut AppModel, &mut Context<AppModel>) -> Task<()>>;

/// The open dialog and those waiting, in the order they were asked for.
#[derive(Default)]
pub(crate) struct Dialogs {
    last: u64,
    open: Option<(DialogId, Task<()>)>,
    waiting: VecDeque<(DialogId, Show)>,
}

impl Dialogs {
    pub(crate) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Whether `id` is open or waiting.
    pub(crate) fn pending(&self, id: DialogId) -> bool {
        self.open.as_ref().is_some_and(|(open, _)| *open == id)
            || self.waiting.iter().any(|(waiting, _)| *waiting == id)
    }
}

impl AppModel {
    /// Shows a dialog on `window` once those asked for before it are
    /// answered. `ask` shows it and waits; `answered` gets what it returns.
    pub(crate) fn ask_dialog<T: 'static>(
        &mut self,
        window: AnyWindowHandle,
        ask: impl AsyncFnOnce(&Parent, &mut AsyncApp) -> T + 'static,
        answered: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> DialogId {
        self.dialogs.last += 1;
        let id = DialogId(self.dialogs.last);
        let show: Show = Box::new(move |model, cx| {
            let parent = model.dialog_parent(window);
            cx.spawn(async move |model, cx| {
                let answer = ask(&parent, cx).await;
                let _ = model.update(cx, |model, cx| {
                    model.dialogs.open = None;
                    answered(model, answer, cx);
                    model.show_next_dialog(cx);
                    cx.notify();
                });
            })
        });
        self.dialogs.waiting.push_back((id, show));
        self.show_next_dialog(cx);
        id
    }

    /// Closes dialog `id` without an answer, or takes it out of the queue.
    pub(crate) fn dismiss_dialog(&mut self, id: DialogId, cx: &mut Context<Self>) {
        if self
            .dialogs
            .open
            .as_ref()
            .is_some_and(|(open, _)| *open == id)
        {
            self.dialogs.open = None;
            self.show_next_dialog(cx);
        } else {
            self.dialogs.waiting.retain(|(waiting, _)| *waiting != id);
        }
        cx.notify();
    }

    fn show_next_dialog(&mut self, cx: &mut Context<Self>) {
        if self.dialogs.is_open() {
            return;
        }
        if let Some((id, show)) = self.dialogs.waiting.pop_front() {
            let task = show(self, cx);
            self.dialogs.open = Some((id, task));
        }
    }
}
