#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairOrigin {
    TerminalOsc,
    AgentHook,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub struct DesktopPairEvent<'a> {
    pub origin: PairOrigin,
    pub project_id: &'a str,
    pub worktree_id: &'a str,
    pub area_id: &'a str,
    pub tab_id: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub timestamp: f64,
}

#[derive(Debug, PartialEq, Eq)]
enum TitleKey {
    Completion,
    Exact(String),
}

#[derive(Debug)]
struct Candidate {
    origin: PairOrigin,
    project_id: String,
    worktree_id: String,
    area_id: String,
    tab_id: String,
    title: TitleKey,
    body: String,
    timestamp: f64,
}

#[derive(Debug, Default)]
pub struct DesktopPairCoalescer {
    candidates: Vec<Candidate>,
}

impl DesktopPairCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow_desktop(&mut self, event: DesktopPairEvent<'_>) -> bool {
        if !event.timestamp.is_finite() {
            return true;
        }
        self.candidates.retain(|candidate| {
            let elapsed = event.timestamp - candidate.timestamp;
            (0.0..=2.0).contains(&elapsed)
        });
        if event.origin == PairOrigin::Other {
            return true;
        }
        let title = normalized_title(event.title);
        let matches = self
            .candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (complementary(candidate.origin, event.origin)
                    && candidate.project_id == event.project_id
                    && candidate.worktree_id == event.worktree_id
                    && candidate.area_id == event.area_id
                    && candidate.tab_id == event.tab_id
                    && candidate.title == title
                    && candidate.body == event.body)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => {
                self.candidates.push(Candidate {
                    origin: event.origin,
                    project_id: event.project_id.to_owned(),
                    worktree_id: event.worktree_id.to_owned(),
                    area_id: event.area_id.to_owned(),
                    tab_id: event.tab_id.to_owned(),
                    title,
                    body: event.body.to_owned(),
                    timestamp: event.timestamp,
                });
                true
            }
            [index] => {
                self.candidates.remove(*index);
                false
            }
            _ => true,
        }
    }

    pub fn pending_len(&self) -> usize {
        self.candidates.len()
    }
}

fn complementary(left: PairOrigin, right: PairOrigin) -> bool {
    matches!(
        (left, right),
        (PairOrigin::TerminalOsc, PairOrigin::AgentHook)
            | (PairOrigin::AgentHook, PairOrigin::TerminalOsc)
    )
}

