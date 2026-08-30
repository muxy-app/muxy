#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesiredSessionMode {
    Ordinary,
    Persistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppliedSessionEvidence {
    pub linked_tab_count: usize,
    pub daemon_session_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupTransition {
    ReadyOrdinary,
    ReconcilePersistent,
    EndAllThenClearLinks,
}

pub fn startup_transition(
    desired: DesiredSessionMode,
    evidence: AppliedSessionEvidence,
) -> StartupTransition {
    match desired {
        DesiredSessionMode::Persistent => StartupTransition::ReconcilePersistent,
        DesiredSessionMode::Ordinary
            if evidence.linked_tab_count > 0 || evidence.daemon_session_count > 0 =>
        {
            StartupTransition::EndAllThenClearLinks
        }
        DesiredSessionMode::Ordinary => StartupTransition::ReadyOrdinary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_mode_and_durable_evidence_cover_interrupted_transitions() {
        for linked_tab_count in 0..=1 {
            for daemon_session_count in 0..=1 {
                let evidence = AppliedSessionEvidence {
                    linked_tab_count,
                    daemon_session_count,
                };
                assert_eq!(
                    startup_transition(DesiredSessionMode::Persistent, evidence),
                    StartupTransition::ReconcilePersistent
                );
                assert_eq!(
                    startup_transition(DesiredSessionMode::Ordinary, evidence),
                    if linked_tab_count == 0 && daemon_session_count == 0 {
                        StartupTransition::ReadyOrdinary
                    } else {
                        StartupTransition::EndAllThenClearLinks
                    }
                );
            }
        }
    }
}
