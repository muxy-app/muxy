use std::time::{Duration, Instant};

use crate::TerminalEvent;
use crate::program_status::ProgramStatus;
use libghostty_vt::terminal::ProgressState as EngineState;
use muxy_protocol::{ProgressState, TerminalProgress};

const TIMEOUT: Duration = Duration::from_secs(15);

/// The progress a terminal shows: the program's own status record (OSC 7501)
/// while it has one, otherwise its latest progress report (OSC 9;4).
#[derive(Debug, Default)]
pub(crate) struct Progress {
    reported: Option<TerminalProgress>,
    deadline: Option<Instant>,
    program: Option<ProgramStatus>,
    /// The program that reported `program` has ended; it only shows until
    /// a later program reports progress.
    program_ended: bool,
    program_reports: u64,
    current: Option<TerminalProgress>,
    changed: bool,
    completed: u64,
}

impl Progress {
    pub(crate) fn report(&mut self, state: EngineState, percent: Option<u8>) {
        let previous = self.reported.and_then(|progress| progress.percent);
        let progress = match state {
            EngineState::Remove => None,
            // Ghostty reports a set without a percent only when the percent is malformed.
            EngineState::Set => match percent {
                Some(percent) => Some((ProgressState::Running, Some(percent))),
                None => return,
            },
            EngineState::Error => Some((ProgressState::Error, percent.or(previous))),
            EngineState::Indeterminate => Some((ProgressState::Indeterminate, previous)),
            EngineState::Pause => Some((ProgressState::Paused, percent.or(previous))),
            _ => return,
        };
        if self.program_ended {
            self.replace_program(None);
        }
        self.set_reported(progress.map(|(state, percent)| TerminalProgress { state, percent }));
    }

    pub(crate) fn program(&self) -> Option<&ProgramStatus> {
        self.program.as_ref()
    }

    pub(crate) fn program_reports(&self) -> u64 {
        self.program_reports
    }

    pub(crate) fn set_program(&mut self, program: Option<ProgramStatus>) {
        self.replace_program(program);
        self.refresh();
    }

    fn replace_program(&mut self, program: Option<ProgramStatus>) {
        self.program = program;
        self.program_ended = false;
        self.program_reports = self.program_reports.wrapping_add(1);
    }

    /// The program that reported its status has exited or the shell is back
    /// at its prompt.
    pub(crate) fn end_program(&mut self) {
        let Some(program) = &self.program else {
            return;
        };
        self.reported = None;
        self.deadline = None;
        if program.outlives_program() {
            self.program_ended = true;
            self.refresh();
        } else {
            self.set_program(None);
        }
    }

    fn set_reported(&mut self, progress: Option<TerminalProgress>) {
        self.deadline = progress.map(|_| Instant::now() + TIMEOUT);
        self.reported = progress;
        self.refresh();
    }

    fn refresh(&mut self) {
        let progress = match &self.program {
            Some(program) => program.progress(),
            None => self.reported,
        };
        if self.current.is_some() && progress.is_none() {
            self.completed = self.completed.saturating_add(1);
        }
        if self.current != progress {
            self.current = progress;
            self.changed = true;
        }
    }

    pub(crate) fn take(&mut self, now: Instant) -> Option<TerminalEvent> {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.set_reported(None);
        }
        std::mem::take(&mut self.changed).then_some(TerminalEvent::Progress(
            muxy_protocol::SessionProgress {
                progress: self.current,
                completed: self.completed,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_reports_refresh_expiry_without_redundant_events()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut progress = Progress::default();
        progress.report(EngineState::Indeterminate, None);
        let first = progress.deadline.ok_or("deadline")?;
        assert!(matches!(
            progress.take(Instant::now()),
            Some(TerminalEvent::Progress(muxy_protocol::SessionProgress {
                progress: Some(_),
                ..
            }))
        ));
        progress.deadline = Some(Instant::now());
        progress.report(EngineState::Indeterminate, None);
        assert!(progress.deadline >= Some(first));
        let deadline = progress.deadline.ok_or("refreshed deadline")?;
        assert_eq!(
            progress.take(
                deadline
                    .checked_sub(Duration::from_nanos(1))
                    .ok_or("time")?
            ),
            None
        );
        assert_eq!(
            progress.take(deadline),
            Some(TerminalEvent::Progress(muxy_protocol::SessionProgress {
                progress: None,
                completed: 1
            }))
        );
        assert_eq!(progress.take(deadline + TIMEOUT), None);
        Ok(())
    }
}
