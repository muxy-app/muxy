pub mod double_shift;
pub mod geometry;
pub mod presentation;
pub mod shortcut;

pub use double_shift::{DoubleShiftConfiguration, DoubleShiftDetector, DoubleShiftInput};
pub use geometry::{Point, Rect, Size};
pub use presentation::{PresentationPhase, PresentationState, PresentationTransition};
pub use shortcut::{
    ConflictCandidate, QuickTerminalShortcut, RegistrationIdentity, ShortcutConflict,
};

#[cfg(test)]
mod tests {
    fn portable<T: Clone + Send + Sync + 'static>() {}

    #[test]
    fn quick_terminal_public_contracts_are_portable() {
        portable::<super::QuickTerminalShortcut>();
        portable::<super::RegistrationIdentity>();
        portable::<super::DoubleShiftConfiguration>();
        portable::<super::DoubleShiftDetector>();
        portable::<super::DoubleShiftInput>();
        portable::<super::Point>();
        portable::<super::Rect>();
        portable::<super::Size>();
        portable::<super::PresentationPhase>();
        portable::<super::PresentationState>();
        portable::<super::PresentationTransition>();
    }
}
