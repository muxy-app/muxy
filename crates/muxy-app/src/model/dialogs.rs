//! Dialogs on the app's windows, shown one at a time. A dialog asked for
//! while another is open waits its turn.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::pin;
use std::task::Poll;

use gpui::{AnyWindowHandle, AsyncApp, Context, Task};
use muxy_app_core::ServerId;
use muxy_protocol::DeviceId;
use muxy_ui::dialog::Parent;

use super::AppModel;

/// One dialog asked for, to dismiss it before it is answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DialogId(u64);

/// A question asked once at a time: asking it again while it is open or
/// waiting adds nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Question {
    /// Installing this version.
    Update(String),
    TerminalReset,
    /// Stopping the server, or restarting it.
    ServerControl(ServerId, bool),
    RevokeDevice(DeviceId),
    ForgetServers(Vec<ServerId>),
    InstallServer(ServerId),
    /// Installing the server the form names.
    InstallFormServer(String),
}

type Show =
    Box<dyn FnOnce(&mut AppModel, async_channel::Receiver<()>, &mut Context<AppModel>) -> Task<()>>;

struct Waiting {
    id: DialogId,
    question: Option<Question>,
    window: AnyWindowHandle,
    show: Show,
}

struct Open {
    id: DialogId,
    question: Option<Question>,
    window: AnyWindowHandle,
    /// Closes the dialog from inside its task, so the dialog is gone before
    /// the next one opens on the same window.
    dismiss: async_channel::Sender<()>,
    _task: Task<()>,
}

/// The open dialog and those waiting, in the order they were asked for.
#[derive(Default)]
pub(crate) struct Dialogs {
    last: u64,
    open: Option<Open>,
    waiting: VecDeque<Waiting>,
}

impl Dialogs {
    pub(crate) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn asking(&self, question: &Question) -> bool {
        self.open
            .as_ref()
            .is_some_and(|open| open.question.as_ref() == Some(question))
            || self
                .waiting
                .iter()
                .any(|waiting| waiting.question.as_ref() == Some(question))
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
        self.queue_dialog(None, window, ask, answered, cx)
    }

    /// Like [`Self::ask_dialog`], unless `question` is already open or
    /// waiting.
    pub(crate) fn ask_once<T: 'static>(
        &mut self,
        question: Question,
        window: AnyWindowHandle,
        ask: impl AsyncFnOnce(&Parent, &mut AsyncApp) -> T + 'static,
        answered: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        if !self.dialogs.asking(&question) {
            self.queue_dialog(Some(question), window, ask, answered, cx);
        }
    }

    fn queue_dialog<T: 'static>(
        &mut self,
        question: Option<Question>,
        window: AnyWindowHandle,
        ask: impl AsyncFnOnce(&Parent, &mut AsyncApp) -> T + 'static,
        answered: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> DialogId {
        self.dialogs.last += 1;
        let id = DialogId(self.dialogs.last);
        let show: Show = Box::new(move |model, dismissed, cx| {
            let parent = model.dialog_parent(window);
            cx.spawn(async move |model, cx| {
                let answer = {
                    let mut asking = pin!(ask(&parent, cx));
                    let mut dismissed = pin!(dismissed.recv());
                    std::future::poll_fn(|context| {
                        if dismissed.as_mut().poll(context).is_ready() {
                            Poll::Ready(None)
                        } else {
                            asking.as_mut().poll(context).map(Some)
                        }
                    })
                    .await
                };
                let _ = model.update(cx, |model, cx| {
                    model.dialogs.open = None;
                    if let Some(answer) = answer {
                        answered(model, answer, cx);
                    }
                    model.show_next_dialog(cx);
                    cx.notify();
                });
            })
        });
        self.dialogs.waiting.push_back(Waiting {
            id,
            question,
            window,
            show,
        });
        self.show_next_dialog(cx);
        id
    }

    /// Closes dialog `id` without an answer, or takes it out of the queue.
    pub(crate) fn dismiss_dialog(&mut self, id: DialogId, cx: &mut Context<Self>) {
        if let Some(open) = self.dialogs.open.as_ref().filter(|open| open.id == id) {
            let _ = open.dismiss.try_send(());
        } else {
            self.dialogs.waiting.retain(|waiting| waiting.id != id);
        }
        cx.notify();
    }

    /// `window` closed: its dialogs go unanswered.
    pub(crate) fn window_closed(&mut self, window: AnyWindowHandle, cx: &mut Context<Self>) {
        self.dialogs
            .waiting
            .retain(|waiting| waiting.window != window);
        let open = self.dialogs.open.as_ref();
        if let Some(id) = open
            .filter(|open| open.window == window)
            .map(|open| open.id)
        {
            self.dismiss_dialog(id, cx);
        }
    }

    fn show_next_dialog(&mut self, cx: &mut Context<Self>) {
        if self.dialogs.is_open() {
            return;
        }
        if let Some(next) = self.dialogs.waiting.pop_front() {
            let (dismiss, dismissed) = async_channel::bounded(1);
            let task = (next.show)(self, dismissed, cx);
            self.dialogs.open = Some(Open {
                id: next.id,
                question: next.question,
                window: next.window,
                dismiss,
                _task: task,
            });
        }
    }
}