fn normalized_title(title: &str) -> TitleKey {
    match title {
        "Task completed!" | "Command executed!" => TitleKey::Completion,
        title => TitleKey::Exact(title.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = "22222222-3333-4444-8555-666666666666";
    const WORKTREE: &str = "33333333-4444-4555-8666-777777777777";
    const AREA: &str = "44444444-5555-4666-8777-888888888888";
    const TAB: &str = "55555555-6666-4777-8888-999999999999";

    fn event<'a>(
        origin: PairOrigin,
        title: &'a str,
        body: &'a str,
        timestamp: f64,
    ) -> DesktopPairEvent<'a> {
        DesktopPairEvent {
            origin,
            project_id: PROJECT,
            worktree_id: WORKTREE,
            area_id: AREA,
            tab_id: TAB,
            title,
            body,
            timestamp,
        }
    }

    #[test]
    fn notifications_pair_zero_enrolls_and_one_complement_suppresses_and_consumes() {
        let mut coalescer = DesktopPairCoalescer::new();
        assert!(coalescer.allow_desktop(event(
            PairOrigin::TerminalOsc,
            "Command executed!",
            "done",
            10.0
        )));
        assert_eq!(coalescer.pending_len(), 1);
        assert!(!coalescer.allow_desktop(event(
            PairOrigin::AgentHook,
            "Task completed!",
            "done",
            12.0
        )));
        assert_eq!(coalescer.pending_len(), 0);
    }

    #[test]
    fn notifications_pair_boundary_is_inclusive_and_old_candidates_expire() {
        let mut inclusive = DesktopPairCoalescer::new();
        assert!(inclusive.allow_desktop(event(
            PairOrigin::AgentHook,
            "Task completed!",
            "done",
            4.0
        )));
        assert!(!inclusive.allow_desktop(event(
            PairOrigin::TerminalOsc,
            "Command executed!",
            "done",
            6.0
        )));

        let mut expired = DesktopPairCoalescer::new();
        assert!(expired.allow_desktop(event(
            PairOrigin::AgentHook,
            "Task completed!",
            "done",
            4.0
        )));
        assert!(expired.allow_desktop(event(
            PairOrigin::TerminalOsc,
            "Command executed!",
            "done",
            6.000_001
        )));
        assert_eq!(expired.pending_len(), 1);
    }

    #[test]
    fn notifications_pair_requires_complement_target_title_and_body() {
        let mut same_origin = DesktopPairCoalescer::new();
        assert!(same_origin.allow_desktop(event(
            PairOrigin::AgentHook,
            "Task completed!",
            "done",
            1.0
        )));
        assert!(same_origin.allow_desktop(event(
            PairOrigin::AgentHook,
            "Task completed!",
            "done",
            2.0
        )));
        assert_eq!(same_origin.pending_len(), 2);

        for mismatch in ["project", "worktree", "area", "tab", "title", "body"] {
            let mut coalescer = DesktopPairCoalescer::new();
            assert!(coalescer.allow_desktop(event(
                PairOrigin::TerminalOsc,
                "Command executed!",
                "done",
                1.0
            )));
            let mut complement = event(PairOrigin::AgentHook, "Task completed!", "done", 2.0);
            match mismatch {
                "project" => complement.project_id = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
                "worktree" => complement.worktree_id = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
                "area" => complement.area_id = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
                "tab" => complement.tab_id = "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
                "title" => complement.title = "Different",
                "body" => complement.body = "different",
                _ => unreachable!(),
            }
            assert!(coalescer.allow_desktop(complement));
        }

        let mut exact_title = DesktopPairCoalescer::new();
        assert!(exact_title.allow_desktop(event(
            PairOrigin::TerminalOsc,
            "completion",
            "done",
            1.0
        )));
        assert!(exact_title.allow_desktop(event(
            PairOrigin::AgentHook,
            "Task completed!",
            "done",
            2.0
        )));
    }

    #[test]
    fn notifications_pair_ambiguous_matches_allow_without_consuming() {
        let mut coalescer = DesktopPairCoalescer::new();
        assert!(coalescer.allow_desktop(event(
            PairOrigin::AgentHook,
            "Task completed!",
            "done",
            1.0
        )));
        assert!(coalescer.allow_desktop(event(
            PairOrigin::AgentHook,
            "Task completed!",
            "done",
            1.5
        )));
        assert_eq!(coalescer.pending_len(), 2);
        assert!(coalescer.allow_desktop(event(
            PairOrigin::TerminalOsc,
            "Command executed!",
            "done",
            2.0
        )));
        assert_eq!(coalescer.pending_len(), 2);
    }

    #[test]
    fn notifications_pair_other_origins_neither_enroll_nor_suppress() {
        let mut coalescer = DesktopPairCoalescer::new();
        assert!(coalescer.allow_desktop(event(PairOrigin::Other, "Task completed!", "done", 1.0)));
        assert_eq!(coalescer.pending_len(), 0);
        assert!(coalescer.allow_desktop(event(
            PairOrigin::TerminalOsc,
            "Command executed!",
            "done",
            2.0
        )));
        assert_eq!(coalescer.pending_len(), 1);
        assert!(coalescer.allow_desktop(event(PairOrigin::Other, "Task completed!", "done", 3.0)));
        assert_eq!(coalescer.pending_len(), 1);
    }
}
