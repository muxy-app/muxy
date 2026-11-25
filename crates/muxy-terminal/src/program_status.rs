use libghostty_vt::terminal::{ProgramStatus as Report, ProgramStatusState as ReportState};
use muxy_protocol::{ProgressState, TerminalProgress};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramStatus {
    pub state: ProgramState,
    pub progress: Option<u8>,
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
