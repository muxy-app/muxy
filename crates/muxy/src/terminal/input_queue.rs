use crate::terminal::TerminalSurfaces;
use crate::views::window::MainWindow;
use gpui::Context;
use muxy_terminal::input::{
    CARRIAGE_RETURN, PASTE_SHORTCUT, QueuedInputTransaction, TerminalInputError,
    TerminalInputQueue, TerminalInputResult, TerminalInputStep, TerminalInputTransaction,
    bracketed_text_bytes, clear_input_bytes,
};
use std::collections::HashMap;
use std::time::Duration;

pub const INITIAL_INPUT_DELAY: Duration = Duration::from_millis(50);
pub const IMAGE_PASTE_DELAY: Duration = Duration::from_millis(300);
const PASTEBOARD_QUEUE_POLL_DELAY: Duration = Duration::from_millis(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PasteboardInputId {
    tab_id: String,
    generation: u64,
    transaction_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PasteboardStepState {
    Acquired,
    Waiting,
    Cancelled,
}

pub(crate) struct PaneInputQueue {
    generation: u64,
    state: TerminalInputQueue,
    completions: HashMap<u64, async_channel::Sender<TerminalInputResult>>,
    pasteboard_active: bool,
    cancelled: bool,
}

impl PaneInputQueue {
    fn new(generation: u64) -> Self {
        Self {
            generation,
            state: TerminalInputQueue::default(),
            completions: HashMap::new(),
            pasteboard_active: false,
            cancelled: false,
        }
    }
}

impl TerminalSurfaces {
    pub(crate) fn enqueue_input_transaction(
        &mut self,
        tab_id: &str,
        transaction: TerminalInputTransaction,
    ) -> (async_channel::Receiver<TerminalInputResult>, Option<u64>) {
        let (sender, receiver) = async_channel::bounded(1);
        if self.handle(tab_id).is_none() {
            let _ = sender.try_send(Err(TerminalInputError::MissingSurface));
            return (receiver, None);
        }
        let generation = if let Some(queue) = self.input_queues.get(tab_id) {
            queue.generation
        } else {
            self.input_queue_generation = self.input_queue_generation.saturating_add(1).max(1);
            let generation = self.input_queue_generation;
            self.input_queues
                .insert(tab_id.to_owned(), PaneInputQueue::new(generation));
            generation
        };
        let queue = self.input_queues.get_mut(tab_id).unwrap();
        let start_worker = queue.state.is_idle();
        let id = queue.state.enqueue(transaction);
        queue.completions.insert(id, sender);
        if start_worker {
            self.set_idle_input_transaction(tab_id, true);
            if let Some(handle) = self.handle(tab_id) {
                handle.set_input_transaction_active(true);
            }
        }
        (receiver, start_worker.then_some(generation))
    }

    pub(crate) fn active_input_transaction(
        &self,
        tab_id: &str,
        generation: u64,
    ) -> Option<QueuedInputTransaction> {
        let queue = self.input_queues.get(tab_id)?;
        (queue.generation == generation)
            .then(|| queue.state.active().cloned())
            .flatten()
    }

    pub(crate) fn send_input_step(
        &self,
        tab_id: &str,
        generation: u64,
        id: u64,
        step: &TerminalInputStep,
    ) -> TerminalInputResult {
        if self.input_queues.get(tab_id).is_none_or(|queue| {
            queue.generation != generation
                || queue.state.active().map(|active| active.id) != Some(id)
                || queue.cancelled
        }) {
            return Err(TerminalInputError::Cancelled);
        }
        let bytes = match step {
            TerminalInputStep::ClearInput { submitted_lines } => {
                clear_input_bytes(*submitted_lines)
            }
            TerminalInputStep::RawBytes(bytes) => bytes.clone(),
            TerminalInputStep::BracketedText(text) => bracketed_text_bytes(text),
            TerminalInputStep::PastePng(_) => return Err(TerminalInputError::UnsupportedImage),
        };
        self.send_bytes(tab_id, &bytes)
            .then_some(())
            .ok_or(TerminalInputError::SendFailed)
    }

    pub(crate) fn begin_pasteboard_step(
        &mut self,
        tab_id: &str,
        generation: u64,
        id: u64,
    ) -> PasteboardStepState {
        let identity = PasteboardInputId {
            tab_id: tab_id.to_owned(),
            generation,
            transaction_id: id,
        };
        let valid = self.input_queues.get(tab_id).is_some_and(|queue| {
            queue.generation == generation
                && queue.state.active().map(|active| active.id) == Some(id)
                && !queue.cancelled
        });
        if !valid {
            self.pasteboard_waiting
                .retain(|candidate| candidate != &identity);
            return PasteboardStepState::Cancelled;
        }
        if self.pasteboard_owner.as_ref() == Some(&identity) {
            if let Some(queue) = self.input_queues.get_mut(tab_id) {
                queue.pasteboard_active = true;
            }
            return PasteboardStepState::Acquired;
        }
        if !self
            .pasteboard_waiting
            .iter()
            .any(|candidate| candidate == &identity)
        {
            self.pasteboard_waiting.push_back(identity.clone());
        }
        if self.pasteboard_owner.is_none() && self.pasteboard_waiting.front() == Some(&identity) {
            self.pasteboard_waiting.pop_front();
            self.pasteboard_owner = Some(identity);
            if let Some(queue) = self.input_queues.get_mut(tab_id) {
                queue.pasteboard_active = true;
            }
            PasteboardStepState::Acquired
        } else {
            PasteboardStepState::Waiting
        }
    }

    pub(crate) fn finish_pasteboard_step(&mut self, tab_id: &str, generation: u64, id: u64) {
        let identity = PasteboardInputId {
            tab_id: tab_id.to_owned(),
            generation,
            transaction_id: id,
        };
        self.pasteboard_waiting
            .retain(|candidate| candidate != &identity);
        if self.pasteboard_owner.as_ref() == Some(&identity) {
            self.pasteboard_owner = None;
        }
        if let Some(queue) = self.input_queues.get_mut(tab_id)
            && queue.generation == generation
            && queue.state.active().map(|active| active.id) == Some(id)
        {
            queue.pasteboard_active = false;
            return;
        }
        if let Some(handle) = self.handle(tab_id) {
            handle.cancel_input_transaction();
        }
    }

    pub(crate) fn input_transaction_cancelled(
        &self,
        tab_id: &str,
        generation: u64,
        id: u64,
    ) -> bool {
        self.input_queues.get(tab_id).is_none_or(|queue| {
            queue.generation != generation
                || queue.state.active().map(|active| active.id) != Some(id)
                || queue.cancelled
        })
    }

    pub(crate) fn send_input_rollback(
        &self,
        tab_id: &str,
        generation: u64,
        id: u64,
        submitted_lines: usize,
    ) -> TerminalInputResult {
        self.send_input_step(
            tab_id,
            generation,
            id,
            &TerminalInputStep::ClearInput { submitted_lines },
        )
    }

    pub(crate) fn send_input_return(
        &self,
        tab_id: &str,
        generation: u64,
        id: u64,
    ) -> TerminalInputResult {
        if self.input_queues.get(tab_id).is_none_or(|queue| {
            queue.generation != generation
                || queue.state.active().map(|active| active.id) != Some(id)
                || queue.cancelled
        }) {
            return Err(TerminalInputError::Cancelled);
        }
        self.send_bytes(tab_id, CARRIAGE_RETURN)
            .then_some(())
            .ok_or(TerminalInputError::SendFailed)
    }

    pub(crate) fn complete_input_transaction(
        &mut self,
        tab_id: &str,
        generation: u64,
        id: u64,
        result: TerminalInputResult,
    ) -> bool {
        let Some(queue) = self.input_queues.get_mut(tab_id) else {
            return false;
        };
        if queue.generation != generation || !queue.state.complete(id) {
            return false;
        }
        if let Some(completion) = queue.completions.remove(&id) {
            let _ = completion.try_send(result);
        }
        if queue.state.is_idle() {
            self.input_queues.remove(tab_id);
            self.set_idle_input_transaction(tab_id, false);
            if let Some(handle) = self.handle(tab_id) {
                handle.set_input_transaction_active(false);
            }
            false
        } else {
            true
        }
    }

    pub(crate) fn cancel_input_queue(&mut self, tab_id: &str) {
        let active_identity = self.input_queues.get(tab_id).and_then(|queue| {
            queue.state.active().map(|active| PasteboardInputId {
                tab_id: tab_id.to_owned(),
                generation: queue.generation,
                transaction_id: active.id,
            })
        });
        if self
            .input_queues
            .get(tab_id)
            .is_some_and(|queue| queue.pasteboard_active)
        {
            let queue = self.input_queues.get_mut(tab_id).unwrap();
            queue.cancelled = true;
            for id in queue.state.cancel_pending() {
                if let Some(completion) = queue.completions.remove(&id) {
                    let _ = completion.try_send(Err(TerminalInputError::Cancelled));
                }
            }
            return;
        }
        if let Some(identity) = active_identity {
            self.pasteboard_waiting
                .retain(|candidate| candidate != &identity);
            if self.pasteboard_owner.as_ref() == Some(&identity) {
                self.pasteboard_owner = None;
            }
        }
        let Some(mut queue) = self.input_queues.remove(tab_id) else {
            return;
        };
        for id in queue.state.cancel_all() {
            if let Some(completion) = queue.completions.remove(&id) {
                let _ = completion.try_send(Err(TerminalInputError::Cancelled));
            }
        }
        self.set_idle_input_transaction(tab_id, false);
        if let Some(handle) = self.handle(tab_id) {
            handle.cancel_input_transaction();
        }
    }
}

impl MainWindow {
    pub(crate) fn enqueue_terminal_input(
        &mut self,
        tab_id: String,
        transaction: TerminalInputTransaction,
        cx: &mut Context<Self>,
    ) -> async_channel::Receiver<TerminalInputResult> {
        let (completion, worker_generation) = self
            .terminal_runtime
            .surfaces
            .enqueue_input_transaction(&tab_id, transaction);
        if let Some(generation) = worker_generation {
            self.start_terminal_input_worker(tab_id, generation, cx);
        }
        completion
    }

    fn start_terminal_input_worker(
        &mut self,
        tab_id: String,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |window, cx| {
            loop {
                let active = window
                    .update(cx, |window, _| {
                        window
                            .terminal_runtime
                            .surfaces
                            .active_input_transaction(&tab_id, generation)
                    })
                    .ok()
                    .flatten();
                let Some(active) = active else {
                    return;
                };
                let mut result = Ok(());
                let mut submitted_lines = 0;
                for step in &active.transaction.steps {
                    if let TerminalInputStep::PastePng(png) = step {
                        loop {
                            let state = window
                                .update(cx, |window, _| {
                                    window
                                        .terminal_runtime
                                        .surfaces
                                        .begin_pasteboard_step(&tab_id, generation, active.id)
                                })
                                .unwrap_or(PasteboardStepState::Cancelled);
                            match state {
                                PasteboardStepState::Acquired => break,
                                PasteboardStepState::Waiting => {
                                    cx.background_executor()
                                        .timer(PASTEBOARD_QUEUE_POLL_DELAY)
                                        .await;
                                }
                                PasteboardStepState::Cancelled => {
                                    result = Err(TerminalInputError::Cancelled);
                                    break;
                                }
                            }
                        }
                        if result.is_ok() {
                            let replacement = window
                                .update(cx, |_, _| crate::pasteboard::replace_with_png(png))
                                .ok()
                                .and_then(Result::ok);
                            match replacement {
                                Some(replacement) => {
                                    result = window
                                        .update(cx, |window, _| {
                                            window.terminal_runtime.surfaces.send_input_step(
                                                &tab_id,
                                                generation,
                                                active.id,
                                                &TerminalInputStep::RawBytes(
                                                    PASTE_SHORTCUT.to_vec(),
                                                ),
                                            )
                                        })
                                        .unwrap_or(Err(TerminalInputError::Cancelled));
                                    cx.background_executor().timer(IMAGE_PASTE_DELAY).await;
                                    let restored = window
                                        .update(cx, |_, _| replacement.restore())
                                        .ok()
                                        .is_some_and(|result| result.is_ok());
                                    let cancelled = window
                                        .update(cx, |window, _| {
                                            window
                                                .terminal_runtime
                                                .surfaces
                                                .input_transaction_cancelled(
                                                    &tab_id, generation, active.id,
                                                )
                                        })
                                        .unwrap_or(true);
                                    if result.is_ok() && !restored {
                                        result = Err(TerminalInputError::SendFailed);
                                    } else if result.is_ok() && cancelled {
                                        result = Err(TerminalInputError::Cancelled);
                                    }
                                }
                                None => result = Err(TerminalInputError::UnsupportedImage),
                            }
                            let _ = window.update(cx, |window, _| {
                                window
                                    .terminal_runtime
                                    .surfaces
                                    .finish_pasteboard_step(&tab_id, generation, active.id);
                            });
                        }
                    } else {
                        result = window
                            .update(cx, |window, _| {
                                window
                                    .terminal_runtime
                                    .surfaces
                                    .send_input_step(&tab_id, generation, active.id, step)
                            })
                            .unwrap_or(Err(TerminalInputError::Cancelled));
                    }
                    if result.is_err() {
                        break;
                    }
                    match step {
                        TerminalInputStep::ClearInput { .. } => {
                            cx.background_executor().timer(INITIAL_INPUT_DELAY).await;
                        }
                        TerminalInputStep::BracketedText(text) => {
                            submitted_lines +=
                                text.chars().filter(|character| *character == '\n').count();
                        }
                        TerminalInputStep::RawBytes(_) | TerminalInputStep::PastePng(_) => {}
                    }
                }
                if result.is_err()
                    && result != Err(TerminalInputError::Cancelled)
                    && active.transaction.rollback_on_failure
                {
                    let _ = window.update(cx, |window, _| {
                        window.terminal_runtime.surfaces.send_input_rollback(
                            &tab_id,
                            generation,
                            active.id,
                            submitted_lines,
                        )
                    });
                }
                if result.is_ok() && active.transaction.append_return {
                    result = window
                        .update(cx, |window, _| {
                            window
                                .terminal_runtime
                                .surfaces
                                .send_input_return(&tab_id, generation, active.id)
                        })
                        .unwrap_or(Err(TerminalInputError::Cancelled));
                }
                let has_next = window
                    .update(cx, |window, _| {
                        window
                            .terminal_runtime
                            .surfaces
                            .complete_input_transaction(&tab_id, generation, active.id, result)
                    })
                    .unwrap_or(false);
                if !has_next {
                    return;
                }
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_transaction_delays_match_the_retained_windows() {
        assert_eq!(INITIAL_INPUT_DELAY, Duration::from_millis(50));
        assert_eq!(IMAGE_PASTE_DELAY, Duration::from_millis(300));
    }
}
