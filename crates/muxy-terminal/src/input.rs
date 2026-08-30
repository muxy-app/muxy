use std::collections::VecDeque;

pub const BRACKETED_PASTE_START: &[u8] = b"\x1b[200~";
pub const BRACKETED_PASTE_END: &[u8] = b"\x1b[201~";
pub const CARRIAGE_RETURN: &[u8] = b"\r";
pub const PASTE_SHORTCUT: &[u8] = b"\x16";
pub const KILL_INPUT: u8 = 0x15;
pub const BACKSPACE: u8 = 0x08;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalInputStep {
    ClearInput { submitted_lines: usize },
    RawBytes(Vec<u8>),
    BracketedText(String),
    PastePng(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalInputTransaction {
    pub steps: Vec<TerminalInputStep>,
    pub append_return: bool,
    pub rollback_on_failure: bool,
}

impl TerminalInputTransaction {
    pub fn new(steps: Vec<TerminalInputStep>, append_return: bool) -> Self {
        Self {
            steps,
            append_return,
            rollback_on_failure: false,
        }
    }

    pub fn with_rollback_on_failure(mut self) -> Self {
        self.rollback_on_failure = true;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalInputError {
    Cancelled,
    MissingSurface,
    SendFailed,
    UnsupportedImage,
}

pub type TerminalInputResult = Result<(), TerminalInputError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedInputTransaction {
    pub id: u64,
    pub transaction: TerminalInputTransaction,
}

#[derive(Default)]
pub struct TerminalInputQueue {
    next_id: u64,
    active: Option<QueuedInputTransaction>,
    pending: VecDeque<QueuedInputTransaction>,
}

impl TerminalInputQueue {
    pub fn enqueue(&mut self, transaction: TerminalInputTransaction) -> u64 {
        self.next_id = self.next_id.saturating_add(1).max(1);
        let queued = QueuedInputTransaction {
            id: self.next_id,
            transaction,
        };
        if self.active.is_none() {
            self.active = Some(queued);
        } else {
            self.pending.push_back(queued);
        }
        self.next_id
    }

    pub fn active(&self) -> Option<&QueuedInputTransaction> {
        self.active.as_ref()
    }

    pub fn complete(&mut self, id: u64) -> bool {
        if self.active.as_ref().map(|active| active.id) != Some(id) {
            return false;
        }
        self.active = self.pending.pop_front();
        true
    }

    pub fn cancel(&mut self, id: u64) -> bool {
        if self.active.as_ref().map(|active| active.id) == Some(id) {
            self.active = self.pending.pop_front();
            return true;
        }
        let Some(index) = self.pending.iter().position(|pending| pending.id == id) else {
            return false;
        };
        self.pending.remove(index);
        true
    }

    pub fn cancel_pending(&mut self) -> Vec<u64> {
        self.pending.drain(..).map(|pending| pending.id).collect()
    }

    pub fn cancel_all(&mut self) -> Vec<u64> {
        let mut cancelled = Vec::new();
        if let Some(active) = self.active.take() {
            cancelled.push(active.id);
        }
        cancelled.extend(self.cancel_pending());
        cancelled
    }

    pub fn is_idle(&self) -> bool {
        self.active.is_none() && self.pending.is_empty()
    }

    pub fn len(&self) -> usize {
        usize::from(self.active.is_some()) + self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.is_idle()
    }
}

pub fn sanitize_bracketed_text(text: &str) -> String {
    text.replace(std::str::from_utf8(BRACKETED_PASTE_END).unwrap(), "")
}

pub fn bracketed_text_bytes(text: &str) -> Vec<u8> {
    let sanitized = sanitize_bracketed_text(text);
    let mut bytes = Vec::with_capacity(
        BRACKETED_PASTE_START.len() + sanitized.len() + BRACKETED_PASTE_END.len(),
    );
    bytes.extend_from_slice(BRACKETED_PASTE_START);
    bytes.extend_from_slice(sanitized.as_bytes());
    bytes.extend_from_slice(BRACKETED_PASTE_END);
    bytes
}

pub fn clear_input_bytes(submitted_lines: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(1 + submitted_lines.saturating_mul(2));
    bytes.push(KILL_INPUT);
    for _ in 0..submitted_lines {
        bytes.extend_from_slice(&[BACKSPACE, KILL_INPUT]);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction(value: u8) -> TerminalInputTransaction {
        TerminalInputTransaction::new(vec![TerminalInputStep::RawBytes(vec![value])], false)
    }

    #[test]
    fn bracketed_text_strips_embedded_terminators_and_frames_exact_bytes() {
        assert_eq!(
            bracketed_text_bytes("one\x1b[201~two"),
            b"\x1b[200~onetwo\x1b[201~"
        );
    }

    #[test]
    fn clear_input_repeats_backspace_and_kill_for_submitted_lines() {
        assert_eq!(clear_input_bytes(0), [KILL_INPUT]);
        assert_eq!(
            clear_input_bytes(2),
            [KILL_INPUT, BACKSPACE, KILL_INPUT, BACKSPACE, KILL_INPUT]
        );
    }

    #[test]
    fn queue_serializes_completes_cancels_and_returns_to_idle() {
        let mut queue = TerminalInputQueue::default();
        let first = queue.enqueue(transaction(1));
        let second = queue.enqueue(transaction(2));
        let third = queue.enqueue(transaction(3));
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.active().unwrap().id, first);
        assert!(queue.cancel(second));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.cancel_pending(), [third]);
        assert_eq!(queue.len(), 1);
        assert!(queue.complete(first));
        assert!(queue.is_idle());
        let fourth = queue.enqueue(transaction(4));
        assert!(!queue.complete(first));
        assert_eq!(queue.cancel_all(), [fourth]);
        assert!(queue.is_idle());
    }
}
