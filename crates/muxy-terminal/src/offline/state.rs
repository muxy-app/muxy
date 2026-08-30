use super::policy::OfflineCandidate;
use std::collections::VecDeque;

pub const MAX_WAKE_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineTimer {
    pub tab_id: String,
    pub backing_identity: Option<String>,
    pub surface_identity: u64,
    pub activity_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SurfaceLifecycle {
    Absent,
    Materializing,
    Live { surface_identity: u64 },
    SleepPending { timer: OfflineTimer },
    Sleeping { backing_identity: Option<String> },
    Waking { backing_identity: Option<String> },
    Failed { message: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineStateMachine {
    tab_id: String,
    backing_identity: Option<String>,
    activity_generation: u64,
    lifecycle: SurfaceLifecycle,
}

impl OfflineStateMachine {
    pub fn new(tab_id: impl Into<String>, backing_identity: Option<String>) -> Self {
        Self {
            tab_id: tab_id.into(),
            backing_identity,
            activity_generation: 0,
            lifecycle: SurfaceLifecycle::Absent,
        }
    }

    pub fn lifecycle(&self) -> &SurfaceLifecycle {
        &self.lifecycle
    }

    pub fn activity_generation(&self) -> u64 {
        self.activity_generation
    }

    pub fn begin_materialization(&mut self) {
        self.record_activity();
        self.lifecycle = SurfaceLifecycle::Materializing;
    }

    pub fn materialized(&mut self, surface_identity: u64) {
        self.record_activity();
        self.lifecycle = SurfaceLifecycle::Live { surface_identity };
    }

    pub fn record_activity(&mut self) {
        self.activity_generation = self.activity_generation.wrapping_add(1);
        if let SurfaceLifecycle::SleepPending { timer } = &self.lifecycle {
            self.lifecycle = SurfaceLifecycle::Live {
                surface_identity: timer.surface_identity,
            };
        }
    }

    pub fn schedule_sleep(&mut self) -> Option<OfflineTimer> {
        let SurfaceLifecycle::Live { surface_identity } = self.lifecycle else {
            return None;
        };
        let timer = OfflineTimer {
            tab_id: self.tab_id.clone(),
            backing_identity: self.backing_identity.clone(),
            surface_identity,
            activity_generation: self.activity_generation,
        };
        self.lifecycle = SurfaceLifecycle::SleepPending {
            timer: timer.clone(),
        };
        Some(timer)
    }

    pub fn sleep_if_current(&mut self, timer: &OfflineTimer, candidate: OfflineCandidate) -> bool {
        let SurfaceLifecycle::SleepPending { timer: held } = &self.lifecycle else {
            return false;
        };
        if held != timer
            || timer.tab_id != self.tab_id
            || timer.backing_identity != self.backing_identity
            || timer.activity_generation != self.activity_generation
            || candidate.activity_generation != timer.activity_generation
            || candidate.timer_activity_generation != timer.activity_generation
            || !candidate.is_eligible()
        {
            return false;
        }
        self.lifecycle = SurfaceLifecycle::Sleeping {
            backing_identity: self.backing_identity.clone(),
        };
        true
    }

    pub fn begin_wake(&mut self) -> bool {
        if !matches!(self.lifecycle, SurfaceLifecycle::Sleeping { .. }) {
            return false;
        }
        self.record_activity();
        self.lifecycle = SurfaceLifecycle::Waking {
            backing_identity: self.backing_identity.clone(),
        };
        true
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.lifecycle = SurfaceLifecycle::Failed {
            message: message.into(),
        };
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WakeInputQueue {
    chunks: VecDeque<Vec<u8>>,
    byte_count: usize,
    capacity: usize,
}

impl WakeInputQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            chunks: VecDeque::new(),
            byte_count: 0,
            capacity: capacity.min(MAX_WAKE_INPUT_BYTES),
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> Result<(), WakeQueueError> {
        let next = self
            .byte_count
            .checked_add(bytes.len())
            .ok_or(WakeQueueError::Overflow)?;
        if next > self.capacity {
            return Err(WakeQueueError::Overflow);
        }
        if !bytes.is_empty() {
            self.chunks.push_back(bytes.to_vec());
            self.byte_count = next;
        }
        Ok(())
    }

    pub fn drain(&mut self) -> Vec<Vec<u8>> {
        self.byte_count = 0;
        self.chunks.drain(..).collect()
    }

    pub fn byte_count(&self) -> usize {
        self.byte_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeQueueError {
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::policy::{ForegroundState, ProcessSafety, TerminalSafetyFacts};

    fn eligible(timer: &OfflineTimer) -> OfflineCandidate {
        OfflineCandidate {
            enabled: true,
            renderer_live: true,
            hidden: true,
            focused: false,
            input_transaction_active: false,
            queued_input: false,
            resize_active: false,
            materialization_active: false,
            facts: TerminalSafetyFacts {
                foreground: ForegroundState::Idle,
                process_safety: ProcessSafety::SafeToLoseOrdinaryShell,
                alternate_screen: false,
            },
            activity_generation: timer.activity_generation,
            timer_activity_generation: timer.activity_generation,
            last_activity_milliseconds: 100,
            now_milliseconds: 400,
            timeout_milliseconds: 300,
        }
    }

    #[test]
    fn stale_timer_cannot_sleep_replaced_or_active_surface() {
        let mut state = OfflineStateMachine::new("tab", Some("session".into()));
        state.begin_materialization();
        state.materialized(10);
        let stale = state.schedule_sleep().unwrap();
        state.record_activity();
        assert!(!state.sleep_if_current(&stale, eligible(&stale)));
        state.materialized(11);
        let current = state.schedule_sleep().unwrap();
        assert!(state.sleep_if_current(&current, eligible(&current)));
        assert!(state.begin_wake());
        state.materialized(12);
        assert_eq!(
            state.lifecycle(),
            &SurfaceLifecycle::Live {
                surface_identity: 12
            }
        );
    }

    #[test]
    fn sleep_rechecks_every_eligibility_predicate_when_timer_fires() {
        let mutations: [fn(&mut OfflineCandidate); 12] = [
            |value| value.enabled = false,
            |value| value.renderer_live = false,
            |value| value.hidden = false,
            |value| value.focused = true,
            |value| value.input_transaction_active = true,
            |value| value.queued_input = true,
            |value| value.resize_active = true,
            |value| value.materialization_active = true,
            |value| value.facts.foreground = ForegroundState::Busy,
            |value| value.facts.process_safety = ProcessSafety::Unsafe,
            |value| value.facts.alternate_screen = true,
            |value| value.now_milliseconds = 399,
        ];
        for mutate in mutations {
            let mut state = OfflineStateMachine::new("tab", Some("session".into()));
            state.materialized(10);
            let timer = state.schedule_sleep().unwrap();
            let mut candidate = eligible(&timer);
            mutate(&mut candidate);
            assert!(!state.sleep_if_current(&timer, candidate));
        }
    }

    #[test]
    fn timer_captures_tab_backing_surface_and_activity_generation() {
        let mut state = OfflineStateMachine::new("tab", Some("session".into()));
        state.materialized(42);
        let timer = state.schedule_sleep().unwrap();
        assert_eq!(timer.tab_id, "tab");
        assert_eq!(timer.backing_identity.as_deref(), Some("session"));
        assert_eq!(timer.surface_identity, 42);
        assert_eq!(timer.activity_generation, state.activity_generation());
    }

    #[test]
    fn wake_input_queue_is_bounded_and_preserves_fifo_chunks() {
        let mut queue = WakeInputQueue::new(5);
        queue.push(b"ab").unwrap();
        queue.push(b"cde").unwrap();
        assert_eq!(queue.byte_count(), 5);
        assert_eq!(queue.push(b"f"), Err(WakeQueueError::Overflow));
        assert_eq!(queue.drain(), vec![b"ab".to_vec(), b"cde".to_vec()]);
        assert_eq!(queue.byte_count(), 0);
    }
}
