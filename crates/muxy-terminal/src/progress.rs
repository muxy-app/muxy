use std::time::{Duration, Instant};

use crate::TerminalEvent;
use libghostty_vt::terminal::ProgressState as EngineState;
use muxy_protocol::{ProgressState, TerminalProgress};

const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Default)]
pub(crate) struct Progress {
    current: Option<TerminalProgress>,
    changed: bool,
    completed: u64,
    deadline: Option<Instant>,
}

impl Progress {
    pub(crate) fn report(&mut self, state: EngineState, percent: Option<u8>) {
        let previous = self.current.and_then(|progress| progress.percent);
        let (state, percent) = match state {
            EngineState::Remove => return self.set(None),
            // Ghostty reports a set without a percent only when the percent is malformed.
            EngineState::Set => match percent {
                Some(percent) => (ProgressState::Running, Some(percent)),
                None => return,
            },
            EngineState::Error => (ProgressState::Error, percent.or(previous)),
            EngineState::Indeterminate => (ProgressState::Indeterminate, previous),
            EngineState::Pause => (ProgressState::Paused, percent.or(previous)),
            _ => return,
        };
        self.set(Some(TerminalProgress { state, percent }));
    }

    fn set(&mut self, progress: Option<TerminalProgress>) {
        self.deadline = progress.map(|_| Instant::now() + TIMEOUT);
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
            self.set(None);
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
