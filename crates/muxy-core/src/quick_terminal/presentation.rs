#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PresentationPhase {
    #[default]
    Hidden,
    Showing,
    Visible,
    Hiding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationTransition {
    pub identifier: u64,
    pub shows_panel: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PresentationState {
    pub phase: PresentationPhase,
    pub target_is_visible: bool,
    transition_identifier: u64,
}

impl PresentationState {
    pub fn request_visibility(&mut self, visible: bool) -> Option<PresentationTransition> {
        if self.target_is_visible == visible {
            return None;
        }
        self.target_is_visible = visible;
        self.transition_identifier = self.transition_identifier.wrapping_add(1);
        self.phase = if visible {
            PresentationPhase::Showing
        } else {
            PresentationPhase::Hiding
        };
        Some(PresentationTransition {
            identifier: self.transition_identifier,
            shows_panel: visible,
        })
    }

    pub fn complete(&mut self, transition: PresentationTransition) -> bool {
        if transition.identifier != self.transition_identifier
            || transition.shows_panel != self.target_is_visible
        {
            return false;
        }
        self.phase = if self.target_is_visible {
            PresentationPhase::Visible
        } else {
            PresentationPhase::Hidden
        };
        true
    }
}

pub fn should_capture_focus(has_snapshot: bool, panel_is_key: bool) -> bool {
    !has_snapshot || !panel_is_key
}

pub fn should_restore_focus(requested: bool, panel_is_key: bool) -> bool {
    requested && panel_is_key
}

#[cfg(test)]
mod tests {
    use super::{PresentationPhase, PresentationState, should_capture_focus, should_restore_focus};

    #[test]
    fn quick_terminal_presentation_generations_ignore_stale_completions() {
        let mut state = PresentationState::default();
        let show = state.request_visibility(true).unwrap();
        assert_eq!(state.phase, PresentationPhase::Showing);
        let hide = state.request_visibility(false).unwrap();
        let second_show = state.request_visibility(true).unwrap();
        assert!(!state.complete(show));
        assert!(!state.complete(hide));
        assert!(state.complete(second_show));
        assert_eq!(state.phase, PresentationPhase::Visible);
        assert!(state.request_visibility(true).is_none());
    }

    #[test]
    fn quick_terminal_focus_policy_matches_retained_behavior() {
        assert!(should_capture_focus(false, true));
        assert!(should_capture_focus(true, false));
        assert!(!should_capture_focus(true, true));
        assert!(should_restore_focus(true, true));
        assert!(!should_restore_focus(true, false));
        assert!(!should_restore_focus(false, true));
    }
}
