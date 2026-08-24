use std::collections::VecDeque;
use std::fmt;
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConfirmationKind {
    Paste,
    Osc52Read,
    Osc52Write,
    ActiveProcessClose,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfirmationId(NonZeroU64);

impl ConfirmationId {
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl fmt::Display for ConfirmationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ConfirmationRequest<T> {
    pub id: ConfirmationId,
    pub kind: ConfirmationKind,
    pub payload: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfirmationDecision {
    Approve,
    Deny,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedConfirmation<T> {
    pub id: ConfirmationId,
    pub kind: ConfirmationKind,
    pub decision: ConfirmationDecision,
    pub payload: T,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionError {
    QueueEmpty,
    NotActive {
        active: ConfirmationId,
        requested: ConfirmationId,
    },
}

#[derive(Debug)]
pub struct ConfirmationQueue<T> {
    pending: VecDeque<ConfirmationRequest<T>>,
    next_id: u64,
}

impl<T> Default for ConfirmationQueue<T> {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            next_id: 1,
        }
    }
}

impl<T> ConfirmationQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, kind: ConfirmationKind, payload: T) -> ConfirmationId {
        let id = self.allocate_id();
        self.pending
            .push_back(ConfirmationRequest { id, kind, payload });
        id
    }

    pub fn active(&self) -> Option<&ConfirmationRequest<T>> {
        self.pending.front()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn decide(
        &mut self,
        id: ConfirmationId,
        decision: ConfirmationDecision,
    ) -> Result<ResolvedConfirmation<T>, DecisionError> {
        let Some(active) = self.pending.front() else {
            return Err(DecisionError::QueueEmpty);
        };
        if active.id != id {
            return Err(DecisionError::NotActive {
                active: active.id,
                requested: id,
            });
        }

        let request = self
            .pending
            .pop_front()
            .expect("front request exists after identity check");
        Ok(ResolvedConfirmation {
            id: request.id,
            kind: request.kind,
            decision,
            payload: request.payload,
        })
    }

    pub fn discard(&mut self, mut predicate: impl FnMut(&ConfirmationRequest<T>) -> bool) -> usize {
        let before = self.pending.len();
        self.pending.retain(|request| !predicate(request));
        before - self.pending.len()
    }

    pub fn deny_all(&mut self) -> Vec<ResolvedConfirmation<T>> {
        self.pending
            .drain(..)
            .map(|request| ResolvedConfirmation {
                id: request.id,
                kind: request.kind,
                decision: ConfirmationDecision::Deny,
                payload: request.payload,
            })
            .collect()
    }

    fn allocate_id(&mut self) -> ConfirmationId {
        loop {
            let candidate = NonZeroU64::new(self.next_id).unwrap_or(NonZeroU64::MIN);
            self.next_id = candidate.get().checked_add(1).unwrap_or(1);
            let id = ConfirmationId(candidate);
            if self.pending.iter().all(|request| request.id != id) {
                return id;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_serializes_requests_and_owns_payloads() {
        let mut queue = ConfirmationQueue::new();
        let paste = queue.enqueue(ConfirmationKind::Paste, String::from("paste token"));
        let osc = queue.enqueue(ConfirmationKind::Osc52Write, String::from("write token"));

        assert_eq!(queue.active().map(|request| request.id), Some(paste));
        assert_eq!(queue.len(), 2);
        let resolved = queue
            .decide(paste, ConfirmationDecision::Approve)
            .expect("active decision");
        assert_eq!(resolved.payload, "paste token");
        assert_eq!(resolved.decision, ConfirmationDecision::Approve);
        assert_eq!(queue.active().map(|request| request.id), Some(osc));
    }

    #[test]
    fn stale_and_out_of_order_decisions_cannot_complete_payload() {
        let mut queue = ConfirmationQueue::new();
        let first = queue.enqueue(ConfirmationKind::Osc52Read, 1);
        let second = queue.enqueue(ConfirmationKind::ActiveProcessClose, 2);

        assert_eq!(
            queue.decide(second, ConfirmationDecision::Approve),
            Err(DecisionError::NotActive {
                active: first,
                requested: second,
            })
        );
        assert_eq!(queue.len(), 2);

        assert!(queue.decide(first, ConfirmationDecision::Deny).is_ok());
        assert_eq!(
            queue.decide(first, ConfirmationDecision::Approve),
            Err(DecisionError::NotActive {
                active: second,
                requested: first,
            })
        );
    }

    #[test]
    fn teardown_denies_every_request_once_in_fifo_order() {
        let mut queue = ConfirmationQueue::new();
        let first = queue.enqueue(ConfirmationKind::Paste, "first");
        let second = queue.enqueue(ConfirmationKind::Osc52Write, "second");

        let denied = queue.deny_all();
        assert_eq!(denied.len(), 2);
        assert_eq!(denied[0].id, first);
        assert_eq!(denied[1].id, second);
        assert!(
            denied
                .iter()
                .all(|resolved| resolved.decision == ConfirmationDecision::Deny)
        );
        assert!(queue.is_empty());
        assert!(queue.deny_all().is_empty());
    }

    #[test]
    fn discarding_a_destroyed_surface_drops_only_its_requests() {
        let mut queue = ConfirmationQueue::new();
        queue.enqueue(ConfirmationKind::Paste, "gone");
        let kept = queue.enqueue(ConfirmationKind::Osc52Read, "live");
        queue.enqueue(ConfirmationKind::Osc52Write, "gone");

        assert_eq!(queue.discard(|request| request.payload == "gone"), 2);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.active().map(|request| request.id), Some(kept));
        assert_eq!(queue.discard(|request| request.payload == "gone"), 0);
    }

    #[test]
    fn identifiers_remain_nonzero_across_counter_wrap() {
        let mut queue = ConfirmationQueue::<()>::new();
        queue.next_id = u64::MAX;
        let maximum = queue.enqueue(ConfirmationKind::Paste, ());
        let wrapped = queue.enqueue(ConfirmationKind::Paste, ());

        assert_eq!(maximum.get(), u64::MAX);
        assert_eq!(wrapped.get(), 1);
    }
}
