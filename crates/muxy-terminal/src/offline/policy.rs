#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForegroundState {
    Idle,
    Busy,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessSafety {
    SafeToLoseOrdinaryShell,
    Unsafe,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalSafetyFacts {
    pub foreground: ForegroundState,
    pub process_safety: ProcessSafety,
    pub alternate_screen: bool,
}

impl TerminalSafetyFacts {
    pub const fn unknown() -> Self {
        Self {
            foreground: ForegroundState::Unknown,
            process_safety: ProcessSafety::Unknown,
            alternate_screen: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OfflineCandidate {
    pub enabled: bool,
    pub renderer_live: bool,
    pub hidden: bool,
    pub focused: bool,
    pub input_transaction_active: bool,
    pub queued_input: bool,
    pub resize_active: bool,
    pub materialization_active: bool,
    pub facts: TerminalSafetyFacts,
    pub activity_generation: u64,
    pub timer_activity_generation: u64,
    pub last_activity_milliseconds: u64,
    pub now_milliseconds: u64,
    pub timeout_milliseconds: u64,
}

impl OfflineCandidate {
    pub fn is_eligible(self) -> bool {
        self.enabled
            && self.renderer_live
            && self.hidden
            && !self.focused
            && !self.input_transaction_active
            && !self.queued_input
            && !self.resize_active
            && !self.materialization_active
            && self.facts.foreground == ForegroundState::Idle
            && self.facts.process_safety == ProcessSafety::SafeToLoseOrdinaryShell
            && !self.facts.alternate_screen
            && self.activity_generation == self.timer_activity_generation
            && self
                .now_milliseconds
                .saturating_sub(self.last_activity_milliseconds)
                >= self.timeout_milliseconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eligible() -> OfflineCandidate {
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
            activity_generation: 7,
            timer_activity_generation: 7,
            last_activity_milliseconds: 100,
            now_milliseconds: 400,
            timeout_milliseconds: 300,
        }
    }

    #[test]
    fn offline_policy_requires_every_sleep_predicate() {
        assert!(eligible().is_eligible());
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
            |value| value.activity_generation += 1,
        ];
        for mutate in mutations {
            let mut candidate = eligible();
            mutate(&mut candidate);
            assert!(!candidate.is_eligible());
        }
    }

    #[test]
    fn unknown_sampling_and_short_timeout_fail_awake() {
        let mut candidate = eligible();
        candidate.facts.foreground = ForegroundState::Unknown;
        assert!(!candidate.is_eligible());
        candidate = eligible();
        candidate.facts.process_safety = ProcessSafety::Unknown;
        assert!(!candidate.is_eligible());
        candidate = eligible();
        candidate.now_milliseconds = 399;
        assert!(!candidate.is_eligible());
    }
}
