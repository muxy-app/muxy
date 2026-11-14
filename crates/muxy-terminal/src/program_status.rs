//! The root record of the program status protocol (OSC 7501), in which a
//! program reports what it is doing. Records about parts of the work (`id=…`)
//! are not kept, because nothing shows them.

use libghostty_vt::terminal::{ProgramStatus as Report, ProgramStatusState as ReportState};
use muxy_protocol::{ProgressState, TerminalProgress};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramStatus {
    pub state: ProgramState,
    /// How far along the work is, from 0 through 100, for working and blocked
    /// programs that say.
    pub progress: Option<u8>,
    /// The program's machine-readable name, such as `cargo` or `claude-code`.
    /// Empty when it didn't say.
    pub app: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramState {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
}

/// What a report does to the root record. Reports about other records
/// leave it alone.
#[derive(Debug)]
pub(crate) enum Change {
    Replace(ProgramStatus),
    Clear,
}

impl Change {
    pub(crate) fn of(report: Report<'_>) -> Option<Self> {
        if !report.id().is_empty() {
            return None;
        }
        let state = match report.state().ok()? {
            ReportState::Clear => return Some(Self::Clear),
            ReportState::Idle => ProgramState::Idle,
            ReportState::Working => ProgramState::Working,
            ReportState::Done => ProgramState::Done,
            ReportState::Blocked => ProgramState::Blocked,
            ReportState::Error => ProgramState::Error,
            _ => return None,
        };
        Some(Self::Replace(ProgramStatus {
            state,
            progress: report.progress(),
            app: String::from_utf8_lossy(report.app()).into_owned(),
        }))
    }
}

impl ProgramStatus {
    /// Finished work stays until a later report replaces it; everything
    /// else ends with the program that reported it.
    pub(crate) fn outlives_program(&self) -> bool {
        matches!(self.state, ProgramState::Done | ProgramState::Error)
    }

    pub(crate) fn progress(&self) -> Option<TerminalProgress> {
        let state = match self.state {
            ProgramState::Working if self.progress.is_some() => ProgressState::Running,
            ProgramState::Working => ProgressState::Indeterminate,
            ProgramState::Blocked => ProgressState::Paused,
            ProgramState::Error => ProgressState::Error,
            ProgramState::Idle | ProgramState::Done => return None,
        };
        Some(TerminalProgress {
            state,
            percent: self.progress,
        })
    }
}
