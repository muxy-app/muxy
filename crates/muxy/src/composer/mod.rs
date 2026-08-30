mod controller;
pub mod submission;
pub mod view;

pub use controller::{
    ComposerController, ComposerTarget, StagedComposerRestore, TargetTransition,
    clear_on_target_transition, picker_target_matches, target_transition,
};
